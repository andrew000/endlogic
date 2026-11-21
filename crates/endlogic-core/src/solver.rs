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
//!
//! A search may start with some pieces already fixed in place
//! ([`Search::with_fixed`]); it then only places the rest.

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
#[derive(Debug, Clone)]
pub struct Search {
    board: Option<Board>,
    /// The candidate lists of every open level.
    lists: Lists,
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
        Search::with_fixed(puzzle, max_solutions, &[])
    }

    /// Prepares a search in which some pieces are already placed:
    /// `fixed[i]`, when present and `Some`, is where piece `i` stands. Only
    /// the other pieces are searched for, and every solution keeps the fixed
    /// placements as given.
    ///
    /// Identical pieces are still counted once: a fixed piece may take the
    /// place of any of its copies, and solutions that differ only by
    /// swapping the free copies are one solution. If the fixed placements
    /// are invalid on their own (outside the grid, on a locked or dead cell,
    /// overlapping, or over a clue), the search finds no solution.
    #[must_use]
    pub fn with_fixed(
        puzzle: &Puzzle,
        max_solutions: usize,
        fixed: &[Option<Placement>],
    ) -> Search {
        let mut search = Search {
            board: None,
            lists: Lists::default(),
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
        let Some(mut board) = Board::new(puzzle).and_then(|board| board.with_fixed(puzzle, fixed))
        else {
            search.finished = Some(StopReason::Exhausted);
            return search;
        };
        // The root level: every candidate of every type.
        for piece_type in &board.types {
            let start = search.lists.items.len();
            search
                .lists
                .items
                .extend((0..piece_type.candidates.len()).map(index16));
            search.lists.push_range(start);
        }
        match board.filter(0, &mut search.lists) {
            Some(level) => {
                search.board = Some(board);
                search.enter(level);
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

    /// Starts on a board whose valid candidates are the lists of `level`:
    /// records a solution if every piece is placed, otherwise pushes a
    /// frame for the type with the fewest candidates.
    fn enter(&mut self, level: usize) {
        let Some(board) = &self.board else { return };
        let next = (0..board.types.len())
            .filter(|&t| board.remaining[t] > 0)
            .min_by_key(|&t| self.lists.len(level, t));
        match next {
            // Every piece is placed, and `filter` has confirmed every line
            // is at its clue.
            None => {
                self.solutions.push(board.solution());
                self.lists.truncate(level);
            }
            Some(t) => self.stack.push(Frame {
                level,
                t,
                next: 0,
                saved_min: board.min_index[t],
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
            let Some(i) = self.lists.get(frame.level, t, frame.next) else {
                self.lists.truncate(frame.level);
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
            if let Some(level) = board.filter(frame.level, &mut self.lists) {
                self.enter(level);
            }
        }
    }
}

/// `n` as a candidate index; a type has at most 4 × 16 × 16 candidates.
#[allow(clippy::cast_possible_truncation)]
fn index16(n: usize) -> u16 {
    n as u16
}

/// The candidate lists of all open levels of the search in one buffer, so
/// that a node allocates nothing: a level is one range of candidate indices
/// per piece type, and levels are added and dropped like a stack.
#[derive(Debug, Clone, Default)]
struct Lists {
    /// Candidate indices, ascending within each range.
    items: Vec<u16>,
    /// `(start, end)` in `items`, one per type, level after level.
    ranges: Vec<(usize, usize)>,
}

impl Lists {
    /// Closes the range that began at `start`.
    fn push_range(&mut self, start: usize) {
        self.ranges.push((start, self.items.len()));
    }

    fn len(&self, level: usize, t: usize) -> usize {
        let (start, end) = self.ranges[level + t];
        end - start
    }

    /// The `k`-th candidate of type `t` on `level`.
    fn get(&self, level: usize, t: usize, k: usize) -> Option<u16> {
        let (start, end) = self.ranges[level + t];
        (start + k < end).then(|| self.items[start + k])
    }

    /// Drops `level` and every level after it.
    fn truncate(&mut self, level: usize) {
        if let Some(&(start, _)) = self.ranges.get(level) {
            self.items.truncate(start);
        }
        self.ranges.truncate(level);
    }
}

/// One level of the depth-first search: the type placed at this level.
#[derive(Debug, Clone)]
struct Frame {
    /// Where this level's candidate lists start in [`Lists::ranges`].
    level: usize,
    t: usize,
    /// Position in the type's list of the next candidate to try.
    next: usize,
    /// `min_index[t]` before this level changed it.
    saved_min: usize,
    /// The candidate currently on the board, to be taken back.
    applied: Option<u16>,
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

/// One way to put a piece type on the board: an orientation and the
/// top-left corner of its bounding box.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    orientation: u8,
    row: u8,
    col: u8,
}

/// An orientation of a piece type as bit rows and line counts. Shapes are
/// normalized, so every row and column of the bounding box has a cell.
#[derive(Debug, Clone)]
struct Layout {
    height: usize,
    width: usize,
    /// The cells of each row, bit `c` for column `c`.
    bits: [u16; MAX_GRID_SIZE],
    /// Cells in each row.
    row_cells: [u8; MAX_GRID_SIZE],
    /// Cells in each column.
    col_cells: [u8; MAX_GRID_SIZE],
}

impl Layout {
    fn of(shape: &Shape) -> Layout {
        let mut layout = Layout {
            height: shape.height(),
            width: shape.width(),
            bits: [0; MAX_GRID_SIZE],
            row_cells: [0; MAX_GRID_SIZE],
            col_cells: [0; MAX_GRID_SIZE],
        };
        for &(r, c) in shape.cells() {
            layout.bits[r] |= 1 << c;
            layout.row_cells[r] += 1;
            layout.col_cells[c] += 1;
        }
        layout
    }
}

#[derive(Debug, Clone)]
struct PieceType {
    /// Index into the board's colors.
    color: usize,
    /// Indices of the pieces of this type, ascending.
    members: Vec<usize>,
    /// Positions in `members` of the copies the search places, ascending;
    /// the others are fixed.
    free: Vec<usize>,
    /// The distinct orientations, as indexed by candidates.
    orientations: Vec<Shape>,
    /// The same orientations as bits and counts.
    layouts: Vec<Layout>,
    /// For each member, for each orientation, the rotation of that member's
    /// own shape that produces it.
    member_rotations: Vec<Vec<Rotation>>,
    candidates: Vec<Candidate>,
    /// For each candidate, its cells, and its cells in each row and each
    /// column, one byte per line.
    masks: Vec<Bits>,
    row_counts: Vec<u128>,
    col_counts: Vec<u128>,
    /// Most cells one piece can put into a single line.
    max_span: usize,
}

/// A count per row or column.
type Lines = [u8; MAX_GRID_SIZE];

/// The board during a search: what is placed and what each line still
/// needs.
#[derive(Debug, Clone)]
struct Board {
    width: usize,
    height: usize,
    piece_count: usize,
    types: Vec<PieceType>,
    /// For each color, the cells each row still needs: clue minus filled.
    row_room: Vec<Lines>,
    /// The same for columns.
    col_room: Vec<Lines>,
    occupied: [u16; MAX_GRID_SIZE],
    remaining: Vec<usize>,
    /// Smallest candidate index the next copy of each type may use.
    min_index: Vec<usize>,
    /// Candidate indices chosen so far for each type, ascending.
    chosen: Vec<Vec<u16>>,
    /// Pieces fixed before the search, with their placements.
    fixed: Vec<(usize, Placement)>,
    /// Scratch for `filter`, per color: the cells some candidate can still
    /// cover, and the most cells the remaining pieces can put into a line.
    cover: Vec<Bits>,
    span: Vec<usize>,
}

/// `n` as a small count; counts and coordinates are at most 16.
#[allow(clippy::cast_possible_truncation)]
fn count8(n: usize) -> u8 {
    n as u8
}

/// A set of cells of a 16 × 16 board, four rows to a word: cell `(r, c)` is
/// bit `(r % 4) * 16 + c` of word `r / 4`.
type Bits = [u64; 4];

/// The top bit of every byte: line counts are compared one byte per line.
const LANE_TOPS: u128 = u128::from_le_bytes([0x80; 16]);

fn meets(a: &Bits, b: &Bits) -> bool {
    (a[0] & b[0]) | (a[1] & b[1]) | (a[2] & b[2]) | (a[3] & b[3]) != 0
}

/// Cells of row `r` in `bits`.
fn row_count(bits: &Bits, r: usize) -> u8 {
    count8(((bits[r / 4] >> ((r % 4) * 16)) & 0xFFFF).count_ones() as usize)
}

impl Board {
    /// Builds the empty board, or returns `None` if a quick check already
    /// shows there is no solution.
    fn new(puzzle: &Puzzle) -> Option<Board> {
        let grid = puzzle.grid();
        let (width, height) = (grid.width(), grid.height());
        let colors: Vec<Color> = puzzle.colors();
        let color_index = |color: Color| colors.binary_search(&color).unwrap_or(0);

        let mut row_target = vec![[0usize; MAX_GRID_SIZE]; colors.len()];
        let mut col_target = vec![[0usize; MAX_GRID_SIZE]; colors.len()];
        let mut row_filled = vec![[0usize; MAX_GRID_SIZE]; colors.len()];
        let mut col_filled = vec![[0usize; MAX_GRID_SIZE]; colors.len()];
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
        // What each line still needs; no clue can exceed a line, so it
        // fits a `u8`.
        let room = |target: &[usize; MAX_GRID_SIZE], filled: &[usize; MAX_GRID_SIZE]| {
            let mut lines: Lines = [0; MAX_GRID_SIZE];
            for (line, (t, f)) in lines.iter_mut().zip(target.iter().zip(filled)) {
                *line = count8(t - f);
            }
            lines
        };
        let row_room = row_target
            .iter()
            .zip(&row_filled)
            .map(|(t, f)| room(t, f))
            .collect();
        let col_room = col_target
            .iter()
            .zip(&col_filled)
            .map(|(t, f)| room(t, f))
            .collect();

        let types = build_types(puzzle, &color_index, &free);
        let type_count = types.len();
        Some(Board {
            width,
            height,
            piece_count: puzzle.pieces().len(),
            remaining: types.iter().map(|t| t.members.len()).collect(),
            types,
            row_room,
            col_room,
            occupied: [0; MAX_GRID_SIZE],
            min_index: vec![0; type_count],
            chosen: vec![Vec::new(); type_count],
            fixed: Vec::new(),
            cover: vec![[0; 4]; colors.len()],
            span: vec![0; colors.len()],
        })
    }

    /// Puts the fixed pieces on the board, or returns `None` if one of them
    /// cannot stand where it is given.
    fn with_fixed(mut self, puzzle: &Puzzle, fixed: &[Option<Placement>]) -> Option<Board> {
        let pieces = puzzle.pieces();
        for (index, placement) in fixed.iter().enumerate().take(pieces.len()) {
            let Some(placement) = *placement else {
                continue;
            };
            let shape = pieces[index].shape.rotated(placement.rotation);
            let (t, m) = self.types.iter().enumerate().find_map(|(t, piece_type)| {
                let m = piece_type.members.iter().position(|&p| p == index)?;
                Some((t, m))
            })?;
            let piece_type = &self.types[t];
            let orientation = piece_type.orientations.iter().position(|s| *s == shape)?;
            let i = piece_type.candidates.iter().position(|c| {
                usize::from(c.orientation) == orientation
                    && usize::from(c.row) == placement.row
                    && usize::from(c.col) == placement.col
            })?;
            if !self.fits(piece_type, piece_type.candidates[i]) {
                return None;
            }
            self.apply(t, index16(i), true);
            self.remaining[t] -= 1;
            self.types[t].free.retain(|&f| f != m);
            self.fixed.push((index, placement));
        }
        Some(self)
    }

    /// Whether `candidate` of `piece_type` stands on uncovered cells and
    /// puts no more cells into any line than it still needs.
    fn fits(&self, piece_type: &PieceType, candidate: Candidate) -> bool {
        let layout = &piece_type.layouts[usize::from(candidate.orientation)];
        let (row, col) = (usize::from(candidate.row), usize::from(candidate.col));
        let k = piece_type.color;
        let rows = row..row + layout.height;
        let rows_fit = layout.bits[..layout.height]
            .iter()
            .zip(&layout.row_cells)
            .zip(
                self.occupied[rows.clone()]
                    .iter()
                    .zip(&self.row_room[k][rows]),
            )
            .all(|((&bits, &cells), (&taken, &room))| taken & (bits << col) == 0 && cells <= room);
        rows_fit
            && layout.col_cells[..layout.width]
                .iter()
                .zip(&self.col_room[k][col..col + layout.width])
                .all(|(&cells, &room)| cells <= room)
    }

    /// Narrows the candidate lists of `parent` to those that still fit, as
    /// a new level, and checks that every line can still reach its clue.
    /// Returns the new level, or `None` (adding nothing) if the current
    /// board is a dead end.
    fn filter(&mut self, parent: usize, lists: &mut Lists) -> Option<usize> {
        let level = lists.ranges.len();
        let mut occupied = [0u64; 4];
        for (r, &row) in self.occupied.iter().enumerate() {
            occupied[r / 4] |= u64::from(row) << ((r % 4) * 16);
        }
        let mut cover = core::mem::take(&mut self.cover);
        let mut span = core::mem::take(&mut self.span);
        cover.fill([0; 4]);
        span.fill(0);
        let mut alive = true;
        for (t, piece_type) in self.types.iter().enumerate() {
            let start = lists.items.len();
            if self.remaining[t] > 0 {
                let k = piece_type.color;
                span[k] += self.remaining[t] * piece_type.max_span;
                let row_room = u128::from_le_bytes(self.row_room[k]) | LANE_TOPS;
                let col_room = u128::from_le_bytes(self.col_room[k]) | LANE_TOPS;
                let (from, to) = lists.ranges[parent + t];
                // Lists are ascending: skip the indices below the minimum.
                let min = self.min_index[t];
                let from = from + lists.items[from..to].partition_point(|&i| usize::from(i) < min);
                // Room for every candidate; the list keeps those that fit.
                let mut kept = start;
                lists.items.resize(start + (to - from), 0);
                let mut type_cover = [0u64; 4];
                for p in from..to {
                    let i = lists.items[p];
                    let c = usize::from(i);
                    let mask = &piece_type.masks[c];
                    // Each byte lane holds room + 128 minus cells: its top bit
                    // stays set exactly when the line has room enough.
                    // `&`, not `&&`: no branch per candidate.
                    #[allow(clippy::needless_bitwise_bool)]
                    let fits = !meets(mask, &occupied)
                        & ((row_room - piece_type.row_counts[c]) & LANE_TOPS == LANE_TOPS)
                        & ((col_room - piece_type.col_counts[c]) & LANE_TOPS == LANE_TOPS);
                    lists.items[kept] = i;
                    kept += usize::from(fits);
                    let keep = 0u64.wrapping_sub(u64::from(fits));
                    for (cover, bits) in type_cover.iter_mut().zip(mask) {
                        *cover |= bits & keep;
                    }
                }
                lists.items.truncate(kept);
                for (cover, bits) in cover[k].iter_mut().zip(type_cover) {
                    *cover |= bits;
                }
                if lists.items.len() - start < self.remaining[t] {
                    alive = false;
                }
            }
            lists.push_range(start);
            if !alive {
                break;
            }
        }
        alive = alive && self.lines_reachable(&cover, &span);
        self.cover = cover;
        self.span = span;
        if alive {
            Some(level)
        } else {
            lists.truncate(level);
            None
        }
    }

    /// Whether every line can still get what it needs: no more than the
    /// remaining pieces of its color can put into one line, and no more
    /// than the cells some candidate can still cover.
    fn lines_reachable(&self, cover: &[Bits], span: &[usize]) -> bool {
        for (k, &span) in span.iter().enumerate() {
            let mut cols = [0u8; MAX_GRID_SIZE];
            for r in 0..self.height {
                let need = self.row_room[k][r];
                if need > 0 && (usize::from(need) > span || need > row_count(&cover[k], r)) {
                    return false;
                }
                let mut bits = (cover[k][r / 4] >> ((r % 4) * 16)) & 0xFFFF;
                while bits != 0 {
                    cols[bits.trailing_zeros() as usize] += 1;
                    bits &= bits - 1;
                }
            }
            for (&need, &coverable) in self.col_room[k][..self.width].iter().zip(&cols) {
                if need > 0 && (usize::from(need) > span || need > coverable) {
                    return false;
                }
            }
        }
        true
    }

    fn apply(&mut self, t: usize, i: u16, add: bool) {
        let piece_type = &self.types[t];
        let k = piece_type.color;
        let candidate = piece_type.candidates[usize::from(i)];
        let layout = &piece_type.layouts[usize::from(candidate.orientation)];
        let (row, col) = (usize::from(candidate.row), usize::from(candidate.col));
        for dr in 0..layout.height {
            self.occupied[row + dr] ^= layout.bits[dr] << col;
            if add {
                self.row_room[k][row + dr] -= layout.row_cells[dr];
            } else {
                self.row_room[k][row + dr] += layout.row_cells[dr];
            }
        }
        for dc in 0..layout.width {
            if add {
                self.col_room[k][col + dc] -= layout.col_cells[dc];
            } else {
                self.col_room[k][col + dc] += layout.col_cells[dc];
            }
        }
    }

    /// Puts candidate `i` of type `t` on the board.
    fn place(&mut self, t: usize, i: u16) {
        self.apply(t, i, true);
        self.remaining[t] -= 1;
        self.min_index[t] = usize::from(i) + 1;
        self.chosen[t].push(i);
    }

    /// Takes back [`place`](Board::place) and restores `min_index[t]`.
    fn undo(&mut self, t: usize, i: u16, saved_min: usize) {
        self.chosen[t].pop();
        self.min_index[t] = saved_min;
        self.remaining[t] += 1;
        self.apply(t, i, false);
    }

    fn solution(&self) -> Solution {
        let mut placements = vec![Placement::default(); self.piece_count];
        for (piece_type, chosen) in self.types.iter().zip(&self.chosen) {
            for (&m, &i) in piece_type.free.iter().zip(chosen) {
                let candidate = piece_type.candidates[usize::from(i)];
                placements[piece_type.members[m]] = Placement {
                    rotation: piece_type.member_rotations[m][usize::from(candidate.orientation)],
                    row: usize::from(candidate.row),
                    col: usize::from(candidate.col),
                };
            }
        }
        for &(index, placement) in &self.fixed {
            placements[index] = placement;
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
            let layouts: Vec<Layout> = orientations.iter().map(Layout::of).collect();
            let candidates = enumerate_candidates(&layouts, grid.width(), grid.height(), free);
            let mut masks = Vec::with_capacity(candidates.len());
            let mut row_counts = Vec::with_capacity(candidates.len());
            let mut col_counts = Vec::with_capacity(candidates.len());
            for candidate in &candidates {
                let layout = &layouts[usize::from(candidate.orientation)];
                let (row, col) = (usize::from(candidate.row), usize::from(candidate.col));
                let mut mask = [0; 4];
                let (mut rows, mut cols) = ([0u8; MAX_GRID_SIZE], [0u8; MAX_GRID_SIZE]);
                for (dr, &bits) in layout.bits[..layout.height].iter().enumerate() {
                    mask[(row + dr) / 4] |= u64::from(bits << col) << (((row + dr) % 4) * 16);
                    rows[row + dr] = layout.row_cells[dr];
                }
                cols[col..col + layout.width].copy_from_slice(&layout.col_cells[..layout.width]);
                masks.push(mask);
                row_counts.push(u128::from_le_bytes(rows));
                col_counts.push(u128::from_le_bytes(cols));
            }
            types.push(PieceType {
                color: color_index(piece.color),
                members: Vec::new(),
                free: Vec::new(),
                orientations,
                layouts,
                member_rotations: Vec::new(),
                candidates,
                masks,
                row_counts,
                col_counts,
                max_span: piece.shape.max_line_span(),
            });
            keys.push(key);
            types.len() - 1
        };
        let rotations = types[t]
            .orientations
            .iter()
            .map(|target| {
                own.iter()
                    .find(|(_, s)| s == target)
                    .map_or(Rotation::R0, |(r, _)| *r)
            })
            .collect();
        let next = types[t].members.len();
        types[t].free.push(next);
        types[t].members.push(index);
        types[t].member_rotations.push(rotations);
    }
    types
}

/// Every placement of every orientation on free cells, orientation by
/// orientation, row by row, column by column.
fn enumerate_candidates(
    layouts: &[Layout],
    width: usize,
    height: usize,
    free: &[u16; MAX_GRID_SIZE],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (orientation, layout) in layouts.iter().enumerate() {
        if layout.height > height || layout.width > width {
            continue;
        }
        for row in 0..=height - layout.height {
            for col in 0..=width - layout.width {
                let on_free = layout.bits[..layout.height]
                    .iter()
                    .zip(&free[row..])
                    .all(|(&bits, &free)| (bits << col) & !free == 0);
                if on_free {
                    candidates.push(Candidate {
                        orientation: count8(orientation),
                        row: count8(row),
                        col: count8(col),
                    });
                }
            }
        }
    }
    candidates
}
