//! Parkett: a window over one Parquet file.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use parkett::{Field, Footer};
use parquet::basic::Repetition;

/// Matched by a Wayland compositor against the `.desktop` entry's basename.
const APP_ID: &str = "parkett";
const PRODUCT: &str = "Parkett";

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);

fn main() -> eframe::Result {
    let opened = std::env::args_os().nth(1).map(|path| Opened::open(PathBuf::from(path)));

    let viewport = egui::ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_title(PRODUCT)
        .with_inner_size([1100.0, 720.0]);
    let options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native(
        PRODUCT,
        options,
        Box::new(move |cc| {
            portal_theme::follow(&cc.egui_ctx);
            Ok(Box::new(App { opened, picking: None, titled: None }))
        }),
    )
}

struct Opened {
    path: PathBuf,
    footer: Result<Footer, String>,
}

impl Opened {
    fn open(path: PathBuf) -> Self {
        let footer = Footer::read(&path);
        Self { path, footer }
    }

    fn name(&self) -> String {
        self.path.file_name().map_or_else(|| self.path.display().to_string(), |n| n.to_string_lossy().into_owned())
    }
}

struct App {
    opened: Option<Opened>,
    /// The file dialog runs on its own thread and answers here.
    picking: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// The file the window's title names.
    titled: Option<PathBuf>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        if let Some(answer) = self.picking.as_ref().and_then(|r| r.try_recv().ok()) {
            self.picking = None;
            if let Some(path) = answer {
                self.opened = Some(Opened::open(path));
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&OPEN)) {
            self.pick(ctx);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&QUIT)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.title(ctx);

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("Open…").shortcut_text(ctx.format_shortcut(&OPEN))).clicked() {
                        self.pick(ctx);
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Quit").shortcut_text(ctx.format_shortcut(&QUIT))).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        if let Some(Opened { footer: Ok(footer), .. }) = &self.opened {
            egui::Panel::right("file")
                .resizable(true)
                .default_size(420.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| panel(ui, footer));
                });
        }

        egui::CentralPanel::default().show(ui, |ui| match &self.opened {
            None => {
                ui.centered_and_justified(|ui| {
                    if ui.button("Open a Parquet file…").clicked() {
                        self.pick(ctx);
                    }
                });
            }
            Some(opened) => match &opened.footer {
                Err(error) => {
                    ui.heading(opened.name());
                    ui.label(format!("This is not a Parquet file Parkett can read: {error}"));
                }
                Ok(footer) => {
                    ui.heading(opened.name());
                    ui.label(format!(
                        "{} rows · {} columns",
                        grouped(footer.rows.cast_unsigned()),
                        footer.schema.len()
                    ));
                }
            },
        });
    }
}

impl App {
    fn pick(&mut self, ctx: &egui::Context) {
        if self.picking.is_some() {
            return;
        }
        let (send, receive) = mpsc::channel();
        let ctx = ctx.clone();
        let folder = self.opened.as_ref().and_then(|o| o.path.parent().map(Path::to_path_buf));
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new()
                .set_title("Open a Parquet file")
                .add_filter("Parquet files", &["parquet", "parq", "pq"])
                .add_filter("All files", &["*"]);
            if let Some(folder) = folder {
                dialog = dialog.set_directory(folder);
            }
            let _ = send.send(dialog.pick_file());
            ctx.request_repaint();
        });
        self.picking = Some(receive);
    }

    fn title(&mut self, ctx: &egui::Context) {
        let path = self.opened.as_ref().map(|o| o.path.clone());
        if path != self.titled {
            let title = match &self.opened {
                Some(opened) => format!("{} — {PRODUCT}", opened.name()),
                None => PRODUCT.to_owned(),
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
            self.titled = path;
        }
    }
}

/// The file panel. DESIGN.md §6.
fn panel(ui: &mut egui::Ui, footer: &Footer) {
    egui::CollapsingHeader::new(RichText::new("File").strong()).default_open(true).show(ui, |ui| {
        egui::Grid::new("file").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Rows");
            ui.label(grouped(footer.rows.cast_unsigned()));
            ui.end_row();
            ui.label("Row groups");
            ui.label(grouped(footer.row_groups.len() as u64));
            ui.end_row();
            let compressed: i64 = footer.row_groups.iter().map(|g| g.compressed).sum();
            let uncompressed: i64 = footer.row_groups.iter().map(|g| g.uncompressed).sum();
            ui.label("Data");
            ui.label(format!("{} compressed, {} uncompressed", size(compressed), size(uncompressed)));
            ui.end_row();
            if let Some(writer) = &footer.writer {
                ui.label("Written by");
                ui.label(writer);
                ui.end_row();
            }
        });
    });

    egui::CollapsingHeader::new(RichText::new("Schema").strong()).default_open(true).show(ui, |ui| {
        for field in &footer.schema {
            schema_field(ui, field);
        }
    });

    egui::CollapsingHeader::new(RichText::new("Columns").strong()).show(ui, |ui| {
        egui::Grid::new("columns").num_columns(6).striped(true).show(ui, |ui| {
            for heading in ["Column", "Codec", "Encodings", "Compressed", "Uncompressed", "Nulls"] {
                ui.label(RichText::new(heading).weak());
            }
            ui.end_row();
            for column in &footer.columns {
                ui.label(&column.path);
                ui.label(column.codecs.join(", "));
                ui.label(column.encodings.join(", "));
                ui.label(size(column.compressed));
                ui.label(size(column.uncompressed));
                ui.label(column.nulls.map_or_else(|| "—".to_owned(), grouped));
                ui.end_row();
            }
        });
    });

    egui::CollapsingHeader::new(RichText::new("Row groups").strong()).show(ui, |ui| {
        for (index, group) in footer.row_groups.iter().enumerate() {
            let title = format!("{}: {} rows, {}", index + 1, grouped(group.rows.cast_unsigned()), size(group.compressed));
            egui::CollapsingHeader::new(title).id_salt(("row group", index)).show(ui, |ui| {
                egui::Grid::new(("chunks", index)).num_columns(4).striped(true).show(ui, |ui| {
                    for heading in ["Column", "Minimum", "Maximum", "Nulls"] {
                        ui.label(RichText::new(heading).weak());
                    }
                    ui.end_row();
                    for (column, chunk) in footer.columns.iter().zip(&group.chunks) {
                        ui.label(&column.path);
                        ui.label(chunk.min.as_deref().unwrap_or("—"));
                        ui.label(chunk.max.as_deref().unwrap_or("—"));
                        ui.label(chunk.nulls.map_or_else(|| "—".to_owned(), grouped));
                        ui.end_row();
                    }
                });
            });
        }
    });

    if !footer.metadata.is_empty() {
        egui::CollapsingHeader::new(RichText::new("Metadata").strong()).show(ui, |ui| {
            egui::Grid::new("metadata").num_columns(2).striped(true).show(ui, |ui| {
                for (key, value) in &footer.metadata {
                    ui.label(key);
                    ui.label(value.as_deref().unwrap_or("—"));
                    ui.end_row();
                }
            });
        });
    }
}

fn schema_field(ui: &mut egui::Ui, field: &Field) {
    let repetition = match field.repetition {
        Repetition::REQUIRED => "",
        Repetition::OPTIONAL => " · nullable",
        Repetition::REPEATED => " · repeated",
    };
    let physical = match &field.physical {
        Some(physical) if *physical != field.kind => format!(" ({physical})"),
        _ => String::new(),
    };
    let line = format!("{}  {}{physical}{repetition}", field.name, field.kind);
    if field.children.is_empty() {
        ui.label(line);
    } else {
        egui::CollapsingHeader::new(line).show(ui, |ui| {
            for child in &field.children {
                schema_field(ui, child);
            }
        });
    }
}

fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[allow(clippy::cast_precision_loss)]
fn size(bytes: i64) -> String {
    let bytes = bytes as f64;
    match bytes {
        b if b >= 1e9 => format!("{:.1} GB", b / 1e9),
        b if b >= 1e6 => format!("{:.1} MB", b / 1e6),
        b if b >= 1e3 => format!("{:.1} kB", b / 1e3),
        b => format!("{b} B"),
    }
}
