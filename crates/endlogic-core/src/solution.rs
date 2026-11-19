//! Solutions and their independent verification.

use core::fmt;

use crate::color::Color;
use crate::error::Axis;
use crate::grid::Cell;
use crate::puzzle::Puzzle;
use crate::shape::Rotation;

/// Where one piece goes: its rotation and the top-left corner of the rotated
/// shape's bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Placement {
    /// Rotation applied to the piece's shape as given in the puzzle.
    pub rotation: Rotation,
    /// Row of the bounding box's top edge.
    pub row: usize,
    /// Column of the bounding box's left edge.
    pub col: usize,
}

/// One placement per piece, in the same order as [`Puzzle::pieces`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Solution {
    /// `placements[i]` positions piece `i`.
    pub placements: Vec<Placement>,
}

impl Solution {
    /// For every grid cell in row-major order, the index of the piece covering
    /// it, or `None`.
    ///
    /// # Errors
    ///
    /// The first geometric problem found: wrong number of placements, a piece
    /// outside the grid, on a locked or dead cell, or overlapping another
    /// piece. Line counts are not checked here; see [`verify`].
    pub fn cell_owners(&self, puzzle: &Puzzle) -> Result<Vec<Option<usize>>, VerifyError> {
        let grid = puzzle.grid();
        let pieces = puzzle.pieces();
        if self.placements.len() != pieces.len() {
            return Err(VerifyError::PlacementCount {
                expected: pieces.len(),
                found: self.placements.len(),
            });
        }
        let mut owners: Vec<Option<usize>> = vec![None; grid.width() * grid.height()];
        for (index, (piece, placement)) in pieces.iter().zip(&self.placements).enumerate() {
            let shape = piece.shape.rotated(placement.rotation);
            for &(dr, dc) in shape.cells() {
                let (Some(row), Some(col)) =
                    (placement.row.checked_add(dr), placement.col.checked_add(dc))
                else {
                    return Err(VerifyError::OutOfBounds { piece: index });
                };
                match grid.get(row, col) {
                    None => return Err(VerifyError::OutOfBounds { piece: index }),
                    Some(Cell::Lock(_)) => {
                        return Err(VerifyError::OnLockedCell {
                            piece: index,
                            row,
                            col,
                        });
                    }
                    Some(Cell::Dead) => {
                        return Err(VerifyError::OnDeadCell {
                            piece: index,
                            row,
                            col,
                        });
                    }
                    Some(Cell::Free) => {}
                }
                let slot = &mut owners[row * grid.width() + col];
                if let Some(other) = *slot {
                    return Err(VerifyError::Overlap {
                        first: other,
                        second: index,
                        row,
                        col,
                    });
                }
                *slot = Some(index);
            }
        }
        Ok(owners)
    }
}

/// Why a solution is not valid for a puzzle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The solution does not have exactly one placement per piece.
    PlacementCount {
        /// Number of pieces in the puzzle.
        expected: usize,
        /// Number of placements in the solution.
        found: usize,
    },
    /// Part of a piece lies outside the grid.
    OutOfBounds {
        /// Index of the piece.
        piece: usize,
    },
    /// A piece covers a locked cell.
    OnLockedCell {
        /// Index of the piece.
        piece: usize,
        /// Row of the cell.
        row: usize,
        /// Column of the cell.
        col: usize,
    },
    /// A piece covers a dead cell.
    OnDeadCell {
        /// Index of the piece.
        piece: usize,
        /// Row of the cell.
        row: usize,
        /// Column of the cell.
        col: usize,
    },
    /// Two pieces cover the same cell.
    Overlap {
        /// Index of the piece placed first.
        first: usize,
        /// Index of the piece placed second.
        second: usize,
        /// Row of the cell.
        row: usize,
        /// Column of the cell.
        col: usize,
    },
    /// A row or column does not have the required number of filled cells.
    LineCount {
        /// Color whose count is wrong.
        color: Color,
        /// Row or column.
        axis: Axis,
        /// Index of the line.
        index: usize,
        /// Clue value.
        expected: usize,
        /// Actual number of filled cells of that color.
        found: usize,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::PlacementCount { expected, found } => {
                write!(f, "expected {expected} placements, found {found}")
            }
            VerifyError::OutOfBounds { piece } => write!(f, "piece {piece} is outside the grid"),
            VerifyError::OnLockedCell { piece, row, col } => {
                write!(f, "piece {piece} covers locked cell ({row}, {col})")
            }
            VerifyError::OnDeadCell { piece, row, col } => {
                write!(f, "piece {piece} covers dead cell ({row}, {col})")
            }
            VerifyError::Overlap {
                first,
                second,
                row,
                col,
            } => write!(f, "pieces {first} and {second} overlap at ({row}, {col})"),
            VerifyError::LineCount {
                color,
                axis,
                index,
                expected,
                found,
            } => write!(
                f,
                "color {}: {axis} {index} has {found} filled cells, expected {expected}",
                color.0
            ),
        }
    }
}

impl core::error::Error for VerifyError {}

/// Checks a solution against a puzzle without using the solver.
///
/// A solution is valid when every piece is placed once, entirely on free
/// cells inside the grid, without overlaps, and for every color each row and
/// column has exactly as many filled cells (locks plus piece cells) as its
/// clue.
///
/// # Errors
///
/// The first problem found, as a [`VerifyError`].
pub fn verify(puzzle: &Puzzle, solution: &Solution) -> Result<(), VerifyError> {
    let owners = solution.cell_owners(puzzle)?;
    let grid = puzzle.grid();
    let pieces = puzzle.pieces();
    let color_at = |row: usize, col: usize| -> Option<Color> {
        match grid.get(row, col) {
            Some(Cell::Lock(color)) => Some(color),
            _ => owners[row * grid.width() + col].map(|piece| pieces[piece].color),
        }
    };
    for color in puzzle.colors() {
        for row in 0..grid.height() {
            let found = (0..grid.width())
                .filter(|&col| color_at(row, col) == Some(color))
                .count();
            let expected = puzzle.row_clue(color, row);
            if found != expected {
                return Err(VerifyError::LineCount {
                    color,
                    axis: Axis::Row,
                    index: row,
                    expected,
                    found,
                });
            }
        }
        for col in 0..grid.width() {
            let found = (0..grid.height())
                .filter(|&row| color_at(row, col) == Some(color))
                .count();
            let expected = puzzle.col_clue(color, col);
            if found != expected {
                return Err(VerifyError::LineCount {
                    color,
                    axis: Axis::Column,
                    index: col,
                    expected,
                    found,
                });
            }
        }
    }
    Ok(())
}
