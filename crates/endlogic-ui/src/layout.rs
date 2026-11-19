//! Screen layout, computed from the area size and safe-area insets alone.
//!
//! There are three arrangements:
//!
//! - [`Arrangement::Wide`] (a monitor): a left panel and a tray column of
//!   the same width, controls on a top and a bottom row;
//! - [`Arrangement::LowWide`] (a phone on its side): no top or bottom row,
//!   every control in two equal side columns, the board using the full
//!   height;
//! - [`Arrangement::Tall`] (a phone upright): a slim header, tabs, the
//!   board, the tray and a bottom row, stacked.
//!
//! In all of them the grid of cells is centered on one vertical axis: the
//! row counters take a band on its left and an empty band of the same width
//! is kept on its right.

use crate::painter::{Point, Rect};
use crate::shell::Insets;
use crate::text::{Lang, Text, difficulty_label, text};
use endlogic_core::generator::Difficulty;

/// Smallest side of the touch zone of anything that can be tapped.
pub const MIN_TARGET: f32 = 44.0;
/// Below this height (logical pixels) a wide area gets the low arrangement.
pub const LOW_WIDE_HEIGHT: f32 = 520.0;

/// Depth of a counter band, in cells.
pub const BAND: f32 = 0.75;
/// Space between the grid and its counters, in cells.
pub const COUNTER_GAP: f32 = 0.15;
/// Space between the grid and the corner brackets, in cells.
const FRAME_PAD: f32 = 0.1;
/// Largest board cell, in logical pixels.
const MAX_CELL: f32 = 96.0;
/// Smallest board cell.
const MIN_CELL: f32 = 6.0;
/// Widest the tray may be in the wide arrangement, as a share of the width.
pub const MAX_TRAY_SHARE: f32 = 0.24;
/// Largest piece cell in the tray, as a share of the board cell.
pub const TRAY_SCALE: f32 = 0.5;
/// Smallest piece cell in the tray in the tall arrangement, as a share of
/// the board cell.
pub const TRAY_MIN_SCALE: f32 = 1.0 / 3.0;
/// Space between tray cards in the low wide arrangement.
const LOW_TRAY_GAP: f32 = 4.0;
/// Share of a card a piece may span.
const TRAY_FILL: f32 = 0.88;

/// Which arrangement a layout uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arrangement {
    /// Landscape with room for a top and a bottom row.
    Wide,
    /// Landscape with little height: controls in two side columns.
    LowWide,
    /// Portrait: everything stacked.
    Tall,
}

/// What the layout depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutInput {
    /// Area width.
    pub width: f32,
    /// Area height.
    pub height: f32,
    /// Safe-area insets; nothing that can be tapped goes there.
    pub insets: Insets,
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

/// Where everything goes. Rectangles are what is drawn; touch zones come
/// from [`Layout::zone`].
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The arrangement.
    pub kind: Arrangement,
    /// Font size of buttons and tabs.
    pub font: f32,
    /// Font size of the title.
    pub title_font: f32,
    /// Font size of the puzzle id.
    pub id_font: f32,
    /// The area inside the safe-area insets.
    pub safe: Rect,
    /// The vertical axis the grid is centered on.
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
    /// Difficulty tabs, easiest first: one row, or a 2×2 grid.
    pub tabs: [Rect; 4],
    /// Language switch, `EN | UA`.
    pub lang: Rect,
    /// Share button.
    pub share: Rect,
    /// The share button carries the short label.
    pub share_short: bool,
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
    /// The empty band right of the cells, as wide as the row counters.
    pub right_band: Rect,
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
    /// Win banner: under the board (wide) or over the empty tray.
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
        let input = LayoutInput {
            width: w,
            height: h,
            ..*input
        };
        if w >= h * 1.2 {
            if h < LOW_WIDE_HEIGHT {
                low_wide(&input)
            } else {
                wide(&input)
            }
        } else {
            tall(&input)
        }
    }

    /// The touch zone of a control: `rect` grown around its center to at
    /// least [`MIN_TARGET`] in each direction.
    #[must_use]
    pub fn zone(rect: Rect) -> Rect {
        let w = rect.w.max(MIN_TARGET);
        let h = rect.h.max(MIN_TARGET);
        Rect::centered(rect.center(), w, h)
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

    /// Board, counters and the reserved right band together.
    #[must_use]
    pub fn board_block(&self) -> Rect {
        let x = self.row_counters.x;
        let y = self.col_counters.y;
        Rect::new(x, y, self.right_band.right() - x, self.frame.bottom() - y)
    }

    /// The difficulty tabs as one rectangle.
    #[must_use]
    pub fn tab_bar(&self) -> Rect {
        let first = self.tabs[0];
        let right = self.tabs.iter().map(|t| t.right()).fold(first.x, f32::max);
        let bottom = self.tabs.iter().map(|t| t.bottom()).fold(first.y, f32::max);
        Rect::new(first.x, first.y, right - first.x, bottom - first.y)
    }

    /// Whether the tabs form one row.
    #[must_use]
    pub fn tabs_in_a_row(&self) -> bool {
        self.tabs.iter().all(|t| (t.y - self.tabs[0].y).abs() < 0.5)
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
        if self.kind == Arrangement::Wide {
            rects.push(("banner", self.banner));
        }
        rects
    }

    /// Touch zones of everything that reacts to a tap while playing, with
    /// names for test messages. The next button is left out: it only
    /// exists after a win, over the then empty tray or under the board.
    #[must_use]
    pub fn targets(&self) -> Vec<(&'static str, Rect)> {
        let (grid, numbers) = Layout::halves(self.counters);
        let (en, ua) = Layout::halves(self.lang);
        let mut targets: Vec<(&'static str, Rect)> = [
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
        ]
        .into_iter()
        .map(|(name, rect)| (name, Layout::zone(rect)))
        .collect();
        targets.extend(self.cards.iter().map(|&card| ("card", Layout::zone(card))));
        targets
    }
}

// ---- shared helpers ----

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

/// Width of a button labelled `label` in `font`.
fn button_width(label: &str, font: f32) -> f32 {
    (estimate_width(label, font) + 1.8 * font).max(MIN_TARGET)
}

/// Width of a two-way switch: both halves as wide as the wider label.
fn switch_width(a: &str, b: &str, font: f32) -> f32 {
    2.0 * button_width(a, font).max(button_width(b, font))
}

fn share_label(lang: Lang, short: bool) -> &'static str {
    text(lang, if short { Text::ShareShort } else { Text::Share })
}

/// Width and height of the board block for a cell size: counters band,
/// grid and reserved band across; counters band, grid and frame margin
/// down.
fn board_size(input: &LayoutInput, cell: f32) -> (f32, f32) {
    let side = BAND + COUNTER_GAP;
    (
        (to_f32(input.grid_width) + 2.0 * side) * cell,
        (to_f32(input.grid_height) + side + FRAME_PAD) * cell,
    )
}

/// Board rectangles.
struct Board {
    frame: Rect,
    cells: Rect,
    row_counters: Rect,
    col_counters: Rect,
    right_band: Rect,
}

/// The board with its grid centered on `axis` and its block's top at `top`.
fn board_at(input: &LayoutInput, axis: f32, top: f32, cell: f32) -> Board {
    let band = BAND * cell;
    let offset = band + COUNTER_GAP * cell;
    let width = to_f32(input.grid_width) * cell;
    let cells = Rect::new(
        axis - width / 2.0,
        top + offset,
        width,
        to_f32(input.grid_height) * cell,
    );
    Board {
        frame: cells.inset(-FRAME_PAD * cell),
        row_counters: Rect::new(cells.x - offset, cells.y, band, cells.h),
        col_counters: Rect::new(cells.x, cells.y - offset, cells.w, band),
        right_band: Rect::new(cells.right() + COUNTER_GAP * cell, cells.y, band, cells.h),
        cells,
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

/// `n` square cards of side `card`, `cols` per row, the grid centered on
/// `axis` with its top at `top`, `gap` around and between them.
fn cards_at(n: usize, cols: usize, card: f32, gap: f32, axis: f32, top: f32) -> Vec<Rect> {
    let cols = cols.max(1);
    let row_w = to_f32(cols) * card + to_f32(cols - 1) * gap;
    let x0 = axis - row_w / 2.0;
    (0..n)
        .map(|i| {
            Rect::new(
                x0 + to_f32(i % cols) * (card + gap),
                top + gap + to_f32(i / cols) * (card + gap),
                card,
                card,
            )
        })
        .collect()
}

fn tray_cell(input: &LayoutInput, card: f32, cell: f32) -> f32 {
    (card * TRAY_FILL / to_f32(input.max_piece_side.max(1))).min(cell * TRAY_SCALE)
}

/// The banner with its next button: one row (text, then the button) when
/// wide enough, otherwise the text above a centered button.
fn banner_parts(rect: Rect, font: f32, gap: f32) -> (Rect, Rect) {
    let next_w = (font * 7.0).max(MIN_TARGET * 2.0).min(rect.w - 2.0 * gap);
    let next = if rect.w >= 300.0 {
        Rect::new(
            rect.right() - gap - next_w,
            rect.center().y - MIN_TARGET / 2.0,
            next_w,
            MIN_TARGET,
        )
    } else {
        Rect::new(
            rect.center().x - next_w / 2.0,
            rect.bottom() - gap - MIN_TARGET,
            next_w,
            MIN_TARGET,
        )
    };
    (rect, next)
}

/// Height a banner of `width` needs.
fn banner_height(width: f32, gap: f32) -> f32 {
    if width >= 300.0 {
        MIN_TARGET + 2.0 * gap
    } else {
        2.0 * MIN_TARGET + 3.0 * gap
    }
}

// ---- wide ----

/// Sizes of the wide arrangement.
struct Metrics {
    margin: f32,
    gap: f32,
    button: f32,
    font: f32,
}

impl Metrics {
    fn pad(&self) -> f32 {
        self.font * 0.9
    }

    fn button_width(&self, label: &str) -> f32 {
        (estimate_width(label, self.font) + 2.0 * self.pad()).max(MIN_TARGET)
    }

    fn switch_width(&self, a: &str, b: &str) -> f32 {
        2.0 * self.button_width(a).max(self.button_width(b))
    }
}

/// Four equal tabs in a row centered on `axis`, no wider than `2 * half`.
fn tab_row(lang: Lang, font: f32, pad: f32, axis: f32, half: f32, y: f32, h: f32) -> [Rect; 4] {
    let widest = Difficulty::ALL
        .iter()
        .map(|&d| estimate_width(difficulty_label(lang, d), font) + 1.6 * pad)
        .fold(MIN_TARGET, f32::max);
    let tab = squeeze(&[widest * 4.0], 2.0 * half)[0].max(4.0 * MIN_TARGET) / 4.0;
    let start = axis - 2.0 * tab;
    core::array::from_fn(|k| Rect::new(start + tab * to_f32(k), y, tab, h))
}

/// The bottom row: reset on the left, the counter switch centered on the
/// axis (with its key hint to its left), new on the right.
struct BottomRow {
    reset: Rect,
    counters: Rect,
    hint: Option<Rect>,
    new: Rect,
}

#[allow(clippy::too_many_arguments)]
fn bottom_row(
    input: &LayoutInput,
    font: f32,
    gap: f32,
    x0: f32,
    x1: f32,
    axis: f32,
    y: f32,
    h: f32,
) -> BottomRow {
    let t = |key| text(input.lang, key);
    let reset_w = button_width(t(Text::Reset), font);
    let new_w = button_width(t(Text::New), font);
    let ideal = switch_width(t(Text::Grid), t(Text::Numbers), font);
    // The switch is centered on the axis, so it may use twice the room left
    // on its narrower side.
    let room = 2.0 * ((axis - (x0 + reset_w + gap)).min(x1 - new_w - gap - axis));
    let counters_w = ideal.min(room).max(2.0 * MIN_TARGET);
    let reset = Rect::new(x0, y, reset_w, h);
    let new = Rect::new(x1 - new_w, y, new_w, h);
    let counters = Rect::new(axis - counters_w / 2.0, y, counters_w, h);
    let hint_w = estimate_width("TAB", font * 0.75) + font;
    let hint_x = counters.x - gap * 0.5 - hint_w;
    let hint = (!input.touch && hint_x >= reset.right() + gap)
        .then(|| Rect::new(hint_x, y + h * 0.2, hint_w, h * 0.6));
    BottomRow {
        reset,
        counters,
        hint,
        new,
    }
}

/// The left panel content: title, id, subtitle and (when it fits) help.
struct Panel {
    rect: Rect,
    title: Rect,
    puzzle_id: Rect,
    subtitle: Option<TextBlock>,
    help: Vec<HelpRow>,
}

fn panel(input: &LayoutInput, m: &Metrics, x: f32, top: f32, width: f32, max_bottom: f32) -> Panel {
    let pad = m.font * 0.9;
    let inner_x = x + pad;
    let inner_w = (width - 2.0 * pad).max(1.0);
    let title = Rect::new(inner_x, top + pad * 0.5, inner_w, m.font * 2.2);
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
        rect: Rect::new(x, top, width, bottom + pad - top),
        title,
        puzzle_id,
        subtitle,
        help,
    }
}

fn safe_rect(input: &LayoutInput) -> Rect {
    let i = input.insets;
    Rect::new(
        i.left,
        i.top,
        input.width - i.left - i.right,
        input.height - i.top - i.bottom,
    )
}

fn wide(input: &LayoutInput) -> Layout {
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
    // Equal side columns keep the axis in the middle.
    let side = (safe.w * 0.19)
        .clamp(150.0, 340.0)
        .min(input.width * MAX_TRAY_SHARE);
    let bottom_y = safe.bottom() - m.margin - m.button;

    let share_w = m.button_width(t(Text::Share));
    let share = Rect::new(x1 - share_w, top, share_w, m.button);
    let lang_w = m.switch_width("EN", "UA");
    let lang = Rect::new(share.x - m.gap - lang_w, top, lang_w, m.button);
    let half = (axis - (x0 + side + m.gap)).min(lang.x - m.gap - axis);
    let tabs = tab_row(input.lang, m.font, m.pad(), axis, half, top, m.button);
    let bottom = bottom_row(input, m.font, m.gap, x0, x1, axis, bottom_y, m.button);
    let panel = panel(input, &m, x0, top, side, bottom_y - m.gap);

    // The board, with room for the banner under it, between the columns.
    let y0 = top + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let center_w = (x1 - x0) - 2.0 * (side + m.gap);
    let banner_h = banner_height(400.0, m.gap);
    let (unit_w, unit_h) = board_size(input, 1.0);
    let cell = (center_w / unit_w)
        .min((y1 - y0 - banner_h - m.gap) / unit_h)
        .clamp(MIN_CELL, MAX_CELL)
        .floor();
    let (block_w, block_h) = board_size(input, cell);
    let stack = block_h + m.gap + banner_h;
    let board_top = y0 + ((y1 - y0) - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, board_top, cell);
    let banner_w = block_w.max(MIN_TARGET * 7.0).min(center_w.max(block_w));
    let (banner, next) = banner_parts(
        Rect::new(
            axis - banner_w / 2.0,
            board_top + block_h + m.gap,
            banner_w,
            banner_h,
        ),
        m.font,
        m.gap,
    );

    // The tray: two cards per row in the right column.
    let rows = input.pieces.max(1).div_ceil(2);
    let card = ((side - 3.0 * m.gap) / 2.0)
        .min((y1 - y0 - to_f32(rows + 1) * m.gap) / to_f32(rows))
        .max(1.0);
    let tray_w = 2.0 * card + 3.0 * m.gap;
    let tray_h = to_f32(rows) * card + to_f32(rows + 1) * m.gap;
    let column_axis = x1 - side / 2.0;
    let tray_top = y0 + ((y1 - y0) - tray_h).max(0.0) / 2.0;
    let tray = Rect::new(column_axis - tray_w / 2.0, tray_top, tray_w, tray_h);
    Layout {
        kind: Arrangement::Wide,
        font: m.font,
        title_font: m.font * 1.4,
        id_font: m.font * 0.9,
        safe,
        axis,
        panel: Some(panel.rect),
        title: panel.title,
        puzzle_id: panel.puzzle_id,
        subtitle: panel.subtitle,
        help: panel.help,
        tabs,
        lang,
        share,
        share_short: false,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(input.pieces, 2, card, m.gap, column_axis, tray_top),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset: bottom.reset,
        counters: bottom.counters,
        tab_hint: bottom.hint,
        new: bottom.new,
        banner,
        next,
    }
}

// ---- tall ----

/// Side margin of the tall arrangement.
const TALL_SIDE: f32 = 16.0;
/// Header height of the tall arrangement.
const TALL_HEADER: f32 = 44.0;
/// Height of the header controls of the tall arrangement.
const TALL_HEADER_BUTTON: f32 = 28.0;
/// Height of the tabs of the tall arrangement.
const TALL_TABS: f32 = 32.0;
/// Height of the bottom buttons of the tall arrangement.
const TALL_BUTTON: f32 = 40.0;
/// Space kept above the bottom safe inset.
const TALL_BOTTOM: f32 = 12.0;

fn tall(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let gap = 8.0;
    let font = 14.0;
    let small_font = 13.0;
    let x0 = safe.x + TALL_SIDE;
    let x1 = safe.right() - TALL_SIDE;
    let width = (x1 - x0).max(1.0);
    let axis = safe.center().x;
    let top = safe.y + 8.0;

    // Header: title and id on the left, language and link on the right.
    let header_mid = top + TALL_HEADER / 2.0;
    let share_w = button_width(share_label(input.lang, true), small_font);
    let share = Rect::new(
        x1 - share_w,
        header_mid - TALL_HEADER_BUTTON / 2.0,
        share_w,
        TALL_HEADER_BUTTON,
    );
    let lang_w = switch_width("EN", "UA", small_font);
    let lang = Rect::new(
        share.x - gap - lang_w,
        header_mid - TALL_HEADER_BUTTON / 2.0,
        lang_w,
        TALL_HEADER_BUTTON,
    );
    let title_w = (lang.x - gap - x0).max(1.0);
    let title = Rect::new(x0, top + 3.0, title_w, 22.0);
    let puzzle_id = Rect::new(x0, title.bottom(), title_w, 16.0);

    // Tabs below; their touch zones reach 6 above and below.
    let tabs_y = top + TALL_HEADER + gap;
    let tab_w = width / 4.0;
    let tabs =
        core::array::from_fn(|k| Rect::new(x0 + tab_w * to_f32(k), tabs_y, tab_w, TALL_TABS));

    let bottom_y = safe.bottom() - TALL_BOTTOM - TALL_BUTTON;
    let bottom = bottom_row(input, font, gap, x0, x1, axis, bottom_y, TALL_BUTTON);

    // Board and tray share the height between tabs and the bottom row.
    let region_top = tabs_y + TALL_TABS + gap;
    let region_bottom = bottom_y - gap;
    let region = (region_bottom - region_top).max(0.0);
    let n = input.pieces.max(1);
    let (cols, rows) = if n <= 4 { (n, 1) } else { (n.div_ceil(2), 2) };
    let card_by_width = (width - to_f32(cols + 1) * gap) / to_f32(cols);
    let side = to_f32(input.max_piece_side.max(1));
    let card_for = |cell: f32| {
        let (_, block_h) = board_size(input, cell);
        let room = region - block_h - gap - to_f32(rows + 1) * gap;
        card_by_width.min(room / to_f32(rows))
    };
    let fits = |cell: f32| {
        let (block_w, _) = board_size(input, cell);
        let card = card_for(cell);
        block_w <= width
            && card >= MIN_TARGET
            && card * TRAY_FILL / side >= cell * TRAY_MIN_SCALE - 0.01
    };
    let cell = largest(fits);
    let card = card_for(cell).max(1.0);
    let (_, block_h) = board_size(input, cell);
    let tray_h = to_f32(rows) * card + to_f32(rows + 1) * gap;
    let stack = block_h + gap + tray_h;
    let board_top = region_top + (region - stack).max(0.0) / 2.0;
    let board = board_at(input, axis, board_top, cell);
    let tray_top = board.frame.bottom() + gap;
    let tray = Rect::new(x0, tray_top, width, tray_h);
    // After a win the tray is empty: the banner takes its place.
    let banner_h = banner_height(width, gap).min(tray.h);
    let (banner, next) = banner_parts(
        Rect::new(x0, tray.center().y - banner_h / 2.0, width, banner_h),
        font,
        gap,
    );
    Layout {
        kind: Arrangement::Tall,
        font,
        title_font: 15.0,
        id_font: 11.0,
        safe,
        axis,
        panel: None,
        title,
        puzzle_id,
        subtitle: None,
        help: Vec::new(),
        tabs,
        lang,
        share,
        share_short: true,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(input.pieces, cols, card, gap, axis, tray_top),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset: bottom.reset,
        counters: bottom.counters,
        tab_hint: None,
        new: bottom.new,
        banner,
        next,
    }
}

// ---- low wide ----

/// The right column of the low wide arrangement.
struct RightColumn {
    counters: Rect,
    new: Rect,
    tray: Rect,
    /// Height available to the tray (and to the banner after a win).
    tray_room: f32,
    card: f32,
}

/// Tray (two cards per row) on top, then the counter switch, then new at
/// the bottom. When stacking leaves cards smaller than a touch target, the
/// switch and new share the bottom row.
fn right_column(
    input: &LayoutInput,
    rx: f32,
    side: f32,
    y0: f32,
    y1: f32,
    gap: f32,
    font: f32,
) -> RightColumn {
    let t = |key| text(input.lang, key);
    let button = MIN_TARGET;
    let x1 = rx + side;
    let new_w = button_width(t(Text::New), font);
    let ideal_counters = switch_width(t(Text::Grid), t(Text::Numbers), font).min(side);
    let rows = input.pieces.max(1).div_ceil(2);
    // Cards sit close; a card's touch zone is grown to a full target, so a
    // card plus its gap must reach that size.
    let tgap = LOW_TRAY_GAP;
    let card_in = |room: f32| {
        ((side - 3.0 * tgap) / 2.0).min((room - to_f32(rows + 1) * tgap) / to_f32(rows))
    };
    let stacked_room = (y1 - y0) - 2.0 * button - 2.0 * gap;
    let one_row_counters = ideal_counters.min(side - gap - new_w);
    let one_row = card_in(stacked_room) + tgap < MIN_TARGET && one_row_counters >= 2.0 * MIN_TARGET;
    let new = Rect::new(x1 - new_w, y1 - button, new_w, button);
    let counters_w = ideal_counters;
    let (counters, tray_room) = if one_row {
        (
            Rect::new(rx, y1 - button, one_row_counters, button),
            (y1 - y0) - button - gap,
        )
    } else {
        (
            Rect::new(
                rx + (side - counters_w) / 2.0,
                y1 - 2.0 * button - gap,
                counters_w,
                button,
            ),
            stacked_room,
        )
    };
    let card = card_in(tray_room).max(1.0);
    let tray_w = 2.0 * card + 3.0 * tgap;
    let tray_h = to_f32(rows) * card + to_f32(rows + 1) * tgap;
    let tray = Rect::new(rx + (side - tray_w) / 2.0, y0, tray_w, tray_h);
    RightColumn {
        counters,
        new,
        tray,
        tray_room,
        card,
    }
}

fn low_wide(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let gap = 8.0;
    let font = 14.0;
    let button = MIN_TARGET;
    let t = |key| text(input.lang, key);
    let x0 = safe.x + gap;
    let x1 = safe.right() - gap;
    let y0 = safe.y + gap;
    let y1 = safe.bottom() - gap;
    let axis = safe.center().x;
    let side = ((x1 - x0) * 0.27).clamp(150.0, 260.0);

    // Left column: name, difficulty 2×2, language and link, reset.
    let lx = x0;
    let title = Rect::new(lx, y0, side, 22.0);
    let puzzle_id = Rect::new(lx, title.bottom(), side, 16.0);
    let tab_w = (side - gap) / 2.0;
    let tabs_y = puzzle_id.bottom() + gap;
    let tabs = core::array::from_fn(|k| {
        Rect::new(
            lx + to_f32(k % 2) * (tab_w + gap),
            tabs_y + to_f32(k / 2) * (button + 6.0),
            tab_w,
            button,
        )
    });
    let row_y = tabs_y + 2.0 * button + 6.0 + gap;
    let lang_w = switch_width("EN", "UA", font);
    let lang = Rect::new(lx, row_y, lang_w, button);
    let share_w = button_width(share_label(input.lang, true), font).min(side - lang_w - gap);
    let share = Rect::new(lx + side - share_w, row_y, share_w, button);
    let reset = Rect::new(lx, y1 - button, button_width(t(Text::Reset), font), button);

    // Right column: tray, then the counter switch, then new at the bottom.
    let right = right_column(input, x1 - side, side, y0, y1, gap, font);
    let (counters, new, tray, tray_room, card) = (
        right.counters,
        right.new,
        right.tray,
        right.tray_room,
        right.card,
    );
    let column_axis = x1 - side / 2.0;
    let rx = x1 - side;

    // The board takes the full height between the columns.
    let center_w = (x1 - x0) - 2.0 * (side + gap);
    let (unit_w, unit_h) = board_size(input, 1.0);
    let cell = (center_w / unit_w)
        .min((y1 - y0) / unit_h)
        .clamp(MIN_CELL, MAX_CELL)
        .floor();
    let (_, block_h) = board_size(input, cell);
    let board = board_at(input, axis, y0 + ((y1 - y0) - block_h).max(0.0) / 2.0, cell);

    // After a win the banner takes the empty tray's place.
    let banner_h = banner_height(side, gap).min(tray_room);
    let (banner, next) = banner_parts(
        Rect::new(
            rx,
            y0 + (tray_room - banner_h).max(0.0) / 2.0,
            side,
            banner_h,
        ),
        font,
        gap,
    );
    Layout {
        kind: Arrangement::LowWide,
        font,
        title_font: 15.0,
        id_font: 11.0,
        safe,
        axis,
        panel: None,
        title,
        puzzle_id,
        subtitle: None,
        help: Vec::new(),
        tabs,
        lang,
        share,
        share_short: true,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(input.pieces, 2, card, LOW_TRAY_GAP, column_axis, y0),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset,
        counters,
        tab_hint: None,
        new,
        banner,
        next,
    }
}
