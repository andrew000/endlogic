//! Structural reasons why a puzzle cannot have a solution, found without a
//! search.

use alloc::vec::Vec;

use crate::color::Color;
use crate::error::Axis;
use crate::grid::Cell;
use crate::puzzle::Puzzle;

/// One reason why a puzzle has no solution. Each is certain: a puzzle with
/// any of them has none. A puzzle without any may still have none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Problem {
    /// The row clues and the column clues of a color add up differently.
    LineSums {
        /// The color.
        color: Color,
        /// Sum of the row clues.
        rows: usize,
        /// Sum of the column clues.
        cols: usize,
    },
    /// The clues of a color ask for a different number of cells than its
    /// locks and pieces fill.
    Area {
        /// The color.
        color: Color,
        /// Sum of the row clues.
        clues: usize,
        /// Locked cells plus the area of all pieces of the color.
        cells: usize,
    },
    /// A clue is larger than the number of cells of its line that can be
    /// filled with its color at all: free cells and its own locks.
    LineTooLong {
        /// The color.
        color: Color,
        /// Row or column.
        axis: Axis,
        /// Index of the line, from 0.
        index: usize,
        /// The clue.
        clue: usize,
        /// Cells of the line that can hold the color.
        room: usize,
    },
    /// A line has more locks of a color than its clue for that color.
    TooManyLocks {
        /// The color.
        color: Color,
        /// Row or column.
        axis: Axis,
        /// Index of the line, from 0.
        index: usize,
        /// The clue.
        clue: usize,
        /// Locks of the color in the line.
        locks: usize,
    },
    /// The puzzle has no pieces.
    NoPieces,
}

/// Lists every structural problem of `puzzle`, color by color, in this
/// order: line sums, area, lines that are too long, lines with too many
/// locks (rows before columns, top to bottom and left to right), and
/// finally a missing piece list.
#[must_use]
pub fn diagnose(puzzle: &Puzzle) -> Vec<Problem> {
    let grid = puzzle.grid();
    let (width, height) = (grid.width(), grid.height());
    let mut problems = Vec::new();
    for color in puzzle.colors() {
        let row_clue = |r: usize| puzzle.row_clue(color, r);
        let col_clue = |c: usize| puzzle.col_clue(color, c);
        let rows: usize = (0..height).map(row_clue).sum();
        let cols: usize = (0..width).map(col_clue).sum();
        if rows != cols {
            problems.push(Problem::LineSums { color, rows, cols });
        }
        let locks = grid
            .cells()
            .iter()
            .filter(|&&c| c == Cell::Lock(color))
            .count();
        let pieces: usize = puzzle
            .pieces()
            .iter()
            .filter(|p| p.color == color)
            .map(|p| p.shape.area())
            .sum();
        if rows != locks + pieces {
            problems.push(Problem::Area {
                color,
                clues: rows,
                cells: locks + pieces,
            });
        }

        let row_cells = |r: usize| (0..width).filter_map(move |c| grid.get(r, c));
        let col_cells = |c: usize| (0..height).filter_map(move |r| grid.get(r, c));
        let room = |cell: Cell| cell == Cell::Free || cell == Cell::Lock(color);
        let lines = || {
            (0..height)
                .map(|r| {
                    let cells: Vec<Cell> = row_cells(r).collect();
                    (Axis::Row, r, row_clue(r), cells)
                })
                .chain((0..width).map(|c| {
                    let cells: Vec<Cell> = col_cells(c).collect();
                    (Axis::Column, c, col_clue(c), cells)
                }))
        };
        for (axis, index, clue, cells) in lines() {
            let room = cells.iter().filter(|&&cell| room(cell)).count();
            if clue > room {
                problems.push(Problem::LineTooLong {
                    color,
                    axis,
                    index,
                    clue,
                    room,
                });
            }
        }
        for (axis, index, clue, cells) in lines() {
            let locks = cells.iter().filter(|&&c| c == Cell::Lock(color)).count();
            if locks > clue {
                problems.push(Problem::TooManyLocks {
                    color,
                    axis,
                    index,
                    clue,
                    locks,
                });
            }
        }
    }
    if puzzle.pieces().is_empty() {
        problems.push(Problem::NoPieces);
    }
    problems
}
