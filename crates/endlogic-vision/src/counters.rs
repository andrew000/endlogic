//! The counters: ticks, their stacks next to the grid, and the rows and
//! columns they stand for.
//!
//! A tick is a small outlined (or filled) bar of a game color, about 0.31
//! cell long and a third as thick. Ticks of one color in one line form a
//! stack that grows away from the grid; every stack of the row counters ends
//! at the same distance left of the grid, every stack of the column counters
//! at the same distance above it. A line with several colors has a stack per
//! color: for a row one above another, for a column side by side.

use alloc::vec::Vec;

use crate::blobs::Blob;
use crate::image::{round, to_f32};

/// Tick length over the cell pitch, as the game draws it.
pub(crate) const TICK_PER_CELL: f32 = 0.31;

/// One tick.
#[derive(Debug, Clone, Copy)]
struct Tick {
    blob: Blob,
    /// Vertical bar: a row counter's tick.
    vertical: bool,
}

/// The ticks of one color in one line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Stack {
    /// Hue of its ticks.
    pub hue: f32,
    /// Ticks counted from the stack's length.
    pub count: usize,
    /// Tick blobs found, when not the same as `count`.
    pub blobs: usize,
    /// Filled ticks among them.
    pub filled: usize,
    /// Center across the stack: y for a row counter, x for a column.
    pub center: f32,
    /// The end next to the grid: right edge for a row, bottom for a column.
    pub base: f32,
}

/// One row or column with counters.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    /// Its center: y for a row, x for a column.
    pub center: f32,
    /// Its stacks, one per color.
    pub stacks: Vec<Stack>,
    /// Index in the lattice of its axis.
    pub index: isize,
}

/// The counters along one side of the grid.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Side {
    /// Lines with ticks, in order.
    pub lines: Vec<Line>,
    /// Where every stack ends next to the grid.
    pub base: f32,
    /// Center of the line with index 0.
    pub origin: f32,
    /// Distance between neighbouring line centers.
    pub pitch: f32,
}

impl Side {
    /// Center of the line with index `k`.
    pub fn center(&self, k: isize) -> f32 {
        // Line indices are small.
        #[allow(clippy::cast_precision_loss)]
        let k = k as f32;
        self.origin + self.pitch * k
    }
}

/// Every counter found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Counters {
    /// Tick length in pixels.
    pub long: f32,
    /// Row counters, left of the grid.
    pub rows: Side,
    /// Column counters, above the grid.
    pub cols: Side,
}

/// Difference of two hues in degrees, 0 to 180.
pub(crate) fn hue_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    if d > 180.0 { 360.0 - d } else { d }
}

fn median(values: &mut [f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    crate::sort::sort(values);
    Some(values[values.len() / 2])
}

/// Bars of one size and shape: the ticks, with their length.
fn ticks(blobs: &[Blob]) -> Option<(f32, Vec<Tick>)> {
    let candidates: Vec<(Blob, f32, f32)> = blobs
        .iter()
        .filter_map(|b| {
            let (long, short) = (b.w().max(b.h()), b.w().min(b.h()));
            let ratio = long / short;
            (short >= 2.0 && long >= 7.0 && (2.2..=5.0).contains(&ratio) && b.fill() >= 0.25)
                .then_some((*b, long, short))
        })
        .take(4000)
        .collect();
    let like = |a: (f32, f32), b: (f32, f32), slack: f32| {
        (b.0 / a.0 - 1.0).abs() <= 0.12 * slack && (b.1 / a.1 - 1.0).abs() <= 0.3 * slack
    };
    // The most common size.
    let mut best = (0, 0.0, 0.0);
    for &(_, long, short) in &candidates {
        let n = candidates
            .iter()
            .filter(|c| like((long, short), (c.1, c.2), 1.0))
            .count();
        if n > best.0 {
            best = (n, long, short);
        }
    }
    if best.0 < 2 {
        return None;
    }
    let mut longs: Vec<f32> = candidates
        .iter()
        .filter(|c| like((best.1, best.2), (c.1, c.2), 1.0))
        .map(|c| c.1)
        .collect();
    let mut shorts: Vec<f32> = candidates
        .iter()
        .filter(|c| like((best.1, best.2), (c.1, c.2), 1.0))
        .map(|c| c.2)
        .collect();
    let size = (median(&mut longs)?, median(&mut shorts)?);
    let ticks = candidates
        .iter()
        .filter(|c| like(size, (c.1, c.2), 1.3))
        .map(|c| Tick {
            blob: c.0,
            vertical: c.0.h() > c.0.w(),
        })
        .collect();
    Some((size.0, ticks))
}

/// Chains ticks of one orientation into stacks. For column counters the
/// axes are swapped: "along" is y, "across" is x.
fn stacks(ticks: &[Tick], vertical: bool, long: f32) -> Vec<(Stack, f32)> {
    // Position along the stack and across it.
    let along = |b: &Blob| if vertical { b.cx() } else { b.cy() };
    let across = |b: &Blob| if vertical { b.cy() } else { b.cx() };
    let mut mine: Vec<Blob> = ticks
        .iter()
        .filter(|t| t.vertical == vertical)
        .map(|t| t.blob)
        .collect();
    crate::sort::sort_by_key(&mut mine, |b| along(b));
    // Chains of blobs: (blobs, last along, across, hue).
    let mut chains: Vec<Vec<Blob>> = Vec::new();
    for b in mine {
        let hit = chains.iter_mut().find(|c| {
            let last = c[c.len() - 1];
            let step = along(&b) - along(&last);
            (across(&b) - across(&last)).abs() <= 0.25 * long
                && step > 0.2 * long
                && step <= 1.3 * long
                && hue_gap(b.hue(), last.hue()) <= 30.0
        });
        match hit {
            Some(chain) => chain.push(b),
            None => chains.push(alloc::vec![b]),
        }
    }
    // The usual step between neighbouring ticks.
    let mut steps: Vec<f32> = chains
        .iter()
        .flat_map(|c| c.windows(2).map(|w| along(&w[1]) - along(&w[0])))
        .filter(|&s| s <= 0.7 * long)
        .collect();
    let pitch = median(&mut steps).unwrap_or(0.48 * long);
    chains
        .into_iter()
        .map(|chain| {
            let first = chain[0];
            let last = chain[chain.len() - 1];
            let span = along(&last) - along(&first);
            let count = round(span / pitch) + 1;
            let mut hues: Vec<f32> = chain.iter().map(Blob::hue).collect();
            let hue = median(&mut hues).unwrap_or(0.0);
            let center = chain.iter().map(across).sum::<f32>() / to_f32(chain.len() as u64);
            // The grid end: the right of a row stack, the bottom of a
            // column stack (both last along).
            let base = if vertical {
                to_f32(last.x1 as u64)
            } else {
                to_f32(last.y1 as u64)
            };
            let filled = chain.iter().filter(|b| b.fill() >= 0.8).count();
            (
                Stack {
                    hue,
                    count,
                    blobs: chain.len(),
                    filled,
                    center,
                    base,
                },
                pitch,
            )
        })
        .collect()
}

/// Keeps the stacks that end where most stacks end, next to the grid.
fn at_base(stacks: Vec<Stack>, long: f32) -> (Vec<Stack>, f32) {
    let mut best = (0, 0.0);
    for s in &stacks {
        let n = stacks
            .iter()
            .filter(|o| (o.base - s.base).abs() <= 0.3 * long)
            .count();
        if n > best.0 {
            best = (n, s.base);
        }
    }
    let kept: Vec<Stack> = stacks
        .into_iter()
        .filter(|s| (s.base - best.1).abs() <= 0.3 * long)
        .collect();
    let base = kept.iter().map(|s| s.base).fold(best.1, f32::max);
    (kept, base)
}

/// Groups stacks into lines and fits evenly spaced line centers.
fn side(mut stacks: Vec<Stack>, base: f32, pitch_guess: f32) -> Option<Side> {
    crate::sort::sort_by_key(&mut stacks, |s| s.center);
    let mut lines: Vec<Line> = Vec::new();
    for s in stacks {
        let join = lines.last_mut().filter(|line| {
            s.center - line.stacks[0].center < 0.5 * pitch_guess
                && line.stacks.iter().all(|o| hue_gap(o.hue, s.hue) > 30.0)
        });
        match join {
            Some(line) => line.stacks.push(s),
            None => lines.push(Line {
                center: 0.0,
                stacks: alloc::vec![s],
                index: 0,
            }),
        }
    }
    if lines.is_empty() {
        return None;
    }
    for line in &mut lines {
        let n = line.stacks.len();
        line.center = f32::midpoint(line.stacks[0].center, line.stacks[n - 1].center);
    }
    // Pitch from neighbouring lines, each gap a whole number of pitches.
    let (mut span, mut steps) = (0.0, 0usize);
    for w in lines.windows(2) {
        let d = w[1].center - w[0].center;
        let k = round(d / pitch_guess).max(1);
        span += d;
        steps += k;
    }
    let mut pitch = if steps > 0 {
        span / to_f32(steps as u64)
    } else {
        pitch_guess
    };
    let first = lines[0].center;
    for line in &mut lines {
        // Indices from the first line; at most a few dozen.
        #[allow(clippy::cast_possible_wrap)]
        let k = round((line.center - first) / pitch) as isize;
        line.index = k;
    }
    // Least squares on (index, center).
    let n = to_f32(lines.len() as u64);
    #[allow(clippy::cast_precision_loss)]
    let (sk, sc, skk, skc) = lines.iter().fold((0.0, 0.0, 0.0, 0.0), |acc, l| {
        let k = l.index as f32;
        (
            acc.0 + k,
            acc.1 + l.center,
            acc.2 + k * k,
            acc.3 + k * l.center,
        )
    });
    let den = n * skk - sk * sk;
    let origin = if den.abs() > 1e-3 {
        pitch = (n * skc - sk * sc) / den;
        (sc - pitch * sk) / n
    } else {
        first
    };
    Some(Side {
        lines,
        base,
        origin,
        pitch,
    })
}

/// Finds the counters among the colored blobs.
pub(crate) fn counters(blobs: &[Blob]) -> Option<Counters> {
    let (long, ticks) = ticks(blobs)?;
    let pitch_guess = long / TICK_PER_CELL;
    let mut sides = [true, false].map(|vertical| {
        let found: Vec<Stack> = stacks(&ticks, vertical, long)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        let (kept, base) = at_base(found, long);
        side(kept, base, pitch_guess)
    });
    let cols = sides[1].take()?;
    let rows = sides[0].take()?;
    let mut counters = Counters { long, rows, cols };
    // A side with one line has no pitch of its own: take the other's.
    let (rn, cn) = (counters.rows.lines.len(), counters.cols.lines.len());
    if rn < 2 && cn >= 2 {
        counters.rows.pitch = counters.cols.pitch;
    } else if cn < 2 && rn >= 2 {
        counters.cols.pitch = counters.rows.pitch;
    } else if rn < 2 && cn < 2 {
        counters.rows.pitch = pitch_guess;
        counters.cols.pitch = pitch_guess;
    }
    Some(counters)
}
