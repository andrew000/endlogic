//! The clue sums: for each color, the row clues, the column clues and the
//! locks plus the cells of its pieces count the same cells. When they do
//! not, other readings of the pieces and of unclear counters are tried; the
//! cheapest set of changes that makes them agree wins.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use endlogic_core::{Cell, Color, Grid, LineClues, Shape};

use crate::tray::TrayPiece;
use crate::{Doubt, Place, Reason};

/// The shapes chosen and the places left doubtful.
pub(crate) struct Settled {
    pub shapes: Vec<Shape>,
    pub doubts: Vec<Doubt>,
}

/// Most unclear counters of one color tried in every combination.
const MAX_TRIED: usize = 10;

/// The cheapest choice of readings, one per piece, whose areas add up to
/// `target`: the index of the reading for each piece, and the cost.
fn choose(options: &[Vec<usize>], target: usize) -> Option<(Vec<usize>, usize)> {
    // best[s] = (cost, choices) for area s after the pieces so far.
    let mut best: Vec<Option<(usize, Vec<usize>)>> = alloc::vec![None; target + 1];
    best[0] = Some((0, Vec::new()));
    for areas in options {
        let mut next: Vec<Option<(usize, Vec<usize>)>> = alloc::vec![None; target + 1];
        for (s, slot) in best.iter().enumerate() {
            let Some((cost, picks)) = slot else { continue };
            for (k, &a) in areas.iter().enumerate() {
                let t = s + a;
                if t > target {
                    continue;
                }
                let c = cost + k;
                if next[t].as_ref().is_none_or(|(old, _)| c < *old) {
                    let mut p = picks.clone();
                    p.push(k);
                    next[t] = Some((c, p));
                }
            }
        }
        best = next;
    }
    best[target].take().map(|(c, p)| (p, c))
}

/// The sum of the row clues with the unclear counters in `mask` read the
/// other way.
fn rows_total(lines: &LineClues, unclear: &[(Place, usize)], mask: usize) -> usize {
    let mut sum: usize = lines.rows.iter().sum();
    for (bit, &(place, alt)) in unclear.iter().enumerate() {
        if mask & (1 << bit) != 0
            && let Place::Clue {
                row: true, index, ..
            } = place
        {
            sum = sum - lines.rows[index] + alt;
        }
    }
    sum
}

/// Settles the sums of every color, changing `clues` where an unclear
/// counter reads better another way.
#[allow(clippy::too_many_lines)]
pub(crate) fn settle(
    grid: &Grid,
    clues: &mut BTreeMap<Color, LineClues>,
    pieces: &[TrayPiece],
    colors: &[Color],
    alternatives: &[(Place, usize)],
) -> Settled {
    let mut picks: Vec<usize> = alloc::vec![0; pieces.len()];
    let mut doubts = Vec::new();
    for (&color, lines) in clues.iter_mut() {
        let locks = grid
            .cells()
            .iter()
            .filter(|&&c| c == Cell::Lock(color))
            .count();
        let mine: Vec<usize> = (0..pieces.len()).filter(|&i| colors[i] == color).collect();
        let options: Vec<Vec<usize>> = mine
            .iter()
            .map(|&i| pieces[i].readings.iter().map(|r| r.shape.area()).collect())
            .collect();
        let unclear: Vec<(Place, usize)> = alternatives
            .iter()
            .filter(|(p, _)| matches!(p, Place::Clue { color: c, .. } if *c == color))
            .take(MAX_TRIED)
            .copied()
            .collect();
        // Every combination of unclear counters: (cost, changed, choice).
        let mut found: Option<(usize, usize, Vec<usize>)> = None;
        for mask in 0..(1usize << unclear.len()) {
            let (mut rows, mut cols) = (
                lines.rows.iter().sum::<usize>(),
                lines.cols.iter().sum::<usize>(),
            );
            for (bit, &(place, alt)) in unclear.iter().enumerate() {
                if mask & (1 << bit) != 0
                    && let Place::Clue { row, index, .. } = place
                {
                    let list = if row { &lines.rows } else { &lines.cols };
                    let sum = if row { &mut rows } else { &mut cols };
                    *sum = *sum - list[index] + alt;
                }
            }
            if rows != cols || rows < locks {
                continue;
            }
            if let Some((choice, cost)) = choose(&options, rows - locks) {
                let cost = cost + mask.count_ones() as usize;
                if found.as_ref().is_none_or(|(c, _, _)| cost < *c) {
                    found = Some((cost, mask, choice));
                }
            }
        }
        if let Some((_, mask, choice)) = found {
            for (bit, &(place, alt)) in unclear.iter().enumerate() {
                if let Place::Clue { row, index, .. } = place {
                    if mask & (1 << bit) != 0 {
                        let list = if row {
                            &mut lines.rows
                        } else {
                            &mut lines.cols
                        };
                        list[index] = alt;
                    }
                    doubts.push(Doubt {
                        place,
                        reason: Reason::Unclear,
                    });
                }
            }
            let target = rows_total(lines, &unclear, mask) - locks;
            let best: usize = choice.iter().sum();
            for (k, &i) in mine.iter().enumerate() {
                picks[i] = choice[k];
                // Another reading of this piece that adds up as cheaply
                // leaves it unclear.
                let other = (0..options[k].len()).filter(|&r| r != choice[k]).any(|r| {
                    let mut forced = options.clone();
                    forced[k] = alloc::vec![options[k][r]];
                    choose(&forced, target).is_some_and(|(_, cost)| cost + r <= best)
                });
                if other {
                    doubts.push(Doubt {
                        place: Place::Piece(i),
                        reason: Reason::Unclear,
                    });
                }
            }
        } else {
            // Nothing adds up: keep the first readings and point at
            // what could be wrong.
            for &(place, _) in &unclear {
                doubts.push(Doubt {
                    place,
                    reason: Reason::Sums,
                });
            }
            if unclear.is_empty() {
                for &i in &mine {
                    doubts.push(Doubt {
                        place: Place::Piece(i),
                        reason: Reason::Sums,
                    });
                }
                for (row, list) in [(true, &lines.rows), (false, &lines.cols)] {
                    for (index, &v) in list.iter().enumerate() {
                        if v > 0 {
                            doubts.push(Doubt {
                                place: Place::Clue { color, row, index },
                                reason: Reason::Sums,
                            });
                        }
                    }
                }
            }
        }
    }
    let shapes = pieces
        .iter()
        .zip(&picks)
        .map(|(p, &k)| p.readings[k].shape.clone())
        .collect();
    Settled { shapes, doubts }
}
