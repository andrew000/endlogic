//! Hints: a silhouette of the solution, then one piece at a time.
//!
//! Hints lean on a *reference* solution. It is kept while it agrees with
//! the pieces on the board; otherwise a search with those pieces fixed looks
//! for another one, and if there is none, the pieces on the board are wrong
//! and a solution from scratch serves as the reference. Searches run a few
//! nodes per frame, like the generator.

use alloc::vec::Vec;

use endlogic_core::solver::{Progress, Search};
use endlogic_core::{Color, Solution};

use crate::app::{App, Fly};
use crate::board::Board;
use crate::text::Text;
use crate::theme;

/// The hint state of one puzzle.
#[derive(Debug, Clone, Default)]
pub(crate) struct Hints {
    /// Hints given so far.
    pub used: u32,
    /// For each cell, row-major, the color of the piece that covers it in
    /// the reference solution, if any. Empty until the first hint.
    pub silhouette: Vec<Option<Color>>,
    /// The solution hints lean on.
    pub reference: Option<Solution>,
    /// A search in progress for the hint asked for.
    pub job: Option<Job>,
}

/// A search for a reference solution.
#[derive(Debug, Clone)]
pub(crate) struct Job {
    search: Search,
    /// Searching from scratch, after the pieces on the board turned out to
    /// fit no solution.
    scratch: bool,
    /// Nodes spent on this hint so far.
    spent: u64,
}

/// A place of the solution: the color of its piece and its sorted cells.
type Slot = (Color, Vec<(usize, usize)>);

/// The places of every piece in `solution`.
fn piece_cells(board: &Board, solution: &Solution) -> Vec<Slot> {
    let puzzle = board.puzzle();
    puzzle
        .pieces()
        .iter()
        .zip(&solution.placements)
        .map(|(piece, at)| {
            let cells = piece
                .shape
                .rotated(at.rotation)
                .cells()
                .iter()
                .map(|&(r, c)| (at.row + r, at.col + c))
                .collect();
            (piece.color, cells)
        })
        .collect()
}

/// The place piece `i` holds on the board.
fn slot_of(board: &Board, i: usize) -> Slot {
    (board.piece_color(i), board.cells_of(i))
}

/// The first piece on the board that does not stand where some piece of
/// `solution` of its color stands (identical pieces are interchangeable).
fn misplaced(board: &Board, solution: &Solution) -> Option<usize> {
    let slots = piece_cells(board, solution);
    (0..board.piece_count())
        .find(|&i| board.piece(i).at.is_some() && !slots.contains(&slot_of(board, i)))
}

/// The next piece to put down by `solution`: the first tray piece that
/// can take a place of the solution no piece on the board holds yet, with
/// the turns and top-left cell to put it at.
fn next_move(board: &Board, solution: &Solution) -> Option<(usize, u8, usize, usize)> {
    let slots = piece_cells(board, solution);
    let taken: Vec<Slot> = (0..board.piece_count())
        .map(|i| slot_of(board, i))
        .filter(|(_, cells)| !cells.is_empty())
        .collect();
    let open: Vec<&Slot> = slots.iter().filter(|s| !taken.contains(s)).collect();
    (0..board.piece_count())
        .filter(|&i| board.piece(i).at.is_none())
        .find_map(|i| {
            let color = board.piece_color(i);
            open.iter()
                .filter(|(c, _)| *c == color)
                .find_map(|(_, slot)| {
                    let row = slot.iter().map(|c| c.0).min()?;
                    let col = slot.iter().map(|c| c.1).min()?;
                    (0..4u8).find_map(|turns| {
                        let cells: Vec<(usize, usize)> = board
                            .shape_turned(i, turns)
                            .cells()
                            .iter()
                            .map(|&(r, c)| (row + r, col + c))
                            .collect();
                        (cells == *slot).then_some((i, turns, row, col))
                    })
                })
        })
}

/// What a frame of hint search came to.
enum Outcome {
    Waiting,
    Found(Solution, bool),
    Failed,
}

impl App {
    /// Whether a hint is being computed.
    #[must_use]
    pub fn is_hinting(&self) -> bool {
        self.game.as_ref().is_some_and(|g| g.hints.job.is_some())
    }

    /// Hints used on the current puzzle.
    #[must_use]
    pub fn hints_used(&self) -> u32 {
        self.game.as_ref().map_or(0, |g| g.hints.used)
    }

    /// For each cell, row-major, the color of the hint silhouette on it,
    /// if it covers the cell; empty when there is no silhouette.
    #[must_use]
    pub fn silhouette(&self) -> &[Option<Color>] {
        self.game.as_ref().map_or(&[], |g| &g.hints.silhouette)
    }

    /// Asks for a hint: answers at once when the reference solution still
    /// agrees with the board, otherwise starts a search.
    pub(crate) fn request_hint(&mut self) {
        if !self.can_move() || self.dragged_piece().is_some() {
            return;
        }
        let Some(game) = &mut self.game else {
            return;
        };
        if game.hints.job.is_some() {
            return;
        }
        let kept = game
            .hints
            .reference
            .clone()
            .filter(|reference| misplaced(&game.board, reference).is_none());
        if let Some(reference) = kept {
            self.give_hint(&reference, true);
            return;
        }
        let board = &game.board;
        let search = Search::with_fixed(board.puzzle(), 1, &board.placements());
        game.hints.job = Some(Job {
            search,
            scratch: false,
            spent: 0,
        });
    }

    /// Runs the hint search for one frame.
    pub(crate) fn step_hint(&mut self) {
        let budget = self.budgets;
        let per_frame = self.frame_budget();
        let outcome = {
            let Some(game) = &mut self.game else {
                return;
            };
            let Some(job) = &mut game.hints.job else {
                return;
            };
            let before = job.search.nodes();
            let allowed = per_frame.min(budget.hint.saturating_sub(job.spent));
            let progress = job.search.step(allowed);
            job.spent += job.search.nodes() - before;
            match progress {
                Progress::Done(result) => match result.solutions.into_iter().next() {
                    Some(solution) => Outcome::Found(solution, !job.scratch),
                    None if job.scratch => Outcome::Failed,
                    // The pieces on the board fit no solution.
                    None => {
                        if let Some(reference) = game.hints.reference.clone() {
                            Outcome::Found(reference, false)
                        } else {
                            *job = Job {
                                search: Search::new(game.board.puzzle(), 1),
                                scratch: true,
                                spent: job.spent,
                            };
                            Outcome::Waiting
                        }
                    }
                },
                Progress::Working if job.spent >= budget.hint => Outcome::Failed,
                Progress::Working => Outcome::Waiting,
            }
        };
        match outcome {
            Outcome::Waiting => {}
            Outcome::Found(solution, fits) => {
                if let Some(game) = &mut self.game {
                    game.hints.job = None;
                }
                self.give_hint(&solution, fits);
            }
            Outcome::Failed => self.hint_failed(),
        }
    }

    fn hint_failed(&mut self) {
        if let Some(game) = &mut self.game {
            game.hints.job = None;
        }
        self.show_toast(Text::HintFailed, true);
    }

    /// Gives a hint from `reference`. `fits` tells whether the pieces on the
    /// board agree with it.
    fn give_hint(&mut self, reference: &Solution, fits: bool) {
        let Some(game) = &mut self.game else {
            return;
        };
        let first = game.hints.silhouette.is_empty();
        let owners = reference
            .cell_owners(game.board.puzzle())
            .unwrap_or_default();
        let puzzle = game.board.puzzle();
        game.hints.silhouette = owners
            .iter()
            .map(|owner| owner.map(|i| puzzle.pieces()[i].color))
            .collect();
        game.hints.reference = Some(reference.clone());
        if first {
            game.hints.used += 1;
            return;
        }
        if !fits {
            let Some(i) = misplaced(&game.board, reference) else {
                self.hint_failed();
                return;
            };
            game.hints.used += 1;
            if let Some(anim) = game.anims.get_mut(i) {
                anim.wobble = Some(0.0);
                anim.blame = Some(0.0);
            }
            return;
        }
        let Some((i, turns, row, col)) = next_move(&game.board, reference) else {
            return;
        };
        game.hints.used += 1;
        let from = self.piece_visual(i);
        let Some(game) = &mut self.game else {
            return;
        };
        game.board.set_turns(i, turns);
        let (row, col) = (
            isize::try_from(row).unwrap_or(0),
            isize::try_from(col).unwrap_or(0),
        );
        game.board.place(i, row, col);
        if let Some(anim) = game.anims.get_mut(i) {
            anim.fly = Some(Fly {
                center: from.0,
                cell: from.1,
                elapsed: 0.0,
                duration: theme::HINT_FLY_TIME,
            });
        }
        self.check_win();
    }
}
