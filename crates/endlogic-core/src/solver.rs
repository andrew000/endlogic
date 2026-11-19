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
use core::ops::ControlFlow;

use crate::color::Color;
use crate::grid::{Cell, MAX_GRID_SIZE};
use crate::puzzle::Puzzle;
use crate::shape::{Rotation, Shape};
use crate::solution::{Placement, Solution};

/// Result of counting solutions up to a limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Count {
    /// The search finished; the puzzle has exactly this many solutions.
    Exact(usize),
    /// The search stopped after finding this many solutions (the limit);
    /// there may be more.
    LimitReached(usize),
}

impl Count {
    /// Number of solutions found.
    #[must_use]
    pub fn found(self) -> usize {
        match self {
            Count::Exact(n) | Count::LimitReached(n) => n,
        }
    }

    /// Whether the search ran to completion.
    #[must_use]
    pub fn is_exact(self) -> bool {
        matches!(self, Count::Exact(_))
    }
}

/// Solutions found up to a limit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SolutionList {
    /// Solutions in the solver's deterministic order.
    pub solutions: Vec<Solution>,
    /// `true` if the search stopped at the limit (there may be more
    /// solutions); `false` if `solutions` is the complete set.
    pub limit_reached: bool,
}

/// Finds the first solution in the solver's deterministic order.
#[must_use]
pub fn solve(puzzle: &Puzzle) -> Option<Solution> {
    let mut first = None;
    search(puzzle, &mut |solution| {
        first = Some(solution);
        ControlFlow::Break(())
    });
    first
}

/// Counts solutions, stopping as soon as `limit` are found.
///
/// Returns [`Count::Exact`] if the search finished below the limit and
/// [`Count::LimitReached`] otherwise. A `limit` of 0 returns
/// `LimitReached(0)` without searching.
#[must_use]
pub fn count_solutions(puzzle: &Puzzle, limit: usize) -> Count {
    let mut found = 0;
    let limit_reached = search_limited(puzzle, limit, &mut |_| found += 1);
    if limit_reached {
        Count::LimitReached(found)
    } else {
        Count::Exact(found)
    }
}

/// Collects up to `limit` solutions in the solver's deterministic order.
/// A `limit` of 0 returns an empty list with `limit_reached` set.
#[must_use]
pub fn find_solutions(puzzle: &Puzzle, limit: usize) -> SolutionList {
    let mut solutions = Vec::new();
    let limit_reached = search_limited(puzzle, limit, &mut |s| solutions.push(s));
    SolutionList {
        solutions,
        limit_reached,
    }
}

/// Passes up to `limit` solutions to `on_solution`; returns whether the
/// limit was reached.
fn search_limited(puzzle: &Puzzle, limit: usize, on_solution: &mut dyn FnMut(Solution)) -> bool {
    if limit == 0 {
        return true;
    }
    let mut found = 0;
    search(puzzle, &mut |solution| {
        on_solution(solution);
        found += 1;
        if found >= limit {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    found >= limit
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

struct Search {
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

type Visit<'a> = dyn FnMut(Solution) -> ControlFlow<()> + 'a;

fn search(puzzle: &Puzzle, visit: &mut Visit<'_>) {
    let Some(mut state) = Search::new(puzzle) else {
        return;
    };
    let all: Vec<Vec<usize>> = state
        .types
        .iter()
        .map(|t| (0..t.candidates.len()).collect())
        .collect();
    if let Some(candidates) = state.filter(&all) {
        let _ = state.descend(&candidates, visit);
    }
}

impl Search {
    /// Builds the search state, or returns `None` if a quick check already
    /// shows there is no solution.
    fn new(puzzle: &Puzzle) -> Option<Search> {
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
        Some(Search {
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

    fn descend(&mut self, candidates: &[Vec<usize>], visit: &mut Visit<'_>) -> ControlFlow<()> {
        let next = (0..self.types.len())
            .filter(|&t| self.remaining[t] > 0)
            .min_by_key(|&t| candidates[t].len());
        let Some(t) = next else {
            // Every piece is placed, and `filter` has confirmed every line
            // is at its clue.
            return visit(self.solution());
        };

        let saved_min = self.min_index[t];
        for &i in &candidates[t] {
            self.apply(t, i, true);
            self.remaining[t] -= 1;
            self.min_index[t] = i + 1;
            self.chosen[t].push(i);

            let flow = match self.filter(candidates) {
                Some(child) => self.descend(&child, visit),
                None => ControlFlow::Continue(()),
            };

            self.chosen[t].pop();
            self.min_index[t] = saved_min;
            self.remaining[t] += 1;
            self.apply(t, i, false);
            flow?;
        }
        ControlFlow::Continue(())
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
