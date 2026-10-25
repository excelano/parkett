//! The grid: a file's rows, drawn for the viewport and no more. DESIGN.md §4.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

// Row positions are `f64` rows and screen positions `f32` points, and every
// conversion between them and row indexes is bounded by the row count.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Event, FontId, Key, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Visuals,
    pos2, vec2,
};
use parkett::{BlockId, Blocks, Table, values};

const BAR: f32 = 10.0;
const PADDING: f32 = 6.0;
const MIN_WIDTH: f32 = 40.0;
const MAX_FITTED_WIDTH: f32 = 360.0;
/// How near a header's edge a pointer resizes rather than selects.
const EDGE: f32 = 4.0;
/// How many of a column's first values its width is fitted to.
const FIT_SAMPLE: usize = 200;
/// How much of a value one cell lays out.
const CELL_CHARACTERS: usize = 400;

enum Drag {
    Vertical { grab: f32 },
    Horizontal { grab: f32 },
    Resize { column: usize, from: f32, width: f32 },
}

pub struct Grid {
    /// The row at the top of the body, and how far into it the view has scrolled.
    top: f64,
    left: f32,
    widths: Vec<f32>,
    fitted: Vec<bool>,
    numeric: Vec<bool>,
    pub selected: Option<(u64, usize)>,
    drag: Option<Drag>,
}

/// Where each part of the grid is this frame.
struct Layout {
    header: Rect,
    numbers: Rect,
    body: Rect,
    vertical: Rect,
    horizontal: Rect,
    row: f32,
    /// The left of each column within the body, and finally the right of the last.
    starts: Vec<f32>,
}

impl Layout {
    fn visible_rows(&self) -> f64 {
        f64::from(self.body.height() / self.row)
    }

    fn max_top(&self, rows: u64) -> f64 {
        (rows as f64 - self.visible_rows()).max(0.0)
    }

    fn max_left(&self) -> f32 {
        (self.starts.last().copied().unwrap_or(0.0) - self.body.width()).max(0.0)
    }

    fn column_at(&self, x: f32) -> Option<usize> {
        let index = self.starts.partition_point(|&start| start <= x);
        (index > 0 && index < self.starts.len()).then(|| index - 1)
    }
}

impl Grid {
    pub fn new(table: &Table) -> Self {
        Self {
            top: 0.0,
            left: 0.0,
            widths: Vec::new(),
            fitted: vec![false; table.names.len()],
            numeric: table.types.iter().map(values::is_numeric).collect(),
            selected: None,
            drag: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, table: &Table, blocks: &mut Blocks) {
        blocks.begin();
        let font = egui::TextStyle::Body.resolve(ui.style());
        if self.widths.is_empty() {
            self.widths = (0..table.names.len()).map(|c| header_width(ui, table, c, &font)).collect();
        }
        self.fit(ui, table, blocks, &font);

        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click_and_drag());
        let layout = self.layout(ui, area, table, &font);
        let rows = table.rows();

        self.scroll(ui, &response, &layout);
        self.pointer(ui, &response, &layout, rows);
        self.keys(ui, &layout, table);
        let copy = !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Copy)));
        if copy && let Some(Ok(Some(text))) = self.selected_value(table, blocks, false) {
            ui.ctx().copy_text(text);
        }
        self.top = self.top.clamp(0.0, layout.max_top(rows));
        self.left = self.left.clamp(0.0, layout.max_left());

        let wanted = self.paint(ui, &layout, table, blocks, &font);
        blocks.want(wanted);
    }

    fn layout(&self, ui: &Ui, area: Rect, table: &Table, font: &FontId) -> Layout {
        let row = (ui.fonts_mut(|f| f.row_height(font)) + 2.0 * PADDING / 1.5).round();
        let digits = table.rows().max(1).to_string().len();
        let number_width = measure(ui, &"9".repeat(digits), font) + 2.0 * PADDING;
        let header_height = 2.0 * row;
        let left = area.left() + number_width;
        let top = area.top() + header_height;
        let (right, bottom) = (area.right() - BAR, area.bottom() - BAR);
        let mut starts = Vec::with_capacity(self.widths.len() + 1);
        let mut at = 0.0;
        for width in &self.widths {
            starts.push(at);
            at += width;
        }
        starts.push(at);
        Layout {
            header: Rect::from_min_max(pos2(left, area.top()), pos2(right, top)),
            numbers: Rect::from_min_max(pos2(area.left(), top), pos2(left, bottom)),
            body: Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
            vertical: Rect::from_min_max(pos2(right, top), pos2(area.right(), bottom)),
            horizontal: Rect::from_min_max(pos2(left, bottom), pos2(right, area.bottom())),
            row,
            starts,
        }
    }

    /// Widen each column on screen to fit the first of its values, once.
    fn fit(&mut self, ui: &Ui, table: &Table, blocks: &mut Blocks, font: &FontId) {
        for column in 0..table.names.len() {
            if self.fitted[column] || table.rows() == 0 {
                continue;
            }
            let (id, _) = table.block_of(column, 0);
            let Some(decoded) = blocks.get(id) else { continue };
            self.fitted[column] = true;
            let Ok(array) = decoded else { continue };
            let widest = (0..array.len().min(FIT_SAMPLE))
                .filter_map(|row| values::text(array.as_ref(), row))
                .map(|text| measure(ui, &one_line(&text), font))
                .fold(0.0_f32, f32::max);
            let width = (widest + 2.0 * PADDING).min(MAX_FITTED_WIDTH);
            self.widths[column] = self.widths[column].max(width);
        }
    }

    fn scroll(&mut self, ui: &Ui, response: &egui::Response, layout: &Layout) {
        if !response.hovered() {
            return;
        }
        let (mut delta, shift) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift));
        if shift && delta.x == 0.0 {
            delta = vec2(delta.y, 0.0);
        }
        self.top -= f64::from(delta.y / layout.row);
        self.left -= delta.x;
    }

    fn pointer(&mut self, ui: &Ui, response: &egui::Response, layout: &Layout, rows: u64) {
        let Some(at) = response.interact_pointer_pos().or_else(|| response.hover_pos()) else {
            return;
        };
        let resizing = self.edge_at(layout, at);
        if resizing.is_some() || matches!(self.drag, Some(Drag::Resize { .. })) {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }

        if response.drag_started() {
            self.drag = if let Some(column) = resizing {
                Some(Drag::Resize { column, from: at.x, width: self.widths[column] })
            } else if layout.vertical.contains(at) {
                let (thumb, _) = self.vertical_thumb(layout, rows);
                let grab = if thumb.y_range().contains(at.y) { at.y - thumb.top() } else { thumb.height() / 2.0 };
                Some(Drag::Vertical { grab })
            } else if layout.horizontal.contains(at) {
                let (thumb, _) = self.horizontal_thumb(layout);
                let grab = if thumb.x_range().contains(at.x) { at.x - thumb.left() } else { thumb.width() / 2.0 };
                Some(Drag::Horizontal { grab })
            } else {
                None
            };
        }
        if response.drag_stopped() {
            self.drag = None;
        }
        match self.drag {
            Some(Drag::Resize { column, from, width }) => {
                self.widths[column] = (width + at.x - from).max(MIN_WIDTH);
            }
            Some(Drag::Vertical { grab }) => {
                let (_, track) = self.vertical_thumb(layout, rows);
                if track > 0.0 {
                    let fraction = ((at.y - grab - layout.vertical.top()) / track).clamp(0.0, 1.0);
                    self.top = f64::from(fraction) * layout.max_top(rows);
                }
            }
            Some(Drag::Horizontal { grab }) => {
                let (_, track) = self.horizontal_thumb(layout);
                if track > 0.0 {
                    let fraction = ((at.x - grab - layout.horizontal.left()) / track).clamp(0.0, 1.0);
                    self.left = fraction * layout.max_left();
                }
            }
            None => {}
        }

        if response.clicked() && layout.body.contains(at) {
            let offset = f64::from((at.y - layout.body.top()) / layout.row);
            let row = (self.top + offset).floor();
            if let Some(column) = layout.column_at(at.x - layout.body.left() + self.left)
                && row >= 0.0
                && (row as u64) < rows
            {
                self.selected = Some((row as u64, column));
            }
        }
    }

    /// The column whose right edge the pointer is on, within the header.
    fn edge_at(&self, layout: &Layout, at: Pos2) -> Option<usize> {
        if !layout.header.contains(at) || self.drag.is_some() {
            return None;
        }
        let x = at.x - layout.header.left() + self.left;
        (0..self.widths.len()).find(|&c| (layout.starts[c + 1] - x).abs() <= EDGE)
    }

    fn vertical_thumb(&self, layout: &Layout, rows: u64) -> (Rect, f32) {
        let bar = layout.vertical;
        let shown = if rows == 0 { 1.0 } else { (layout.visible_rows() / rows as f64).min(1.0) as f32 };
        let height = (bar.height() * shown).max(24.0).min(bar.height());
        let track = bar.height() - height;
        let max_top = layout.max_top(rows);
        let fraction = if max_top > 0.0 { (self.top / max_top) as f32 } else { 0.0 };
        let top = bar.top() + track * fraction;
        (Rect::from_min_size(pos2(bar.left(), top), vec2(bar.width(), height)), track)
    }

    fn horizontal_thumb(&self, layout: &Layout) -> (Rect, f32) {
        let bar = layout.horizontal;
        let total = layout.starts.last().copied().unwrap_or(0.0).max(1.0);
        let width = (bar.width() * (layout.body.width() / total).min(1.0)).max(24.0).min(bar.width());
        let track = bar.width() - width;
        let max_left = layout.max_left();
        let fraction = if max_left > 0.0 { self.left / max_left } else { 0.0 };
        let left = bar.left() + track * fraction;
        (Rect::from_min_size(pos2(left, bar.top()), vec2(width, bar.height())), track)
    }

    fn keys(&mut self, ui: &Ui, layout: &Layout, table: &Table) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let rows = table.rows();
        let columns = table.names.len();
        if rows == 0 || columns == 0 {
            return;
        }
        let page = (layout.visible_rows().floor() as u64).saturating_sub(1).max(1);
        let (row, column) = self.selected.unwrap_or((self.top.floor() as u64, 0));
        let moved = ui.input(|i| {
            let command = i.modifiers.command;
            if i.key_pressed(Key::ArrowDown) {
                Some((if command { rows - 1 } else { (row + 1).min(rows - 1) }, column))
            } else if i.key_pressed(Key::ArrowUp) {
                Some((if command { 0 } else { row.saturating_sub(1) }, column))
            } else if i.key_pressed(Key::ArrowRight) {
                Some((row, if command { columns - 1 } else { (column + 1).min(columns - 1) }))
            } else if i.key_pressed(Key::ArrowLeft) {
                Some((row, if command { 0 } else { column.saturating_sub(1) }))
            } else if i.key_pressed(Key::PageDown) {
                Some(((row + page).min(rows - 1), column))
            } else if i.key_pressed(Key::PageUp) {
                Some((row.saturating_sub(page), column))
            } else if i.key_pressed(Key::Home) {
                Some((if command { 0 } else { row }, 0))
            } else if i.key_pressed(Key::End) {
                Some((if command { rows - 1 } else { row }, columns - 1))
            } else {
                None
            }
        });
        if let Some((row, column)) = moved {
            self.selected = Some((row, column));
            let visible = layout.visible_rows().floor().max(1.0);
            if (row as f64) < self.top {
                self.top = row as f64;
            } else if row as f64 + 1.0 > self.top + visible {
                self.top = row as f64 + 1.0 - visible;
            }
            let (start, end) = (layout.starts[column], layout.starts[column + 1]);
            if start < self.left {
                self.left = start;
            } else if end > self.left + layout.body.width() {
                self.left = (end - layout.body.width()).min(start);
            }
        }
    }

    /// The selected cell's value, whole when `whole`: `None` while its block
    /// is decoding, and `Ok(None)` for null.
    pub fn selected_value(&self, table: &Table, blocks: &mut Blocks, whole: bool) -> Option<Result<Option<String>, String>> {
        let (row, column) = self.selected?;
        let (id, offset) = table.block_of(column, row);
        Some(match blocks.get(id)? {
            Ok(array) if whole => Ok(values::whole(array.as_ref(), offset)),
            Ok(array) => Ok(values::text(array.as_ref(), offset)),
            Err(error) => Err(error.clone()),
        })
    }

    fn visible(&self, layout: &Layout, rows: u64) -> Visible {
        let first = self.top.floor() as u64;
        Visible {
            first,
            fraction: (self.top - self.top.floor()) as f32,
            count: (layout.visible_rows().ceil() as u64 + 1).min(rows.saturating_sub(first)),
            columns: (0..self.widths.len())
                .filter(|&c| layout.starts[c + 1] > self.left && layout.starts[c] < self.left + layout.body.width())
                .collect(),
        }
    }

    fn x_of(&self, layout: &Layout, column: usize) -> f32 {
        layout.body.left() + layout.starts[column] - self.left
    }

    /// Paint, and answer which blocks the painted cells wanted.
    fn paint(&self, ui: &Ui, layout: &Layout, table: &Table, blocks: &mut Blocks, font: &FontId) -> Vec<BlockId> {
        let visuals = ui.visuals();
        let painter = ui.painter_at(layout.header.union(layout.numbers).union(layout.body).union(layout.horizontal).union(layout.vertical));
        let visible = self.visible(layout, table.rows());
        painter.rect_filled(layout.body, 0.0, visuals.extreme_bg_color);
        let wanted = self.paint_cells(&painter, visuals, layout, &visible, table, blocks, font);
        self.paint_header(&painter, visuals, layout, &visible, table, font);
        self.paint_numbers(&painter, visuals, layout, &visible, font);
        let thumb_color = visuals.widgets.inactive.bg_fill;
        let (thumb, _) = self.vertical_thumb(layout, table.rows());
        painter.rect_filled(thumb.shrink(2.0), 3.0, thumb_color);
        let (thumb, _) = self.horizontal_thumb(layout);
        painter.rect_filled(thumb.shrink(2.0), 3.0, thumb_color);
        wanted
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_cells(
        &self,
        painter: &Painter,
        visuals: &Visuals,
        layout: &Layout,
        visible: &Visible,
        table: &Table,
        blocks: &mut Blocks,
        font: &FontId,
    ) -> Vec<BlockId> {
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let small = FontId::new(font.size * 0.85, font.family.clone());
        let stripe = visuals.faint_bg_color.gamma_multiply(0.5);
        let body = painter.with_clip_rect(layout.body);
        let mut wanted = Vec::new();
        let mut want = |id: BlockId| {
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        };

        for r in visible.rows() {
            let y = visible.y_of(layout, r);
            if r % 2 == 1 {
                let band = Rect::from_min_size(pos2(layout.body.left(), y), vec2(layout.body.width(), layout.row));
                body.rect_filled(band, 0.0, stripe);
            }
            for &c in &visible.columns {
                let cell = Rect::from_min_size(pos2(self.x_of(layout, c), y), vec2(self.widths[c], layout.row));
                let (id, offset) = table.block_of(c, r);
                let shown = match blocks.get(id) {
                    None => {
                        want(id);
                        continue;
                    }
                    Some(Ok(array)) => values::text(array.as_ref(), offset).map(|t| one_line(&t)),
                    Some(Err(_)) => Some("⚠".to_owned()),
                };
                let inside = painter.with_clip_rect(cell.shrink2(vec2(PADDING, 0.0)).intersect(layout.body));
                let (left, right) = (pos2(cell.left() + PADDING, cell.center().y), pos2(cell.right() - PADDING, cell.center().y));
                match shown {
                    Some(text) if self.numeric[c] => {
                        inside.text(right, Align2::RIGHT_CENTER, text, font.clone(), visuals.text_color());
                    }
                    Some(text) => {
                        inside.text(left, Align2::LEFT_CENTER, text, font.clone(), visuals.text_color());
                    }
                    None => {
                        inside.text(left, Align2::LEFT_CENTER, "null", small.clone(), visuals.weak_text_color().gamma_multiply(0.6));
                    }
                }
            }
        }
        // The stretch just below the screen, so that scrolling on finds it decoded.
        let below = visible.first + 2 * visible.count;
        if below < table.rows() {
            for &c in &visible.columns {
                want(table.block_of(c, below).0);
            }
        }

        for &c in &visible.columns {
            let x = self.x_of(layout, c) + self.widths[c];
            body.line_segment([pos2(x, layout.body.top()), pos2(x, layout.body.bottom())], line);
        }
        if let Some((row, column)) = self.selected
            && visible.rows().contains(&row)
        {
            let at = pos2(self.x_of(layout, column), visible.y_of(layout, row));
            let cell = Rect::from_min_size(at, vec2(self.widths[column], layout.row));
            let selection = visuals.selection;
            body.rect(cell, 0.0, selection.bg_fill.gamma_multiply(0.25), selection.stroke, StrokeKind::Inside);
        }
        wanted
    }

    fn paint_header(&self, painter: &Painter, visuals: &Visuals, layout: &Layout, visible: &Visible, table: &Table, font: &FontId) {
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let small = FontId::new(font.size * 0.85, font.family.clone());
        let header = painter.with_clip_rect(layout.header);
        header.rect_filled(layout.header, 0.0, visuals.faint_bg_color);
        for &c in &visible.columns {
            let x = self.x_of(layout, c);
            let top = layout.header.top();
            let clip = Rect::from_min_size(pos2(x, top), vec2(self.widths[c] - PADDING, layout.header.height()));
            let cell = painter.with_clip_rect(clip.intersect(layout.header));
            let name = pos2(x + PADDING, top + layout.row * 0.5);
            cell.text(name, Align2::LEFT_CENTER, &table.names[c], font.clone(), visuals.strong_text_color());
            let kind_at = pos2(x + PADDING, top + layout.row * 1.4);
            cell.text(kind_at, Align2::LEFT_CENTER, kind(table, c), small.clone(), visuals.weak_text_color());
            let edge = x + self.widths[c];
            header.line_segment([pos2(edge, top), pos2(edge, layout.header.bottom())], line);
        }
        header.line_segment([layout.header.left_bottom(), layout.header.right_bottom()], line);
    }

    fn paint_numbers(&self, painter: &Painter, visuals: &Visuals, layout: &Layout, visible: &Visible, font: &FontId) {
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let numbers = painter.with_clip_rect(layout.numbers);
        numbers.rect_filled(layout.numbers, 0.0, visuals.faint_bg_color);
        for r in visible.rows() {
            let at = pos2(layout.numbers.right() - PADDING, visible.y_of(layout, r) + layout.row / 2.0);
            let color = if self.selected.is_some_and(|(row, _)| row == r) {
                visuals.text_color()
            } else {
                visuals.weak_text_color()
            };
            numbers.text(at, Align2::RIGHT_CENTER, (r + 1).to_string(), font.clone(), color);
        }
        numbers.line_segment([layout.numbers.right_top(), layout.numbers.right_bottom()], line);
        let corner = Rect::from_min_max(pos2(layout.numbers.left(), layout.header.top()), layout.header.left_bottom());
        painter.rect_filled(corner, 0.0, visuals.faint_bg_color);
    }
}

/// The rows and columns in view this frame.
struct Visible {
    first: u64,
    /// How far into the first row the view has scrolled, as a fraction of it.
    fraction: f32,
    count: u64,
    columns: Vec<usize>,
}

impl Visible {
    fn rows(&self) -> std::ops::Range<u64> {
        self.first..self.first + self.count
    }

    fn y_of(&self, layout: &Layout, row: u64) -> f32 {
        layout.body.top() + ((row - self.first) as f32 - self.fraction) * layout.row
    }
}

fn kind(table: &Table, column: usize) -> &str {
    table.footer.schema.get(column).map_or("", |f| f.kind.as_str())
}

fn header_width(ui: &Ui, table: &Table, column: usize, font: &FontId) -> f32 {
    let small = FontId::new(font.size * 0.85, font.family.clone());
    let name = measure(ui, &table.names[column], font);
    let kind = measure(ui, kind(table, column), &small);
    (name.max(kind) + 2.0 * PADDING).clamp(MIN_WIDTH * 2.0, MAX_FITTED_WIDTH)
}

fn measure(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x)
}

/// A value on one line and cut to what a cell can show.
fn one_line(text: &str) -> String {
    text.chars().take(CELL_CHARACTERS).map(|c| if c.is_control() { ' ' } else { c }).collect()
}
