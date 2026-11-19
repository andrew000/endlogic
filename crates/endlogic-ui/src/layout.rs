//! Screen layout, computed from the area size alone.
//!
//! In the wide arrangement the left panel and the tray column have the same
//! width, so the difficulty tabs, the board with its counters, the counter
//! switch and the win banner all share one vertical axis: the middle of the
//! area. In the tall arrangement everything is stacked and centered on the
//! same axis.

use crate::painter::{Point, Rect};
use crate::text::{Lang, Text, difficulty_label, text};
use endlogic_core::generator::Difficulty;

/// Smallest side of anything that can be tapped.
pub const MIN_TARGET: f32 = 44.0;

/// Width of the counter band beside the board, in cells.
const BAND: f32 = 1.0;
/// Space between the grid and its counters, in cells.
const COUNTER_GAP: f32 = 0.15;
/// Space between the grid and the corner brackets, in cells.
const FRAME_PAD: f32 = 0.1;
/// Largest board cell, in logical pixels.
const MAX_CELL: f32 = 96.0;
/// Smallest board cell.
const MIN_CELL: f32 = 6.0;
/// Smallest tray card.
const MIN_CARD: f32 = 56.0;
/// Widest the tray may be in the wide arrangement, as a share of the width.
pub const MAX_TRAY_SHARE: f32 = 0.24;
/// Largest piece cell in the tray, as a share of the board cell.
pub const TRAY_SCALE: f32 = 0.5;

/// What the layout depends on besides the area size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutInput {
    /// Area width.
    pub width: f32,
    /// Area height.
    pub height: f32,
    /// Main input is touch: no keyboard help or key hints.
    pub touch: bool,
    /// Language of the labels, which decides widths and line breaks.
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

/// Text broken into lines, drawn from the top of `rect`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    /// Where the lines go.
    pub rect: Rect,
    /// The lines, top to bottom.
    pub lines: Vec<String>,
    /// Font size.
    pub size: f32,
}

impl TextBlock {
    /// Height of one line.
    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.size * LINE
    }
}

/// One row of the controls help.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpRow {
    /// Box the icon is centered in. All rows share its horizontal position,
    /// and it is centered vertically on the text.
    pub icon: Rect,
    /// The explanation.
    pub text: TextBlock,
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
    /// The shared vertical axis.
    pub axis: f32,
    /// The left panel, in the wide arrangement; as tall as its content.
    pub panel: Option<Rect>,
    /// `// ENDLOGIC`.
    pub title: Rect,
    /// The puzzle id line.
    pub puzzle_id: Rect,
    /// The subtitle, when there is room.
    pub subtitle: Option<TextBlock>,
    /// The three help rows; empty when the help is not shown.
    pub help: Vec<HelpRow>,
    /// Difficulty tabs, easiest first.
    pub tabs: [Rect; 4],
    /// Language switch, `EN | UA`.
    pub lang: Rect,
    /// Share button.
    pub share: Rect,
    /// Board frame: the cells plus a small margin, marked by corner brackets.
    pub frame: Rect,
    /// The cells.
    pub cells: Rect,
    /// Side of one cell.
    pub cell: f32,
    /// Row counters, left of the cells.
    pub row_counters: Rect,
    /// Column counters, above the cells.
    pub col_counters: Rect,
    /// Tray background.
    pub tray: Rect,
    /// One card per piece, in piece order, all the same size.
    pub cards: Vec<Rect>,
    /// Side of one cell of a piece drawn in the tray, the same for every
    /// card and at most half the board cell.
    pub tray_cell: f32,
    /// Reset button.
    pub reset: Rect,
    /// Counter display switch, `Grid | Numbers`.
    pub counters: Rect,
    /// The `TAB` key hint next to the switch, when shown.
    pub tab_hint: Option<Rect>,
    /// New puzzle button.
    pub new: Rect,
    /// Win banner: under the board (wide) or over the empty tray (tall).
    pub banner: Rect,
    /// Next button inside the banner.
    pub next: Rect,
    /// Center of toasts.
    pub notice: Point,
}

/// Line height as a multiple of the font size.
const LINE: f32 = 1.35;

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

    /// The difficulty tabs as one rectangle.
    #[must_use]
    pub fn tab_bar(&self) -> Rect {
        let first = self.tabs[0];
        Rect::new(first.x, first.y, self.tabs[3].right() - first.x, first.h)
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
    /// names for test messages. Toasts are not included; the banner is
    /// included in the wide arrangement, where it has a place of its own.
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
        if self.wide {
            rects.push(("banner", self.banner));
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

    /// Height of the win banner: one row with the text and the button.
    fn banner_height(&self) -> f32 {
        self.button + 2.0 * self.gap
    }
}

/// A cautious text width for layout; drawing shrinks text that is wider.
fn estimate_width(text: &str, size: f32) -> f32 {
    to_f32(text.chars().count()) * size * 0.62
}

/// Breaks `text` into lines whose estimated width fits `max_width`.
fn wrap(text: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || estimate_width(&candidate, size) <= max_width {
            line = candidate;
        } else {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn text_block(text: &str, x: f32, y: f32, width: f32, size: f32) -> TextBlock {
    let lines = wrap(text, size, width);
    let height = to_f32(lines.len()) * size * LINE;
    TextBlock {
        rect: Rect::new(x, y, width, height),
        lines,
        size,
    }
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

/// Four equal tabs centered on `axis`, no wider than `2 * half`.
fn tab_rects(input: &LayoutInput, m: &Metrics, axis: f32, half: f32, y: f32) -> [Rect; 4] {
    let widest = Difficulty::ALL
        .iter()
        .map(|&d| estimate_width(difficulty_label(input.lang, d), m.font) + 1.6 * m.pad())
        .fold(MIN_TARGET, f32::max);
    let tab = squeeze(&[widest * 4.0], 2.0 * half)[0].max(4.0 * MIN_TARGET) / 4.0;
    let start = axis - 2.0 * tab;
    core::array::from_fn(|k| Rect::new(start + tab * to_f32(k), y, tab, m.button))
}

/// The bottom row: reset on the left, the counter switch centered on the
/// axis (with its key hint to its left), new on the right.
struct BottomRow {
    reset: Rect,
    counters: Rect,
    hint: Option<Rect>,
    new: Rect,
}

fn bottom_row(input: &LayoutInput, m: &Metrics, x0: f32, x1: f32, y: f32) -> BottomRow {
    let t = |key| text(input.lang, key);
    let axis = f32::midpoint(x0, x1);
    let ideal = [
        m.button_width(t(Text::Reset)),
        m.switch_width(t(Text::Grid), t(Text::Numbers)),
        m.button_width(t(Text::New)),
    ];
    // The switch is centered, so each side gets half of what is left.
    let side = ideal[0].max(ideal[2]);
    let widths = squeeze(&[side, ideal[1], side], x1 - x0 - 2.0 * m.gap);
    let reset = Rect::new(x0, y, ideal[0].min(widths[0]), m.button);
    let new_w = ideal[2].min(widths[2]);
    let new = Rect::new(x1 - new_w, y, new_w, m.button);
    let counters = Rect::new(axis - widths[1] / 2.0, y, widths[1], m.button);
    let hint_w = m.hint_width();
    let hint_x = counters.x - m.gap * 0.5 - hint_w;
    let hint = (!input.touch && hint_x >= reset.right() + m.gap)
        .then(|| Rect::new(hint_x, y + m.button * 0.2, hint_w, m.button * 0.6));
    BottomRow {
        reset,
        counters,
        hint,
        new,
    }
}

/// Board size for a cell size: `(width, height)` including counters.
fn board_size(input: &LayoutInput, cell: f32) -> (f32, f32) {
    let extra = (BAND + COUNTER_GAP + FRAME_PAD) * cell;
    (
        to_f32(input.grid_width) * cell + extra,
        to_f32(input.grid_height) * cell + extra,
    )
}

/// Board rectangles.
struct Board {
    frame: Rect,
    cells: Rect,
    row_counters: Rect,
    col_counters: Rect,
}

/// The board block of `cell` centered on `axis` with its top at `top`.
fn board_at(input: &LayoutInput, axis: f32, top: f32, cell: f32) -> Board {
    let (block_w, _) = board_size(input, cell);
    let band = BAND * cell;
    let offset = band + COUNTER_GAP * cell;
    let cells = Rect::new(
        axis - block_w / 2.0 + offset,
        top + offset,
        to_f32(input.grid_width) * cell,
        to_f32(input.grid_height) * cell,
    );
    Board {
        frame: cells.inset(-FRAME_PAD * cell),
        row_counters: Rect::new(cells.x - offset, cells.y, band, cells.h),
        col_counters: Rect::new(cells.x, cells.y - offset, cells.w, band),
        cells,
    }
}

/// Smallest acceptable card for pieces with this longest side.
fn min_card(input: &LayoutInput) -> f32 {
    MIN_CARD.max(to_f32(input.max_piece_side) * 9.0)
}

/// Cards laid out `cols` per row, top-left at `origin`.
struct Tray {
    rect: Rect,
    cards: Vec<Rect>,
}

fn tray_at(origin: Point, n: usize, cols: usize, card: f32, gap: f32) -> Tray {
    let cols = cols.max(1);
    let rows = n.div_ceil(cols).max(1);
    let rect = Rect::new(
        origin.x,
        origin.y,
        to_f32(cols) * card + to_f32(cols + 1) * gap,
        to_f32(rows) * card + to_f32(rows + 1) * gap,
    );
    let cards = (0..n)
        .map(|i| {
            Rect::new(
                origin.x + gap + to_f32(i % cols) * (card + gap),
                origin.y + gap + to_f32(i / cols) * (card + gap),
                card,
                card,
            )
        })
        .collect();
    Tray { rect, cards }
}

/// The best `(cols, card)` for `n` square cards in a `w`×`h` area, with
/// `gap` around and between them.
fn card_grid(n: usize, w: f32, h: f32, gap: f32) -> (usize, f32) {
    let n = n.max(1);
    let mut best = (n, 0.0);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let side = ((w - gap * to_f32(cols + 1)) / to_f32(cols))
            .min((h - gap * to_f32(rows + 1)) / to_f32(rows));
        if side > best.1 {
            best = (cols, side);
        }
    }
    best
}

fn tray_cell(input: &LayoutInput, card: f32, cell: f32) -> f32 {
    (card * 0.8 / to_f32(input.max_piece_side.max(1))).min(cell * TRAY_SCALE)
}

/// The banner with its next button, `rect` given.
fn banner_parts(m: &Metrics, rect: Rect) -> (Rect, Rect) {
    let next_w = (m.font * 7.0).max(MIN_TARGET * 2.0).min(rect.w * 0.45);
    let next = Rect::new(
        rect.right() - m.gap - next_w,
        rect.center().y - m.button / 2.0,
        next_w,
        m.button,
    );
    (rect, next)
}

/// The left panel content: title, id, subtitle and (when it fits) help.
struct Panel {
    rect: Rect,
    title: Rect,
    puzzle_id: Rect,
    subtitle: Option<TextBlock>,
    help: Vec<HelpRow>,
}

fn panel(input: &LayoutInput, m: &Metrics, x: f32, width: f32, max_bottom: f32) -> Panel {
    let pad = m.font * 0.9;
    let inner_x = x + pad;
    let inner_w = (width - 2.0 * pad).max(1.0);
    let title = Rect::new(inner_x, m.margin + pad * 0.5, inner_w, m.font * 2.2);
    let puzzle_id = Rect::new(inner_x, title.bottom(), inner_w, m.font * 1.6);
    let small = m.font * 0.85;
    let mut bottom = puzzle_id.bottom();

    let subtitle = text_block(
        text(input.lang, Text::Subtitle),
        inner_x,
        bottom + m.gap,
        inner_w,
        small,
    );
    let subtitle = (subtitle.rect.bottom() + pad <= max_bottom).then(|| {
        bottom = subtitle.rect.bottom();
        subtitle
    });

    let mut help = Vec::new();
    if !input.touch && subtitle.is_some() {
        let icon_w = (m.font * 2.8).min(inner_w * 0.3);
        let icon_h = m.font * 1.9;
        let text_x = inner_x + icon_w + m.gap;
        let text_w = (inner_w - icon_w - m.gap).max(1.0);
        let mut y = bottom + m.gap;
        for key in [Text::HelpDrag, Text::HelpRotate, Text::HelpDisplay] {
            let block = text_block(text(input.lang, key), text_x, y, text_w, small);
            let row_h = block.rect.h.max(icon_h);
            // The text is centered in the row, the icon on the text.
            let text_y = y + (row_h - block.rect.h) / 2.0;
            let block = TextBlock {
                rect: Rect::new(text_x, text_y, text_w, block.rect.h),
                ..block
            };
            let icon = Rect::new(
                inner_x,
                block.rect.center().y - icon_h / 2.0,
                icon_w,
                icon_h,
            );
            help.push(HelpRow { icon, text: block });
            y += row_h + m.gap;
        }
        let help_bottom = y - m.gap;
        if help_bottom + pad <= max_bottom {
            bottom = help_bottom;
        } else {
            help.clear();
        }
    }
    Panel {
        rect: Rect::new(x, m.margin, width, bottom + pad - m.margin),
        title,
        puzzle_id,
        subtitle,
        help,
    }
}

fn wide(input: &LayoutInput, m: &Metrics) -> Layout {
    let (w, h) = (input.width, input.height);
    let t = |key| text(input.lang, key);
    let axis = w / 2.0;
    // Equal side columns keep the axis in the middle of the area.
    let side = (w * 0.19).clamp(150.0, 340.0).min(w * MAX_TRAY_SHARE);
    let bottom_y = h - m.margin - m.button;

    let share_w = m.button_width(t(Text::Share));
    let share = Rect::new(w - m.margin - share_w, m.margin, share_w, m.button);
    let lang_w = m.switch_width("EN", "UA");
    let lang = Rect::new(share.x - m.gap - lang_w, m.margin, lang_w, m.button);
    let half = (axis - (m.margin + side + m.gap)).min(lang.x - m.gap - axis);
    let tabs = tab_rects(input, m, axis, half, m.margin);
    let bottom = bottom_row(input, m, m.margin, w - m.margin, bottom_y);
    let panel = panel(input, m, m.margin, side, bottom_y - m.gap);

    // The board, with room for the banner under it, between the columns.
    let y0 = m.margin + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let center_w = w - 2.0 * (m.margin + side + m.gap);
    let banner_h = m.banner_height();
    let (unit_w, unit_h) = board_size(input, 1.0);
    let cell = (center_w / unit_w)
        .min((y1 - y0 - banner_h - m.gap) / unit_h)
        .clamp(MIN_CELL, MAX_CELL)
        .floor();
    let (block_w, block_h) = board_size(input, cell);
    let stack = block_h + m.gap + banner_h;
    let top = y0 + ((y1 - y0) - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, top, cell);
    let banner_w = block_w.max(MIN_TARGET * 7.0).min(center_w.max(block_w));
    let (banner, next) = banner_parts(
        m,
        Rect::new(
            axis - banner_w / 2.0,
            top + block_h + m.gap,
            banner_w,
            banner_h,
        ),
    );

    // The tray: two cards per row in the right column.
    let rows = input.pieces.max(1).div_ceil(2);
    let card = ((side - 3.0 * m.gap) / 2.0)
        .min((y1 - y0 - to_f32(rows + 1) * m.gap) / to_f32(rows))
        .max(1.0);
    let tray_w = 2.0 * card + 3.0 * m.gap;
    let tray_h = to_f32(rows) * card + to_f32(rows + 1) * m.gap;
    let tray = tray_at(
        Point::new(
            w - m.margin - side + (side - tray_w) / 2.0,
            y0 + ((y1 - y0) - tray_h).max(0.0) / 2.0,
        ),
        input.pieces,
        2,
        card,
        m.gap,
    );
    Layout {
        wide: true,
        font: m.font,
        button_height: m.button,
        axis,
        panel: Some(panel.rect),
        title: panel.title,
        puzzle_id: panel.puzzle_id,
        subtitle: panel.subtitle,
        help: panel.help,
        tabs,
        lang,
        share,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        tray: tray.rect,
        cards: tray.cards,
        tray_cell: tray_cell(input, card, cell),
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
    let axis = w / 2.0;

    let share_w = m.button_width(t(Text::Share));
    let lang_w = m.switch_width("EN", "UA");
    let widths = squeeze(&[share_w, lang_w], (x1 - x0) * 0.6);
    let share = Rect::new(x1 - widths[0], m.margin, widths[0], m.button);
    let lang = Rect::new(share.x - m.gap - widths[1], m.margin, widths[1], m.button);
    let title_w = (lang.x - m.gap - x0).max(0.0);
    let title = Rect::new(x0, m.margin, title_w, m.button * 0.55);
    let puzzle_id = Rect::new(x0, title.bottom(), title_w, m.button * 0.45);

    let tabs_y = m.margin + m.button + m.gap;
    let tabs = tab_rects(input, m, axis, (x1 - x0) / 2.0, tabs_y);
    let bottom_y = h - m.margin - m.button;
    let bottom = bottom_row(input, m, x0, x1, bottom_y);

    let y0 = tabs_y + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let tray_area = |cell: f32| {
        let (_, block_h) = board_size(input, cell);
        let tray_h = (y1 - y0 - block_h - m.gap).max(0.0);
        card_grid(input.pieces, x1 - x0, tray_h, m.gap)
    };
    let fits = |cell: f32| {
        let (block_w, _) = board_size(input, cell);
        block_w <= x1 - x0 && tray_area(cell).1 >= min_card(input)
    };
    let cell = largest(fits);
    let (_, block_h) = board_size(input, cell);
    let (cols, card) = tray_area(cell);
    let card = card.max(1.0);
    let rows = input.pieces.max(1).div_ceil(cols);
    let tray_w = to_f32(cols) * card + to_f32(cols + 1) * m.gap;
    let tray_h = to_f32(rows) * card + to_f32(rows + 1) * m.gap;
    let stack = block_h + m.gap + tray_h;
    let top = y0 + ((y1 - y0) - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, top, cell);
    let tray = tray_at(
        Point::new(axis - tray_w / 2.0, top + block_h + m.gap),
        input.pieces,
        cols,
        card,
        m.gap,
    );
    // After a win the tray is empty: the banner takes its place.
    let banner_h = m.banner_height().min(tray.rect.h);
    let banner_w = tray.rect.w.max((x1 - x0).min(MIN_TARGET * 7.0));
    let (banner, next) = banner_parts(
        m,
        Rect::new(
            axis - banner_w / 2.0,
            tray.rect.center().y - banner_h / 2.0,
            banner_w,
            banner_h,
        ),
    );
    Layout {
        wide: false,
        font: m.font,
        button_height: m.button,
        axis,
        panel: None,
        title,
        puzzle_id,
        subtitle: None,
        help: Vec::new(),
        tabs,
        lang,
        share,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        tray: tray.rect,
        cards: tray.cards,
        tray_cell: tray_cell(input, card, cell),
        reset: bottom.reset,
        counters: bottom.counters,
        tab_hint: bottom.hint,
        new: bottom.new,
        banner,
        next,
    }
}

/// The largest cell size in `MIN_CELL..=MAX_CELL` for which `fits` holds,
/// or `MIN_CELL` if none does.
fn largest(fits: impl Fn(f32) -> bool) -> f32 {
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
