//! Errors raised when constructing model values.

use core::fmt;

use crate::color::Color;

/// Which kind of line a clue refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// A horizontal line (row), indexed from the top.
    Row,
    /// A vertical line (column), indexed from the left.
    Column,
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Axis::Row => "row",
            Axis::Column => "column",
        })
    }
}

/// A structural problem found while constructing a [`Grid`](crate::Grid),
/// [`Shape`](crate::Shape) or [`Puzzle`](crate::Puzzle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// Grid width or height is outside `1..=MAX_GRID_SIZE`.
    GridSize {
        /// Requested width.
        width: usize,
        /// Requested height.
        height: usize,
    },
    /// The number of cells does not equal `width * height`.
    CellCount {
        /// `width * height`.
        expected: usize,
        /// Number of cells supplied.
        found: usize,
    },
    /// A shape has no cells.
    EmptyShape,
    /// A shape's cells are not 4-connected.
    DisconnectedShape,
    /// More than `MAX_PIECES` pieces.
    TooManyPieces(usize),
    /// A piece, lock or clue uses a color past the first
    /// [`MAX_COLORS`](crate::MAX_COLORS).
    ColorOutOfRange(Color),
    /// A clue list for some color does not have one value per line.
    ClueLength {
        /// Color whose clues are malformed.
        color: Color,
        /// Rows or columns.
        axis: Axis,
        /// Number of lines in the grid along that axis.
        expected: usize,
        /// Number of clue values supplied.
        found: usize,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::GridSize { width, height } => write!(
                f,
                "grid size {width}x{height} is outside 1..={max}",
                max = crate::MAX_GRID_SIZE
            ),
            ModelError::CellCount { expected, found } => {
                write!(f, "expected {expected} cells, found {found}")
            }
            ModelError::EmptyShape => f.write_str("shape has no cells"),
            ModelError::DisconnectedShape => f.write_str("shape is not connected"),
            ModelError::TooManyPieces(n) => {
                write!(f, "{n} pieces, at most {} allowed", crate::MAX_PIECES)
            }
            ModelError::ColorOutOfRange(color) => write!(
                f,
                "color {} is outside the {} a puzzle can use",
                color.0,
                crate::MAX_COLORS
            ),
            ModelError::ClueLength {
                color,
                axis,
                expected,
                found,
            } => write!(
                f,
                "color {}: expected {expected} {axis} clues, found {found}",
                color.0
            ),
        }
    }
}

impl core::error::Error for ModelError {}
