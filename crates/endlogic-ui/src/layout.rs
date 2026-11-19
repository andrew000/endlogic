//! Screen layout, computed from the area size alone.

use crate::painter::{Point, Rect};
use crate::text::{Lang, Text, difficulty_label, text};
use endlogic_core::generator::Difficulty;

/// Smallest side of anything that can be tapped.
pub const MIN_TARGET: f32 = 44.0;

/// Width of the counter band beside the board, in cells.
const BAND: f32 = 1.0;
/// Space between the cells and the board frame, in cells.
const FRAME_PAD: f32 = 0.35;
/// Largest board cell, in logical pixels.
const MAX_CELL: f32 = 64.0;
/// Smallest board cell the search for a cell size considers.
const MIN_CELL: f32 = 6.0;
/// Smallest tray card.
const MIN_CARD: f32 = 56.0;

/// What the layout depends on besides the area size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutInput {
    /// Area width.
    pub width: f32,
    /// Area height.
    pub height: f32,
    /// Main input is touch: no keyboard help or key hints.
    pub touch: bool,
    /// Language of the labels, which decides button widths.
    pub lang: Lang,
    /// Board width in cells.
    pub grid_width: usize,
    /// Board height in cells.
    pub grid_height: usize,
    /// Number of tray cards.
    pub pieces: usize,
    /// Longest side of any piece, in cells.
    pub max_piece_side: usize,
}

/// Where everything goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Wide (landscape) or tall (portrait) arrangement.
    pub wide: bool,
    /// Base font size.
    pub font: f32,
    /// Height of buttons, tabs and switches.
    pub button_height: f32,
    /// The left column panel, in the wide arrangement.
    pub panel: Option<Rect>,
    /// `// ENDLOGIC`.
    pub title: Rect,
    /// The puzzle id line.
    pub puzzle_id: Rect,
    /// The subtitle, when there is room.
    pub subtitle: Option<Rect>,
    /// The three help rows, when shown.
    pub help: Option<Rect>,
    /// Difficulty tabs, easiest first.
    pub tabs: [Rect; 4],
    /// Language switch, `EN | UA`.
    pub lang: Rect,
    /// Share button.
    pub share: Rect,
    /// Board frame: the cells plus a margin, with corner brackets.
    pub frame: Rect,
    /// The cells.
    pub cells: Rect,
    /// Side of one cell.
    pub cell: f32,
    /// Row counters, left of the frame.
    pub row_counters: Rect,
    /// Column counters, above the frame.
    pub col_counters: Rect,
    /// Tray background.
    pub tray: Rect,
    /// One card per piece, in piece order.
    pub cards: Vec<Rect>,
    /// Side of one cell of a piece drawn in the tray.
    pub tray_cell: f32,
    /// Reset button.
    pub reset: Rect,
    /// Counter display switch, `Grid | Numbers`.
    pub counters: Rect,
    /// The `TAB` key hint next to the switch, when shown.
    pub tab_hint: Option<Rect>,
    /// New puzzle button.
    pub new: Rect,
    /// Win banner (drawn over the board).
    pub banner: Rect,
    /// Next button inside the banner.
    pub next: Rect,
    /// Center of toasts and the generating indicator (drawn over the board).
    pub notice: Point,
}

impl Layout {
    /// Lays the screen out.
    #[must_use]
    pub fn compute(input: &LayoutInput) -> Layout {
        let w = input.width.max(1.0);
        let h = input.height.max(1.0);
        let short = w.min(h);
        let metrics = Metrics {
            margin: (short * 0.025).clamp(10.0, 28.0),
            gap: (short * 0.015).clamp(6.0, 16.0),
            button: (h * 0.05).clamp(MIN_TARGET, 52.0),
            font: (h * 0.02).clamp(14.0, 18.0),
        };
        if w >= h * 1.2 {
            wide(input, &metrics)
        } else {
            tall(input, &metrics)
        }
    }

    /// The two halves of a two-way switch.
    #[must_use]
    pub fn halves(rect: Rect) -> (Rect, Rect) {
        let half = rect.w / 2.0;
        (
            Rect::new(rect.x, rect.y, half, rect.h),
            Rect::new(rect.x + half, rect.y, half, rect.h),
        )
    }

    /// Board and counters together.
    #[must_use]
    pub fn board_block(&self) -> Rect {
        let x = self.row_counters.x;
        let y = self.col_counters.y;
        Rect::new(x, y, self.frame.right() - x, self.frame.bottom() - y)
    }

    /// Screen rectangle of the cell at `(row, col)`.
    #[must_use]
    pub fn cell_rect(&self, row: usize, col: usize) -> Rect {
        Rect::new(
            self.cells.x + to_f32(col) * self.cell,
            self.cells.y + to_f32(row) * self.cell,
            self.cell,
            self.cell,
        )
    }

    /// The elements that must neither overlap nor leave the screen, with
    /// names for test messages. Overlays (banner, toasts) are not included.
    #[must_use]
    pub fn static_rects(&self) -> Vec<(&'static str, Rect)> {
        let mut rects = vec![];
        if let Some(panel) = self.panel {
            rects.push(("panel", panel));
        } else {
            rects.push(("title", self.title));
            rects.push(("puzzle id", self.puzzle_id));
        }
        for (tab, name) in self.tabs.iter().zip(["easy", "normal", "hard", "expert"]) {
            rects.push((name, *tab));
        }
        rects.extend([
            ("lang", self.lang),
            ("share", self.share),
            ("board", self.board_block()),
            ("tray", self.tray),
            ("reset", self.reset),
            ("counters", self.counters),
            ("new", self.new),
        ]);
        if let Some(hint) = self.tab_hint {
            rects.push(("tab hint", hint));
        }
        rects
    }

    /// Everything that reacts to a tap, with names for test messages.
    #[must_use]
    pub fn targets(&self) -> Vec<(&'static str, Rect)> {
        let (grid, numbers) = Layout::halves(self.counters);
        let (en, ua) = Layout::halves(self.lang);
        let mut targets = vec![
            ("easy", self.tabs[0]),
            ("normal", self.tabs[1]),
            ("hard", self.tabs[2]),
            ("expert", self.tabs[3]),
            ("en", en),
            ("ua", ua),
            ("share", self.share),
            ("reset", self.reset),
            ("grid", grid),
            ("numbers", numbers),
            ("new", self.new),
            ("next", self.next),
        ];
        targets.extend(self.cards.iter().map(|&card| ("card", card)));
        targets
    }
}

/// Sizes shared by both arrangements.
struct Metrics {
    margin: f32,
    gap: f32,
    button: f32,
    font: f32,
}

impl Metrics {
    /// Inner horizontal padding of a button.
    fn pad(&self) -> f32 {
        self.font * 0.9
    }

    /// Ideal width of a button labelled `label`.
    fn button_width(&self, label: &str) -> f32 {
        (estimate_width(label, self.font) + 2.0 * self.pad()).max(MIN_TARGET)
    }

    /// Ideal width of a two-way switch.
    fn switch_width(&self, a: &str, b: &str) -> f32 {
        2.0 * self.button_width(a).max(self.button_width(b))
    }

    fn hint_width(&self) -> f32 {
        estimate_width("TAB", self.font * 0.75) + self.font
    }
}

/// A rough text width for layout; drawing shrinks text that does not fit.
fn estimate_width(text: &str, size: f32) -> f32 {
    to_f32(text.chars().count()) * size * 0.62
}

/// Converts a small count to `f32`.
#[must_use]
pub fn to_f32(n: usize) -> f32 {
    // Counts here are grid sizes and piece counts, far below 2^24.
    #[allow(clippy::cast_precision_loss)]
    let x = n as f32;
    x
}

/// Shrinks `ideal` widths to fit `available`, never below [`MIN_TARGET`].
fn squeeze(ideal: &[f32], available: f32) -> Vec<f32> {
    let total: f32 = ideal.iter().sum();
    if total <= available {
        return ideal.to_vec();
    }
    let scale = available / total;
    ideal.iter().map(|&w| (w * scale).max(MIN_TARGET)).collect()
}

/// Lays `widths` out left to right from `x`, `gap` apart.
fn row(x: f32, y: f32, h: f32, widths: &[f32], gap: f32) -> Vec<Rect> {
    let mut x = x;
    widths
        .iter()
        .map(|&w| {
            let r = Rect::new(x, y, w, h);
            x += w + gap;
            r
        })
        .collect()
}

fn tab_rects(input: &LayoutInput, m: &Metrics, x0: f32, x1: f32, y: f32) -> [Rect; 4] {
    let ideal: Vec<f32> = Difficulty::ALL
        .iter()
        .map(|&d| estimate_width(difficulty_label(input.lang, d), m.font) + 1.6 * m.pad())
        .map(|w| w.max(MIN_TARGET))
        .collect();
    let widest = ideal.iter().copied().fold(0.0, f32::max);
    let widths = squeeze(&[widest; 4], x1 - x0);
    let total: f32 = widths.iter().sum();
    let start = x0 + ((x1 - x0) - total).max(0.0) / 2.0;
    let rects = row(start, y, m.button, &widths, 0.0);
    [rects[0], rects[1], rects[2], rects[3]]
}

/// The bottom row: reset on the left, the counter switch (with its key hint)
/// in the middle, new on the right.
struct BottomRow {
    reset: Rect,
    counters: Rect,
    hint: Option<Rect>,
    new: Rect,
}

fn bottom_row(input: &LayoutInput, m: &Metrics, x0: f32, x1: f32, y: f32) -> BottomRow {
    let t = |key| text(input.lang, key);
    let ideal = [
        m.button_width(t(Text::Reset)),
        m.switch_width(t(Text::Grid), t(Text::Numbers)),
        m.button_width(t(Text::New)),
    ];
    let widths = squeeze(&ideal, x1 - x0 - 2.0 * m.gap);
    let reset = Rect::new(x0, y, widths[0], m.button);
    let new = Rect::new(x1 - widths[2], y, widths[2], m.button);
    let (lo, hi) = (reset.right() + m.gap, new.x - m.gap);
    let hint_w = m.hint_width();
    let with_hint = widths[1] + m.gap * 0.5 + hint_w;
    let center = f32::midpoint(x0, x1);
    let fits_centered = |w: f32| center - w / 2.0 >= lo && center + w / 2.0 <= hi;
    let place = |w: f32| {
        if fits_centered(w) {
            center - w / 2.0
        } else {
            lo + ((hi - lo) - w).max(0.0) / 2.0
        }
    };
    if !input.touch && with_hint <= hi - lo {
        let start = place(with_hint);
        let hint = Rect::new(start, y + m.button * 0.2, hint_w, m.button * 0.6);
        let counters = Rect::new(hint.right() + m.gap * 0.5, y, widths[1], m.button);
        BottomRow {
            reset,
            counters,
            hint: Some(hint),
            new,
        }
    } else {
        let counters = Rect::new(place(widths[1]), y, widths[1], m.button);
        BottomRow {
            reset,
            counters,
            hint: None,
            new,
        }
    }
}

/// The best square card size for `n` cards in a `w`×`h` area.
fn card_grid(n: usize, w: f32, h: f32, gap: f32) -> (usize, usize, f32) {
    let n = n.max(1);
    let mut best = (n, 1, 0.0);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let side = ((w - gap * to_f32(cols - 1)) / to_f32(cols))
            .min((h - gap * to_f32(rows - 1)) / to_f32(rows));
        if side > best.2 {
            best = (cols, rows, side);
        }
    }
    best
}

/// Smallest acceptable card for pieces with this longest side.
fn min_card(input: &LayoutInput) -> f32 {
    MIN_CARD.max(to_f32(input.max_piece_side) * 9.0)
}

/// Board block size for a cell size: `(width, height)` including counters.
fn board_size(input: &LayoutInput, cell: f32) -> (f32, f32) {
    let extra = (BAND + 2.0 * FRAME_PAD) * cell;
    (
        to_f32(input.grid_width) * cell + extra,
        to_f32(input.grid_height) * cell + extra,
    )
}

/// The largest cell size in `MIN_CELL..=MAX_CELL` for which `fits` holds,
/// or `MIN_CELL` if none does.
fn largest_cell(fits: impl Fn(f32) -> bool) -> f32 {
    if fits(MAX_CELL) {
        return MAX_CELL;
    }
    let (mut lo, mut hi) = (MIN_CELL, MAX_CELL);
    for _ in 0..24 {
        let mid = f32::midpoint(lo, hi);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo.floor()
}

/// Board rectangles for a block whose top-left corner is `origin`.
struct Board {
    frame: Rect,
    cells: Rect,
    row_counters: Rect,
    col_counters: Rect,
}

fn board_at(input: &LayoutInput, origin: Point, cell: f32) -> Board {
    let band = BAND * cell;
    let pad = FRAME_PAD * cell;
    let cells_w = to_f32(input.grid_width) * cell;
    let cells_h = to_f32(input.grid_height) * cell;
    let frame = Rect::new(
        origin.x + band,
        origin.y + band,
        cells_w + 2.0 * pad,
        cells_h + 2.0 * pad,
    );
    Board {
        frame,
        cells: Rect::new(frame.x + pad, frame.y + pad, cells_w, cells_h),
        row_counters: Rect::new(origin.x, frame.y, band, frame.h),
        col_counters: Rect::new(frame.x, origin.y, frame.w, band),
    }
}

/// The tray: background and cards, top-left at `origin`.
struct Tray {
    rect: Rect,
    cards: Vec<Rect>,
    cell: f32,
}

fn tray_size(input: &LayoutInput, w: f32, h: f32, gap: f32, cell: f32) -> (usize, usize, f32) {
    let pad = gap;
    let (cols, rows, side) = card_grid(input.pieces, w - 2.0 * pad, h - 2.0 * pad, gap);
    let longest = to_f32(input.max_piece_side.max(1));
    let largest = (longest * cell * 0.8 + cell).max(min_card(input));
    (cols, rows, side.min(largest))
}

fn tray_at(
    input: &LayoutInput,
    origin: Point,
    grid: (usize, usize, f32),
    gap: f32,
    cell: f32,
) -> Tray {
    let (cols, rows, side) = grid;
    let pad = gap;
    let w = to_f32(cols) * side + to_f32(cols - 1) * gap + 2.0 * pad;
    let h = to_f32(rows) * side + to_f32(rows.saturating_sub(1)) * gap + 2.0 * pad;
    let cards = (0..input.pieces)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            Rect::new(
                origin.x + pad + to_f32(c) * (side + gap),
                origin.y + pad + to_f32(r) * (side + gap),
                side,
                side,
            )
        })
        .collect();
    let longest = to_f32(input.max_piece_side.max(1));
    Tray {
        rect: Rect::new(origin.x, origin.y, w, h),
        cards,
        cell: (side * 0.8 / longest).min(cell),
    }
}

fn tray_extent(grid: (usize, usize, f32), gap: f32) -> (f32, f32) {
    let (cols, rows, side) = grid;
    (
        to_f32(cols) * side + to_f32(cols - 1) * gap + 2.0 * gap,
        to_f32(rows) * side + to_f32(rows.saturating_sub(1)) * gap + 2.0 * gap,
    )
}

/// Banner and next button centered on the frame.
fn banner(m: &Metrics, frame: Rect) -> (Rect, Rect) {
    let w = (frame.w * 0.92).clamp(MIN_TARGET * 4.0, 420.0);
    let h = m.button * 2.6;
    let banner = Rect::centered(frame.center(), w, h);
    let next_w = (m.font * 8.0).max(MIN_TARGET * 2.0);
    let next = Rect::new(
        banner.center().x - next_w / 2.0,
        banner.bottom() - m.button - m.gap,
        next_w,
        m.button,
    );
    (banner, next)
}

fn wide(input: &LayoutInput, m: &Metrics) -> Layout {
    let (w, h) = (input.width, input.height);
    let t = |key| text(input.lang, key);
    let column = (w * 0.2).clamp(150.0, 300.0);
    let bottom_y = h - m.margin - m.button;

    let share_w = m.button_width(t(Text::Share));
    let share = Rect::new(w - m.margin - share_w, m.margin, share_w, m.button);
    let lang_w = m.switch_width("EN", "UA");
    let lang = Rect::new(share.x - m.gap - lang_w, m.margin, lang_w, m.button);
    let tabs = tab_rects(
        input,
        m,
        m.margin + column + m.gap,
        lang.x - m.gap,
        m.margin,
    );

    let bottom = bottom_row(input, m, m.margin, w - m.margin, bottom_y);
    let panel = Rect::new(m.margin, m.margin, column, bottom_y - m.gap - m.margin);
    let pad = m.font * 0.9;
    let inner_w = column - 2.0 * pad;
    let title = Rect::new(panel.x + pad, panel.y + pad * 0.5, inner_w, m.font * 2.2);
    let puzzle_id = Rect::new(panel.x + pad, title.bottom(), inner_w, m.font * 1.6);
    let small = m.font * 0.85;
    let subtitle = Rect::new(
        panel.x + pad,
        puzzle_id.bottom() + m.gap,
        inner_w,
        small * 1.35 * 3.0,
    );
    let help_h = 3.0 * (small * 1.35 * 2.0 + m.gap);
    let help_y = subtitle.bottom() + m.gap * 2.0;
    let help = (!input.touch && help_y + help_h <= panel.bottom() - pad)
        .then(|| Rect::new(panel.x + pad, help_y, inner_w, help_h));
    let subtitle = (subtitle.bottom() <= panel.bottom() - pad).then_some(subtitle);

    let y0 = m.margin + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let x0 = panel.right() + m.gap;
    let x1 = w - m.margin;
    let fits = |cell: f32| {
        let (bw, bh) = board_size(input, cell);
        let tray_w = x1 - x0 - bw - m.gap;
        bh <= y1 - y0
            && tray_w > 0.0
            && tray_size(input, tray_w, y1 - y0, m.gap, cell).2 >= min_card(input)
    };
    let cell = largest_cell(fits);
    let (bw, bh) = board_size(input, cell);
    let grid = tray_size(input, x1 - x0 - bw - m.gap, y1 - y0, m.gap, cell);
    let (tw, th) = tray_extent(grid, m.gap);
    let tray = tray_at(
        input,
        Point::new(x1 - tw, y0 + ((y1 - y0) - th).max(0.0) / 2.0),
        grid,
        m.gap,
        cell,
    );
    let board_x = x0 + ((x1 - tw - m.gap - x0) - bw).max(0.0) / 2.0;
    let board = board_at(
        input,
        Point::new(board_x, y0 + ((y1 - y0) - bh).max(0.0) / 2.0),
        cell,
    );
    let (banner, next) = banner(m, board.frame);
    Layout {
        wide: true,
        font: m.font,
        button_height: m.button,
        panel: Some(panel),
        title,
        puzzle_id,
        subtitle,
        help,
        tabs,
        lang,
        share,
        notice: Point::new(board.frame.center().x, board.frame.bottom() - m.button),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        tray: tray.rect,
        cards: tray.cards,
        tray_cell: tray.cell,
        reset: bottom.reset,
        counters: bottom.counters,
        tab_hint: bottom.hint,
        new: bottom.new,
        banner,
        next,
    }
}

fn tall(input: &LayoutInput, m: &Metrics) -> Layout {
    let (w, h) = (input.width, input.height);
    let t = |key| text(input.lang, key);
    let x0 = m.margin;
    let x1 = w - m.margin;

    let share_w = m.button_width(t(Text::Share));
    let lang_w = m.switch_width("EN", "UA");
    let widths = squeeze(&[share_w, lang_w], (x1 - x0) * 0.6);
    let share = Rect::new(x1 - widths[0], m.margin, widths[0], m.button);
    let lang = Rect::new(share.x - m.gap - widths[1], m.margin, widths[1], m.button);
    let title_w = (lang.x - m.gap - x0).max(0.0);
    let title = Rect::new(x0, m.margin, title_w, m.button * 0.55);
    let puzzle_id = Rect::new(x0, title.bottom(), title_w, m.button * 0.45);

    let tabs_y = m.margin + m.button + m.gap;
    let tabs = tab_rects(input, m, x0, x1, tabs_y);
    let bottom_y = h - m.margin - m.button;
    let bottom = bottom_row(input, m, x0, x1, bottom_y);

    let y0 = tabs_y + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let fits = |cell: f32| {
        let (bw, bh) = board_size(input, cell);
        let tray_h = y1 - y0 - bh - m.gap;
        bw <= x1 - x0
            && tray_h > 0.0
            && tray_size(input, x1 - x0, tray_h, m.gap, cell).2 >= min_card(input)
    };
    let cell = largest_cell(fits);
    let (bw, bh) = board_size(input, cell);
    let grid = tray_size(input, x1 - x0, y1 - y0 - bh - m.gap, m.gap, cell);
    let (tw, th) = tray_extent(grid, m.gap);
    let stack = bh + m.gap + th;
    let top = y0 + ((y1 - y0) - stack).max(0.0) / 2.0;
    let board = board_at(
        input,
        Point::new(x0 + ((x1 - x0) - bw).max(0.0) / 2.0, top),
        cell,
    );
    let tray = tray_at(
        input,
        Point::new(x0 + ((x1 - x0) - tw).max(0.0) / 2.0, top + bh + m.gap),
        grid,
        m.gap,
        cell,
    );
    let (banner, next) = banner(m, board.frame);
    Layout {
        wide: false,
        font: m.font,
        button_height: m.button,
        panel: None,
        title,
        puzzle_id,
        subtitle: None,
        help: None,
        tabs,
        lang,
        share,
        notice: Point::new(board.frame.center().x, board.frame.bottom() - m.button),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        tray: tray.rect,
        cards: tray.cards,
        tray_cell: tray.cell,
        reset: bottom.reset,
        counters: bottom.counters,
        tab_hint: bottom.hint,
        new: bottom.new,
        banner,
        next,
    }
}
