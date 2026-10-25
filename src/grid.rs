//! The grid: a file's rows, drawn for the viewport and no more. DESIGN.md §4.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

// Row positions are `f64` rows and screen positions `f32` points, and every
// conversion between them and row indexes is bounded by the row count.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]

use std::ops::RangeInclusive;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, Event, FontId, Key, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Visuals,
    pos2, vec2,
};
use parkett::{BlockId, Blocks, Table, copy, values};

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

/// A rectangle of cells: the cell it was begun from, and the cell it reaches,
/// which is the one the keys move and the pane beneath the grid shows.
#[derive(Clone, Copy)]
pub struct Selection {
    pub anchor: (u64, usize),
    pub cursor: (u64, usize),
}

impl Selection {
    fn cell(at: (u64, usize)) -> Self {
        Self { anchor: at, cursor: at }
    }

    pub fn rows(&self) -> RangeInclusive<u64> {
        self.anchor.0.min(self.cursor.0)..=self.anchor.0.max(self.cursor.0)
    }

    pub fn columns(&self) -> RangeInclusive<usize> {
        self.anchor.1.min(self.cursor.1)..=self.anchor.1.max(self.cursor.1)
    }

    pub fn is_cell(&self) -> bool {
        self.anchor == self.cursor
    }
}

enum Drag {
    Vertical { grab: f32 },
    Horizontal { grab: f32 },
    Resize { column: usize, from: f32, width: f32 },
    Select,
}

pub struct Grid {
    /// The row at the top of the body, and how far into it the view has scrolled.
    top: f64,
    left: f32,
    widths: Vec<f32>,
    fitted: Vec<bool>,
    numeric: Vec<bool>,
    pub selection: Option<Selection>,
    /// Why the last copy put nothing on the clipboard.
    pub notice: Option<String>,
    drag: Option<Drag>,
}

/// Where each part of the grid is this frame.
struct Layout {
    header: Rect,
    numbers: Rect,
    body: Rect,
    corner: Rect,
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
            selection: None,
            notice: None,
            drag: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, table: &Table, blocks: &mut Blocks) {
        blocks.begin();
        let font = egui::TextStyle::Body.resolve(ui.style());
        if self.widths.is_empty() {
            self.widths = (0..table.names.len()).map(|c| header_width(ui, table, c, &font)).collect();
        }
        self.fit_first_values(ui, table, blocks, &font);

        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click_and_drag());
        let layout = self.layout(ui, area, table, &font);
        let rows = table.rows();

        self.scroll(ui, &response, &layout);
        self.pointer(ui, &response, &layout, table, blocks, &font);
        self.keys(ui, &layout, table);
        self.copy(ui, table, blocks);
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
            corner: Rect::from_min_max(area.left_top(), pos2(left, top)),
            vertical: Rect::from_min_max(pos2(right, top), pos2(area.right(), bottom)),
            horizontal: Rect::from_min_max(pos2(left, bottom), pos2(right, area.bottom())),
            row,
            starts,
        }
    }

    /// Widen each column on screen to fit the first of its values, once.
    fn fit_first_values(&mut self, ui: &Ui, table: &Table, blocks: &mut Blocks, font: &FontId) {
        for column in 0..table.names.len() {
            if self.fitted[column] || table.rows() == 0 {
                continue;
            }
            let (id, _) = table.block_of(column, 0);
            let Some(decoded) = blocks.get(id) else { continue };
            self.fitted[column] = true;
            let Ok(array) = decoded else { continue };
            let widest = widest(ui, font, (0..array.len().min(FIT_SAMPLE)).filter_map(|row| values::text(array.as_ref(), row)));
            self.widths[column] = self.widths[column].max((widest + 2.0 * PADDING).min(MAX_FITTED_WIDTH));
        }
    }

    /// Fit a column to its header and to the values in view, however wide.
    fn fit_to_view(&mut self, ui: &Ui, layout: &Layout, table: &Table, blocks: &mut Blocks, font: &FontId, column: usize) {
        let visible = self.visible(layout, table.rows());
        let texts: Vec<String> = visible
            .rows()
            .filter_map(|row| {
                let (id, offset) = table.block_of(column, row);
                match blocks.get(id) {
                    Some(Ok(array)) => values::text(array.as_ref(), offset),
                    _ => None,
                }
            })
            .collect();
        let widest = widest(ui, font, texts.into_iter());
        self.widths[column] = (widest + 2.0 * PADDING).max(header_width(ui, table, column, font));
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

    /// The cell under `at`, held to the grid's edges when `clamped`.
    fn cell_at(&self, layout: &Layout, at: Pos2, rows: u64, clamped: bool) -> Option<(u64, usize)> {
        if rows == 0 || self.widths.is_empty() {
            return None;
        }
        let row = (self.top + f64::from((at.y - layout.body.top()) / layout.row)).floor();
        let x = at.x - layout.body.left() + self.left;
        if clamped {
            let row = row.clamp(0.0, (rows - 1) as f64) as u64;
            let last = self.widths.len() - 1;
            let column = layout.column_at(x).unwrap_or(if x < 0.0 { 0 } else { last });
            return Some((row, column));
        }
        let column = layout.column_at(x)?;
        (row >= 0.0 && (row as u64) < rows).then_some((row as u64, column))
    }

    fn pointer(&mut self, ui: &Ui, response: &egui::Response, layout: &Layout, table: &Table, blocks: &mut Blocks, font: &FontId) {
        let rows = table.rows();
        let Some(at) = response.interact_pointer_pos().or_else(|| response.hover_pos()) else {
            return;
        };
        if self.edge_at(layout, at).is_some() || matches!(self.drag, Some(Drag::Resize { .. })) {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let pressed = ui.input(|i| i.pointer.press_origin());

        if response.double_clicked()
            && let Some(column) = pressed.and_then(|p| self.edge_at(layout, p))
        {
            self.fit_to_view(ui, layout, table, blocks, font, column);
            return;
        }

        // A drag is reported only once the pointer has moved, so what it grabbed
        // is decided where the button went down.
        if response.drag_started()
            && let Some(pressed) = pressed
        {
            self.drag = if let Some(column) = self.edge_at(layout, pressed) {
                Some(Drag::Resize { column, from: pressed.x, width: self.widths[column] })
            } else if layout.vertical.contains(pressed) {
                let (thumb, _) = self.vertical_thumb(layout, rows);
                let grab =
                    if thumb.y_range().contains(pressed.y) { pressed.y - thumb.top() } else { thumb.height() / 2.0 };
                Some(Drag::Vertical { grab })
            } else if layout.horizontal.contains(pressed) {
                let (thumb, _) = self.horizontal_thumb(layout);
                let grab =
                    if thumb.x_range().contains(pressed.x) { pressed.x - thumb.left() } else { thumb.width() / 2.0 };
                Some(Drag::Horizontal { grab })
            } else if let Some(cell) = layout.body.contains(pressed).then(|| self.cell_at(layout, pressed, rows, false)).flatten() {
                self.select(cell, shift);
                Some(Drag::Select)
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
            Some(Drag::Select) => {
                self.scroll_toward(ui, layout, at);
                if let (Some(cell), Some(selection)) = (self.cell_at(layout, at, rows, true), &mut self.selection) {
                    selection.cursor = cell;
                }
            }
            None => {}
        }

        if response.clicked() {
            self.click(layout, at, table, shift);
        }
    }

    /// A drag past the body's edge scrolls the grid toward it.
    fn scroll_toward(&mut self, ui: &Ui, layout: &Layout, at: Pos2) {
        let body = layout.body;
        let beyond = vec2(
            if at.x < body.left() { at.x - body.left() } else { (at.x - body.right()).max(0.0) },
            if at.y < body.top() { at.y - body.top() } else { (at.y - body.bottom()).max(0.0) },
        );
        if beyond != egui::Vec2::ZERO {
            self.top += f64::from(beyond.y / layout.row * 0.25);
            self.left += beyond.x * 0.25;
            ui.ctx().request_repaint();
        }
    }

    fn click(&mut self, layout: &Layout, at: Pos2, table: &Table, shift: bool) {
        let rows = table.rows();
        let columns = table.names.len();
        if rows == 0 || columns == 0 {
            return;
        }
        if layout.body.contains(at) {
            if let Some(cell) = self.cell_at(layout, at, rows, false) {
                self.select(cell, shift);
            }
        } else if layout.header.contains(at) && self.edge_at(layout, at).is_none() {
            if let Some(column) = layout.column_at(at.x - layout.header.left() + self.left) {
                let anchor = match (shift, self.selection) {
                    (true, Some(selection)) => (0, selection.anchor.1),
                    _ => (0, column),
                };
                self.selection = Some(Selection { anchor, cursor: (rows - 1, column) });
            }
        } else if layout.numbers.contains(at) {
            let row = (self.top + f64::from((at.y - layout.body.top()) / layout.row)).floor();
            if row >= 0.0 && (row as u64) < rows {
                let row = row as u64;
                let anchor = match (shift, self.selection) {
                    (true, Some(selection)) => (selection.anchor.0, 0),
                    _ => (row, 0),
                };
                self.selection = Some(Selection { anchor, cursor: (row, columns - 1) });
            }
        } else if layout.corner.contains(at) {
            self.selection = Some(Selection { anchor: (0, 0), cursor: (rows - 1, columns - 1) });
        }
    }

    fn select(&mut self, cell: (u64, usize), extend: bool) {
        self.selection = match (extend, self.selection) {
            (true, Some(selection)) => Some(Selection { anchor: selection.anchor, cursor: cell }),
            _ => Some(Selection::cell(cell)),
        };
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
        let (row, column) = self.selection.map_or((self.top.floor() as u64, 0), |s| s.cursor);
        let (moved, shift, all, collapse) = ui.input(|i| {
            let command = i.modifiers.command;
            let moved = if i.key_pressed(Key::ArrowDown) {
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
            };
            (moved, i.modifiers.shift, command && i.key_pressed(Key::A), i.key_pressed(Key::Escape))
        });
        if all {
            self.selection = Some(Selection { anchor: (0, 0), cursor: (rows - 1, columns - 1) });
        }
        if collapse && let Some(selection) = &mut self.selection {
            selection.anchor = selection.cursor;
        }
        if let Some(cell) = moved {
            self.select(cell, shift);
            self.reveal(layout, cell);
        }
    }

    /// Scroll so that `cell` is in view.
    fn reveal(&mut self, layout: &Layout, (row, column): (u64, usize)) {
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

    /// Put the selection on the clipboard: one cell as its text, a range as
    /// tab-separated rows. DESIGN.md §7.
    fn copy(&mut self, ui: &Ui, table: &Table, blocks: &mut Blocks) {
        let asked = !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Copy)));
        let Some(selection) = self.selection.filter(|_| asked) else { return };
        self.notice = None;
        let copied = if selection.is_cell() {
            let (row, column) = selection.cursor;
            let (id, offset) = table.block_of(column, row);
            blocks.array(table, id).map(|array| values::text(array.as_ref(), offset).unwrap_or_default())
        } else {
            copy::tab_separated(table, blocks, selection.rows(), selection.columns())
        };
        match copied {
            Ok(text) => ui.ctx().copy_text(text),
            Err(error) => self.notice = Some(error),
        }
    }

    /// The cursor cell's value, whole when `whole`: `None` while its block
    /// is decoding, and `Ok(None)` for null.
    pub fn cursor_value(&self, table: &Table, blocks: &mut Blocks, whole: bool) -> Option<Result<Option<String>, String>> {
        let (row, column) = self.selection?.cursor;
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
        painter.rect_filled(painter.clip_rect(), 0.0, visuals.panel_fill);
        painter.rect_filled(layout.body, 0.0, visuals.extreme_bg_color);
        let wanted = self.paint_cells(&painter, visuals, layout, &visible, table, blocks, font);
        self.paint_selection(&painter, visuals, layout, &visible);
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
                body.rect_filled(band, 0.0, stripe(visuals));
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
                let inside = painter.with_clip_rect(cell.intersect(layout.body));
                match shown {
                    Some(text) => fitted(&inside, cell, text, font, visuals.text_color(), self.numeric[c]),
                    None => fitted(&inside, cell, "null".to_owned(), &small, visuals.weak_text_color().gamma_multiply(0.6), false),
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
        wanted
    }

    fn paint_selection(&self, painter: &Painter, visuals: &Visuals, layout: &Layout, visible: &Visible) {
        let Some(selection) = self.selection else { return };
        let body = painter.with_clip_rect(layout.body);
        let (rows, columns) = (selection.rows(), selection.columns());
        let shown = visible.rows();
        let first = (*rows.start()).max(shown.start);
        let last = (*rows.end()).min(shown.end.saturating_sub(1));
        if first <= last && !shown.is_empty() {
            let left = self.x_of(layout, *columns.start());
            let right = self.x_of(layout, *columns.end()) + self.widths[*columns.end()];
            let top = visible.y_of(layout, first);
            let bottom = visible.y_of(layout, last) + layout.row;
            let range = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
            body.rect(range, 0.0, visuals.selection.bg_fill.gamma_multiply(0.2), Stroke::NONE, StrokeKind::Inside);
        }
        let (row, column) = selection.cursor;
        if shown.contains(&row) {
            let cell = Rect::from_min_size(pos2(self.x_of(layout, column), visible.y_of(layout, row)), vec2(self.widths[column], layout.row));
            body.rect_stroke(cell, 0.0, visuals.selection.stroke, StrokeKind::Inside);
        }
    }

    fn paint_header(&self, painter: &Painter, visuals: &Visuals, layout: &Layout, visible: &Visible, table: &Table, font: &FontId) {
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let small = FontId::new(font.size * 0.85, font.family.clone());
        let header = painter.with_clip_rect(layout.header);
        header.rect_filled(layout.header, 0.0, chrome(visuals));
        let selected = self.selection.map(|s| s.columns());
        for &c in &visible.columns {
            let x = self.x_of(layout, c);
            let top = layout.header.top();
            let column = Rect::from_min_size(pos2(x, top), vec2(self.widths[c], layout.header.height()));
            if selected.as_ref().is_some_and(|s| s.contains(&c)) {
                header.rect_filled(column, 0.0, visuals.selection.bg_fill.gamma_multiply(0.25));
            }
            let cell = painter.with_clip_rect(column.intersect(layout.header));
            let name = Rect::from_min_size(column.min, vec2(column.width(), layout.row));
            fitted(&cell, name, table.names[c].clone(), font, visuals.strong_text_color(), false);
            let kind_at = Rect::from_min_size(pos2(x, top + layout.row * 0.85), vec2(column.width(), layout.row));
            fitted(&cell, kind_at, kind(table, c).to_owned(), &small, visuals.weak_text_color(), false);
            header.line_segment([column.right_top(), column.right_bottom()], line);
        }
        header.line_segment([layout.header.left_bottom(), layout.header.right_bottom()], line);
    }

    fn paint_numbers(&self, painter: &Painter, visuals: &Visuals, layout: &Layout, visible: &Visible, font: &FontId) {
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let numbers = painter.with_clip_rect(layout.numbers);
        numbers.rect_filled(layout.numbers, 0.0, chrome(visuals));
        let selected = self.selection.map(|s| s.rows());
        for r in visible.rows() {
            let y = visible.y_of(layout, r);
            let color = if selected.as_ref().is_some_and(|s| s.contains(&r)) {
                let band = Rect::from_min_size(pos2(layout.numbers.left(), y), vec2(layout.numbers.width(), layout.row));
                numbers.rect_filled(band, 0.0, visuals.selection.bg_fill.gamma_multiply(0.25));
                visuals.text_color()
            } else {
                visuals.weak_text_color()
            };
            let at = pos2(layout.numbers.right() - PADDING, y + layout.row / 2.0);
            numbers.text(at, Align2::RIGHT_CENTER, (r + 1).to_string(), font.clone(), color);
        }
        numbers.line_segment([layout.numbers.right_top(), layout.numbers.right_bottom()], line);
        painter.rect_filled(layout.corner, 0.0, chrome(visuals));
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

/// Paint `text` within `cell`, ending in an ellipsis where it does not fit.
fn fitted(painter: &Painter, cell: Rect, text: String, font: &FontId, color: Color32, right: bool) {
    let mut job = LayoutJob::simple_singleline(text, font.clone(), color);
    job.wrap = TextWrapping::truncate_at_width((cell.width() - 2.0 * PADDING).max(0.0));
    let galley = painter.layout_job(job);
    let y = cell.center().y - galley.size().y / 2.0;
    let x = if right { cell.right() - PADDING - galley.size().x } else { cell.left() + PADDING };
    painter.galley(pos2(x, y), galley, color);
}

/// The headers' fill, set apart from the body in either theme.
fn chrome(visuals: &Visuals) -> Color32 {
    if visuals.dark_mode { Color32::from_gray(32) } else { Color32::from_gray(236) }
}

/// Every other row's tint, which has to darken a light body and lighten a dark one.
fn stripe(visuals: &Visuals) -> Color32 {
    if visuals.dark_mode { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(8) }
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

fn widest(ui: &Ui, font: &FontId, texts: impl Iterator<Item = String>) -> f32 {
    texts.map(|text| measure(ui, &one_line(&text), font)).fold(0.0, f32::max)
}

fn measure(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x)
}

/// A value on one line and cut to what a cell can show.
fn one_line(text: &str) -> String {
    text.chars().take(CELL_CHARACTERS).map(|c| if c.is_control() { ' ' } else { c }).collect()
}
