//! The solver against a reference: a plain search over every placement of
//! every piece, written to be obviously right rather than fast, on hundreds
//! of small random puzzles made by this file's own generator.
//!
//! The reference tries, piece by piece in order, every rotation at every
//! position. It only skips a placement that is plainly impossible: one
//! that leaves the grid, covers a cell that is not free or already covered,
//! or puts more cells of a color into a line than the line's clue (counts
//! only grow, so such a branch can never become a solution). Complete
//! assignments are checked with [`verify`].

use std::collections::{BTreeMap, BTreeSet};

use endlogic_core::solver::{Progress, Search, StopReason};
use endlogic_core::{
    Cell, Color, Grid, LineClues, Piece, Placement, Puzzle, Rotation, Shape, Solution, verify,
};

/// Every piece's cells, identical pieces' cell sets sorted among them.
type Canonical = Vec<Vec<(usize, usize)>>;

/// A small generator of test numbers (xorshift64*), independent of the
/// library's.
struct Rand(u64);

impl Rand {
    fn new(seed: u64) -> Rand {
        Rand(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }

    /// True with chance `percent` in 100.
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

fn cells_of(puzzle: &Puzzle, i: usize, at: Placement) -> Vec<(usize, usize)> {
    let mut cells: Vec<(usize, usize)> = puzzle.pieces()[i]
        .shape
        .rotated(at.rotation)
        .cells()
        .iter()
        .map(|&(r, c)| (at.row + r, at.col + c))
        .collect();
    cells.sort_unstable();
    cells
}

fn identical(puzzle: &Puzzle, a: usize, b: usize) -> bool {
    let (pa, pb) = (&puzzle.pieces()[a], &puzzle.pieces()[b]);
    pa.color == pb.color
        && Rotation::ALL
            .iter()
            .any(|&r| pa.shape.rotated(r) == pb.shape)
}

fn canonical(puzzle: &Puzzle, solution: &Solution) -> Canonical {
    let count = puzzle.pieces().len();
    let mut out: Canonical = (0..count)
        .map(|i| cells_of(puzzle, i, solution.placements[i]))
        .collect();
    let mut done = vec![false; count];
    for i in 0..count {
        if done[i] {
            continue;
        }
        let group: Vec<usize> = (i..count).filter(|&j| identical(puzzle, i, j)).collect();
        let mut sets: Vec<_> = group.iter().map(|&j| out[j].clone()).collect();
        sets.sort();
        for (&j, set) in group.iter().zip(sets) {
            out[j] = set;
            done[j] = true;
        }
    }
    out
}

// ---- the reference solver ----

struct Reference<'a> {
    puzzle: &'a Puzzle,
    fixed: &'a [Option<Placement>],
    covered: Vec<bool>,
    rows: BTreeMap<Color, Vec<usize>>,
    cols: BTreeMap<Color, Vec<usize>>,
    placements: Vec<Placement>,
    found: BTreeSet<Canonical>,
}

impl Reference<'_> {
    fn width(&self) -> usize {
        self.puzzle.grid().width()
    }

    /// Whether piece `i` can stand at `at` now.
    fn can_place(&self, i: usize, at: Placement) -> bool {
        let grid = self.puzzle.grid();
        let color = self.puzzle.pieces()[i].color;
        let shape = self.puzzle.pieces()[i].shape.rotated(at.rotation);
        if at.row + shape.height() > grid.height() || at.col + shape.width() > grid.width() {
            return false;
        }
        let mut rows = self.rows[&color].clone();
        let mut cols = self.cols[&color].clone();
        for &(dr, dc) in shape.cells() {
            let (r, c) = (at.row + dr, at.col + dc);
            if grid.get(r, c) != Some(Cell::Free) || self.covered[r * self.width() + c] {
                return false;
            }
            rows[r] += 1;
            cols[c] += 1;
        }
        let over = |counts: &[usize], clue: &dyn Fn(usize) -> usize| {
            counts.iter().enumerate().any(|(k, &n)| n > clue(k))
        };
        !over(&rows, &|r| self.puzzle.row_clue(color, r))
            && !over(&cols, &|c| self.puzzle.col_clue(color, c))
    }

    fn set(&mut self, i: usize, at: Placement, on: bool) {
        let color = self.puzzle.pieces()[i].color;
        let width = self.width();
        for (r, c) in cells_of(self.puzzle, i, at) {
            self.covered[r * width + c] = on;
            let (row, col) = (
                &mut self.rows.get_mut(&color).unwrap()[r],
                &mut self.cols.get_mut(&color).unwrap()[c],
            );
            if on {
                *row += 1;
                *col += 1;
            } else {
                *row -= 1;
                *col -= 1;
            }
        }
    }

    fn place_from(&mut self, i: usize) {
        if i == self.puzzle.pieces().len() {
            let solution = Solution {
                placements: self.placements.clone(),
            };
            if verify(self.puzzle, &solution).is_ok() {
                self.found.insert(canonical(self.puzzle, &solution));
            }
            return;
        }
        let tries: Vec<Placement> = if let Some(at) = self.fixed.get(i).copied().flatten() {
            vec![at]
        } else {
            let grid = self.puzzle.grid();
            let mut all = Vec::new();
            for rotation in Rotation::ALL {
                for row in 0..grid.height() {
                    for col in 0..grid.width() {
                        all.push(Placement { rotation, row, col });
                    }
                }
            }
            all
        };
        for at in tries {
            if self.can_place(i, at) {
                self.set(i, at, true);
                self.placements[i] = at;
                self.place_from(i + 1);
                self.set(i, at, false);
            }
        }
    }
}

/// Every solution of `puzzle` with the pieces in `fixed` where given.
fn reference(puzzle: &Puzzle, fixed: &[Option<Placement>]) -> BTreeSet<Canonical> {
    let grid = puzzle.grid();
    let (w, h) = (grid.width(), grid.height());
    let mut rows = BTreeMap::new();
    let mut cols = BTreeMap::new();
    for color in puzzle.colors() {
        let mut r = vec![0; h];
        let mut c = vec![0; w];
        for (row, count) in r.iter_mut().enumerate() {
            for (col, total) in c.iter_mut().enumerate() {
                if grid.get(row, col) == Some(Cell::Lock(color)) {
                    *count += 1;
                    *total += 1;
                }
            }
        }
        rows.insert(color, r);
        cols.insert(color, c);
    }
    let mut search = Reference {
        puzzle,
        fixed,
        covered: vec![false; w * h],
        rows,
        cols,
        placements: vec![Placement::default(); puzzle.pieces().len()],
        found: BTreeSet::new(),
    };
    search.place_from(0);
    search.found
}

// ---- random puzzles ----

/// What a test puzzle has, for counting the kinds covered.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct Kinds {
    dead: bool,
    locks: bool,
    identical: bool,
    zero_clue: bool,
    fixed: bool,
    colors: bool,
}

const NEIGHBOURS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Grows a piece of up to `size` cells on uncovered free cells from a random
/// start; returns its cells.
fn grow(rng: &mut Rand, open: &[Vec<bool>], size: usize) -> Vec<(usize, usize)> {
    let (h, w) = (open.len(), open[0].len());
    let starts: Vec<(usize, usize)> = (0..h)
        .flat_map(|r| (0..w).map(move |c| (r, c)))
        .filter(|&(r, c)| open[r][c])
        .collect();
    if starts.is_empty() {
        return Vec::new();
    }
    let mut cells = vec![starts[rng.below(starts.len())]];
    for _ in 1..size {
        let mut next = Vec::new();
        for &(r, c) in &cells {
            for (dr, dc) in NEIGHBOURS {
                let (Some(nr), Some(nc)) = (r.checked_add_signed(dr), c.checked_add_signed(dc))
                else {
                    continue;
                };
                if nr < h && nc < w && open[nr][nc] && !cells.contains(&(nr, nc)) {
                    next.push((nr, nc));
                }
            }
        }
        if next.is_empty() {
            break;
        }
        cells.push(next[rng.below(next.len())]);
    }
    cells
}

/// The cells moved to the origin.
fn normalized(cells: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let r0 = cells.iter().map(|c| c.0).min().unwrap();
    let c0 = cells.iter().map(|c| c.1).min().unwrap();
    cells.iter().map(|&(r, c)| (r - r0, c - c0)).collect()
}

/// Every place where `shape` (any rotation) fits on open cells.
fn fits_anywhere(open: &[Vec<bool>], shape: &Shape) -> Vec<Vec<(usize, usize)>> {
    let (h, w) = (open.len(), open[0].len());
    let mut out = Vec::new();
    for (_, s) in shape.orientations() {
        if s.height() > h || s.width() > w {
            continue;
        }
        for row in 0..=h - s.height() {
            for col in 0..=w - s.width() {
                let cells: Vec<_> = s.cells().iter().map(|&(r, c)| (row + r, col + c)).collect();
                if cells.iter().all(|&(r, c)| open[r][c]) {
                    out.push(cells);
                }
            }
        }
    }
    out
}

/// A random puzzle (built from a planted solution, sometimes broken on
/// purpose) and pieces to fix.
#[allow(clippy::too_many_lines)]
fn case(seed: u64) -> (Puzzle, Vec<Option<Placement>>, Kinds) {
    let mut rng = Rand::new(seed);
    let mut kinds = Kinds::default();
    let (w, h) = (2 + rng.below(5), 2 + rng.below(5));
    let two_colors = rng.chance(20);
    kinds.colors = two_colors;
    let mut cells = vec![vec![Cell::Free; w]; h];
    for row in &mut cells {
        for cell in row.iter_mut() {
            if rng.chance(8) {
                *cell = Cell::Dead;
                kinds.dead = true;
            }
        }
    }
    let mut open: Vec<Vec<bool>> = cells
        .iter()
        .map(|row| row.iter().map(|&c| c == Cell::Free).collect())
        .collect();
    // Planted pieces: color, cells.
    let mut planted: Vec<(Color, Vec<(usize, usize)>)> = Vec::new();
    let wanted = 1 + rng.below(4);
    while planted.len() < wanted {
        let color = Color(u8::from(two_colors && rng.chance(50)));
        let copy = !planted.is_empty() && rng.chance(30);
        let placed = if copy {
            let (color, model) = planted[rng.below(planted.len())].clone();
            let shape = Shape::new(normalized(&model)).unwrap();
            let spots = fits_anywhere(&open, &shape);
            if spots.is_empty() {
                None
            } else {
                kinds.identical = true;
                Some((color, spots[rng.below(spots.len())].clone()))
            }
        } else {
            let size = 1 + rng.below(4);
            let grown = grow(&mut rng, &open, size);
            (!grown.is_empty()).then_some((color, grown))
        };
        let Some((color, piece_cells)) = placed else {
            break;
        };
        for &(r, c) in &piece_cells {
            open[r][c] = false;
        }
        planted.push((color, piece_cells));
    }
    if planted.is_empty() {
        // No room at all: one monomino on a board of dead cells, which has
        // no solution.
        planted.push((Color::DEFAULT, vec![(0, 0)]));
        cells[0][0] = Cell::Dead;
    }
    for (r, row) in cells.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            if open[r][c] && rng.chance(10) {
                *cell = Cell::Lock(Color(u8::from(two_colors && rng.chance(50))));
                kinds.locks = true;
            }
        }
    }
    // Clues from the planted pieces and the locks.
    let colors: Vec<Color> = if two_colors {
        vec![Color(0), Color(1)]
    } else {
        vec![Color(0)]
    };
    let mut clues: BTreeMap<Color, LineClues> = colors
        .iter()
        .map(|&k| {
            (
                k,
                LineClues {
                    rows: vec![0; h],
                    cols: vec![0; w],
                },
            )
        })
        .collect();
    for (color, piece_cells) in &planted {
        let clue = clues.get_mut(color).unwrap();
        for &(r, c) in piece_cells {
            clue.rows[r] += 1;
            clue.cols[c] += 1;
        }
    }
    for (r, row) in cells.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Cell::Lock(color) = cell {
                let clue = clues.get_mut(color).unwrap();
                clue.rows[r] += 1;
                clue.cols[c] += 1;
            }
        }
    }
    // Sometimes break it: move one cell of a row clue to another row (the
    // sums still agree), or add a piece.
    if rng.chance(20) {
        let clue = clues.get_mut(&Color(0)).unwrap();
        if let Some(from) = (0..h).find(|&r| clue.rows[r] > 0) {
            let to = (from + 1 + rng.below(h - 1)) % h;
            clue.rows[from] -= 1;
            clue.rows[to] += 1;
        }
    }
    let mut pieces: Vec<Piece> = planted
        .iter()
        .map(|(color, piece_cells)| {
            let shape = Shape::new(normalized(piece_cells)).unwrap();
            Piece::new(*color, shape.rotated(Rotation::ALL[rng.below(4)]))
        })
        .collect();
    if rng.chance(5) {
        pieces.push(Piece::new(Color(0), Shape::new([(0, 0)]).unwrap()));
    }
    kinds.zero_clue = clues
        .values()
        .any(|c| c.rows.contains(&0) || c.cols.contains(&0));
    let grid = Grid::from_cells(w, h, cells.into_iter().flatten().collect()).unwrap();
    let puzzle = Puzzle::new(grid, pieces, clues).unwrap();

    // Fix a piece: where it was planted, or anywhere.
    let mut fixed = vec![None; puzzle.pieces().len()];
    if rng.chance(25) {
        kinds.fixed = true;
        let i = rng.below(planted.len());
        let at = if rng.chance(70) {
            let mut wanted = planted[i].1.clone();
            wanted.sort_unstable();
            let row = wanted.iter().map(|c| c.0).min().unwrap();
            let col = wanted.iter().map(|c| c.1).min().unwrap();
            Rotation::ALL
                .iter()
                .map(|&rotation| Placement { rotation, row, col })
                .find(|&at| cells_of(&puzzle, i, at) == wanted)
                .unwrap()
        } else {
            Placement {
                rotation: Rotation::ALL[rng.below(4)],
                row: rng.below(h),
                col: rng.below(w),
            }
        };
        fixed[i] = Some(at);
    }
    (puzzle, fixed, kinds)
}

#[test]
fn the_solver_finds_what_the_reference_finds() {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let cases = 600;
    for seed in 0..cases {
        let (puzzle, fixed, kinds) = case(seed);
        let want = reference(&puzzle, &fixed);
        let result = match Search::with_fixed(&puzzle, usize::MAX, &fixed).step(u64::MAX) {
            Progress::Done(result) => result,
            Progress::Working => unreachable!("an unlimited step finishes"),
        };
        assert_eq!(result.stop, StopReason::Exhausted, "seed {seed}");
        let got: Vec<Canonical> = result
            .solutions
            .iter()
            .map(|s| {
                assert_eq!(verify(&puzzle, s), Ok(()), "seed {seed}");
                for (at, want) in s.placements.iter().zip(&fixed) {
                    if let Some(want) = want {
                        assert_eq!(at, want, "seed {seed}: a fixed piece moved");
                    }
                }
                canonical(&puzzle, s)
            })
            .collect();
        let set: BTreeSet<Canonical> = got.iter().cloned().collect();
        assert_eq!(set.len(), got.len(), "seed {seed}: a solution found twice");
        assert_eq!(set, want, "seed {seed}: {puzzle:?} fixed {fixed:?}");
        let n = want.len();
        let key = match n {
            0 => "no solution",
            1 => "one solution",
            _ => "several solutions",
        };
        *counts.entry(key).or_default() += 1;
        for (on, name) in [
            (kinds.dead, "dead cells"),
            (kinds.locks, "locks"),
            (kinds.identical, "identical pieces"),
            (kinds.zero_clue, "zero clues"),
            (kinds.fixed, "fixed pieces"),
            (kinds.colors, "two colors"),
        ] {
            if on {
                *counts.entry(name).or_default() += 1;
            }
        }
    }
    println!("{counts:?}");
    for name in [
        "no solution",
        "one solution",
        "several solutions",
        "dead cells",
        "locks",
        "identical pieces",
        "zero clues",
        "fixed pieces",
        "two colors",
    ] {
        assert!(
            counts.get(name).copied().unwrap_or(0) >= 20,
            "{name}: {counts:?}"
        );
    }
}
