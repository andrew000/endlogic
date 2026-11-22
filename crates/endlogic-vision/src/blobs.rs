//! Connected patches of game colors: counter ticks, locks, pieces and
//! whatever else is colorful on the screen.

use alloc::vec::Vec;

use crate::image::{Image, hue, to_f32, vivid};

/// Most runs of colored pixels read before giving up: a game screen has a
/// few thousand, a photo may have millions.
const MAX_RUNS: usize = 600_000;

/// One connected patch of colored pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Blob {
    /// Bounds: `x0..x1`, `y0..y1`.
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
    /// Number of its pixels.
    pub area: usize,
    /// Average color of its pixels.
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Blob {
    pub fn w(&self) -> f32 {
        to_f32((self.x1 - self.x0) as u64)
    }

    pub fn h(&self) -> f32 {
        to_f32((self.y1 - self.y0) as u64)
    }

    pub fn cx(&self) -> f32 {
        to_f32((self.x0 + self.x1) as u64) / 2.0
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        to_f32(self.x1 as u64)
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        to_f32(self.y1 as u64)
    }

    pub fn cy(&self) -> f32 {
        to_f32((self.y0 + self.y1) as u64) / 2.0
    }

    /// Share of the bounding box its pixels cover.
    pub fn fill(&self) -> f32 {
        to_f32(self.area as u64) / (self.w() * self.h())
    }

    pub fn hue(&self) -> f32 {
        hue(self.r, self.g, self.b)
    }

    /// Whether `other` lies inside this blob's bounds.
    pub fn contains(&self, other: &Blob) -> bool {
        other.x0 >= self.x0 && other.x1 <= self.x1 && other.y0 >= self.y0 && other.y1 <= self.y1
    }
}

#[derive(Clone, Copy)]
struct Run {
    x0: u32,
    x1: u32,
    y: u32,
    sum: [u32; 3],
}

fn find(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        let up = parent[parent[i as usize] as usize];
        parent[i as usize] = up;
        i = up;
    }
    i
}

/// Every patch of colored pixels (8-connected), in the order of their first
/// pixel; `None` when the picture is far too colorful to be a game screen.
#[allow(clippy::too_many_lines)]
pub(crate) fn blobs(image: &Image) -> Option<Vec<Blob>> {
    let mut runs: Vec<Run> = Vec::new();
    let mut parent: Vec<u32> = Vec::new();
    let mut prev = 0..0;
    for y in 0..image.height() {
        let row = image.row(y);
        let start = runs.len();
        let mut x = 0;
        let w = image.width();
        while x < w {
            let px = [row[x * 4], row[x * 4 + 1], row[x * 4 + 2]];
            if !vivid(px) {
                x += 1;
                continue;
            }
            let x0 = x;
            let mut sum = [0u32; 3];
            while x < w {
                let px = [row[x * 4], row[x * 4 + 1], row[x * 4 + 2]];
                if !vivid(px) {
                    break;
                }
                for (s, v) in sum.iter_mut().zip(px) {
                    *s += u32::from(v);
                }
                x += 1;
            }
            if runs.len() >= MAX_RUNS {
                return None;
            }
            // Sizes of screenshots fit in u32.
            #[allow(clippy::cast_possible_truncation)]
            let id = runs.len() as u32;
            #[allow(clippy::cast_possible_truncation)]
            runs.push(Run {
                x0: x0 as u32,
                x1: x as u32,
                y: y as u32,
                sum,
            });
            parent.push(id);
            // Join with the runs above that touch this one, corners included;
            // runs above that end before this one starts touch no later run.
            while prev.start < prev.end && (runs[prev.start].x1 as usize) < x0 {
                prev.start += 1;
            }
            for p in prev.clone() {
                let above = runs[p];
                #[allow(clippy::cast_possible_truncation)]
                if above.x0 > x as u32 {
                    break;
                }
                #[allow(clippy::cast_possible_truncation)]
                if above.x1 >= x0 as u32 {
                    #[allow(clippy::cast_possible_truncation)]
                    let a = find(&mut parent, p as u32);
                    let b = find(&mut parent, id);
                    if a != b {
                        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                        parent[hi as usize] = lo;
                    }
                }
            }
        }
        prev = start..runs.len();
    }
    // Gather the runs of each root, in the order of the roots.
    let mut slot = alloc::vec![u32::MAX; runs.len()];
    let mut out: Vec<(Blob, [u64; 3])> = Vec::new();
    for (i, &run) in runs.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let root = find(&mut parent, i as u32) as usize;
        let (x0, x1, y) = (run.x0 as usize, run.x1 as usize, run.y as usize);
        if slot[root] == u32::MAX {
            #[allow(clippy::cast_possible_truncation)]
            {
                slot[root] = out.len() as u32;
            }
            out.push((
                Blob {
                    x0,
                    y0: y,
                    x1,
                    y1: y + 1,
                    area: 0,
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                [0; 3],
            ));
        }
        let (blob, sum) = &mut out[slot[root] as usize];
        blob.x0 = blob.x0.min(x0);
        blob.x1 = blob.x1.max(x1);
        blob.y1 = blob.y1.max(y + 1);
        blob.area += x1 - x0;
        for (s, v) in sum.iter_mut().zip(run.sum) {
            *s += u64::from(v);
        }
    }
    Some(
        out.into_iter()
            .map(|(mut blob, sum)| {
                let n = to_f32(blob.area as u64);
                blob.r = to_f32(sum[0]) / n;
                blob.g = to_f32(sum[1]) / n;
                blob.b = to_f32(sum[2]) / n;
                blob
            })
            .collect(),
    )
}
