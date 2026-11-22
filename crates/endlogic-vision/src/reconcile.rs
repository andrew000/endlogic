//! The clue sums: for each color, the row clues, the column clues and the
//! locks plus the cells of its pieces count the same cells. When they do
//! not, other readings of the pieces and of unclear counters are tried; the
//! cheapest set of changes that makes them agree wins.

use alloc::vec::Vec;

use endlogic_core::{Cell, Color, Grid, LineClues, MAX_COLORS, Shape};

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
/// `target`, never reading `skip.0` as its reading `skip.1`: the reading
/// of each piece (its index, which is also its cost) and the total cost.
fn choose(
    options: &[Vec<usize>],
    target: usize,
    skip: Option<(usize, usize)>,
) -> Option<(Vec<usize>, usize)> {
    // best[s] = (cost, readings) for area s after the pieces so far.
    let mut best: Vec<Option<(usize, Vec<usize>)>> = alloc::vec![None; target + 1];
    best[0] = Some((0, Vec::new()));
    for (i, areas) in options.iter().enumerate() {
        let mut next: Vec<Option<(usize, Vec<usize>)>> = alloc::vec![None; target + 1];
        for (s, slot) in best.iter().enumerate() {
            let Some((cost, picks)) = slot else { continue };
            for (k, &a) in areas.iter().enumerate() {
                let (t, c) = (s + a, cost + k);
                if t <= target
                    && skip != Some((i, k))
                    && next[t].as_ref().is_none_or(|(old, _)| c < *old)
                {
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

/// Settles the sums of every color, changing `clues` where an unclear
/// counter reads better another way.
pub(crate) fn settle(
    grid: &Grid,
    clues: &mut [Option<LineClues>; MAX_COLORS],
    pieces: &[TrayPiece],
    colors: &[Color],
    alternatives: &[(Place, usize)],
) -> Settled {
    let mut picks: Vec<usize> = alloc::vec![0; pieces.len()];
    let mut doubts = Vec::new();
    let mut doubt = |place, reason| doubts.push(Doubt { place, reason });
    for (color, lines) in Color::ALL.into_iter().zip(clues.iter_mut()) {
        let Some(lines) = lines else { continue };
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
        // Every combination of unclear counters read the other way:
        // (cost, combination, readings, piece cells).
        let mut found: Option<(usize, usize, Vec<usize>, usize)> = None;
        for mask in 0..(1usize << unclear.len()) {
            let mut sums = [lines.rows.iter().sum::<usize>(), lines.cols.iter().sum()];
            for (bit, &(place, alt)) in unclear.iter().enumerate() {
                if mask & (1 << bit) != 0
                    && let Place::Clue { row, index, .. } = place
                {
                    let list = if row { &lines.rows } else { &lines.cols };
                    let sum = &mut sums[usize::from(!row)];
                    *sum = *sum - list[index] + alt;
                }
            }
            if sums[0] != sums[1] || sums[0] < locks {
                continue;
            }
            let target = sums[0] - locks;
            if let Some((choice, cost)) = choose(&options, target, None) {
                let cost = cost + mask.count_ones() as usize;
                if found.as_ref().is_none_or(|f| cost < f.0) {
                    found = Some((cost, mask, choice, target));
                }
            }
        }
        if let Some((_, mask, choice, target)) = found {
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
                    doubt(place, Reason::Unclear);
                }
            }
            let best: usize = choice.iter().sum();
            for (k, &i) in mine.iter().enumerate() {
                picks[i] = choice[k];
                // Another reading of this piece that adds up as cheaply
                // leaves it unclear.
                if choose(&options, target, Some((k, choice[k]))).is_some_and(|(_, c)| c <= best) {
                    doubt(Place::Piece(i), Reason::Unclear);
                }
            }
        } else {
            // Nothing adds up: keep the first readings and point at what
            // could be wrong.
            for &(place, _) in &unclear {
                doubt(place, Reason::Sums);
            }
            if unclear.is_empty() {
                for &i in &mine {
                    doubt(Place::Piece(i), Reason::Sums);
                }
                for (row, list) in [(true, &lines.rows), (false, &lines.cols)] {
                    for (index, &v) in list.iter().enumerate() {
                        if v > 0 {
                            doubt(Place::Clue { color, row, index }, Reason::Sums);
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
