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
//! is kept on its right. The depth of the counter bands comes with the
//! input: as deep as the longest counter of the puzzle.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::math;

use crate::counters::ticks_fit;
use crate::font::{Font, MIN_FONT_SIZE, text_width};
use crate::painter::{Point, Rect};
use crate::shell::Insets;
use crate::text::{Lang, Text, difficulty_label, text};
use endlogic_core::generator::Difficulty;

mod edit;

pub use edit::{EditLayout, Mode, PieceEditor, ResultPanel, Stepper};

/// Smallest side of the touch zone of anything that can be tapped.
pub const MIN_TARGET: f32 = 44.0;
/// Below this height (logical pixels) a wide area gets the low arrangement.
pub const LOW_WIDE_HEIGHT: f32 = 520.0;

/// Depth of a counter band that shows numbers (or the editor's clue
/// fields), in cells.
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
/// Height of a difficulty tab in the low wide arrangement.
const LOW_TAB: f32 = 40.0;
/// Share of a card a piece may span.
const TRAY_FILL: f32 = 0.88;
/// Smallest piece cell in the tray before the tray is split into pages.
pub const MIN_TRAY_CELL: f32 = 6.0;
/// Height of the page switcher under the tray cards.
const PAGER_H: f32 = 40.0;

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
    /// Playing or editing.
    pub mode: Mode,
    /// Depth of the row counter band, in cells.
    pub row_band: f32,
    /// Depth of the column counter band, in cells.
    pub col_band: f32,
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

/// The page switcher of a tray with more cards than fit: `‹ 2 / 4 ›`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pager {
    /// Previous page.
    pub prev: Rect,
    /// Next page.
    pub next: Rect,
    /// The page number.
    pub label: Rect,
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
    /// Tabs: the four difficulties, easiest first, then the custom tab.
    /// One row, or a 2×2 grid with the custom tab under it.
    pub tabs: [Rect; 5],
    /// Language switch: `EN | UA`, or with [`compact`](Layout::compact)
    /// one button showing the current language.
    pub lang: Rect,
    /// The language and counter switches are single buttons that show the
    /// current choice and switch on a tap (the tall arrangement).
    pub compact: bool,
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
    /// The card slots of one tray page, all the same size. Piece `i` takes
    /// slot `i % per_page` on page `i / per_page`.
    pub cards: Vec<Rect>,
    /// Cards on one page: all of them unless the tray has pages.
    pub per_page: usize,
    /// The page switcher, when the cards do not fit on one page.
    pub pager: Option<Pager>,
    /// Side of one cell of a piece drawn in the tray, the same for every
    /// card and at most half the board cell.
    pub tray_cell: f32,
    /// Reset button.
    pub reset: Rect,
    /// Hint button.
    pub hint: Rect,
    /// Counter display switch: `Grid | Numbers`, or with
    /// [`compact`](Layout::compact) one button in the header.
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
    /// The editor's controls, in the edit mode. The play controls (reset,
    /// hint, new, the counter switch, the banner) are then empty.
    pub edit: Option<EditLayout>,
    /// Ticks would be too thin at this cell size: every counter shows
    /// numbers, in bands [`BAND`] deep.
    pub numbers_only: bool,
    /// The switch of the number of colors (`1 | 2`), or with
    /// [`compact`](Layout::compact) one button showing the current number;
    /// `None` in the editor.
    pub colors: Option<Rect>,
}

/// Names of the tabs for test messages.
const TAB_NAMES: [&str; 5] = ["easy", "normal", "hard", "expert", "custom"];

/// The label of tab `k`: a difficulty, or the custom tab.
#[must_use]
pub fn tab_label(lang: Lang, k: usize) -> &'static str {
    Difficulty::ALL
        .get(k)
        .map_or_else(|| text(lang, Text::Custom), |&d| difficulty_label(lang, d))
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
        let kind = if w < h * 1.2 {
            Arrangement::Tall
        } else if h < LOW_WIDE_HEIGHT {
            Arrangement::LowWide
        } else {
            Arrangement::Wide
        };
        if input.mode == Mode::Edit {
            return match kind {
                Arrangement::Wide => edit::wide(&input),
                Arrangement::LowWide => edit::low_wide(&input),
                Arrangement::Tall => edit::tall(&input),
            };
        }
        let play = |input: &LayoutInput| match kind {
            Arrangement::Wide => wide(input),
            Arrangement::LowWide => low_wide(input),
            Arrangement::Tall => tall(input),
        };
        let layout = play(&input);
        if ticks_fit(layout.cell) {
            return layout;
        }
        // Ticks this thin would not read: numbers, in bands of their own.
        let numbers = LayoutInput {
            row_band: BAND,
            col_band: BAND,
            ..input
        };
        Layout {
            numbers_only: true,
            ..play(&numbers)
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

    /// Number of tray pages for `cards` cards.
    #[must_use]
    pub fn pages(&self, cards: usize) -> usize {
        cards.div_ceil(self.per_page.max(1)).max(1)
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
        for (tab, name) in self.tabs.iter().zip(TAB_NAMES) {
            rects.push((name, *tab));
        }
        rects.extend([
            ("lang", self.lang),
            ("share", self.share),
            ("board", self.board_block()),
            ("tray", self.tray),
        ]);
        if let Some(edit) = &self.edit {
            // What sits inside the left panel is part of it.
            let inside = |r: &Rect| self.panel.is_some_and(|p| p.contains_rect(*r));
            rects.extend(edit.static_rects().into_iter().filter(|(_, r)| !inside(r)));
            return rects;
        }
        rects.extend([
            ("reset", self.reset),
            ("hint", self.hint),
            ("counters", self.counters),
            ("new", self.new),
        ]);
        if let Some(colors) = self.colors {
            rects.push(("colors", colors));
        }
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
        let mut targets: Vec<(&'static str, Rect)> = vec![
            ("easy", self.tabs[0]),
            ("normal", self.tabs[1]),
            ("hard", self.tabs[2]),
            ("expert", self.tabs[3]),
            ("custom", self.tabs[4]),
        ];
        if let Some(edit) = &self.edit {
            if self.compact {
                targets.push(("lang", self.lang));
            } else {
                let (en, ua) = Layout::halves(self.lang);
                targets.extend([("en", en), ("ua", ua)]);
            }
            targets.push(("share", self.share));
            targets.extend(edit.targets());
        } else if self.compact {
            targets.extend([("lang", self.lang), ("counters", self.counters)]);
            targets.extend(self.colors.map(|c| ("colors", c)));
        } else {
            let (grid, numbers) = Layout::halves(self.counters);
            let (en, ua) = Layout::halves(self.lang);
            targets.extend([("en", en), ("ua", ua), ("grid", grid), ("numbers", numbers)]);
            if let Some(colors) = self.colors {
                let (one, two) = Layout::halves(colors);
                targets.extend([("one color", one), ("two colors", two)]);
            }
        }
        if self.edit.is_none() {
            targets.extend([
                ("share", self.share),
                ("reset", self.reset),
                ("hint", self.hint),
                ("new", self.new),
            ]);
        }
        targets.extend(self.cards.iter().map(|&card| ("card", card)));
        if let Some(pager) = self.pager {
            targets.extend([("previous page", pager.prev), ("next page", pager.next)]);
        }
        targets
            .into_iter()
            .map(|(name, rect)| (name, Layout::zone(rect)))
            .collect()
    }
}

// ---- shared helpers ----

/// The title drawn in the header.
pub(crate) const TITLE: &str = "// ENDLOGIC";
/// Title font in the low wide arrangement, where it may share a row.
const LOW_TITLE_FONT: f32 = 15.0;
/// Puzzle id font in the low wide arrangement.
pub(crate) const LOW_ID_FONT: f32 = 11.0;

/// `size`, or smaller if the bold title would not fit `width`.
pub(crate) fn title_size(size: f32, width: f32) -> f32 {
    let w = text_width(TITLE, Font::new(size, true));
    if w <= width {
        size
    } else {
        (size * width / w).max(MIN_FONT_SIZE)
    }
}

/// Width of `text` in the font of `size`, as it will be drawn.
fn estimate_width(text: &str, size: f32) -> f32 {
    text_width(text, Font::new(size, false))
}

/// Breaks `text` into lines whose width fits `max_width`.
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
            lines.push(core::mem::replace(&mut line, word.to_owned()));
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

/// Width of the new button, which reads "Edit" in the custom tab.
fn new_width(lang: Lang, font: f32) -> f32 {
    button_width(text(lang, Text::New), font).max(button_width(text(lang, Text::Edit), font))
}

/// Width of the hint button: room for the label and a two-digit count.
fn hint_width(lang: Lang, font: f32) -> f32 {
    button_width(&format!("{} · 88", text(lang, Text::Hint)), font)
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
/// The largest cell for which the board takes at most `width` by `height`
/// (before rounding and clamping).
fn cell_to_fit(input: &LayoutInput, width: f32, height: f32) -> f32 {
    let (unit_w, unit_h) = board_size(input, 1.0);
    (width / unit_w).min(height / unit_h)
}

fn board_size(input: &LayoutInput, cell: f32) -> (f32, f32) {
    (
        (to_f32(input.grid_width) + 2.0 * (input.row_band + COUNTER_GAP)) * cell,
        (to_f32(input.grid_height) + input.col_band + COUNTER_GAP + FRAME_PAD) * cell,
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
    let (row_band, col_band) = (input.row_band * cell, input.col_band * cell);
    let gap = COUNTER_GAP * cell;
    let width = to_f32(input.grid_width) * cell;
    let cells = Rect::new(
        axis - width / 2.0,
        top + col_band + gap,
        width,
        to_f32(input.grid_height) * cell,
    );
    Board {
        frame: cells.inset(-FRAME_PAD * cell),
        row_counters: Rect::new(cells.x - gap - row_band, cells.y, row_band, cells.h),
        col_counters: Rect::new(cells.x, cells.y - gap - col_band, cells.w, col_band),
        right_band: Rect::new(cells.right() + gap, cells.y, row_band, cells.h),
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
    math::floor(lo)
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

/// Smallest card: its touch zone (grown to [`MIN_TARGET`] across the gap)
/// must not reach the next card, and the longest piece must keep cells of
/// at least [`MIN_TRAY_CELL`].
fn min_card(input: &LayoutInput, gap: f32) -> f32 {
    (MIN_TARGET - gap).max(MIN_TRAY_CELL * to_f32(input.max_piece_side.max(1)) / TRAY_FILL)
}

/// How the cards of a tray are arranged on a page.
#[derive(Debug, Clone, Copy)]
struct TrayFit {
    per_page: usize,
    cols: usize,
    rows: usize,
    card: f32,
    paged: bool,
}

impl TrayFit {
    /// Height of the tray: cards, gaps and the pager.
    fn height(&self, gap: f32) -> f32 {
        let pager = if self.paged { PAGER_H + gap } else { 0.0 };
        to_f32(self.rows) * self.card + to_f32(self.rows + 1) * gap + pager
    }
}

/// Whole cells of `size` that fit in `room`, at least one.
fn count_in(room: f32, size: f32) -> usize {
    let n = math::floor(room / size).max(1.0);
    // A small positive count.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = n as usize;
    n
}

/// `n` cards two per row in a column `side` wide with `room` of height,
/// split into pages when they would be smaller than `min`.
fn column_fit(n: usize, side: f32, room: f32, gap: f32, min: f32) -> TrayFit {
    let n = n.max(1);
    let width_card = (side - 3.0 * gap) / 2.0;
    let rows = n.div_ceil(2);
    let card = width_card.min((room - to_f32(rows + 1) * gap) / to_f32(rows));
    if card >= min {
        return TrayFit {
            per_page: n,
            cols: 2,
            rows,
            card,
            paged: false,
        };
    }
    let room = room - PAGER_H - gap;
    let rows = count_in(room - gap, min + gap).min(rows);
    let per_page = (2 * rows).min(n);
    let card = width_card.min((room - to_f32(rows + 1) * gap) / to_f32(rows));
    TrayFit {
        per_page,
        cols: 2,
        rows,
        card: card.max(1.0),
        paged: per_page < n,
    }
}

/// The pager under the cards of a tray at `tray` (whose last `PAGER_H`
/// plus `gap` are the pager row).
fn pager_in(tray: Rect, gap: f32) -> Pager {
    let y = tray.bottom() - gap - PAGER_H;
    let w = MIN_TARGET;
    let prev = Rect::new(tray.x + gap, y, w, PAGER_H);
    let next = Rect::new(tray.right() - gap - w, y, w, PAGER_H);
    let label = Rect::new(prev.right(), y, (next.x - prev.right()).max(0.0), PAGER_H);
    Pager { prev, next, label }
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

/// Five equal tabs in a row centered on `axis`, no wider than `2 * half`.
fn tab_row(lang: Lang, font: f32, pad: f32, axis: f32, half: f32, y: f32, h: f32) -> [Rect; 5] {
    let widest = (0..5)
        .map(|k| estimate_width(tab_label(lang, k), font) + 1.6 * pad)
        .fold(MIN_TARGET, f32::max);
    let tab = squeeze(&[widest * 5.0], 2.0 * half)[0].max(5.0 * MIN_TARGET) / 5.0;
    let start = axis - 2.5 * tab;
    core::array::from_fn(|k| Rect::new(start + tab * to_f32(k), y, tab, h))
}

/// The bottom row: reset and hint on the left, the counter switch centered
/// on the axis (with its key hint to its left), new on the right.
struct BottomRow {
    reset: Rect,
    hint: Rect,
    counters: Rect,
    key_hint: Option<Rect>,
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
    let new_w = new_width(input.lang, font);
    let ideal = switch_width(t(Text::Grid), t(Text::Numbers), font);
    let reset = Rect::new(x0, y, reset_w, h);
    let hint = Rect::new(reset.right() + gap, y, hint_width(input.lang, font), h);
    // The switch is centered on the axis, so it may use twice the room left
    // on its narrower side.
    let room = 2.0 * ((axis - (hint.right() + gap)).min(x1 - new_w - gap - axis));
    let counters_w = ideal.min(room).max(2.0 * MIN_TARGET);
    let new = Rect::new(x1 - new_w, y, new_w, h);
    let counters = Rect::new(axis - counters_w / 2.0, y, counters_w, h);
    let hint_w = estimate_width("TAB", font * 0.75) + font;
    let hint_x = counters.x - gap * 0.5 - hint_w;
    let key_hint = (!input.touch && hint_x >= hint.right() + gap)
        .then(|| Rect::new(hint_x, y + h * 0.2, hint_w, h * 0.6));
    BottomRow {
        reset,
        hint,
        counters,
        key_hint,
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
    // The colors switch right of the tabs, which stay centered on the axis.
    let colors_w = m.switch_width("1", "2");
    let half = (axis - (x0 + side + m.gap)).min(lang.x - 2.0 * m.gap - colors_w - axis);
    let tabs = tab_row(input.lang, m.font, m.pad(), axis, half, top, m.button);
    let colors = Rect::new(tabs[4].right() + m.gap, top, colors_w, m.button);
    let bottom = bottom_row(input, m.font, m.gap, x0, x1, axis, bottom_y, m.button);
    let panel = panel(input, &m, x0, top, side, bottom_y - m.gap);

    // The board, with room for the banner under it, between the columns.
    let y0 = top + m.button + m.gap;
    let y1 = bottom_y - m.gap;
    let center_w = (x1 - x0) - 2.0 * (side + m.gap);
    let banner_h = banner_height(400.0, m.gap);
    let cell = math::floor(
        cell_to_fit(input, center_w, y1 - y0 - banner_h - m.gap).clamp(MIN_CELL, MAX_CELL),
    );
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
    let fit = column_fit(input.pieces, side, y1 - y0, m.gap, min_card(input, m.gap));
    let card = fit.card;
    // A paged tray takes the whole column, for the pager.
    let tray_w = if fit.paged {
        side
    } else {
        2.0 * card + 3.0 * m.gap
    };
    let tray_h = fit.height(m.gap);
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
        compact: false,
        share,
        share_short: false,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(fit.per_page, 2, card, m.gap, column_axis, tray_top),
        per_page: fit.per_page,
        pager: fit.paged.then(|| pager_in(tray, m.gap)),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset: bottom.reset,
        hint: bottom.hint,
        counters: bottom.counters,
        tab_hint: bottom.key_hint,
        new: bottom.new,
        banner,
        next,
        edit: None,
        numbers_only: false,
        colors: Some(colors),
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

#[allow(clippy::too_many_lines)]
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

    // Header: title and id on the left; the counter display, the language
    // and the link on the right, each a single button.
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
    let counters = header_button(lang.x - gap - MIN_TARGET, MIN_TARGET);
    let colors = header_button(counters.x - gap - MIN_TARGET, MIN_TARGET);
    let title_w = (colors.x - gap - x0).max(1.0);
    let title = Rect::new(x0, top + 3.0, title_w, 22.0);
    let puzzle_id = Rect::new(x0, title.bottom(), title_w, 16.0);

    // Tabs below; their touch zones reach 6 above and below.
    let tabs_y = top + TALL_HEADER + gap;
    let tab_w = width / 5.0;
    let tabs =
        core::array::from_fn(|k| Rect::new(x0 + tab_w * to_f32(k), tabs_y, tab_w, TALL_TABS));

    // Bottom row: reset on the left, hint in the middle, new on the right.
    let bottom_y = safe.bottom() - TALL_BOTTOM - TALL_BUTTON;
    let reset = Rect::new(
        x0,
        bottom_y,
        button_width(text(input.lang, Text::Reset), font),
        TALL_BUTTON,
    );
    let new_w = new_width(input.lang, font);
    let new = Rect::new(x1 - new_w, bottom_y, new_w, TALL_BUTTON);
    let hint_w = hint_width(input.lang, font).min(new.x - reset.right() - 2.0 * gap);
    // Centered on the axis when there is room, else between the two.
    let hint_x = (axis - hint_w / 2.0)
        .max(reset.right() + gap)
        .min(new.x - gap - hint_w);
    let hint = Rect::new(hint_x, bottom_y, hint_w, TALL_BUTTON);

    // Board and tray share the height between tabs and the bottom row.
    let region_top = tabs_y + TALL_TABS + gap;
    let region_bottom = bottom_y - gap;
    let region = (region_bottom - region_top).max(0.0);
    let (fit, cell) = tall_tray(input, width, region, gap);
    let card = fit.card;
    let (_, block_h) = board_size(input, cell);
    let tray_h = fit.height(gap);
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
        // Drawing shrinks the title when the colors button leaves it less
        // room.
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
        compact: true,
        share,
        share_short: true,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(fit.per_page, fit.cols, card, gap, axis, tray_top),
        per_page: fit.per_page,
        pager: fit.paged.then(|| pager_in(tray, gap)),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset,
        hint,
        counters,
        tab_hint: None,
        new,
        banner,
        next,
        edit: None,
        numbers_only: false,
        colors: Some(colors),
    }
}

/// The tray of the tall arrangement and the board cell that goes with it.
///
/// Up to four cards make one row, more make two. The board gets the largest
/// cell for which the cards stay touch targets and their piece cells stay
/// between a third and a half of the board cell. When no board size allows
/// that, the cards are split into the fewest pages that do.
fn tall_tray(input: &LayoutInput, width: f32, region: f32, gap: f32) -> (TrayFit, f32) {
    tall_tray_with(input, width, region, gap, 0.0, 0.0)
}

/// [`tall_tray`] with `reserved` more height taken from the region besides
/// board and tray, and the tray at least `min_tray` high.
fn tall_tray_with(
    input: &LayoutInput,
    width: f32,
    region: f32,
    gap: f32,
    reserved: f32,
    min_tray: f32,
) -> (TrayFit, f32) {
    let n = input.pieces.max(1);
    let side = to_f32(input.max_piece_side.max(1));
    let shape = |pages: usize| {
        let per_page = n.div_ceil(pages);
        let (cols, rows) = if per_page <= 4 {
            (per_page, 1)
        } else {
            (per_page.div_ceil(2), 2)
        };
        TrayFit {
            per_page,
            cols,
            rows,
            card: 0.0,
            paged: pages > 1,
        }
    };
    let card_for = |fit: TrayFit, cell: f32| {
        let (_, block_h) = board_size(input, cell);
        let pager = if fit.paged { PAGER_H + gap } else { 0.0 };
        let room = region - reserved - block_h - gap - to_f32(fit.rows + 1) * gap - pager;
        let by_width = (width - to_f32(fit.cols + 1) * gap) / to_f32(fit.cols);
        by_width.min(room / to_f32(fit.rows))
    };
    let fits = |fit: TrayFit, cell: f32| {
        let (block_w, block_h) = board_size(input, cell);
        let card = card_for(fit, cell);
        block_w <= width
            && block_h + gap + reserved + min_tray <= region
            && card >= MIN_TARGET
            && card * TRAY_FILL / side >= MIN_TRAY_CELL
            && card * TRAY_FILL / side >= cell * TRAY_MIN_SCALE - 0.01
    };
    let fit = (1..=n)
        .map(shape)
        .find(|&fit| fits(fit, MIN_CELL))
        .unwrap_or_else(|| shape(n));
    let cell = largest(|cell| fits(fit, cell));
    // No bigger than the longest piece needs at half the board cell.
    let cap = (side * cell * TRAY_SCALE / TRAY_FILL).max(MIN_TARGET);
    let card = card_for(fit, cell).min(cap).max(1.0);
    (TrayFit { card, ..fit }, cell)
}

// ---- low wide ----

/// The right column of the low wide arrangement.
struct RightColumn {
    counters: Rect,
    new: Rect,
    tray: Rect,
    /// Height available to the tray (and to the banner after a win).
    tray_room: f32,
    fit: TrayFit,
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
    let new_w = new_width(input.lang, font);
    let ideal_counters = switch_width(t(Text::Grid), t(Text::Numbers), font).min(side);
    // Cards sit close; a card's touch zone is grown to a full target, so a
    // card plus its gap must reach that size.
    let tgap = LOW_TRAY_GAP;
    let min = min_card(input, tgap);
    let card_in = |room: f32| column_fit(input.pieces, side, room, tgap, min);
    let stacked_room = (y1 - y0) - 2.0 * button - 2.0 * gap;
    let one_row_counters = ideal_counters.min(side - gap - new_w);
    let one_row =
        card_in(stacked_room).card + tgap < MIN_TARGET && one_row_counters >= 2.0 * MIN_TARGET;
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
    let fit = card_in(tray_room);
    let tray_w = if fit.paged {
        side
    } else {
        2.0 * fit.card + 3.0 * tgap
    };
    let tray = Rect::new(rx + (side - tray_w) / 2.0, y0, tray_w, fit.height(tgap));
    RightColumn {
        counters,
        new,
        tray,
        tray_room,
        fit,
    }
}

/// The top of the low wide left column: name and id, the tabs (2×2 with
/// the custom tab under them), language and link.
struct LeftTop {
    title: Rect,
    puzzle_id: Rect,
    tabs: [Rect; 5],
    /// The colors switch, beside the custom tab (unused in the editor).
    colors: Rect,
    lang: Rect,
    share: Rect,
}

/// Lays out [`LeftTop`] from `y0`, leaving `below` free above `y1` when it
/// can: if two lines for the name and id leave too little, the id goes
/// next to the name.
#[allow(clippy::too_many_arguments)]
fn left_top(
    input: &LayoutInput,
    x0: f32,
    side: f32,
    y0: f32,
    y1: f32,
    gap: f32,
    font: f32,
    below: f32,
) -> LeftTop {
    let lx = x0;
    let tabs_h = 3.0 * MIN_TARGET - (MIN_TARGET - LOW_TAB);
    let rest = gap + tabs_h + gap + MIN_TARGET;
    let (title, puzzle_id) = if y0 + 38.0 + rest + below <= y1 {
        let title = Rect::new(lx, y0, side, 22.0);
        (title, Rect::new(lx, title.bottom(), side, 16.0))
    } else {
        // The id keeps room for its widest letters; the title takes the rest.
        let id_w = text_width("Δ-X-WWWWWW", Font::new(LOW_ID_FONT, false)) + 16.0;
        let title_w = (side - id_w).clamp(side * 0.45, side * 0.55);
        (
            Rect::new(lx, y0, title_w, 22.0),
            Rect::new(lx + title_w, y0 + 3.0, side - title_w, 16.0),
        )
    };
    let tab_w = (side - gap) / 2.0;
    let tabs_y = title.bottom().max(puzzle_id.bottom()) + gap;
    // Tabs a full target apart, so their zones just touch; under the 2×2
    // grid the custom tab takes the whole width in the editor, and shares
    // the row with the colors switch while playing.
    let playing = input.mode == Mode::Play;
    let tabs = core::array::from_fn(|k| {
        let (w, x) = if k == 4 && !playing {
            (side, lx)
        } else if k == 4 {
            (side - gap - tab_w.max(2.0 * MIN_TARGET), lx)
        } else {
            (tab_w, lx + to_f32(k % 2) * (tab_w + gap))
        };
        Rect::new(x, tabs_y + to_f32(k / 2) * MIN_TARGET, w, LOW_TAB)
    });
    // The switch needs two targets; the custom tab gives it room.
    let colors_w = tab_w.max(2.0 * MIN_TARGET);
    let colors = Rect::new(lx + side - colors_w, tabs[4].y, colors_w, LOW_TAB);
    let row_y = tabs_y + tabs_h + gap;
    let lang_w = switch_width("EN", "UA", font);
    let lang = Rect::new(lx, row_y, lang_w, MIN_TARGET);
    let share_w = button_width(share_label(input.lang, true), font).min(side - lang_w - gap);
    let share = Rect::new(lx + side - share_w, row_y, share_w, MIN_TARGET);
    LeftTop {
        title,
        puzzle_id,
        tabs,
        colors,
        lang,
        share,
    }
}

/// The left column of the low wide arrangement while playing: [`LeftTop`],
/// then hint above reset at the bottom, side by side when the column is
/// too short to stack them.
struct LeftColumn {
    top: LeftTop,
    hint: Rect,
    reset: Rect,
}

fn left_column(
    input: &LayoutInput,
    x0: f32,
    side: f32,
    y0: f32,
    y1: f32,
    gap: f32,
    font: f32,
) -> LeftColumn {
    let button = MIN_TARGET;
    let top = left_top(input, x0, side, y0, y1, gap, font, gap + button);
    let reset_w = button_width(text(input.lang, Text::Reset), font);
    let stacked = top.lang.bottom() + gap + 2.0 * button + gap <= y1;
    let (hint, reset) = if stacked {
        (
            Rect::new(
                x0,
                y1 - 2.0 * button - gap,
                hint_width(input.lang, font).min(side),
                button,
            ),
            Rect::new(x0, y1 - button, reset_w, button),
        )
    } else {
        let half = (side - gap) / 2.0;
        (
            Rect::new(x0 + half + gap, y1 - button, half, button),
            Rect::new(x0, y1 - button, half, button),
        )
    };
    LeftColumn { top, hint, reset }
}

fn low_wide(input: &LayoutInput) -> Layout {
    let safe = safe_rect(input);
    let gap = 8.0;
    let font = 14.0;
    let x0 = safe.x + gap;
    let x1 = safe.right() - gap;
    let y0 = safe.y + gap;
    let y1 = safe.bottom() - gap;
    let axis = safe.center().x;
    let side = ((x1 - x0) * 0.27).clamp(150.0, 260.0);

    let left = left_column(input, x0, side, y0, y1, gap, font);
    let (title, puzzle_id, tabs, lang, share, hint, reset) = (
        left.top.title,
        left.top.puzzle_id,
        left.top.tabs,
        left.top.lang,
        left.top.share,
        left.hint,
        left.reset,
    );

    // Right column: tray, then the counter switch, then new at the bottom.
    let right = right_column(input, x1 - side, side, y0, y1, gap, font);
    let (counters, new, tray, tray_room, fit) = (
        right.counters,
        right.new,
        right.tray,
        right.tray_room,
        right.fit,
    );
    let card = fit.card;
    let column_axis = x1 - side / 2.0;
    let rx = x1 - side;

    // The board takes the full height between the columns.
    let center_w = (x1 - x0) - 2.0 * (side + gap);
    let cell = math::floor(cell_to_fit(input, center_w, y1 - y0).clamp(MIN_CELL, MAX_CELL));
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
        title_font: title_size(LOW_TITLE_FONT, title.w),
        id_font: LOW_ID_FONT,
        safe,
        axis,
        panel: None,
        title,
        puzzle_id,
        subtitle: None,
        help: Vec::new(),
        tabs,
        lang,
        compact: false,
        share,
        share_short: true,
        notice: banner.center(),
        frame: board.frame,
        cells: board.cells,
        cell,
        row_counters: board.row_counters,
        col_counters: board.col_counters,
        right_band: board.right_band,
        cards: cards_at(fit.per_page, 2, card, LOW_TRAY_GAP, column_axis, y0),
        per_page: fit.per_page,
        pager: fit.paged.then(|| pager_in(tray, LOW_TRAY_GAP)),
        tray,
        tray_cell: tray_cell(input, card, cell),
        reset,
        hint,
        counters,
        tab_hint: None,
        new,
        banner,
        next,
        edit: None,
        numbers_only: false,
        colors: Some(left.top.colors),
    }
}
