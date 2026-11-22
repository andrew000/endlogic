//! Pieces, clues and the puzzle itself.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::color::{Color, MAX_COLORS};
use crate::error::{Axis, ModelError};
use crate::grid::{Cell, Grid};
use crate::shape::Shape;

/// Largest number of pieces in one puzzle.
pub const MAX_PIECES: usize = 32;

/// A piece to be placed: a shape and its color.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Piece {
    /// Color whose clues this piece's cells count towards.
    pub color: Color,
    /// Shape in its initial orientation, as given in the puzzle.
    pub shape: Shape,
}

impl Piece {
    /// Creates a piece.
    #[must_use]
    pub fn new(color: Color, shape: Shape) -> Piece {
        Piece { color, shape }
    }
}

/// Target fill counts of one color: one value per row and one per column.
///
/// A value is the total number of filled cells of that color in the line,
/// locked cells included.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LineClues {
    /// Counts for rows, top to bottom.
    pub rows: Vec<usize>,
    /// Counts for columns, left to right.
    pub cols: Vec<usize>,
}

/// A complete puzzle: grid, pieces and clues for each color.
///
/// A color that has no entry in the clues is treated as having a clue of 0 on
/// every line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Puzzle {
    grid: Grid,
    pieces: Vec<Piece>,
    clues: BTreeMap<Color, LineClues>,
}

impl Puzzle {
    /// Assembles a puzzle, checking that the clue lists match the grid size,
    /// that there are at most [`MAX_PIECES`] pieces and that every color is
    /// one of the first [`MAX_COLORS`].
    ///
    /// Logical consistency (for example clue sums versus piece area) is not
    /// checked; an inconsistent puzzle simply has no solutions.
    ///
    /// # Errors
    ///
    /// [`ModelError::TooManyPieces`], [`ModelError::ColorOutOfRange`] or
    /// [`ModelError::ClueLength`].
    pub fn new(
        grid: Grid,
        pieces: Vec<Piece>,
        clues: BTreeMap<Color, LineClues>,
    ) -> Result<Puzzle, ModelError> {
        if pieces.len() > MAX_PIECES {
            return Err(ModelError::TooManyPieces(pieces.len()));
        }
        let locks = grid.cells().iter().filter_map(|cell| match cell {
            Cell::Lock(color) => Some(*color),
            _ => None,
        });
        let used = clues.keys().copied().chain(pieces.iter().map(|p| p.color));
        if let Some(color) = used.chain(locks).find(|c| c.index() >= MAX_COLORS) {
            return Err(ModelError::ColorOutOfRange(color));
        }
        for (&color, line_clues) in &clues {
            if line_clues.rows.len() != grid.height() {
                return Err(ModelError::ClueLength {
                    color,
                    axis: Axis::Row,
                    expected: grid.height(),
                    found: line_clues.rows.len(),
                });
            }
            if line_clues.cols.len() != grid.width() {
                return Err(ModelError::ClueLength {
                    color,
                    axis: Axis::Column,
                    expected: grid.width(),
                    found: line_clues.cols.len(),
                });
            }
        }
        Ok(Puzzle {
            grid,
            pieces,
            clues,
        })
    }

    /// Convenience constructor for a single-color puzzle using
    /// [`Color::DEFAULT`] for the clues; pieces keep their own colors.
    ///
    /// # Errors
    ///
    /// Same as [`Puzzle::new`].
    pub fn single_color(
        grid: Grid,
        pieces: Vec<Piece>,
        rows: Vec<usize>,
        cols: Vec<usize>,
    ) -> Result<Puzzle, ModelError> {
        let mut clues = BTreeMap::new();
        clues.insert(Color::DEFAULT, LineClues { rows, cols });
        Puzzle::new(grid, pieces, clues)
    }

    /// [`Puzzle::new`] with the clues as a list of colors and their clues
    /// (a later entry for a color replaces an earlier one).
    ///
    /// # Errors
    ///
    /// Same as [`Puzzle::new`].
    pub fn with_clues(
        grid: Grid,
        pieces: Vec<Piece>,
        clues: Vec<(Color, LineClues)>,
    ) -> Result<Puzzle, ModelError> {
        let mut map = BTreeMap::new();
        for (color, lines) in clues {
            map.insert(color, lines);
        }
        Puzzle::new(grid, pieces, map)
    }

    /// The board.
    #[must_use]
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The pieces, in puzzle order.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Clues for every color that has them.
    #[must_use]
    pub fn clues(&self) -> &BTreeMap<Color, LineClues> {
        &self.clues
    }

    /// Clues for one color, or `None` if the color has no clues (all zero).
    #[must_use]
    pub fn clues_for(&self, color: Color) -> Option<&LineClues> {
        self.clues.get(&color)
    }

    /// Every color mentioned by a clue, a piece or a locked cell, sorted.
    #[must_use]
    pub fn colors(&self) -> Vec<Color> {
        let mut colors: Vec<Color> = self.clues.keys().copied().collect();
        colors.extend(self.pieces.iter().map(|p| p.color));
        colors.extend(self.grid.cells().iter().filter_map(|cell| match cell {
            Cell::Lock(color) => Some(*color),
            _ => None,
        }));
        crate::sort::sort_dedup(&mut colors);
        colors
    }

    /// Target count for `color` on row `row` (0 if the color has no clues).
    #[must_use]
    pub fn row_clue(&self, color: Color, row: usize) -> usize {
        self.clues_for(color)
            .and_then(|c| c.rows.get(row).copied())
            .unwrap_or(0)
    }

    /// Target count for `color` on column `col` (0 if the color has no clues).
    #[must_use]
    pub fn col_clue(&self, color: Color, col: usize) -> usize {
        self.clues_for(color)
            .and_then(|c| c.cols.get(col).copied())
            .unwrap_or(0)
    }
}
