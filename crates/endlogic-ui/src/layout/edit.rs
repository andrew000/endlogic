//! Layouts of the editor in the custom tab.
//!
//! The board stays on the same axis as in play. The tray's place takes the
//! digit strip, the piece editor and the solver's answer; in the wide
//! arrangement the digit strip and the answer go under the board instead,
//! where the win banner would be, and the piece editor takes the whole
//! right column.

use crate::font::{Font, text_width};

use super::{
    Arrangement, LOW_TRAY_GAP, Layout, LayoutInput, MAX_CELL, MIN_CELL, MIN_TARGET, Metrics, Rect,
    TALL_BOTTOM, TALL_BUTTON, TALL_HEADER, TALL_HEADER_BUTTON, TALL_SIDE, TALL_TABS, Text,
    board_at, board_size, button_width, cards_at, cell_to_fit, column_fit, count_in, min_card,
    pager_in, safe_rect, share_label, tab_row, tall_tray_with, text, to_f32, tray_cell,
};

/// Playing or editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// Playing a puzzle.
    #[default]
    Play,
    /// Editing a puzzle in the custom tab.
    Edit,
}

/// A number with − and + buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stepper {
    /// The label; empty where there is no room for one.
    pub label: Rect,
    /// The − button.
    pub minus: Rect,
    /// The number.
    pub value: Rect,
    /// The + button.
    pub plus: Rect,
}

/// The editor's controls and areas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditLayout {
    /// Board width.
    pub width: Stepper,
    /// Board height.
    pub height: Stepper,
    /// A `×` between the steppers when they share a row without labels.
    pub times: Option<Rect>,
    /// The line that names the puzzle's first problem.
    pub status: Rect,
    /// Clear button.
    pub clear: Rect,
    /// Play button.
    pub play: Rect,
    /// Solve button.
    pub solve: Rect,
    /// Where the digit strip opens.
    pub strip: Rect,
    /// Where the piece editor opens.
    pub piece: Rect,
    /// Where the solver's answer shows.
    pub result: Rect,
    /// Width each of the piece editor's buttons needs for its label.
    pub piece_button: f32,
}

/// The parts of the piece editor.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceEditor {
    /// The grid of cells.
    pub grid: Rect,
    /// Side of one cell.
    pub cell: f32,
    /// Why the piece cannot be accepted yet.
    pub reason: Rect,
    /// Accept the piece.
    pub done: Rect,
    /// Leave without changes.
    pub cancel: Rect,
    /// Add a copy (an existing piece only).
    pub duplicate: Rect,
    /// Remove the piece (an existing piece only).
    pub delete: Rect,
}

/// The parts of the solver's answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResultPanel {
    /// The main line.
    pub title: Rect,
    /// A second line: the reason, or which solution is shown.
    pub detail: Rect,
    /// Previous solution.
    pub prev: Rect,
    /// Next solution.
    pub next: Rect,
    /// Cancel or continue.
    pub action: Rect,
}

/// Height of a status line of two text lines.
const STATUS_H: f32 = 34.0;
/// Smallest area for the piece editor and the digit strip in the tall
/// arrangement.
const PANEL_MIN: f32 = 160.0;
/// Padding inside the editor areas.
const PAD: f32 = 8.0;
/// A digit key, and the distance between neighbouring keys.
const KEY: f32 = 40.0;
const KEY_PITCH: f32 = 44.0;
/// Least room around the digit keys.
const KEY_EDGE: f32 = 2.0;

impl Stepper {
    /// `[label][−][value][+]` in `w` from `x`, buttons `button` wide.
    fn at(x: f32, y: f32, w: f32, h: f32, button: f32, value: f32) -> Stepper {
        let controls = 2.0 * button + value;
        let label = Rect::new(x, y, (w - controls).max(0.0), h);
        let minus = Rect::new(label.right(), y, button, h);
        let value = Rect::new(minus.right(), y, value, h);
        let plus = Rect::new(value.right(), y, button, h);
        Stepper {
            label,
            minus,
            value,
            plus,
        }
    }
}

impl EditLayout {
    /// The editor's elements that must neither overlap nor leave the
    /// screen. The overlay areas are left out: they open over the tray.
    #[must_use]
    pub fn static_rects(&self) -> Vec<(&'static str, Rect)> {
        let mut rects = Vec::new();
        for (name, s) in [("width", self.width), ("height", self.height)] {
            if s.label.w > 0.0 {
                rects.push((name, s.label));
            }
            rects.extend([(name, s.minus), (name, s.value), (name, s.plus)]);
        }
        rects.extend([
            ("status", self.status),
            ("clear", self.clear),
            ("play", self.play),
            ("solve", self.solve),
        ]);
        rects
    }

    /// Touch zones of the editor's buttons outside the overlays.
    #[must_use]
    pub fn targets(&self) -> Vec<(&'static str, Rect)> {
        vec![
            ("width −", self.width.minus),
            ("width +", self.width.plus),
            ("height −", self.height.minus),
            ("height +", self.height.plus),
            ("clear", self.clear),
            ("play", self.play),
            ("solve", self.solve),
        ]
    }

    /// The keys of the digit strip: one per digit from 0, then "Done",
    /// rows of keys a full target apart. "Done" takes two slots when the
    /// rows have room for it.
    #[must_use]
    pub fn digit_keys(&self, digits: usize) -> Vec<Rect> {
        let area = self.strip;
        let spare = KEY_PITCH - KEY;
        let per_row = count_in(area.w - 2.0 * KEY_EDGE + spare, KEY_PITCH).max(2);
        let rows_for = |slots: usize| slots.div_ceil(per_row);
        let rows_fit =
            |slots: usize| to_f32(rows_for(slots)) * KEY_PITCH - spare + 2.0 * KEY_EDGE <= area.h;
        // "Done" starts a new row if only one slot is left in the last.
        let wide_done = digits % per_row != per_row - 1 || rows_fit(digits + 3);
        let done_slot = if digits % per_row == per_row - 1 && wide_done {
            digits + 1
        } else {
            digits
        };
        let wide_done = wide_done && rows_fit(done_slot + 2);
        let slots = done_slot + if wide_done { 2 } else { 1 };
        let rows = rows_for(slots);
        // Centered on the keys of the fullest row.
        let row_w = to_f32(per_row.min(slots)) * KEY_PITCH - spare;
        let col_h = to_f32(rows) * KEY_PITCH - spare;
        let x0 = area.center().x - row_w / 2.0;
        let y0 = area.y + ((area.h - col_h) / 2.0).clamp(KEY_EDGE, PAD);
        let slot = |k: usize, w: f32| {
            Rect::new(
                x0 + to_f32(k % per_row) * KEY_PITCH,
                y0 + to_f32(k / per_row) * KEY_PITCH,
                w,
                KEY,
            )
        };
        let mut keys: Vec<Rect> = (0..digits).map(|k| slot(k, KEY)).collect();
        let done_w = if wide_done { KEY + KEY_PITCH } else { KEY };
        keys.push(slot(done_slot, done_w));
        keys
    }

    /// The piece editor for a grid of `side` × `side` cells: the grid with
    /// the reason and the buttons beside it in a wide area, under it in a
    /// narrow one.
    #[must_use]
    pub fn piece_editor(&self, side: usize) -> PieceEditor {
        let inner = self.piece.inset(PAD);
        let buttons_h = 2.0 * MIN_TARGET + PAD;
        let reason_h = STATUS_H;
        let buttons_w = 2.0 * self.piece_button + PAD;
        let (grid, rest) = if inner.w >= inner.h * 1.3 {
            let grid_side = inner.h.min(inner.w - PAD - buttons_w).max(1.0);
            let grid = Rect::new(
                inner.x,
                inner.y + (inner.h - grid_side) / 2.0,
                grid_side,
                grid_side,
            );
            let rest = Rect::new(
                grid.right() + PAD,
                inner.y,
                inner.right() - grid.right() - PAD,
                inner.h,
            );
            (grid, rest)
        } else {
            let grid_side = inner
                .w
                .min(inner.h - buttons_h - reason_h - 2.0 * PAD)
                .max(1.0);
            let grid = Rect::new(
                inner.center().x - grid_side / 2.0,
                inner.y,
                grid_side,
                grid_side,
            );
            let rest = Rect::new(
                inner.x,
                grid.bottom() + PAD,
                inner.w,
                inner.bottom() - grid.bottom() - PAD,
            );
            (grid, rest)
        };
        let bw = ((rest.w - PAD) / 2.0).max(1.0);
        let row2 = rest.bottom() - MIN_TARGET;
        let row1 = row2 - PAD - MIN_TARGET;
        PieceEditor {
            cell: grid.w / to_f32(side.max(1)),
            grid,
            reason: Rect::new(rest.x, rest.y, rest.w, (row1 - PAD - rest.y).max(0.0)),
            cancel: Rect::new(rest.x, row1, bw, MIN_TARGET),
            done: Rect::new(rest.x + bw + PAD, row1, bw, MIN_TARGET),
            duplicate: Rect::new(rest.x, row2, bw, MIN_TARGET),
            delete: Rect::new(rest.x + bw + PAD, row2, bw, MIN_TARGET),
        }
    }

    /// The parts of the solver's answer in `font`: text on the left and
    /// the buttons on the right when there is room, else text above the
    /// buttons.
    #[must_use]
    pub fn result_panel(&self, lang: crate::text::Lang, font: f32) -> ResultPanel {
        let area = self.result.inset(PAD);
        let action_w = button_width(text(lang, Text::Continue), font)
            .max(button_width(text(lang, Text::Cancel), font))
            .min(area.w - 2.0 * (MIN_TARGET + PAD));
        let line = font * 1.35;
        if area.w >= 360.0 {
            let y = area.center().y - MIN_TARGET / 2.0;
            let action = Rect::new(area.right() - action_w, y, action_w, MIN_TARGET);
            let next = Rect::new(action.x - PAD - MIN_TARGET, y, MIN_TARGET, MIN_TARGET);
            let prev = Rect::new(next.x - MIN_TARGET, y, MIN_TARGET, MIN_TARGET);
            let text_w = prev.x - PAD - area.x;
            let top = area.center().y - line;
            ResultPanel {
                title: Rect::new(area.x, top, text_w, line),
                detail: Rect::new(area.x, top + line, text_w, line),
                prev,
                next,
                action,
            }
        } else {
            let y = area.bottom() - MIN_TARGET;
            let prev = Rect::new(area.x, y, MIN_TARGET, MIN_TARGET);
            let next = Rect::new(prev.right(), y, MIN_TARGET, MIN_TARGET);
            let action = Rect::new(area.right() - action_w, y, action_w, MIN_TARGET);
            let top = (y - PAD - 2.0 * line).max(area.y);
            ResultPanel {
                title: Rect::new(area.x, top, area.w, line),
                detail: Rect::new(area.x, top + line, area.w, line),
                prev,
                next,
                action,
            }
        }
    }
}

/// Height the digit strip needs in an area `width` wide for a board of
/// `size` cells at most.
fn strip_height(width: f32, size: usize) -> f32 {
    let per_row = count_in(width - 2.0 * KEY_EDGE + (KEY_PITCH - KEY), KEY_PITCH).max(2);
    let slots = size + 1 + 2;
    let rows = slots.div_ceil(per_row);
    2.0 * PAD + to_f32(rows) * KEY_PITCH - (KEY_PITCH - KEY)
}

/// Height the solver's answer needs in an area `width` wide.
/// Width of the widest of the piece editor's buttons in `lang`: its label
/// in the bold button font with room at both ends.
fn piece_button_width(lang: crate::text::Lang, font: f32) -> f32 {
    [Text::Cancel, Text::Done, Text::Duplicate, Text::Delete]
        .into_iter()
        .map(|t| text_width(text(lang, t), Font::new(font, true)) + font)
        .fold(MIN_TARGET, f32::max)
}

fn result_height(width: f32, font: f32) -> f32 {
    if width - 2.0 * PAD >= 360.0 {
        2.0 * PAD + MIN_TARGET.max(2.0 * font * 1.35)
    } else {
        3.0 * PAD + MIN_TARGET + 2.0 * font * 1.35
    }
}

/// Width of the clear button, which also reads "Sure?".
fn clear_width(input: &LayoutInput, font: f32) -> f32 {
    button_width(text(input.lang, Text::Clear), font)
        .max(button_width(text(input.lang, Text::Sure), font))
}

/// A layout with nothing of the play controls, for the edit mode.
struct Common {
    kind: Arrangement,
    font: f32,
    title_font: f32,
    id_font: f32,
    safe: Rect,
    axis: f32,
    panel: Option<Rect>,
    title: Rect,
    puzzle_id: Rect,
    tabs: [Rect; 5],
    lang: Rect,
    compact: bool,
    share: Rect,
    share_short: bool,
}

/// Everything an edit layout is made of.
struct Parts {
    common: Common,
    board: super::Board,
    cell: f32,
    tray: Rect,
    cards: Vec<Rect>,
    fit: super::TrayFit,
    tray_cell: f32,
    /// Gap around the tray cards, which places the pager.
    pager_gap: f32,
    edit: EditLayout,
}

fn layout(parts: Parts) -> Layout {
    let Parts {
        common: c,
        board,
        cell,
        tray,
        cards,
        fit,
        tray_cell,
        pager_gap,
        edit,
    } = parts;
    Layout {
        kind: c.kind,
        font: c.font,
        title_font: c.title_font,
        id_font: c.id_font,
        safe: c.safe,
        axis: c.axis,
        panel: c.panel,
        title: c.title,
        puzzle_id: c.puzzle_id,
        subtitle: None,
        help: Vec::new(),
        tabs: c.tabs,
        lang: c.lang,
        compact: c.compact,
        share: c.share,
        share_short: c.share_short,
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        pager: fit.paged.then(|| pager_in(tray, pager_gap)),
        per_page: fit.per_page,
        cards,
        tray,
        tray_cell,
        reset: Rect::default(),
        hint: Rect::default(),
        counters: Rect::default(),
        tab_hint: None,
        new: Rect::default(),
        banner: Rect::default(),
        next: Rect::default(),
        notice: edit.result.center(),
        edit: Some(edit),
    }
}

// ---- tall ----

pub(super) fn tall(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let gap = 8.0;
    let font = 14.0;
    let small_font = 13.0;
    let x0 = safe.x + TALL_SIDE;
    let x1 = safe.right() - TALL_SIDE;
    let width = (x1 - x0).max(1.0);
    let axis = safe.center().x;
    let top = safe.y + 8.0;

    // Header: title and id, then the language and the link.
    let header_mid = top + TALL_HEADER / 2.0;
    let header_button = |x: f32, w: f32| {
        Rect::new(
            x,
            header_mid - TALL_HEADER_BUTTON / 2.0,
            w,
            TALL_HEADER_BUTTON,
        )
    };
    let share_w = button_width(share_label(input.lang, true), small_font);
    let share = header_button(x1 - share_w, share_w);
    let lang = header_button(share.x - gap - MIN_TARGET, MIN_TARGET);
    let title_w = (lang.x - gap - x0).max(1.0);
    let title = Rect::new(x0, top + 3.0, title_w, 22.0);
    let puzzle_id = Rect::new(x0, title.bottom(), title_w, 16.0);
    let tabs_y = top + TALL_HEADER + gap;
    let tab_w = width / 5.0;
    let tabs =
        core::array::from_fn(|k| Rect::new(x0 + tab_w * to_f32(k), tabs_y, tab_w, TALL_TABS));

    // The board size under the tabs.
    let size_y = tabs_y + TALL_TABS + gap;
    let half = (width - gap) / 2.0;
    // The number gives up a little room when a label needs it.
    let label_w = [Text::Width, Text::Height]
        .into_iter()
        .map(|t| text_width(text(input.lang, t), Font::new(font * 0.9, false)) + 4.0)
        .fold(0.0, f32::max);
    let value_w = (half - 2.0 * MIN_TARGET - label_w).clamp(20.0, 28.0);
    let stepper = |x: f32| Stepper::at(x, size_y, half, MIN_TARGET, MIN_TARGET, value_w);
    let width_stepper = stepper(x0);
    let height_stepper = stepper(x0 + half + gap);

    // Bottom row: clear, play, solve.
    let bottom_y = safe.bottom() - TALL_BOTTOM - TALL_BUTTON;
    let clear = Rect::new(x0, bottom_y, clear_width(input, font), TALL_BUTTON);
    let solve_w = button_width(text(input.lang, Text::Solve), font);
    let solve = Rect::new(x1 - solve_w, bottom_y, solve_w, TALL_BUTTON);
    let play_w = button_width(text(input.lang, Text::Play), font);
    let play_x = (axis - play_w / 2.0)
        .max(clear.right() + gap)
        .min(solve.x - gap - play_w);
    let play = Rect::new(play_x, bottom_y, play_w, TALL_BUTTON);

    // Board, status line and the tray (or what opens in its place).
    let region_top = size_y + MIN_TARGET + gap;
    let region = (bottom_y - gap - region_top).max(0.0);
    let (fit, cell) = tall_tray_with(input, width, region, gap, STATUS_H + gap, PANEL_MIN);
    let (_, block_h) = board_size(input, cell);
    let panel_h = fit.height(gap).max(PANEL_MIN);
    let stack = block_h + gap + STATUS_H + gap + panel_h;
    let board_top = region_top + (region - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, board_top, cell);
    let status = Rect::new(x0, board.frame.bottom() + gap, width, STATUS_H);
    let tray = Rect::new(x0, status.bottom() + gap, width, panel_h);
    let cards = cards_at(fit.per_page, fit.cols, fit.card, gap, axis, tray.y);
    let result_h = result_height(width, font).min(tray.h);
    let edit = EditLayout {
        piece_button: piece_button_width(input.lang, font),
        width: width_stepper,
        height: height_stepper,
        times: None,
        status,
        clear,
        play,
        solve,
        strip: tray,
        piece: tray,
        result: Rect::new(x0, tray.center().y - result_h / 2.0, width, result_h),
    };
    let common = Common {
        kind: Arrangement::Tall,
        font,
        title_font: 15.0,
        id_font: 11.0,
        safe,
        axis,
        panel: None,
        title,
        puzzle_id,
        tabs,
        lang,
        compact: true,
        share,
        share_short: true,
    };
    let tray_cell = tray_cell(input, fit.card, cell);
    layout(Parts {
        common,
        board,
        cell,
        tray,
        cards,
        fit,
        tray_cell,
        pager_gap: gap,
        edit,
    })
}

/// The tray in `column`, two cards a row, centered in it (`center`) or at
/// its top.
fn column_tray(
    input: &LayoutInput,
    column: Rect,
    gap: f32,
    center: bool,
) -> (super::TrayFit, Rect, Vec<Rect>) {
    let side = column.w;
    let fit = column_fit(input.pieces, side, column.h, gap, min_card(input, gap));
    let tray_w = if fit.paged {
        side
    } else {
        2.0 * fit.card + 3.0 * gap
    };
    let tray_h = fit.height(gap);
    let top = if center {
        column.y + (column.h - tray_h).max(0.0) / 2.0
    } else {
        column.y
    };
    let axis = column.center().x;
    let tray = Rect::new(axis - tray_w / 2.0, top, tray_w, tray_h);
    let cards = cards_at(fit.per_page, 2, fit.card, gap, axis, top);
    (fit, tray, cards)
}

// ---- wide ----

/// The left panel of the wide editor: name and id, the board size, the
/// status.
struct LeftPanel {
    panel: Rect,
    title: Rect,
    puzzle_id: Rect,
    width: Stepper,
    height: Stepper,
    status: Rect,
}

fn left_panel(m: &Metrics, x0: f32, top: f32, side: f32) -> LeftPanel {
    let pad = m.pad();
    let inner_x = x0 + pad;
    let inner_w = (side - 2.0 * pad).max(1.0);
    let title = Rect::new(inner_x, top + pad * 0.5, inner_w, m.font * 2.2);
    let puzzle_id = Rect::new(inner_x, title.bottom(), inner_w, m.font * 1.6);
    let row = |y: f32| Stepper::at(inner_x, y, inner_w, MIN_TARGET, MIN_TARGET, 32.0);
    let width = row(puzzle_id.bottom() + m.gap);
    let height = row(width.minus.bottom() + m.gap);
    let status = Rect::new(
        inner_x,
        height.minus.bottom() + m.gap,
        inner_w,
        m.font * 0.9 * 1.35 * 4.0,
    );
    LeftPanel {
        panel: Rect::new(x0, top, side, status.bottom() + pad - top),
        title,
        puzzle_id,
        width,
        height,
        status,
    }
}

pub(super) fn wide(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let short = safe.w.min(safe.h);
    let m = Metrics {
        margin: (short * 0.025).clamp(10.0, 28.0),
        gap: (short * 0.015).clamp(6.0, 16.0),
        button: (safe.h * 0.05).clamp(MIN_TARGET, 52.0),
        font: (safe.h * 0.02).clamp(14.0, 18.0),
    };
    let t = |key| text(input.lang, key);
    let axis = safe.center().x;
    let x0 = safe.x + m.margin;
    let x1 = safe.right() - m.margin;
    let top = safe.y + m.margin;
    let side = (safe.w * 0.19)
        .clamp(150.0, 340.0)
        .min(input.width * super::MAX_TRAY_SHARE);
    let bottom_y = safe.bottom() - m.margin - m.button;

    let share_w = m.button_width(t(Text::Share));
    let share = Rect::new(x1 - share_w, top, share_w, m.button);
    let lang_w = m.switch_width("EN", "UA");
    let lang = Rect::new(share.x - m.gap - lang_w, top, lang_w, m.button);
    let half = (axis - (x0 + side + m.gap)).min(lang.x - m.gap - axis);
    let tabs = tab_row(input.lang, m.font, m.pad(), axis, half, top, m.button);

    // Bottom row: clear on the left, play and solve on the right.
    let clear = Rect::new(x0, bottom_y, clear_width(input, m.font), m.button);
    let solve_w = m.button_width(t(Text::Solve));
    let solve = Rect::new(x1 - solve_w, bottom_y, solve_w, m.button);
    let play_w = m.button_width(t(Text::Play));
    let play = Rect::new(solve.x - m.gap - play_w, bottom_y, play_w, m.button);

    let left = left_panel(&m, x0, top, side);

    // The board, with the digit strip and the answer under it.
    let y0 = top + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let center_w = (x1 - x0) - 2.0 * (side + m.gap);
    let longest = input.grid_width.max(input.grid_height);
    let under_h = strip_height(center_w, longest).max(result_height(center_w, m.font));
    let cell = cell_to_fit(input, center_w, y1 - y0 - under_h - m.gap)
        .clamp(MIN_CELL, MAX_CELL)
        .floor();
    let (_, block_h) = board_size(input, cell);
    let stack = block_h + m.gap + under_h;
    let board_top = y0 + ((y1 - y0) - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, board_top, cell);
    let under = Rect::new(
        axis - center_w / 2.0,
        board_top + block_h + m.gap,
        center_w,
        under_h,
    );
    let result_h = result_height(center_w, m.font);
    let result = Rect::new(under.x, under.y, under.w, result_h);

    // The tray in the right column; the piece editor takes the column.
    let column = Rect::new(x1 - side, y0, side, y1 - y0);
    let (fit, tray, cards) = column_tray(input, column, m.gap, true);
    let edit = EditLayout {
        piece_button: piece_button_width(input.lang, m.font),
        width: left.width,
        height: left.height,
        times: None,
        status: left.status,
        clear,
        play,
        solve,
        strip: under,
        piece: column,
        result,
    };
    let common = Common {
        kind: Arrangement::Wide,
        font: m.font,
        title_font: m.font * 1.4,
        id_font: m.font * 0.9,
        safe,
        axis,
        panel: Some(left.panel),
        title: left.title,
        puzzle_id: left.puzzle_id,
        tabs,
        lang,
        compact: false,
        share,
        share_short: false,
    };
    let tray_cell = tray_cell(input, fit.card, cell);
    layout(Parts {
        common,
        board,
        cell,
        tray,
        cards,
        fit,
        tray_cell,
        pager_gap: m.gap,
        edit,
    })
}

// ---- low wide ----

/// Two steppers in one row with a `×` between them; where the column is
/// narrow their buttons are narrower than a target, the zones still apart.
fn stepper_row(x0: f32, side: f32, y: f32) -> (Stepper, Stepper, Rect) {
    let (b, v, sep) = if side >= 2.0 * (2.0 * MIN_TARGET + 24.0) + 16.0 {
        (MIN_TARGET, 24.0, 16.0)
    } else {
        (34.0, 16.0, 12.0)
    };
    let one = 2.0 * b + v;
    let sx = x0 + (side - (2.0 * one + sep)) / 2.0;
    (
        Stepper::at(sx, y, one, MIN_TARGET, b, v),
        Stepper::at(sx + one + sep, y, one, MIN_TARGET, b, v),
        Rect::new(sx + one, y, sep, MIN_TARGET),
    )
}

pub(super) fn low_wide(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let gap = 8.0;
    let font = 14.0;
    let button = MIN_TARGET;
    let x0 = safe.x + gap;
    let x1 = safe.right() - gap;
    let y0 = safe.y + gap;
    let y1 = safe.bottom() - gap;
    let axis = safe.center().x;
    let side = ((x1 - x0) * 0.27).clamp(150.0, 260.0);

    // Left column: name, tabs, language and link, then the board size, the
    // status and clear when they fit.
    let top = super::left_top(input, x0, side, y0, y1, gap, font, gap + button);
    let size_y = top.lang.bottom() + gap;
    let (width_stepper, height_stepper, times) = stepper_row(x0, side, size_y);
    let left_room = size_y + button + gap + 18.0 + gap + button <= y1;

    // Right column: the tray, then the buttons in one row at the bottom.
    let rx = x1 - side;
    let row_y = y1 - button;
    let (clear, play, solve, status) = if left_room {
        let w = (side - gap) / 2.0;
        let clear = Rect::new(x0, row_y, clear_width(input, font).min(side), button);
        let status_y = size_y + button + gap;
        (
            clear,
            Rect::new(rx, row_y, w, button),
            Rect::new(rx + w + gap, row_y, w, button),
            Rect::new(x0, status_y, side, clear.y - gap - status_y),
        )
    } else {
        let w = (side - 2.0 * gap) / 3.0;
        (
            Rect::new(rx, row_y, w, button),
            Rect::new(rx + w + gap, row_y, w, button),
            Rect::new(rx + 2.0 * (w + gap), row_y, w, button),
            Rect::new(rx, row_y - gap - STATUS_H, side, STATUS_H),
        )
    };
    let room_bottom = if left_room { row_y } else { status.y };
    let room = room_bottom - gap - y0;
    let tgap = LOW_TRAY_GAP;
    let (fit, tray, cards) = column_tray(input, Rect::new(rx, y0, side, room), tgap, false);
    let column = Rect::new(rx, y0, side, y1 - y0);

    // The board takes the full height between the columns.
    let center_w = (x1 - x0) - 2.0 * (side + gap);
    let cell = cell_to_fit(input, center_w, y1 - y0)
        .clamp(MIN_CELL, MAX_CELL)
        .floor();
    let (_, block_h) = board_size(input, cell);
    let board = board_at(input, axis, y0 + ((y1 - y0) - block_h).max(0.0) / 2.0, cell);
    let edit = EditLayout {
        piece_button: piece_button_width(input.lang, font),
        width: width_stepper,
        height: height_stepper,
        times: Some(times),
        status,
        clear,
        play,
        solve,
        strip: column,
        piece: column,
        result: {
            let h = result_height(side, font).min(room);
            Rect::new(rx, y0 + (room - h) / 2.0, side, h)
        },
    };
    let common = Common {
        kind: Arrangement::LowWide,
        font,
        title_font: super::title_size(15.0, top.title.w),
        id_font: super::LOW_ID_FONT,
        safe,
        axis,
        panel: None,
        title: top.title,
        puzzle_id: top.puzzle_id,
        tabs: top.tabs,
        lang: top.lang,
        compact: false,
        share: top.share,
        share_short: true,
    };
    let tray_cell = tray_cell(input, fit.card, cell);
    layout(Parts {
        common,
        board,
        cell,
        tray,
        cards,
        fit,
        tray_cell,
        pager_gap: tgap,
        edit,
    })
}
