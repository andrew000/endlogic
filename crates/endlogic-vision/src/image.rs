//! The screenshot and what can be read from one pixel or a small area.

/// A screenshot: RGBA pixels, row by row, four bytes each.
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    width: usize,
    height: usize,
    rgba: &'a [u8],
}

/// Pixels in `[x0, x1) × [y0, y1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Area {
    /// The square of side `2 * half` around a point.
    pub fn around(x: f32, y: f32, half_w: f32, half_h: f32) -> Area {
        Area {
            x0: x - half_w,
            y0: y - half_h,
            x1: x + half_w,
            y1: y + half_h,
        }
    }
}

/// The average color of an area, with its brightness and colorfulness.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Mean {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Mean {
    /// Brightness, 0 to 255.
    pub fn lum(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }

    /// Colorfulness: the largest channel minus the smallest.
    pub fn chroma(self) -> f32 {
        self.r.max(self.g).max(self.b) - self.r.min(self.g).min(self.b)
    }

    /// Sum of the channel differences.
    pub fn distance(self, other: Mean) -> f32 {
        (self.r - other.r).abs() + (self.g - other.g).abs() + (self.b - other.b).abs()
    }
}

impl<'a> Image<'a> {
    /// Wraps pixels; `None` when `rgba` is not `width * height * 4` bytes.
    #[must_use]
    pub fn new(width: usize, height: usize, rgba: &'a [u8]) -> Option<Image<'a>> {
        let len = width.checked_mul(height)?.checked_mul(4)?;
        (rgba.len() == len).then_some(Image {
            width,
            height,
            rgba,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The red, green and blue of one pixel.
    #[inline]
    pub(crate) fn rgb(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    /// The pixel at a point, `None` outside the image.
    #[inline]
    pub(crate) fn at(&self, x: f32, y: f32) -> Option<[u8; 3]> {
        if x < 0.0 || y < 0.0 {
            return None;
        }
        // Non-negative and checked against the size below.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x, y) = (x as usize, y as usize);
        (x < self.width && y < self.height).then(|| self.rgb(x, y))
    }

    /// One row of RGBA bytes.
    #[inline]
    pub(crate) fn row(&self, y: usize) -> &'a [u8] {
        &self.rgba[y * self.width * 4..(y + 1) * self.width * 4]
    }

    /// Pixel bounds of an area, clamped to the image; empty when outside.
    fn bounds(&self, area: Area) -> (usize, usize, usize, usize) {
        let clamp = |v: f32, max: usize| -> usize {
            if v <= 0.0 {
                0
            } else {
                // Rounded and clamped to the image, so it fits.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let v = (v + 0.5) as usize;
                v.min(max)
            }
        };
        (
            clamp(area.x0, self.width),
            clamp(area.y0, self.height),
            clamp(area.x1, self.width),
            clamp(area.y1, self.height),
        )
    }

    /// Average color of an area, reading at most about `samples` pixels per
    /// side; black for an empty area.
    pub(crate) fn mean(&self, area: Area, samples: usize) -> Mean {
        let (x0, y0, x1, y1) = self.bounds(area);
        if x1 <= x0 || y1 <= y0 {
            return Mean::default();
        }
        let sx = ((x1 - x0) / samples.max(1)).max(1);
        let sy = ((y1 - y0) / samples.max(1)).max(1);
        let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
        let mut y = y0;
        while y < y1 {
            let mut x = x0;
            while x < x1 {
                let [pr, pg, pb] = self.rgb(x, y);
                r += u64::from(pr);
                g += u64::from(pg);
                b += u64::from(pb);
                n += 1;
                x += sx;
            }
            y += sy;
        }
        let n = to_f32(n);
        Mean {
            r: to_f32(r) / n,
            g: to_f32(g) / n,
            b: to_f32(b) / n,
        }
    }

    /// Share of pixels in an area for which `test` holds, reading at most
    /// about `samples` pixels per side; 0 for an empty area.
    pub(crate) fn share(&self, area: Area, samples: usize, test: impl Fn([u8; 3]) -> bool) -> f32 {
        let (x0, y0, x1, y1) = self.bounds(area);
        if x1 <= x0 || y1 <= y0 {
            return 0.0;
        }
        let sx = ((x1 - x0) / samples.max(1)).max(1);
        let sy = ((y1 - y0) / samples.max(1)).max(1);
        let (mut hit, mut n) = (0u32, 0u32);
        let mut y = y0;
        while y < y1 {
            let mut x = x0;
            while x < x1 {
                hit += u32::from(test(self.rgb(x, y)));
                n += 1;
                x += sx;
            }
            y += sy;
        }
        to_f32(u64::from(hit)) / to_f32(u64::from(n))
    }
}

/// A count or a size as `f32`; exact for the sizes of screenshots.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn to_f32(v: u64) -> f32 {
    v as f32
}

/// `f32` index into `usize`, rounded to the nearest, never below 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn round(v: f32) -> usize {
    if v <= 0.0 { 0 } else { (v + 0.5) as usize }
}

/// A pixel whose color counts as a game color (a counter, a lock, a piece):
/// clearly colored and not dark.
#[inline]
pub(crate) fn vivid([r, g, b]: [u8; 3]) -> bool {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    chroma >= 48 && max >= 72 && u16::from(chroma) * 3 >= u16::from(max)
}

/// Hue of a color in degrees, 0 to 359.
pub(crate) fn hue(r: f32, g: f32, b: f32) -> f32 {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let c = max - min;
    if c <= 0.0 {
        return 0.0;
    }
    let h = if (max - r).abs() < f32::EPSILON {
        let h = (g - b) / c;
        if h < 0.0 { h + 6.0 } else { h }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / c + 2.0
    } else {
        (r - g) / c + 4.0
    };
    let h = h * 60.0;
    if h >= 360.0 { h - 360.0 } else { h }
}

/// Brightness of a pixel, 0 to 255.
#[inline]
pub(crate) fn lum([r, g, b]: [u8; 3]) -> f32 {
    0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)
}
