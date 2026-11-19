//! The puzzle board.

use alloc::vec;
use alloc::vec::Vec;

use crate::color::Color;
use crate::error::ModelError;

/// Largest allowed grid width and height.
pub const MAX_GRID_SIZE: usize = 16;

/// State of a single grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cell {
    /// Empty cell that a piece may cover.
    #[default]
    Free,
    /// Pre-filled cell. Counts as filled for its color; pieces cannot cover it.
    Lock(Color),
    /// Unusable cell. Never filled and never covered.
    Dead,
}

/// A rectangular board of cells, between 1×1 and 16×16.
///
/// Cells are addressed by `(row, col)`, both zero-based, with row 0 at the top.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    /// Creates a grid where every cell is [`Cell::Free`].
    ///
    /// # Errors
    ///
    /// [`ModelError::GridSize`] if a dimension is outside `1..=MAX_GRID_SIZE`.
    pub fn new(width: usize, height: usize) -> Result<Grid, ModelError> {
        Grid::from_cells(width, height, vec![Cell::Free; width * height])
    }

    /// Creates a grid from cells in row-major order.
    ///
    /// # Errors
    ///
    /// [`ModelError::GridSize`] if a dimension is outside `1..=MAX_GRID_SIZE`,
    /// [`ModelError::CellCount`] if `cells.len() != width * height`.
    pub fn from_cells(width: usize, height: usize, cells: Vec<Cell>) -> Result<Grid, ModelError> {
        if !(1..=MAX_GRID_SIZE).contains(&width) || !(1..=MAX_GRID_SIZE).contains(&height) {
            return Err(ModelError::GridSize { width, height });
        }
        if cells.len() != width * height {
            return Err(ModelError::CellCount {
                expected: width * height,
                found: cells.len(),
            });
        }
        Ok(Grid {
            width,
            height,
            cells,
        })
    }

    /// Number of columns.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Number of rows.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the cell at `(row, col)`, or `None` if it is outside the grid.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> Option<Cell> {
        (row < self.height && col < self.width).then(|| self.cells[row * self.width + col])
    }

    /// Replaces the cell at `(row, col)`.
    ///
    /// # Panics
    ///
    /// Panics if `(row, col)` is outside the grid.
    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        assert!(
            row < self.height && col < self.width,
            "cell ({row}, {col}) is outside a {}x{} grid",
            self.width,
            self.height
        );
        self.cells[row * self.width + col] = cell;
    }

    /// All cells in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cells of one row, left to right.
    ///
    /// # Panics
    ///
    /// Panics if `row >= self.height()`.
    #[must_use]
    pub fn row(&self, row: usize) -> &[Cell] {
        &self.cells[row * self.width..(row + 1) * self.width]
    }
}
