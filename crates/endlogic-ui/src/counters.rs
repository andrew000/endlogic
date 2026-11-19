//! Where the marks of a line counter go in the "Grid" display.
//!
//! A counter sits in a band next to the grid and grows away from it:
//!
//! - clue 0: the `∅` sign;
//! - clue 1–5: one tick per cell (vertical ticks side by side for a row,
//!   horizontal ticks stacked for a column);
//! - clue 6–10: small squares in two lines, clue 11–16: in three lines.
//!   For a row the squares form columns of two (or three) that grow left
//!   from the grid, centered on the row; for a column they form rows that
//!   grow up, centered on the column. An odd remainder sits in the group
//!   farthest from the grid.
//!
//! Marks fill from the grid outwards: nearest group first, and inside a
//! group top to bottom for rows, left to right for columns.
//!
//! All square counters of one puzzle use the same square and the same
//! spacing, set by the largest clue of the puzzle. When squares would be
//! smaller than [`MIN_SQUARE`] every square counter shows a number instead.

use crate::board::Line;
use crate::layout::to_f32;
use crate::painter::{Point, Rect};

/// Smallest square, in logical pixels, before a counter falls back to a
/// number.
pub const MIN_SQUARE: f32 = 3.0;
/// Largest clue drawn with ticks.
pub const MAX_TICKS: usize = 5;
/// Largest clue drawn with squares in two lines.
pub const MAX_TWO_LINES: usize = 10;

/// What a counter shows in the "Grid" display.
#[derive(Debug, Clone, PartialEq)]
pub enum CounterView {
    /// The `∅` sign of a zero clue.
    Empty {
        /// Center of the sign.
        center: Point,
        /// Radius of the sign.
        radius: f32,
    },
    /// One mark per clue unit, nearest to the grid first (the fill order).
    Marks {
        /// The marks.
        marks: Vec<Rect>,
        /// Ticks (clue up to 5) or squares.
        ticks: bool,
    },
    /// Too small for marks: shown as `filled/clue`.
    Number,
}

/// Lays out the counter of `line` in `area`, the part of the counter band
/// beside one row (`row == true`; the grid is to the right of `area`) or
/// above one column (the grid is below `area`). `cell` is the board cell
/// and `largest` the largest clue of the puzzle, rows and columns alike,
/// which sets the size of the squares.
#[must_use]
pub fn counter_view(line: Line, area: Rect, row: bool, cell: f32, largest: usize) -> CounterView {
    let n = line.clue;
    let depth = if row { area.w } else { area.h };
    if n == 0 {
        let radius = cell * 0.16;
        let offset = (depth * 0.45).min(cell * 0.3).max(radius);
        let center = if row {
            Point::new(area.right() - offset, area.center().y)
        } else {
            Point::new(area.center().x, area.bottom() - offset)
        };
        return CounterView::Empty { center, radius };
    }
    if n <= MAX_TICKS {
        return ticks(n, area, row, cell, depth);
    }
    squares(n, area, row, square_pitch(largest.max(n), cell, depth))
}

/// Distance between neighbouring squares when the largest clue is `n`:
/// the square plus the space after it.
fn square_pitch(n: usize, cell: f32, depth: f32) -> f32 {
    let lines = if n <= MAX_TWO_LINES { 2 } else { 3 };
    let groups = n.div_ceil(lines);
    (depth * 0.95 / to_f32(groups)).min(cell * 0.85 / to_f32(lines))
}

fn ticks(n: usize, area: Rect, row: bool, cell: f32, depth: f32) -> CounterView {
    let room = depth * 0.9;
    let spacing = (cell * 0.18).min(room / to_f32(n));
    let thick = (spacing * 0.55).min(cell * 0.08).max(1.5);
    let long = cell * 0.5;
    let start = depth * 0.04;
    let marks = (0..n)
        .map(|k| {
            // Tick 0 is next to the grid.
            let offset = start + spacing * (to_f32(k) + 0.5);
            if row {
                Rect::centered(
                    Point::new(area.right() - offset, area.center().y),
                    thick,
                    long,
                )
            } else {
                Rect::centered(
                    Point::new(area.center().x, area.bottom() - offset),
                    long,
                    thick,
                )
            }
        })
        .collect();
    CounterView::Marks { marks, ticks: true }
}

fn squares(n: usize, area: Rect, row: bool, pitch: f32) -> CounterView {
    let depth = if row { area.w } else { area.h };
    let lines = if n <= MAX_TWO_LINES { 2 } else { 3 };
    let groups = n.div_ceil(lines);
    let side = pitch / 1.35;
    if side < MIN_SQUARE {
        return CounterView::Number;
    }
    let across = to_f32(lines) * pitch;
    let mut marks = Vec::with_capacity(n);
    for g in 0..groups {
        let in_group = (n - g * lines).min(lines);
        for j in 0..in_group {
            let along = pitch * (to_f32(g) + 0.5) + depth * 0.02;
            let cross = -across / 2.0 + pitch * (to_f32(j) + 0.5);
            let center = if row {
                Point::new(area.right() - along, area.center().y + cross)
            } else {
                Point::new(area.center().x + cross, area.bottom() - along)
            };
            marks.push(Rect::centered(center, side, side));
        }
    }
    CounterView::Marks {
        marks,
        ticks: false,
    }
}
