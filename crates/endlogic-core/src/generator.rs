//! Puzzle generator.
//!
//! A puzzle is built backwards from a solution, so a solution always exists.
//! One *attempt*:
//!
//! 1. scatters dead cells and lays locks, mostly as straight runs along a row
//!    or column;
//! 2. grows each piece directly on the board from a random free cell by
//!    adding random free neighbours until it has the chosen size;
//! 3. derives the row and column clues from the pieces and locks;
//! 4. hides the solution by rotating every piece randomly and shuffling the
//!    piece order;
//! 5. checks the result with the solver under a node budget. With
//!    [`GeneratorParams::require_unique`] the attempt is accepted only if the
//!    solution is provably unique; otherwise one confirmed solution suffices.
//!
//! Every attempt draws from its own random stream derived from the seed and
//! the attempt number, so the same parameters and seed always give the same
//! puzzle. [`Generator::step`] spends at most a given number of solver
//! nodes per call, which lets an interactive front end spread even one
//! attempt over several frames; the result does not depend on the step size.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::grid::{Cell, Grid, MAX_GRID_SIZE};
use crate::puzzle::{MAX_PIECES, Piece, Puzzle};
use crate::rng::Rng;
use crate::shape::{Rotation, Shape};
use crate::solution::{Placement, Solution};
use crate::solver::{Progress, Search, StopReason};
use crate::{Color, verify};

/// An inclusive range of counts, `min..=max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizeRange {
    /// Smallest allowed value.
    pub min: usize,
    /// Largest allowed value.
    pub max: usize,
}

impl SizeRange {
    /// Creates `min..=max`.
    #[must_use]
    pub const fn new(min: usize, max: usize) -> SizeRange {
        SizeRange { min, max }
    }

    /// Whether `value` lies in the range.
    #[must_use]
    pub const fn contains(self, value: usize) -> bool {
        self.min <= value && value <= self.max
    }
}

/// Everything that controls generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneratorParams {
    /// Grid width, 1 to 16.
    pub width: usize,
    /// Grid height, 1 to 16.
    pub height: usize,
    /// Number of pieces; at most [`MAX_PIECES`].
    pub pieces: SizeRange,
    /// Cells per piece; at least 1.
    pub piece_size: SizeRange,
    /// Number of locked cells.
    pub locks: SizeRange,
    /// Number of dead cells.
    pub dead: SizeRange,
    /// Solver node budget for checking one attempt.
    pub node_budget: u64,
    /// Attempts before giving up; at least 1.
    pub max_attempts: u32,
    /// Accept only puzzles whose solution is provably unique.
    pub require_unique: bool,
}

/// Why a [`GeneratorParams`] value is unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamsError {
    /// Width or height is outside `1..=16`.
    GridSize,
    /// A range has `min > max`; the field is named.
    EmptyRange(&'static str),
    /// More than [`MAX_PIECES`] pieces requested.
    TooManyPieces,
    /// Pieces must have at least one cell.
    EmptyPieces,
    /// `max_attempts` is zero.
    NoAttempts,
}

impl fmt::Display for ParamsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamsError::GridSize => {
                write!(f, "grid size must be between 1 and {MAX_GRID_SIZE}")
            }
            ParamsError::EmptyRange(field) => write!(f, "`{field}` has min > max"),
            ParamsError::TooManyPieces => write!(f, "at most {MAX_PIECES} pieces"),
            ParamsError::EmptyPieces => f.write_str("pieces need at least one cell"),
            ParamsError::NoAttempts => f.write_str("`max_attempts` must be at least 1"),
        }
    }
}

impl core::error::Error for ParamsError {}

impl GeneratorParams {
    /// Checks the parameters for consistency.
    ///
    /// # Errors
    ///
    /// The first problem found, as a [`ParamsError`].
    pub fn validate(&self) -> Result<(), ParamsError> {
        let sizes = 1..=MAX_GRID_SIZE;
        if !sizes.contains(&self.width) || !sizes.contains(&self.height) {
            return Err(ParamsError::GridSize);
        }
        for (name, range) in [
            ("pieces", self.pieces),
            ("piece_size", self.piece_size),
            ("locks", self.locks),
            ("dead", self.dead),
        ] {
            if range.min > range.max {
                return Err(ParamsError::EmptyRange(name));
            }
        }
        if self.pieces.max > MAX_PIECES {
            return Err(ParamsError::TooManyPieces);
        }
        if self.piece_size.min == 0 {
            return Err(ParamsError::EmptyPieces);
        }
        if self.max_attempts == 0 {
            return Err(ParamsError::NoAttempts);
        }
        Ok(())
    }
}

/// Ready-made parameter sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    /// 5×5.
    Easy,
    /// 6×6.
    Normal,
    /// 7×7.
    Hard,
    /// 8×8.
    Expert,
}

impl Difficulty {
    /// All levels, easiest first.
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Expert,
    ];

    /// Lower-case name: `easy`, `normal`, `hard` or `expert`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
            Difficulty::Expert => "expert",
        }
    }

    /// Looks a level up by its [`name`](Difficulty::name).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.name() == name)
    }

    /// The parameters for this level (unique solutions required).
    #[must_use]
    pub const fn params(self) -> GeneratorParams {
        let (size, pieces, piece_size, locks, dead, node_budget, max_attempts) = match self {
            Difficulty::Easy => (5, (3, 4), (3, 4), (0, 2), (0, 0), 5_000, 100),
            Difficulty::Normal => (6, (4, 5), (3, 5), (0, 4), (0, 2), 10_000, 100),
            Difficulty::Hard => (7, (5, 6), (3, 9), (3, 6), (0, 3), 20_000, 100),
            Difficulty::Expert => (8, (6, 8), (3, 9), (3, 8), (0, 5), 200_000, 100),
        };
        GeneratorParams {
            width: size,
            height: size,
            pieces: SizeRange::new(pieces.0, pieces.1),
            piece_size: SizeRange::new(piece_size.0, piece_size.1),
            locks: SizeRange::new(locks.0, locks.1),
            dead: SizeRange::new(dead.0, dead.1),
            node_budget,
            max_attempts,
            require_unique: true,
        }
    }
}

/// An accepted puzzle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Generated {
    /// The puzzle.
    pub puzzle: Puzzle,
    /// The solution it was built from.
    pub solution: Solution,
    /// The seed passed to the generator.
    pub seed: u64,
    /// Number of the accepted attempt, starting at 1.
    pub attempt: u32,
    /// Solver nodes spent checking the accepted attempt.
    pub nodes: u64,
    /// Solver nodes spent over all attempts, the accepted one included.
    pub total_nodes: u64,
}

/// Result of one [`Generator::step`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    /// The attempt was rejected; call `step` again.
    Working,
    /// A puzzle was accepted. Further calls return it again.
    Done(Generated),
    /// `max_attempts` attempts were rejected. Further calls return this again.
    GaveUp {
        /// Attempts made.
        attempts: u32,
        /// Solver nodes spent over all attempts.
        total_nodes: u64,
    },
}

/// A generator that works in small steps: each [`step`](Generator::step)
/// spends at most a given number of solver nodes and never goes past the end
/// of the current attempt.
///
/// The puzzle produced depends only on the parameters and the seed, not on
/// how the work is split into steps.
#[derive(Debug, Clone)]
pub struct Generator {
    params: GeneratorParams,
    seed: u64,
    attempts: u32,
    total_nodes: u64,
    /// The attempt whose solver check is in progress.
    pending: Option<Pending>,
    finished: Option<Step>,
}

/// A built attempt waiting for its solver check.
#[derive(Debug, Clone)]
struct Pending {
    puzzle: Puzzle,
    solution: Solution,
    search: Search,
}

impl Generator {
    /// Creates a generator for the given parameters and seed.
    ///
    /// # Errors
    ///
    /// The parameters' [`ParamsError`], if any.
    pub fn new(params: GeneratorParams, seed: u64) -> Result<Generator, ParamsError> {
        params.validate()?;
        Ok(Generator {
            params,
            seed,
            attempts: 0,
            total_nodes: 0,
            pending: None,
            finished: None,
        })
    }

    /// Attempts made so far.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Solver nodes spent so far.
    #[must_use]
    pub fn total_nodes(&self) -> u64 {
        self.total_nodes
    }

    /// Continues generation, trying at most `max_nodes` solver placements.
    ///
    /// A step starts a new attempt if none is in progress, then advances
    /// that attempt's solver check. It returns [`Step::Working`] when the
    /// node allowance runs out or the attempt is rejected, and stops there:
    /// one step never finishes more than one attempt. Once the generator has
    /// finished, every call returns the same result.
    pub fn step(&mut self, max_nodes: u64) -> Step {
        if let Some(step) = &self.finished {
            return step.clone();
        }
        if self.pending.is_none() {
            self.attempts += 1;
            let mut rng = Rng::for_stream(self.seed, u64::from(self.attempts));
            let Some((puzzle, solution)) = build_attempt(&self.params, &mut rng) else {
                return self.reject();
            };
            let wanted = if self.params.require_unique { 2 } else { 1 };
            self.pending = Some(Pending {
                search: Search::new(&puzzle, wanted),
                puzzle,
                solution,
            });
        }
        let budget = self.params.node_budget;
        let Some(pending) = &mut self.pending else {
            return Step::Working;
        };
        let before = pending.search.nodes();
        let progress = pending.search.step(max_nodes.min(budget - before));
        let nodes = pending.search.nodes();
        self.total_nodes += nodes - before;
        let accepted = match progress {
            Progress::Done(result) if self.params.require_unique => {
                result.stop == StopReason::Exhausted && result.solutions.len() == 1
            }
            Progress::Done(result) => !result.solutions.is_empty(),
            // The attempt's own budget is spent: the answer is unknown.
            Progress::Working if nodes >= budget => false,
            Progress::Working => return Step::Working,
        };
        if !accepted {
            return self.reject();
        }
        let Some(Pending {
            puzzle, solution, ..
        }) = self.pending.take()
        else {
            return Step::Working;
        };
        let step = Step::Done(Generated {
            puzzle,
            solution,
            seed: self.seed,
            attempt: self.attempts,
            nodes,
            total_nodes: self.total_nodes,
        });
        self.finished = Some(step.clone());
        step
    }

    /// Drops the current attempt; gives up if it was the last one.
    fn reject(&mut self) -> Step {
        self.pending = None;
        if self.attempts < self.params.max_attempts {
            return Step::Working;
        }
        let step = Step::GaveUp {
            attempts: self.attempts,
            total_nodes: self.total_nodes,
        };
        self.finished = Some(step.clone());
        step
    }

    /// Runs attempts until one is accepted or `max_attempts` is reached.
    ///
    /// # Errors
    ///
    /// [`GenerateError::GaveUp`] if every attempt was rejected.
    pub fn run(&mut self) -> Result<Generated, GenerateError> {
        loop {
            match self.step(u64::MAX) {
                Step::Working => {}
                Step::Done(generated) => return Ok(generated),
                Step::GaveUp {
                    attempts,
                    total_nodes,
                } => {
                    return Err(GenerateError::GaveUp {
                        attempts,
                        total_nodes,
                    });
                }
            }
        }
    }
}

/// Why [`generate`] produced no puzzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenerateError {
    /// The parameters are invalid.
    Params(ParamsError),
    /// Every attempt was rejected.
    GaveUp {
        /// Attempts made.
        attempts: u32,
        /// Solver nodes spent over all attempts.
        total_nodes: u64,
    },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Params(err) => write!(f, "invalid parameters: {err}"),
            GenerateError::GaveUp {
                attempts,
                total_nodes,
            } => write!(
                f,
                "no puzzle accepted after {attempts} attempts ({total_nodes} solver nodes)"
            ),
        }
    }
}

impl core::error::Error for GenerateError {}

/// Generates a puzzle, running attempts until one is accepted.
///
/// # Errors
///
/// [`GenerateError::Params`] for invalid parameters,
/// [`GenerateError::GaveUp`] if every attempt was rejected.
pub fn generate(params: GeneratorParams, seed: u64) -> Result<Generated, GenerateError> {
    Generator::new(params, seed)
        .map_err(GenerateError::Params)?
        .run()
}

/// Start cells tried per piece before an attempt is abandoned.
const START_TRIES: usize = 8;
/// Lock runs tried before an attempt is abandoned.
const LOCK_TRIES: usize = 64;

/// Builds one attempt: the puzzle and the solution it was built from, or
/// `None` if the random choices led nowhere.
fn build_attempt(params: &GeneratorParams, rng: &mut Rng) -> Option<(Puzzle, Solution)> {
    let (width, height) = (params.width, params.height);
    let area = width * height;
    let dead = rng.usize_inclusive(params.dead.min, params.dead.max);
    let locks = rng.usize_inclusive(params.locks.min, params.locks.max);
    let piece_count = rng.usize_inclusive(params.pieces.min, params.pieces.max);
    if dead + locks > area {
        return None;
    }

    let mut cells = vec![Cell::Free; area];
    let mut order: Vec<usize> = (0..area).collect();
    rng.shuffle(&mut order);
    for &i in &order[..dead] {
        cells[i] = Cell::Dead;
    }
    if !place_locks(&mut cells, width, height, locks, rng) {
        return None;
    }

    let mut owner: Vec<Option<usize>> = vec![None; area];
    let mut grown_pieces: Vec<Vec<usize>> = Vec::with_capacity(piece_count);
    for piece in 0..piece_count {
        let size = rng.usize_inclusive(params.piece_size.min, params.piece_size.max);
        let grown =
            (0..START_TRIES).find_map(|_| grow_piece(&cells, &owner, width, height, size, rng))?;
        for &i in &grown {
            owner[i] = Some(piece);
        }
        grown_pieces.push(grown);
    }

    // Clues from pieces and locks.
    let mut rows = vec![0; height];
    let mut cols = vec![0; width];
    for (i, cell) in cells.iter().enumerate() {
        if matches!(cell, Cell::Lock(_)) || owner[i].is_some() {
            rows[i / width] += 1;
            cols[i % width] += 1;
        }
    }

    // Hide the solution: rotate each piece and shuffle their order.
    let mut hidden: Vec<(Piece, Placement)> = grown_pieces
        .iter()
        .map(|grown| hide_piece(grown, width, rng))
        .collect();
    rng.shuffle(&mut hidden);
    let (pieces, placements): (Vec<Piece>, Vec<Placement>) = hidden.into_iter().unzip();

    let grid = Grid::from_cells(width, height, cells).expect("validated grid size");
    let puzzle = Puzzle::single_color(grid, pieces, rows, cols).expect("clues match the grid");
    let solution = Solution { placements };
    debug_assert_eq!(verify(&puzzle, &solution), Ok(()));

    Some((puzzle, solution))
}

/// Lays `count` locks on free cells as straight runs along rows or columns.
/// Returns `false` if they do not fit.
fn place_locks(
    cells: &mut [Cell],
    width: usize,
    height: usize,
    count: usize,
    rng: &mut Rng,
) -> bool {
    let mut left = count;
    let mut tries = 0;
    while left > 0 {
        tries += 1;
        if tries > LOCK_TRIES {
            return false;
        }
        let horizontal = rng.chance(1, 2);
        let line = if horizontal { width } else { height };
        let len = rng.usize_inclusive(1, left.min(line));
        let (row, col) = if horizontal {
            (rng.index(height), rng.index(width - len + 1))
        } else {
            (rng.index(height - len + 1), rng.index(width))
        };
        let run: Vec<usize> = (0..len)
            .map(|k| {
                if horizontal {
                    row * width + col + k
                } else {
                    (row + k) * width + col
                }
            })
            .collect();
        if run.iter().all(|&i| cells[i] == Cell::Free) {
            for &i in &run {
                cells[i] = Cell::Lock(Color::DEFAULT);
            }
            left -= len;
        }
    }
    true
}

/// Grows a connected piece of `size` cells from a random free cell. Returns
/// its cell indices, or `None` if it runs out of room.
fn grow_piece(
    cells: &[Cell],
    owner: &[Option<usize>],
    width: usize,
    height: usize,
    size: usize,
    rng: &mut Rng,
) -> Option<Vec<usize>> {
    let available = |i: usize| cells[i] == Cell::Free && owner[i].is_none();
    let free: Vec<usize> = (0..cells.len()).filter(|&i| available(i)).collect();
    let mut piece = vec![*rng.choose(&free)?];
    while piece.len() < size {
        let mut frontier: Vec<usize> = Vec::new();
        for &i in &piece {
            let (r, c) = (i / width, i % width);
            if r > 0 {
                frontier.push(i - width);
            }
            if r + 1 < height {
                frontier.push(i + width);
            }
            if c > 0 {
                frontier.push(i - 1);
            }
            if c + 1 < width {
                frontier.push(i + 1);
            }
        }
        frontier.retain(|&i| available(i) && !piece.contains(&i));
        frontier.sort_unstable();
        frontier.dedup();
        piece.push(*rng.choose(&frontier)?);
    }
    Some(piece)
}

/// Rotates a grown piece randomly and returns it with the placement that
/// puts it back.
fn hide_piece(grown: &[usize], width: usize, rng: &mut Rng) -> (Piece, Placement) {
    let coords: Vec<(usize, usize)> = grown.iter().map(|&i| (i / width, i % width)).collect();
    let top = coords.iter().map(|c| c.0).min().unwrap_or(0);
    let left = coords.iter().map(|c| c.1).min().unwrap_or(0);
    let shape = Shape::new(coords).expect("grown pieces are connected and non-empty");
    let turns = rng.index(4);
    let placement = Placement {
        rotation: Rotation::ALL[(4 - turns) % 4],
        row: top,
        col: left,
    };
    (
        Piece::new(Color::DEFAULT, shape.rotated(Rotation::ALL[turns])),
        placement,
    )
}
