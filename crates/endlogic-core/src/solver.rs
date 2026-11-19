//! Backtracking solver.
//!
//! Pieces with the same color and the same shape up to rotation are grouped
//! into one *piece type*; copies of a type are always placed in increasing
//! candidate order, so permuting identical pieces never yields a second
//! solution. Candidates are built only from distinct orientations, so
//! symmetric shapes do not either.
//!
//! At every node the solver filters each remaining type's candidate
//! placements against the current board, then picks the type with the fewest
//! candidates. A branch is cut when a type has fewer candidates than copies
//! left, or when some row or column can no longer reach its clue, either
//! because too few of its cells are still coverable or because the remaining
//! pieces are too small to supply the missing cells.

use alloc::vec;
use alloc::vec::Vec;

use crate::color::Color;
use crate::grid::{Cell, MAX_GRID_SIZE};
use crate::puzzle::Puzzle;
use crate::shape::{Rotation, Shape};
use crate::solution::{Placement, Solution};

/// Default node budget for one search.
///
/// Every sample in `puzzles/` needs less than a hundredth of it.
pub const DEFAULT_NODE_BUDGET: u64 = 200_000;

/// Why a search stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopReason {
    /// The whole search space was explored: the solutions found are all the
    /// solutions there are.
    Exhausted,
    /// The requested number of solutions was found; there may be more.
    SolutionLimit,
    /// The node budget ran out first; the search is incomplete.
    NodeBudget,
}

/// Outcome of [`search`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchResult {
    /// Solutions found, in the solver's deterministic order.
    pub solutions: Vec<Solution>,
    /// Nodes spent: the number of piece placements tried.
    pub nodes: u64,
    /// Why the search stopped.
    pub stop: StopReason,
}

/// Searches for up to `max_solutions` solutions, trying at most `max_nodes`
/// piece placements.
///
/// A *node* is one attempt to put a piece on the board. The count is
/// deterministic: the same puzzle and limits give the same `nodes`, the same
/// solutions in the same order and the same [`StopReason`] on every platform.
///
/// The limits are checked in this order: once `max_solutions` solutions are
/// found the search stops with [`StopReason::SolutionLimit`]; a placement is
/// only tried while fewer than `max_nodes` nodes have been spent, otherwise
/// the search stops with [`StopReason::NodeBudget`]. A search that finishes
/// within both limits reports [`StopReason::Exhausted`], so `Exhausted` always
/// means the solution list is complete. `max_solutions == 0` returns
/// immediately with `SolutionLimit`.
///
/// This is a single [`Search::step`] of `max_nodes` nodes.
#[must_use]
pub fn search(puzzle: &Puzzle, max_solutions: usize, max_nodes: u64) -> SearchResult {
    let mut search = Search::new(puzzle, max_solutions);
    match search.step(max_nodes) {
        Progress::Done(result) => result,
        Progress::Working => SearchResult {
            solutions: search.solutions,
            nodes: search.nodes,
            stop: StopReason::NodeBudget,
        },
    }
}

/// Result of one [`Search::step`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Progress {
    /// The step used up its nodes; call `step` again to continue.
    Working,
    /// The search has finished. Further calls return the same result.
    Done(SearchResult),
}

/// A search that runs a limited number of nodes at a time.
///
/// Running it to the end in steps of any size gives exactly what [`search`]
/// gives with a large enough budget: the same solutions in the same order,
/// the same node count and the same [`StopReason`]. A front end can call
/// [`step`](Search::step) once per frame so a long search never blocks.
pub struct Search {
    board: Option<Board>,
    stack: Vec<Frame>,
    solutions: Vec<Solution>,
    max_solutions: usize,
    nodes: u64,
    finished: Option<StopReason>,
}

impl Search {
    /// Prepares a search for up to `max_solutions` solutions. No nodes are
    /// spent until [`step`](Search::step) is called.
    #[must_use]
    pub fn new(puzzle: &Puzzle, max_solutions: usize) -> Search {
        let mut search = Search {
            board: None,
            stack: Vec::new(),
            solutions: Vec::new(),
            max_solutions,
            nodes: 0,
            finished: None,
        };
        if max_solutions == 0 {
            search.finished = Some(StopReason::SolutionLimit);
            return search;
        }
        let Some(board) = Board::new(puzzle) else {
            search.finished = Some(StopReason::Exhausted);
            return search;
        };
        let all: Vec<Vec<usize>> = board
            .types
            .iter()
            .map(|t| (0..t.candidates.len()).collect())
            .collect();
        match board.filter(&all) {
            Some(candidates) => {
                search.board = Some(board);
                search.enter(candidates);
            }
            None => search.finished = Some(StopReason::Exhausted),
        }
        search
    }

    /// Nodes spent so far.
    #[must_use]
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// Solutions found so far.
    #[must_use]
    pub fn solutions(&self) -> &[Solution] {
        &self.solutions
    }

    /// Continues the search, trying at most `max_nodes` more placements.
    ///
    /// The step stops right before the placement that would exceed
    /// `max_nodes`, so a search that can end without another placement ends
    /// in this step.
    pub fn step(&mut self, max_nodes: u64) -> Progress {
        if self.finished.is_none() {
            self.finished = self.advance(max_nodes);
        }
        match self.finished {
            Some(stop) => Progress::Done(SearchResult {
                solutions: self.solutions.clone(),
                nodes: self.nodes,
                stop,
            }),
            None => Progress::Working,
        }
    }

    /// Starts on a board whose valid candidates are `candidates`: records a
    /// solution if every piece is placed, otherwise pushes a frame.
    fn enter(&mut self, candidates: Vec<Vec<usize>>) {
        let Some(board) = &self.board else { return };
        let next = (0..board.types.len())
            .filter(|&t| board.remaining[t] > 0)
            .min_by_key(|&t| candidates[t].len());
        match next {
            // Every piece is placed, and `filter` has confirmed every line
            // is at its clue.
            None => self.solutions.push(board.solution()),
            Some(t) => self.stack.push(Frame {
                saved_min: board.min_index[t],
                candidates,
                t,
                next: 0,
                applied: None,
            }),
        }
    }

    /// Runs the depth-first search for at most `max_nodes` placements.
    /// Returns why the search ended, or `None` if it is still going.
    fn advance(&mut self, max_nodes: u64) -> Option<StopReason> {
        let mut spent = 0u64;
        loop {
            if self.solutions.len() >= self.max_solutions {
                return Some(StopReason::SolutionLimit);
            }
            let Some(board) = self.board.as_mut() else {
                return Some(StopReason::Exhausted);
            };
            let Some(frame) = self.stack.last_mut() else {
                return Some(StopReason::Exhausted);
            };
            let t = frame.t;
            if let Some(i) = frame.applied.take() {
                board.undo(t, i, frame.saved_min);
            }
            let Some(&i) = frame.candidates[t].get(frame.next) else {
                self.stack.pop();
                continue;
            };
            if spent >= max_nodes {
                return None;
            }
            spent += 1;
            self.nodes += 1;
            frame.next += 1;
            frame.applied = Some(i);
            board.place(t, i);
            if let Some(child) = board.filter(&frame.candidates) {
                self.enter(child);
            }
        }
    }
}

/// One level of the depth-first search: the type placed at this level and
/// the candidates still valid on the board when the level was entered.
struct Frame {
    candidates: Vec<Vec<usize>>,
    t: usize,
    /// Position in `candidates[t]` of the next candidate to try.
    next: usize,
    /// `min_index[t]` before this level changed it.
    saved_min: usize,
    /// The candidate currently on the board, to be taken back.
    applied: Option<usize>,
}

/// Answer of [`check_uniqueness`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Uniqueness {
    /// The puzzle has no solution.
    NoSolution,
    /// The puzzle has exactly one solution (up to swapping identical pieces).
    Unique(Solution),
    /// The puzzle has at least two solutions.
    Multiple,
    /// The node budget ran out before the question was settled.
    Unknown,
}

/// Outcome of [`check_uniqueness`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniquenessCheck {
    /// The answer.
    pub uniqueness: Uniqueness,
    /// Nodes spent.
    pub nodes: u64,
}

/// Decides whether the puzzle has no, exactly one or several solutions,
/// trying at most `max_nodes` piece placements.
#[must_use]
pub fn check_uniqueness(puzzle: &Puzzle, max_nodes: u64) -> UniquenessCheck {
    let result = search(puzzle, 2, max_nodes);
    let mut solutions = result.solutions;
    let uniqueness = match (result.stop, solutions.len()) {
        (StopReason::Exhausted, 0) => Uniqueness::NoSolution,
        (StopReason::Exhausted, _) => Uniqueness::Unique(solutions.swap_remove(0)),
        (StopReason::SolutionLimit, _) => Uniqueness::Multiple,
        (StopReason::NodeBudget, _) => Uniqueness::Unknown,
    };
    UniquenessCheck {
        uniqueness,
        nodes: result.nodes,
    }
}

/// One way to put a piece type on the board.
struct Candidate {
    /// Index into the type's orientations.
    orientation: usize,
    row: usize,
    col: usize,
    /// `(row, mask, cell count)` for every row the placement touches.
    rows: Vec<(usize, u16, usize)>,
    /// `(col, cell count)` for every column the placement touches.
    cols: Vec<(usize, usize)>,
}

struct PieceType {
    /// Index into `Search::colors`.
    color: usize,
    /// Indices of the pieces of this type, ascending.
    members: Vec<usize>,
    /// For each member, for each orientation, the rotation of that member's
    /// own shape that produces it.
    member_rotations: Vec<Vec<Rotation>>,
    candidates: Vec<Candidate>,
    /// Most cells one piece can put into a single line.
    max_span: usize,
}

type Lines = [usize; MAX_GRID_SIZE];

/// The board during a search: what is placed and what each line holds.
struct Board {
    width: usize,
    height: usize,
    piece_count: usize,
    types: Vec<PieceType>,
    row_target: Vec<Lines>,
    col_target: Vec<Lines>,
    row_filled: Vec<Lines>,
    col_filled: Vec<Lines>,
    occupied: [u16; MAX_GRID_SIZE],
    remaining: Vec<usize>,
    /// Smallest candidate index the next copy of each type may use.
    min_index: Vec<usize>,
    /// Candidate indices chosen so far for each type, ascending.
    chosen: Vec<Vec<usize>>,
}

impl Board {
    /// Builds the empty board, or returns `None` if a quick check already
    /// shows there is no solution.
    fn new(puzzle: &Puzzle) -> Option<Board> {
        let grid = puzzle.grid();
        let (width, height) = (grid.width(), grid.height());
        let colors: Vec<Color> = puzzle.colors();
        let color_index = |color: Color| colors.binary_search(&color).unwrap_or(0);

        let mut row_target = vec![[0; MAX_GRID_SIZE]; colors.len()];
        let mut col_target = vec![[0; MAX_GRID_SIZE]; colors.len()];
        let mut row_filled = vec![[0; MAX_GRID_SIZE]; colors.len()];
        let mut col_filled = vec![[0; MAX_GRID_SIZE]; colors.len()];
        let mut area = vec![0usize; colors.len()];
        let mut free = [0u16; MAX_GRID_SIZE];

        for (k, &color) in colors.iter().enumerate() {
            for (r, target) in row_target[k].iter_mut().enumerate().take(height) {
                *target = puzzle.row_clue(color, r);
            }
            for (c, target) in col_target[k].iter_mut().enumerate().take(width) {
                *target = puzzle.col_clue(color, c);
            }
        }
        for r in 0..height {
            for (c, &cell) in grid.row(r).iter().enumerate() {
                match cell {
                    Cell::Free => free[r] |= 1 << c,
                    Cell::Lock(color) => {
                        let k = color_index(color);
                        row_filled[k][r] += 1;
                        col_filled[k][c] += 1;
                        area[k] += 1;
                    }
                    Cell::Dead => {}
                }
            }
        }
        for piece in puzzle.pieces() {
            area[color_index(piece.color)] += piece.shape.area();
        }

        // Quick rejection: sums must agree, and locks alone must not exceed
        // any clue.
        for k in 0..colors.len() {
            let row_sum: usize = row_target[k].iter().sum();
            let col_sum: usize = col_target[k].iter().sum();
            if row_sum != col_sum || row_sum != area[k] {
                return None;
            }
            if (0..height).any(|r| row_filled[k][r] > row_target[k][r])
                || (0..width).any(|c| col_filled[k][c] > col_target[k][c])
            {
                return None;
            }
        }

        let types = build_types(puzzle, &color_index, &free);
        let type_count = types.len();
        Some(Board {
            width,
            height,
            piece_count: puzzle.pieces().len(),
            remaining: types.iter().map(|t| t.members.len()).collect(),
            types,
            row_target,
            col_target,
            row_filled,
            col_filled,
            occupied: [0; MAX_GRID_SIZE],
            min_index: vec![0; type_count],
            chosen: vec![Vec::new(); type_count],
        })
    }

    fn fits(&self, color: usize, candidate: &Candidate) -> bool {
        candidate.rows.iter().all(|&(r, mask, n)| {
            self.occupied[r] & mask == 0
                && self.row_filled[color][r] + n <= self.row_target[color][r]
        }) && candidate
            .cols
            .iter()
            .all(|&(c, n)| self.col_filled[color][c] + n <= self.col_target[color][c])
    }

    /// Narrows the parent's candidate lists to those that still fit, and
    /// checks that every line can still reach its clue. Returns `None` if the
    /// current board is a dead end.
    fn filter(&self, parent: &[Vec<usize>]) -> Option<Vec<Vec<usize>>> {
        let colors = self.row_target.len();
        let mut cover = vec![[0u16; MAX_GRID_SIZE]; colors];
        let mut span = vec![0usize; colors];
        let mut result = vec![Vec::new(); self.types.len()];

        for (t, piece_type) in self.types.iter().enumerate() {
            if self.remaining[t] == 0 {
                continue;
            }
            let k = piece_type.color;
            span[k] += self.remaining[t] * piece_type.max_span;
            for &i in &parent[t] {
                let candidate = &piece_type.candidates[i];
                if i >= self.min_index[t] && self.fits(k, candidate) {
                    result[t].push(i);
                    for &(r, mask, _) in &candidate.rows {
                        cover[k][r] |= mask;
                    }
                }
            }
            if result[t].len() < self.remaining[t] {
                return None;
            }
        }

        for k in 0..colors {
            for r in 0..self.height {
                let need = self.row_target[k][r] - self.row_filled[k][r];
                if need > 0 && (need > span[k] || need > cover[k][r].count_ones() as usize) {
                    return None;
                }
            }
            for c in 0..self.width {
                let need = self.col_target[k][c] - self.col_filled[k][c];
                if need > 0 {
                    let coverable = (0..self.height)
                        .filter(|&r| cover[k][r] & (1 << c) != 0)
                        .count();
                    if need > span[k] || need > coverable {
                        return None;
                    }
                }
            }
        }
        Some(result)
    }

    fn apply(&mut self, t: usize, i: usize, add: bool) {
        let piece_type = &self.types[t];
        let k = piece_type.color;
        let candidate = &piece_type.candidates[i];
        for &(r, mask, n) in &candidate.rows {
            self.occupied[r] ^= mask;
            if add {
                self.row_filled[k][r] += n;
            } else {
                self.row_filled[k][r] -= n;
            }
        }
        for &(c, n) in &candidate.cols {
            if add {
                self.col_filled[k][c] += n;
            } else {
                self.col_filled[k][c] -= n;
            }
        }
    }

    /// Puts candidate `i` of type `t` on the board.
    fn place(&mut self, t: usize, i: usize) {
        self.apply(t, i, true);
        self.remaining[t] -= 1;
        self.min_index[t] = i + 1;
        self.chosen[t].push(i);
    }

    /// Takes back [`place`](Board::place) and restores `min_index[t]`.
    fn undo(&mut self, t: usize, i: usize, saved_min: usize) {
        self.chosen[t].pop();
        self.min_index[t] = saved_min;
        self.remaining[t] += 1;
        self.apply(t, i, false);
    }

    fn solution(&self) -> Solution {
        let mut placements = vec![Placement::default(); self.piece_count];
        for (piece_type, chosen) in self.types.iter().zip(&self.chosen) {
            for (m, &i) in chosen.iter().enumerate() {
                let candidate = &piece_type.candidates[i];
                placements[piece_type.members[m]] = Placement {
                    rotation: piece_type.member_rotations[m][candidate.orientation],
                    row: candidate.row,
                    col: candidate.col,
                };
            }
        }
        Solution { placements }
    }
}

/// Groups identical pieces and enumerates candidate placements on free cells.
fn build_types(
    puzzle: &Puzzle,
    color_index: &dyn Fn(Color) -> usize,
    free: &[u16; MAX_GRID_SIZE],
) -> Vec<PieceType> {
    let grid = puzzle.grid();
    let mut types: Vec<PieceType> = Vec::new();
    let mut keys: Vec<(Color, Shape)> = Vec::new();
    let mut orientation_sets: Vec<Vec<Shape>> = Vec::new();

    for (index, piece) in puzzle.pieces().iter().enumerate() {
        let own = piece.shape.orientations();
        let canonical = own
            .iter()
            .map(|(_, s)| s)
            .min()
            .cloned()
            .unwrap_or_else(|| piece.shape.clone());
        let key = (piece.color, canonical);
        let t = if let Some(t) = keys.iter().position(|k| *k == key) {
            t
        } else {
            let orientations: Vec<Shape> = own.iter().map(|(_, s)| s.clone()).collect();
            let candidates = enumerate_candidates(&orientations, grid.width(), grid.height(), free);
            types.push(PieceType {
                color: color_index(piece.color),
                members: Vec::new(),
                member_rotations: Vec::new(),
                candidates,
                max_span: piece.shape.max_line_span(),
            });
            keys.push(key);
            orientation_sets.push(orientations);
            types.len() - 1
        };
        let rotations = orientation_sets[t]
            .iter()
            .map(|target| {
                own.iter()
                    .find(|(_, s)| s == target)
                    .map_or(Rotation::R0, |(r, _)| *r)
            })
            .collect();
        types[t].members.push(index);
        types[t].member_rotations.push(rotations);
    }
    types
}

fn enumerate_candidates(
    orientations: &[Shape],
    width: usize,
    height: usize,
    free: &[u16; MAX_GRID_SIZE],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (orientation, shape) in orientations.iter().enumerate() {
        if shape.height() > height || shape.width() > width {
            continue;
        }
        for row in 0..=height - shape.height() {
            for col in 0..=width - shape.width() {
                let mut masks = [0u16; MAX_GRID_SIZE];
                let mut col_counts = [0usize; MAX_GRID_SIZE];
                for &(dr, dc) in shape.cells() {
                    masks[row + dr] |= 1 << (col + dc);
                    col_counts[col + dc] += 1;
                }
                let on_free = (row..row + shape.height()).all(|r| masks[r] & !free[r] == 0);
                if !on_free {
                    continue;
                }
                candidates.push(Candidate {
                    orientation,
                    row,
                    col,
                    rows: (row..row + shape.height())
                        .filter(|&r| masks[r] != 0)
                        .map(|r| (r, masks[r], masks[r].count_ones() as usize))
                        .collect(),
                    cols: (col..col + shape.width())
                        .filter(|&c| col_counts[c] != 0)
                        .map(|c| (c, col_counts[c]))
                        .collect(),
                });
            }
        }
    }
    candidates
}
