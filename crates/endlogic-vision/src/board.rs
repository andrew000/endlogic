//! The grid: where its rows and columns are, how many there are and what
//! each cell holds.
//!
//! Line centers come from the counters. Every row has a counter on its left
//! and every column one above it: ticks, or a grey `∅` when all its clues
//! are 0, so the grid spans the lines with counters, widened by lines with a
//! `∅` at the ends. A free cell is a dark square with a short light stroke
//! in each corner pointing to its middle; these strokes tell cells from a
//! painted area. A lock is filled with its color and has a lighter padlock
//! in the middle; a dead cell is light grey and hatched, without strokes.

use alloc::vec::Vec;

use crate::counters::{Counters, Side};
use crate::image::{Area, Image, hue, lum, to_f32, vivid};

/// Distance from the counters' end to the center of a `∅`, in cells.
const EMPTY_OFFSET: f32 = 0.147;
/// Radius of the `∅` ring, in cells.
const EMPTY_RADIUS: f32 = 0.136;
/// Half the side of the part of a cell that is read, in cells.
const INSIDE: f32 = 0.36;
/// How much lighter than the cell beside them a free cell's corner strokes
/// are at least.
const STROKES: f32 = 8.0;
/// Most lines the grid may have on a side.
#[allow(clippy::cast_possible_wrap)]
const MAX_LINES: isize = endlogic_core::MAX_GRID_SIZE as isize;

/// What a cell looks like.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Look {
    /// A dark free cell, with its brightness.
    Free(f32),
    /// Light grey.
    Dead,
    /// Filled with a color of this hue: a lock with a padlock, else part of
    /// a piece put on the grid.
    Colored { hue: f32, padlock: bool },
}

/// The grid found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Board {
    /// Index (in the counters' lattice) of the first row and column.
    pub row0: isize,
    pub col0: isize,
    /// Number of rows and columns.
    pub height: usize,
    pub width: usize,
    /// Cells, row by row.
    pub cells: Vec<Look>,
    /// Cell pitch in pixels.
    pub pitch: f32,
    /// Lines inside the grid whose counter shows neither ticks nor a `∅`:
    /// (row, index).
    pub unmarked: Vec<(bool, usize)>,
    /// Pixel bounds of the grid.
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Why no grid was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoGrid {
    /// Counters, but no cells where they point.
    Cells,
    /// Too small to read.
    Small,
}

struct Reader<'a> {
    image: &'a Image<'a>,
    counters: &'a Counters,
    pitch: f32,
}

impl Reader<'_> {
    fn x(&self, col: isize) -> f32 {
        self.counters.cols.center(col)
    }

    fn y(&self, row: isize) -> f32 {
        self.counters.rows.center(row)
    }

    /// What the cell at a lattice position looks like.
    fn look(&self, row: isize, col: isize) -> Look {
        let (x, y, p) = (self.x(col), self.y(row), self.pitch);
        let area = Area::around(x, y, INSIDE * p, INSIDE * p);
        let (share, color) = vivid_mean(self.image, area, 24);
        if share >= 0.5 {
            let center = self
                .image
                .mean(Area::around(x, y + 0.02 * p, 0.06 * p, 0.06 * p), 12)
                .lum();
            let around = [(0.27, 0.0), (-0.27, 0.0), (0.0, 0.3)]
                .iter()
                .map(|&(dx, dy)| {
                    self.image
                        .mean(Area::around(x + dx * p, y + dy * p, 0.04 * p, 0.04 * p), 8)
                        .lum()
                })
                .sum::<f32>()
                / 3.0;
            return Look::Colored {
                hue: hue(color.r, color.g, color.b),
                padlock: center - around >= 20.0,
            };
        }
        let mean = self.image.mean(area, 24);
        // Light grey, and its hatching hides any corner strokes.
        if mean.lum() >= 45.0 && mean.chroma() < 25.0 && self.strokes(row, col) < STROKES / 2.0 {
            Look::Dead
        } else {
            Look::Free(mean.lum())
        }
    }

    /// How much lighter the four corner strokes of a free cell are than the
    /// cell beside them.
    fn strokes(&self, row: isize, col: isize) -> f32 {
        let (x, y, p) = (self.x(col), self.y(row), self.pitch);
        let half = 0.473 * p;
        let (mut on, mut off, mut n) = (0.0, 0.0, 0.0);
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            // From the corner towards the center.
            let (cx, cy) = (x + sx * half, y + sy * half);
            let mut t = 0.15;
            while t <= 0.27 {
                let (px, py) = (cx - sx * t * p, cy - sy * t * p);
                let r = 0.012 * p;
                on += self.image.mean(Area::around(px, py, r, r), 4).lum();
                let d = 0.06 * p;
                off += f32::midpoint(
                    self.image
                        .mean(Area::around(px + d, py - sy * sx * d, r, r), 4)
                        .lum(),
                    self.image
                        .mean(Area::around(px - d, py + sy * sx * d, r, r), 4)
                        .lum(),
                );
                n += 1.0;
                t += 0.03;
            }
        }
        (on - off) / n
    }

    /// Whether a `∅` stands at a point.
    fn empty_sign(&self, x: f32, y: f32) -> bool {
        let p = self.pitch;
        let r = EMPTY_RADIUS * p;
        let step = (0.025 * p).max(1.0);
        for dy in [-2.0, -1.0, 0.0, 1.0, 2.0] {
            for dx in [-2.0, -1.0, 0.0, 1.0, 2.0] {
                if ring(self.image, x + dx * step, y + dy * step, r) {
                    return true;
                }
            }
        }
        false
    }

    fn row_sign(&self, row: isize) -> bool {
        let x = self.counters.rows.base - EMPTY_OFFSET * self.pitch;
        self.empty_sign(x, self.y(row))
    }

    fn col_sign(&self, col: isize) -> bool {
        let y = self.counters.cols.base - EMPTY_OFFSET * self.pitch;
        self.empty_sign(self.x(col), y)
    }
}

/// Share of vivid pixels in an area and their average color.
fn vivid_mean(image: &Image, area: Area, samples: usize) -> (f32, crate::image::Mean) {
    let share = image.share(area, samples, vivid);
    let mut sum = [0.0f32; 3];
    let mut n = 0.0f32;
    let (w, h) = (area.x1 - area.x0, area.y1 - area.y0);
    let step = (w.max(h) / to_f32(samples as u64)).max(1.0);
    let mut y = area.y0;
    while y < area.y1 {
        let mut x = area.x0;
        while x < area.x1 {
            if let Some(px) = image.at(x, y)
                && vivid(px)
            {
                for (s, v) in sum.iter_mut().zip(px) {
                    *s += f32::from(v);
                }
                n += 1.0;
            }
            x += step;
        }
        y += step;
    }
    let n = n.max(1.0);
    (
        share,
        crate::image::Mean {
            r: sum[0] / n,
            g: sum[1] / n,
            b: sum[2] / n,
        },
    )
}

/// Whether a light grey ring of radius `r` is centered at a point.
fn ring(image: &Image, x: f32, y: f32, r: f32) -> bool {
    const N: usize = 16;
    // Cosines of 0, 22.5, 45, 67.5 and 90 degrees.
    const COS: [f32; 5] = [
        1.0,
        0.923_88,
        core::f32::consts::FRAC_1_SQRT_2,
        0.382_68,
        0.0,
    ];
    let mut on = [0.0f32; N];
    let mut off = 0.0;
    let mut chroma = 0.0;
    for (i, slot) in on.iter_mut().enumerate() {
        let q = i % 4;
        let (c, s) = match i / 4 {
            0 => (COS[q], COS[4 - q]),
            1 => (-COS[4 - q], COS[q]),
            2 => (-COS[q], -COS[4 - q]),
            _ => (COS[4 - q], -COS[q]),
        };
        let Some(px) = image.at(x + c * r, y + s * r) else {
            return false;
        };
        let Some(out) = image.at(x + c * r * 1.7, y + s * r * 1.7) else {
            return false;
        };
        *slot = lum(px);
        off += lum(out);
        let max = px.iter().max().copied().unwrap_or(0);
        let min = px.iter().min().copied().unwrap_or(0);
        chroma += f32::from(max - min);
    }
    let n = to_f32(N as u64);
    let off = off / n;
    let lit = on.iter().filter(|&&l| l >= off + 15.0).count();
    lit * 4 >= N * 3 && chroma / n < 40.0
}

/// Finds the grid the counters stand by.
#[allow(clippy::too_many_lines)]
pub(crate) fn board(image: &Image, counters: &Counters) -> Result<Board, NoGrid> {
    let pitch = f32::midpoint(counters.rows.pitch, counters.cols.pitch);
    if pitch < 16.0 {
        return Err(NoGrid::Small);
    }
    let reader = Reader {
        image,
        counters,
        pitch,
    };
    let span = |side: &Side| {
        let first = side.lines.first().map_or(0, |l| l.index);
        let last = side.lines.last().map_or(0, |l| l.index);
        (first, last)
    };
    let (mut r0, mut r1) = span(&counters.rows);
    let (mut c0, mut c1) = span(&counters.cols);
    if r1 - r0 >= MAX_LINES || c1 - c0 >= MAX_LINES {
        return Err(NoGrid::Cells);
    }
    // A line next to the grid belongs to it when its counter shows `∅` and
    // its cells look like cells.
    let cell_like = |r: isize, c: isize| match reader.look(r, c) {
        Look::Dead | Look::Colored { padlock: true, .. } => true,
        Look::Free(_) => reader.strokes(r, c) >= STROKES,
        Look::Colored { .. } => false,
    };
    let line_like = |cells: &mut dyn Iterator<Item = bool>| {
        let (mut yes, mut n) = (0, 0);
        for c in cells {
            yes += usize::from(c);
            n += 1;
        }
        n > 0 && yes * 3 >= n * 2
    };
    for _ in 0..2 {
        while r1 - r0 + 1 < MAX_LINES
            && reader.row_sign(r0 - 1)
            && line_like(&mut (c0..=c1).map(|c| cell_like(r0 - 1, c)))
        {
            r0 -= 1;
        }
        while r1 - r0 + 1 < MAX_LINES
            && reader.row_sign(r1 + 1)
            && line_like(&mut (c0..=c1).map(|c| cell_like(r1 + 1, c)))
        {
            r1 += 1;
        }
        while c1 - c0 + 1 < MAX_LINES
            && reader.col_sign(c0 - 1)
            && line_like(&mut (r0..=r1).map(|r| cell_like(r, c0 - 1)))
        {
            c0 -= 1;
        }
        while c1 - c0 + 1 < MAX_LINES
            && reader.col_sign(c1 + 1)
            && line_like(&mut (r0..=r1).map(|r| cell_like(r, c1 + 1)))
        {
            c1 += 1;
        }
    }
    let mut cells = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            cells.push(reader.look(r, c));
        }
    }
    // Free cells have the corner strokes of the game's cells.
    let (mut free, mut marked) = (0, 0);
    let width = c1 - c0 + 1;
    for r in r0..=r1 {
        for c in c0..=c1 {
            let i = usize::try_from((r - r0) * width + c - c0).unwrap_or(0);
            if matches!(cells[i], Look::Free(_)) {
                free += 1;
                marked += usize::from(reader.strokes(r, c) >= STROKES);
            }
        }
    }
    if marked * 2 < free {
        return Err(NoGrid::Cells);
    }
    let mut unmarked = Vec::new();
    let has = |side: &Side, k: isize| side.lines.iter().any(|l| l.index == k);
    for r in r0..=r1 {
        if !has(&counters.rows, r) && !reader.row_sign(r) {
            unmarked.push((true, usize::try_from(r - r0).unwrap_or(0)));
        }
    }
    for c in c0..=c1 {
        if !has(&counters.cols, c) && !reader.col_sign(c) {
            unmarked.push((false, usize::try_from(c - c0).unwrap_or(0)));
        }
    }
    let half = pitch / 2.0;
    Ok(Board {
        row0: r0,
        col0: c0,
        height: usize::try_from(r1 - r0 + 1).unwrap_or(0),
        width: usize::try_from(width).unwrap_or(0),
        cells,
        pitch,
        unmarked,
        left: reader.x(c0) - half,
        right: reader.x(c1) + half,
        top: reader.y(r0) - half,
        bottom: reader.y(r1) + half,
    })
}

/// Whether the grid's corners are framed in color, as endlogic itself draws
/// them; the game marks them in grey.
pub(crate) fn framed(image: &Image, board: &Board) -> bool {
    let p = board.pitch;
    let corners = [
        (board.left, board.top),
        (board.right, board.top),
        (board.left, board.bottom),
        (board.right, board.bottom),
    ];
    corners
        .iter()
        .filter(|&&(x, y)| {
            let dx = if x < board.right { -0.08 } else { 0.08 };
            let dy = if y < board.bottom { -0.08 } else { 0.08 };
            let area = Area::around(x + dx * p, y + dy * p, 0.12 * p, 0.12 * p);
            image.share(area, 24, vivid) >= 0.05
        })
        .count()
        >= 3
}
