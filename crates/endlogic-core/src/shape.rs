//! Polyomino shapes and rotations.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::ModelError;

/// A clockwise rotation by a multiple of 90°.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Rotation {
    /// No rotation.
    #[default]
    R0,
    /// 90° clockwise.
    R90,
    /// 180°.
    R180,
    /// 270° clockwise (90° counter-clockwise).
    R270,
}

impl Rotation {
    /// All rotations in increasing order.
    pub const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

    /// Number of clockwise quarter turns (0 to 3).
    #[must_use]
    pub fn quarter_turns(self) -> usize {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 1,
            Rotation::R180 => 2,
            Rotation::R270 => 3,
        }
    }
}

/// A non-empty, 4-connected set of cells, normalized so that its bounding box
/// starts at `(0, 0)`.
///
/// Cells are `(row, col)` pairs kept sorted in row-major order, so two shapes
/// are equal exactly when they cover the same cells.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Shape {
    cells: Vec<(usize, usize)>,
    width: usize,
    height: usize,
}

impl Shape {
    /// Builds a shape from `(row, col)` cells, removing duplicates and
    /// shifting it to the top-left corner.
    ///
    /// # Errors
    ///
    /// [`ModelError::EmptyShape`] if there are no cells,
    /// [`ModelError::DisconnectedShape`] if the cells are not 4-connected.
    pub fn new(cells: impl IntoIterator<Item = (usize, usize)>) -> Result<Shape, ModelError> {
        let mut cells: Vec<(usize, usize)> = cells.into_iter().collect();
        if cells.is_empty() {
            return Err(ModelError::EmptyShape);
        }
        crate::sort::sort_dedup(&mut cells);
        if !is_connected(&cells) {
            return Err(ModelError::DisconnectedShape);
        }
        Ok(Shape::normalized(cells))
    }

    /// Shifts already deduplicated cells to the origin and computes the size.
    fn normalized(mut cells: Vec<(usize, usize)>) -> Shape {
        let min_row = cells.iter().map(|c| c.0).min().unwrap_or(0);
        let min_col = cells.iter().map(|c| c.1).min().unwrap_or(0);
        for cell in &mut cells {
            cell.0 -= min_row;
            cell.1 -= min_col;
        }
        crate::sort::sort(&mut cells);
        let height = cells.iter().map(|c| c.0).max().map_or(0, |r| r + 1);
        let width = cells.iter().map(|c| c.1).max().map_or(0, |c| c + 1);
        Shape {
            cells,
            width,
            height,
        }
    }

    /// Cells as `(row, col)` pairs in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[(usize, usize)] {
        &self.cells
    }

    /// Width of the bounding box.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height of the bounding box.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Number of cells.
    #[must_use]
    pub fn area(&self) -> usize {
        self.cells.len()
    }

    /// Returns whether the shape covers `(row, col)`.
    #[must_use]
    pub fn contains(&self, row: usize, col: usize) -> bool {
        self.cells.binary_search(&(row, col)).is_ok()
    }

    /// Returns this shape rotated clockwise by `rotation`, normalized again.
    #[must_use]
    pub fn rotated(&self, rotation: Rotation) -> Shape {
        let mut shape = self.clone();
        for _ in 0..rotation.quarter_turns() {
            shape = shape.rotated_once();
        }
        shape
    }

    fn rotated_once(&self) -> Shape {
        // Clockwise: (r, c) -> (c, height - 1 - r).
        let cells = self
            .cells
            .iter()
            .map(|&(r, c)| (c, self.height - 1 - r))
            .collect();
        Shape::normalized(cells)
    }

    /// All distinct orientations reachable by rotation, each paired with the
    /// smallest rotation that produces it, in increasing rotation order.
    ///
    /// Returns between 1 and 4 entries: a 2×2 square has one, a straight line
    /// two, an L-tetromino four. Mirror images are never produced.
    #[must_use]
    pub fn orientations(&self) -> Vec<(Rotation, Shape)> {
        let mut result: Vec<(Rotation, Shape)> = Vec::with_capacity(4);
        let mut shape = self.clone();
        for rotation in Rotation::ALL {
            if !result.iter().any(|(_, s)| *s == shape) {
                result.push((rotation, shape.clone()));
            }
            shape = shape.rotated_once();
        }
        result
    }

    /// Length of the widest row over all orientations: the most cells this
    /// shape can put into a single row or column.
    #[must_use]
    pub fn max_line_span(&self) -> usize {
        let mut per_row = vec![0usize; self.height];
        let mut per_col = vec![0usize; self.width];
        for &(r, c) in &self.cells {
            per_row[r] += 1;
            per_col[c] += 1;
        }
        per_row.into_iter().chain(per_col).max().unwrap_or(0)
    }
}

fn is_connected(sorted_cells: &[(usize, usize)]) -> bool {
    let mut seen = vec![false; sorted_cells.len()];
    let mut stack = vec![0usize];
    seen[0] = true;
    let mut visited = 1;
    while let Some(i) = stack.pop() {
        let (r, c) = sorted_cells[i];
        let neighbors = [
            r.checked_sub(1).map(|r| (r, c)),
            Some((r + 1, c)),
            c.checked_sub(1).map(|c| (r, c)),
            Some((r, c + 1)),
        ];
        for n in neighbors.into_iter().flatten() {
            if let Ok(j) = sorted_cells.binary_search(&n)
                && !seen[j]
            {
                seen[j] = true;
                visited += 1;
                stack.push(j);
            }
        }
    }
    visited == sorted_cells.len()
}
