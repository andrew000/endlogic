//! The tray: cards in two columns right of the grid, one piece in each.
//!
//! Each card scales its piece on its own: the longer side of every piece is
//! drawn about the same length, so a piece's cell size is not known in
//! advance. The silhouette fixes the smallest grid that fits it; a finer
//! grid fits too (each cell split in four), and the shading of the cells,
//! which differs from cell to cell, tells which one is drawn. The choices
//! left are settled later by the clue sums.

use alloc::vec::Vec;

use endlogic_core::Shape;

use crate::blobs::Blob;
use crate::counters::hue_gap;
use crate::image::{Area, Image, hue, to_f32, vivid};

/// Most cells along a piece's longer side.
const MAX_SIDE: usize = 6;
/// Distance between neighbouring cards, in grid cells.
const CARD_PITCH: f32 = 1.72;

/// One way to read a piece: a shape and how well the picture supports it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reading {
    pub shape: Shape,
    /// Share of the cell borders inside the piece that show a seam.
    pub seams: f32,
}

/// A piece in the tray.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrayPiece {
    /// Hue of its fill.
    pub hue: f32,
    /// Ways to read it, the most likely first.
    pub readings: Vec<Reading>,
    /// Pixel bounds of the piece.
    pub bounds: crate::Bounds,
}

/// What the tray shows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tray {
    /// Pieces in card order.
    Pieces(Vec<TrayPiece>),
    /// No pieces.
    Empty,
    /// A card without a piece: some piece is on the grid.
    EmptyCard,
}

/// Bounds of a group of blobs.
#[derive(Debug, Clone, Copy)]
struct Group {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    sum: [f32; 3],
    area: f32,
}

impl Group {
    fn of(b: &Blob) -> Group {
        let a = to_f32(b.area as u64);
        Group {
            x0: to_f32(b.x0 as u64),
            y0: to_f32(b.y0 as u64),
            x1: to_f32(b.x1 as u64),
            y1: to_f32(b.y1 as u64),
            sum: [b.r * a, b.g * a, b.b * a],
            area: a,
        }
    }

    fn near(&self, o: &Group, d: f32) -> bool {
        self.x0 - d <= o.x1 && o.x0 - d <= self.x1 && self.y0 - d <= o.y1 && o.y0 - d <= self.y1
    }

    fn join(&mut self, o: &Group) {
        self.x0 = self.x0.min(o.x0);
        self.y0 = self.y0.min(o.y0);
        self.x1 = self.x1.max(o.x1);
        self.y1 = self.y1.max(o.y1);
        for (s, v) in self.sum.iter_mut().zip(o.sum) {
            *s += v;
        }
        self.area += o.area;
    }

    fn hue(&self) -> f32 {
        hue(self.sum[0], self.sum[1], self.sum[2])
    }
}

/// Clusters of nearby values, each with its mean, in order.
fn clusters(values: &mut [f32], gap: f32) -> Vec<f32> {
    crate::sort::sort(values);
    let mut out: Vec<(f32, f32)> = Vec::new();
    for &v in values.iter() {
        match out.last_mut() {
            Some((sum, n)) if v - *sum / *n < gap => {
                *sum += v;
                *n += 1.0;
            }
            _ => out.push((v, 1.0)),
        }
    }
    out.into_iter().map(|(s, n)| s / n).collect()
}

fn nearest(values: &[f32], v: f32) -> usize {
    let mut best = 0;
    for (i, &c) in values.iter().enumerate() {
        if (c - v).abs() < (values[best] - v).abs() {
            best = i;
        }
    }
    best
}

/// Reads the tray right of `left` (the grid's right edge).
pub(crate) fn tray(image: &Image, blobs: &[Blob], left: f32, pitch: f32, tick: f32) -> Tray {
    let is_tick = |b: &Blob| {
        let (long, short) = (b.w().max(b.h()), b.w().min(b.h()));
        (long / tick - 1.0).abs() <= 0.3 && short <= 0.5 * long
    };
    let mut groups: Vec<Group> = Vec::new();
    for b in blobs {
        if to_f32(b.x0 as u64) < left + 0.4 * pitch || is_tick(b) {
            continue;
        }
        let mut g = Group::of(b);
        // Merge with every group it touches, transitively.
        while let Some(i) = groups.iter().position(|o| o.near(&g, 0.08 * pitch)) {
            let o = groups.remove(i);
            g.join(&o);
        }
        groups.push(g);
    }
    let pieces: Vec<Group> = groups
        .into_iter()
        .filter(|g| {
            let (w, h) = (g.x1 - g.x0, g.y1 - g.y0);
            (0.4 * pitch..=1.05 * pitch).contains(&w.max(h)) && w.min(h) >= 0.1 * pitch
        })
        .collect();
    if pieces.is_empty() {
        return Tray::Empty;
    }
    let card = CARD_PITCH * pitch;
    let center = |g: &Group| (f32::midpoint(g.x0, g.x1), f32::midpoint(g.y0, g.y1));
    let mut xs: Vec<f32> = pieces.iter().map(|g| center(g).0).collect();
    let mut ys: Vec<f32> = pieces.iter().map(|g| center(g).1).collect();
    let cols = clusters(&mut xs, 0.5 * card);
    let rows = clusters(&mut ys, 0.5 * card);
    let mut placed: Vec<(usize, Group)> = pieces
        .iter()
        .map(|g| {
            let (x, y) = center(g);
            (nearest(&rows, y) * 2 + nearest(&cols, x), *g)
        })
        .collect();
    crate::sort::sort_by_key(&mut placed, |(slot, _)| to_f32(*slot as u64));
    if empty_card(image, &cols, &rows, &placed, pitch) {
        return Tray::EmptyCard;
    }
    Tray::Pieces(
        placed
            .iter()
            .map(|(_, g)| {
                let hue = g.hue();
                TrayPiece {
                    hue,
                    readings: readings(image, g, hue),
                    bounds: crate::Bounds {
                        x0: g.x0,
                        y0: g.y0,
                        x1: g.x1,
                        y1: g.y1,
                    },
                }
            })
            .collect(),
    )
}

/// Whether a card without a piece stands among the cards.
fn empty_card(
    image: &Image,
    cols: &[f32],
    rows: &[f32],
    placed: &[(usize, Group)],
    pitch: f32,
) -> bool {
    if cols.len() != 2 {
        return false;
    }
    // A card is darker than the panel between cards.
    let panel = image
        .mean(
            Area::around(
                f32::midpoint(cols[0], cols[1]),
                rows[0],
                0.03 * pitch,
                0.3 * pitch,
            ),
            12,
        )
        .lum();
    let card_at = |x: f32, y: f32| {
        let sides = [(0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6)];
        let l = sides
            .iter()
            .map(|&(dx, dy)| {
                image
                    .mean(
                        Area::around(x + dx * pitch, y + dy * pitch, 0.05 * pitch, 0.05 * pitch),
                        6,
                    )
                    .lum()
            })
            .sum::<f32>()
            / 4.0;
        l < panel - 6.0
    };
    // The test must hold where pieces are.
    let (x, y) = (cols[0], rows[0]);
    if !card_at(x, y) {
        return false;
    }
    let last = placed.last().map_or(0, |(s, _)| *s);
    let step = (rows.len() > 1)
        .then(|| (rows[rows.len() - 1] - rows[0]) / to_f32((rows.len() - 1) as u64));
    let step = step.unwrap_or(CARD_PITCH * pitch);
    (0..=last + 1).any(|slot| {
        if placed.iter().any(|(s, _)| *s == slot) {
            return false;
        }
        let y = rows[0] + step * to_f32((slot / 2) as u64);
        card_at(cols[slot % 2], y)
    })
}

/// The ways to read a piece's shape, the most likely first.
fn readings(image: &Image, g: &Group, piece_hue: f32) -> Vec<Reading> {
    let (w, h) = (g.x1 - g.x0, g.y1 - g.y0);
    let long = w.max(h);
    let colored = |px: [u8; 3]| {
        vivid(px)
            && hue_gap(
                hue(f32::from(px[0]), f32::from(px[1]), f32::from(px[2])),
                piece_hue,
            ) <= 30.0
    };
    let mut out: Vec<Reading> = Vec::new();
    for n in 1..=MAX_SIDE {
        let c = long / to_f32(n as u64);
        let cols = crate::image::round(w / c).max(1);
        let rows = crate::image::round(h / c).max(1);
        if cols.max(rows) != n
            || (w - to_f32(cols as u64) * c).abs() > 0.3 * c
            || (h - to_f32(rows as u64) * c).abs() > 0.3 * c
        {
            continue;
        }
        let (cw, ch) = (w / to_f32(cols as u64), h / to_f32(rows as u64));
        let cell_center = |r: usize, k: usize| {
            (
                g.x0 + (to_f32(k as u64) + 0.5) * cw,
                g.y0 + (to_f32(r as u64) + 0.5) * ch,
            )
        };
        // A cell is filled when all four of its quarters are colored and
        // empty when none is; anything else does not fit this grid.
        let mut cells = Vec::new();
        let mut clear = true;
        for r in 0..rows {
            for k in 0..cols {
                let (x, y) = cell_center(r, k);
                let lit = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                    .iter()
                    .filter(|&&(sx, sy)| {
                        let area = Area::around(
                            x + sx * 0.2 * cw,
                            y + sy * 0.2 * ch,
                            0.08 * cw,
                            0.08 * ch,
                        );
                        image.share(area, 4, colored) >= 0.25
                    })
                    .count();
                match lit {
                    4 => cells.push((r, k)),
                    0 => {}
                    _ => clear = false,
                }
            }
        }
        if !clear {
            continue;
        }
        let Ok(shape) = Shape::new(cells.iter().copied()) else {
            continue;
        };
        if shape.width() != cols || shape.height() != rows {
            continue;
        }
        let seams = seams(image, g, &shape, cw, ch);
        out.push(Reading { shape, seams });
    }
    // The finest reading whose borders show seams, then the others from
    // the best supported.
    let pick = out.iter().rposition(|r| r.seams >= 0.6).unwrap_or(0);
    if !out.is_empty() {
        let first = out.remove(pick);
        crate::sort::sort_by_key(&mut out, |r| -r.seams);
        out.insert(0, first);
    }
    out
}

/// Share of the borders between cells of a reading that show a change of
/// shade, against the change across the middle of a cell.
fn seams(image: &Image, g: &Group, shape: &Shape, cw: f32, ch: f32) -> f32 {
    // Change of shade across a line: vertical line at `x` over rows
    // `y0..y1`, or horizontal at `y` over `x0..x1`.
    let across = |vertical: bool, at: f32, from: f32, to: f32| {
        let (near, far) = if vertical {
            (0.06 * cw, 0.22 * cw)
        } else {
            (0.06 * ch, 0.22 * ch)
        };
        let (near, far) = (near.max(1.0), far.max(near.max(1.0) + 1.0));
        let strip = |a: f32, b: f32| {
            let area = if vertical {
                Area {
                    x0: at + a,
                    x1: at + b,
                    y0: from,
                    y1: to,
                }
            } else {
                Area {
                    x0: from,
                    x1: to,
                    y0: at + a,
                    y1: at + b,
                }
            };
            image.mean(area, 12)
        };
        strip(-far, -near).distance(strip(near, far))
    };
    let mut borders = Vec::new();
    let mut middles = Vec::new();
    for &(r, c) in shape.cells() {
        let (x0, y0) = (g.x0 + to_f32(c as u64) * cw, g.y0 + to_f32(r as u64) * ch);
        let (ya, yb) = (y0 + 0.2 * ch, y0 + 0.8 * ch);
        let (xa, xb) = (x0 + 0.2 * cw, x0 + 0.8 * cw);
        middles.push(across(true, x0 + 0.5 * cw, ya, yb));
        middles.push(across(false, y0 + 0.5 * ch, xa, xb));
        if shape.contains(r, c + 1) {
            borders.push(across(true, x0 + cw, ya, yb));
        }
        if shape.contains(r + 1, c) {
            borders.push(across(false, y0 + ch, xa, xb));
        }
    }
    if borders.is_empty() {
        return 1.0;
    }
    crate::sort::sort(&mut middles);
    let calm = middles[middles.len() / 2];
    let seen = borders
        .iter()
        .filter(|&&d| d >= (1.6 * calm).max(calm + 10.0))
        .count();
    to_f32(seen as u64) / to_f32(borders.len() as u64)
}
