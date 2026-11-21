//! Where the marks of a line counter go in the "Grid" display, as in the
//! game.
//!
//! A counter sits in a band next to the grid and grows away from it: one
//! tick per cell of the clue, of a fixed size relative to the cell however
//! many there are; vertical ticks side by side that grow left for a row,
//! horizontal ticks stacked that grow up for a column. Ticks fill from the
//! grid outwards.
//!
//! A line with clues of several colors splits its width across the line
//! evenly among them: for a row the counters stand one above another, the
//! first color at the bottom; for a column side by side, the first color on
//! the left. A color whose clue in the line is 0 is left out, unless cells
//! of that color stand in the line: then it gets a red `∅` in its place. A
//! line whose clues are all 0 shows one grey `∅`.
//!
//! The band of the row counters is as deep as the longest row counter of
//! the puzzle, that of the column counters as the longest column counter
//! ([`band_depth`]). When a tick would be thinner than [`MIN_TICK`]
//! logical pixels, the whole puzzle shows numbers instead.

use endlogic_core::{Color, MAX_COLORS};

use crate::board::Line;
use crate::layout::to_f32;
use crate::painter::{Point, Rect};

/// Distance between neighbouring ticks along the counter, in cells.
pub const TICK_PITCH: f32 = 0.14;
/// Thickness of a tick, in cells.
pub const TICK_THICK: f32 = 0.07;
/// Length of a tick across the line when it has the line to itself, in
/// cells.
pub const TICK_LONG: f32 = 0.31;
/// Share of the cell across the line that the counters of a line divide
/// among their colors.
pub const ACROSS: f32 = 0.72;
/// Thinnest tick, in logical pixels, before counters become numbers.
pub const MIN_TICK: f32 = 1.5;
/// Depth of a band of `∅` signs only, in cells.
pub const EMPTY_DEPTH: f32 = 0.4;
/// Radius of the `∅` sign, in cells.
const EMPTY_RADIUS: f32 = 0.16;

/// Depth of a counter band whose longest counter has `ticks` ticks, in
/// cells.
#[must_use]
pub fn band_depth(ticks: usize) -> f32 {
    (to_f32(ticks) * TICK_PITCH).max(EMPTY_DEPTH)
}

/// Whether ticks drawn with cells of `cell` logical pixels are thick
/// enough; if not, every counter shows numbers.
#[must_use]
pub fn ticks_fit(cell: f32) -> bool {
    TICK_THICK * cell >= MIN_TICK
}

/// What one color's counter of a line shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterView {
    /// The `∅` sign: grey for a line whose clues are all 0, red for a color
    /// with clue 0 that has cells in the line.
    Empty {
        /// Center of the sign.
        center: Point,
        /// Radius of the sign.
        radius: f32,
    },
    /// One tick per clue unit, `count` of them, nearest to the grid first
    /// (the fill order), see [`CounterView::marks`].
    Ticks {
        /// Where the counter's center line meets the band's edge next to the
        /// grid.
        edge: Point,
        /// Distance between neighbouring ticks.
        pitch: f32,
        /// Size of a tick along the counter.
        thick: f32,
        /// Size of a tick across the counter.
        long: f32,
        /// A row counter (ticks grow left), else a column counter (up).
        row: bool,
        /// Number of ticks.
        count: usize,
    },
}

impl Default for CounterView {
    fn default() -> CounterView {
        CounterView::Empty {
            center: Point::default(),
            radius: 0.0,
        }
    }
}

impl CounterView {
    /// The ticks, nearest to the grid first; none for a sign.
    pub fn marks(&self) -> impl Iterator<Item = Rect> + use<> {
        let view = *self;
        let count = match view {
            CounterView::Ticks { count, .. } => count,
            CounterView::Empty { .. } => 0,
        };
        (0..count).map(move |k| {
            let CounterView::Ticks {
                edge,
                pitch,
                thick,
                long,
                row,
                ..
            } = view
            else {
                return Rect::default();
            };
            // Tick 0 is next to the grid.
            let offset = pitch * (to_f32(k) + 0.5);
            if row {
                Rect::centered(Point::new(edge.x - offset, edge.y), thick, long)
            } else {
                Rect::centered(Point::new(edge.x, edge.y - offset), long, thick)
            }
        })
    }
}

/// The counters of one line: one per color shown, at most four.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LineCounters {
    items: [Counter; MAX_COLORS],
    len: usize,
}

impl core::ops::Deref for LineCounters {
    type Target = [Counter];

    fn deref(&self) -> &[Counter] {
        &self.items[..self.len]
    }
}

/// One color's counter in a line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Counter {
    /// The color.
    pub color: Color,
    /// Its cells and clue in the line.
    pub line: Line,
    /// The part of the line's band it takes: for a row a horizontal strip,
    /// for a column a vertical one.
    pub slot: Rect,
    /// What it shows.
    pub view: CounterView,
}

/// Lays out the counters of one line in `area`, the part of the counter
/// band beside one row (`row == true`; the grid is to the right of `area`)
/// or above one column (the grid is below `area`). `lines` holds every
/// color of the puzzle with its cells and clue in this line, in color
/// order; `cell` is the board cell.
#[must_use]
pub fn line_counters(lines: &[(Color, Line)], area: Rect, row: bool, cell: f32) -> LineCounters {
    let shown = |entry: &&(Color, Line)| entry.1.clue > 0 || entry.1.filled > 0;
    let mut out = LineCounters::default();
    let n = lines.iter().filter(shown).count().min(MAX_COLORS);
    if n == 0 {
        // All clues 0 and nothing in the line: one grey sign.
        let (color, line) = lines.first().copied().unwrap_or_default();
        out.items[0] = Counter {
            color,
            line,
            slot: area,
            view: empty(area, row, cell, cell),
        };
        out.len = 1;
        return out;
    }
    let k = to_f32(n);
    let across = ACROSS * cell / k;
    for (j, &(color, line)) in lines.iter().filter(shown).take(n).enumerate() {
        out.items[j] = {
            let j = to_f32(j);
            let slot = if row {
                // The first color at the bottom.
                let y = area.center().y + ((k - 1.0) / 2.0 - j) * across;
                Rect::new(area.x, y - across / 2.0, area.w, across)
            } else {
                let x = area.center().x + (j - (k - 1.0) / 2.0) * across;
                Rect::new(x - across / 2.0, area.y, across, area.h)
            };
            let view = if line.clue == 0 {
                empty(slot, row, cell, across)
            } else {
                ticks(line.clue, slot, row, cell, across)
            };
            Counter {
                color,
                line,
                slot,
                view,
            }
        };
    }
    out.len = n;
    out
}

/// The `∅` sign next to the grid in `slot`, at most `across` wide.
fn empty(slot: Rect, row: bool, cell: f32, across: f32) -> CounterView {
    let radius = (EMPTY_RADIUS * cell).min(across * 0.4);
    let offset = EMPTY_DEPTH * cell / 2.0;
    let center = if row {
        Point::new(slot.right() - offset, slot.center().y)
    } else {
        Point::new(slot.center().x, slot.bottom() - offset)
    };
    CounterView::Empty { center, radius }
}

fn ticks(n: usize, slot: Rect, row: bool, cell: f32, across: f32) -> CounterView {
    let pitch = TICK_PITCH * cell;
    let thick = TICK_THICK * cell;
    let long = (TICK_LONG * cell).min(across * 0.85);
    let edge = if row {
        Point::new(slot.right(), slot.center().y)
    } else {
        Point::new(slot.center().x, slot.bottom())
    };
    CounterView::Ticks {
        edge,
        pitch,
        thick,
        long,
        row,
        count: n,
    }
}
