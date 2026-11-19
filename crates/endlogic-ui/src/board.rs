//! The player's board: where each piece is, line counts and the win test.

use endlogic_core::{Cell, Placement, Puzzle, Rotation, Shape};

/// Where a piece is and how it is turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PieceState {
    /// Clockwise quarter turns from the shape in the puzzle, 0 to 3.
    pub turns: u8,
    /// Top-left cell of the turned shape's bounding box, or `None` in the
    /// tray.
    pub at: Option<(usize, usize)>,
}

/// How a row or column compares with its clue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineState {
    /// Fewer cells than the clue.
    Under,
    /// Exactly the clue.
    Exact,
    /// More cells than the clue.
    Over,
}

/// Filled cells and clue of one row or column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Line {
    /// Filled cells: placed piece cells plus locks.
    pub filled: usize,
    /// The clue.
    pub clue: usize,
}

impl Line {
    /// Comparison with the clue.
    #[must_use]
    pub fn state(self) -> LineState {
        match self.filled.cmp(&self.clue) {
            core::cmp::Ordering::Less => LineState::Under,
            core::cmp::Ordering::Equal => LineState::Exact,
            core::cmp::Ordering::Greater => LineState::Over,
        }
    }
}

/// Where a rotated piece ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Turned {
    /// It was in the tray and stays there.
    InTray,
    /// It stays on the board at this top-left cell.
    Stays(usize, usize),
    /// It did not fit in the new orientation, so it stayed where it was,
    /// unturned.
    Blocked,
}

/// The puzzle plus the player's piece positions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Board {
    puzzle: Puzzle,
    /// `shapes[i][t]` is piece `i` turned `t` times.
    shapes: Vec<[Shape; 4]>,
    pieces: Vec<PieceState>,
    /// Piece covering each cell, row-major.
    owner: Vec<Option<usize>>,
}

impl Board {
    /// A board with every piece in the tray, unturned.
    #[must_use]
    pub fn new(puzzle: Puzzle) -> Board {
        let shapes = puzzle
            .pieces()
            .iter()
            .map(|p| Rotation::ALL.map(|r| p.shape.rotated(r)))
            .collect();
        let cells = puzzle.grid().width() * puzzle.grid().height();
        Board {
            pieces: vec![PieceState::default(); puzzle.pieces().len()],
            owner: vec![None; cells],
            shapes,
            puzzle,
        }
    }

    /// The puzzle.
    #[must_use]
    pub fn puzzle(&self) -> &Puzzle {
        &self.puzzle
    }

    /// Board width in cells.
    #[must_use]
    pub fn width(&self) -> usize {
        self.puzzle.grid().width()
    }

    /// Board height in cells.
    #[must_use]
    pub fn height(&self) -> usize {
        self.puzzle.grid().height()
    }

    /// Number of pieces.
    #[must_use]
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// State of piece `i`.
    #[must_use]
    pub fn piece(&self, i: usize) -> PieceState {
        self.pieces[i]
    }

    /// Piece `i` turned `turns` times.
    #[must_use]
    pub fn shape_turned(&self, i: usize, turns: u8) -> &Shape {
        &self.shapes[i][usize::from(turns % 4)]
    }

    /// Piece `i` in its current orientation.
    #[must_use]
    pub fn shape(&self, i: usize) -> &Shape {
        self.shape_turned(i, self.pieces[i].turns)
    }

    /// Longest side of any piece in any orientation.
    #[must_use]
    pub fn max_piece_side(&self) -> usize {
        self.shapes
            .iter()
            .map(|s| s[0].width().max(s[0].height()))
            .max()
            .unwrap_or(1)
    }

    /// The board cell at `(row, col)`.
    #[must_use]
    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.puzzle.grid().get(row, col).unwrap_or(Cell::Dead)
    }

    /// The piece covering `(row, col)`, if any.
    #[must_use]
    pub fn owner(&self, row: usize, col: usize) -> Option<usize> {
        if row < self.height() && col < self.width() {
            self.owner[row * self.width() + col]
        } else {
            None
        }
    }

    /// Whether piece `i`, turned `turns` times, fits with its top-left
    /// corner at `(row, col)`: every cell on the board, on a free cell, and
    /// not on another piece. Piece `i`'s own current cells count as empty.
    #[must_use]
    pub fn fits(&self, i: usize, turns: u8, row: isize, col: isize) -> bool {
        self.shape_turned(i, turns).cells().iter().all(|&(dr, dc)| {
            let (Ok(r), Ok(c)) = (
                usize::try_from(row + to_isize(dr)),
                usize::try_from(col + to_isize(dc)),
            ) else {
                return false;
            };
            r < self.height()
                && c < self.width()
                && self.cell(r, c) == Cell::Free
                && self.owner(r, c).is_none_or(|o| o == i)
        })
    }

    /// Puts piece `i` (in its current orientation) at `(row, col)` if it
    /// fits. Returns whether it was placed.
    pub fn place(&mut self, i: usize, row: isize, col: isize) -> bool {
        if !self.fits(i, self.pieces[i].turns, row, col) {
            return false;
        }
        let (Ok(row), Ok(col)) = (usize::try_from(row), usize::try_from(col)) else {
            return false;
        };
        self.remove(i);
        self.pieces[i].at = Some((row, col));
        self.mark(i, Some(i));
        true
    }

    /// Moves piece `i` to the tray.
    pub fn remove(&mut self, i: usize) {
        self.mark(i, None);
        self.pieces[i].at = None;
    }

    /// Sets piece `i`'s turns; the piece must be in the tray.
    pub fn set_turns(&mut self, i: usize, turns: u8) {
        debug_assert!(self.pieces[i].at.is_none());
        self.pieces[i].turns = turns % 4;
    }

    /// Turns piece `i` a quarter clockwise. A piece on the board turns
    /// about its center; if it does not fit there turned, nothing changes.
    pub fn rotate(&mut self, i: usize) -> Turned {
        let state = self.pieces[i];
        let turns = (state.turns + 1) % 4;
        let Some((row, col)) = state.at else {
            self.pieces[i].turns = turns;
            return Turned::InTray;
        };
        let old = self.shape(i);
        let (old_h, old_w) = (to_isize(old.height()), to_isize(old.width()));
        let (row, col) = (to_isize(row), to_isize(col));
        // Keep the center: the bounding box swaps sides, so the corner moves
        // by half the difference, rounded either way.
        let dr = [
            (old_h - old_w).div_euclid(2),
            (old_h - old_w + 1).div_euclid(2),
        ];
        let dc = [
            (old_w - old_h).div_euclid(2),
            (old_w - old_h + 1).div_euclid(2),
        ];
        let spot = dr
            .iter()
            .flat_map(|&a| dc.iter().map(move |&b| (row + a, col + b)))
            .find(|&(r, c)| self.fits(i, turns, r, c));
        let Some((r, c)) = spot else {
            return Turned::Blocked;
        };
        self.remove(i);
        self.pieces[i].turns = turns;
        self.place(i, r, c);
        let (r, c) = self.pieces[i].at.unwrap_or_default();
        Turned::Stays(r, c)
    }

    /// Where each piece stands, as solver placements; `None` in the tray.
    #[must_use]
    pub fn placements(&self) -> Vec<Option<Placement>> {
        self.pieces
            .iter()
            .map(|p| {
                p.at.map(|(row, col)| Placement {
                    rotation: Rotation::ALL[usize::from(p.turns % 4)],
                    row,
                    col,
                })
            })
            .collect()
    }

    /// The cells piece `i` covers, sorted; empty in the tray.
    #[must_use]
    pub fn cells_of(&self, i: usize) -> Vec<(usize, usize)> {
        let Some((row, col)) = self.pieces[i].at else {
            return Vec::new();
        };
        self.shape(i)
            .cells()
            .iter()
            .map(|&(r, c)| (row + r, col + c))
            .collect()
    }

    /// Moves every piece to the tray, keeping orientations.
    pub fn reset(&mut self) {
        for i in 0..self.pieces.len() {
            self.remove(i);
        }
    }

    /// Row counts, top to bottom.
    #[must_use]
    pub fn rows(&self) -> Vec<Line> {
        (0..self.height())
            .map(|r| Line {
                filled: (0..self.width()).filter(|&c| self.filled(r, c)).count(),
                clue: self.clue(r, true),
            })
            .collect()
    }

    /// Column counts, left to right.
    #[must_use]
    pub fn cols(&self) -> Vec<Line> {
        (0..self.width())
            .map(|c| Line {
                filled: (0..self.height()).filter(|&r| self.filled(r, c)).count(),
                clue: self.clue(c, false),
            })
            .collect()
    }

    /// Whether every piece is on the board and every line matches its clue.
    #[must_use]
    pub fn is_solved(&self) -> bool {
        self.pieces.iter().all(|p| p.at.is_some())
            && self
                .rows()
                .into_iter()
                .chain(self.cols())
                .all(|line| line.state() == LineState::Exact)
    }

    fn filled(&self, row: usize, col: usize) -> bool {
        matches!(self.cell(row, col), Cell::Lock(_)) || self.owner(row, col).is_some()
    }

    fn clue(&self, index: usize, row: bool) -> usize {
        // Only single-color puzzles are played, so the first color's clues
        // are the clues.
        self.puzzle.clues().values().next().map_or(0, |clues| {
            let line = if row { &clues.rows } else { &clues.cols };
            line.get(index).copied().unwrap_or(0)
        })
    }

    fn mark(&mut self, i: usize, value: Option<usize>) {
        let Some((row, col)) = self.pieces[i].at else {
            return;
        };
        let width = self.width();
        for &(dr, dc) in self.shapes[i][usize::from(self.pieces[i].turns)].cells() {
            self.owner[(row + dr) * width + col + dc] = value;
        }
    }
}

/// Converts a grid coordinate to `isize`.
fn to_isize(n: usize) -> isize {
    // Grid coordinates are at most 16.
    isize::try_from(n).unwrap_or(isize::MAX)
}
