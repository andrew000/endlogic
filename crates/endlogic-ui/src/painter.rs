//! The drawing interface a shell implements.

/// A color with straight (not premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red, 0–255.
    pub r: u8,
    /// Green, 0–255.
    pub g: u8,
    /// Blue, 0–255.
    pub b: u8,
    /// Alpha, 0 (transparent) to 255 (opaque).
    pub a: u8,
}

impl Rgba {
    /// An opaque color from `0xRRGGBB`.
    #[must_use]
    // Each channel is masked to its byte by the cast.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn hex(rgb: u32) -> Rgba {
        Rgba {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
            a: 255,
        }
    }

    /// The same color with its alpha multiplied by `factor` (0 to 1).
    #[must_use]
    pub fn alpha(self, factor: f32) -> Rgba {
        let a = (f32::from(self.a) * factor.clamp(0.0, 1.0)).round();
        Rgba {
            // In range: `a` is between 0 and 255.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            a: a as u8,
            ..self
        }
    }

    /// Linear mix towards `other`: `t = 0` is `self`, `t = 1` is `other`.
    #[must_use]
    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let channel = |a: u8, b: u8| {
            let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
            // In range: `v` lies between two `u8` values.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = v.round() as u8;
            v
        };
        Rgba {
            r: channel(self.r, other.r),
            g: channel(self.g, other.g),
            b: channel(self.b, other.b),
            a: channel(self.a, other.a),
        }
    }
}

/// A point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Distance from the left edge.
    pub x: f32,
    /// Distance from the top edge.
    pub y: f32,
}

impl Point {
    /// Creates a point.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// An axis-aligned rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width, never negative.
    pub w: f32,
    /// Height, never negative.
    pub h: f32,
}

impl Rect {
    /// Creates a rectangle; negative sizes are clamped to zero.
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            w: w.max(0.0),
            h: h.max(0.0),
        }
    }

    /// A `w`×`h` rectangle centered on `center`.
    #[must_use]
    pub fn centered(center: Point, w: f32, h: f32) -> Rect {
        Rect::new(center.x - w / 2.0, center.y - h / 2.0, w, h)
    }

    /// Right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// Center point.
    #[must_use]
    pub fn center(self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Whether `p` lies inside (left and top edges included).
    #[must_use]
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// The rectangle shrunk by `d` on every side.
    #[must_use]
    pub fn inset(self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }

    /// Whether the two rectangles share any area (touching edges do not count).
    #[must_use]
    pub fn intersects(self, other: Rect) -> bool {
        // Rectangles that only touch, up to rounding, do not intersect.
        const EPS: f32 = 0.01;
        self.x < other.right() - EPS
            && other.x < self.right() - EPS
            && self.y < other.bottom() - EPS
            && other.y < self.bottom() - EPS
    }

    /// Whether `other` lies entirely inside this rectangle.
    #[must_use]
    pub fn contains_rect(self, other: Rect) -> bool {
        const EPS: f32 = 0.01;
        other.x >= self.x - EPS
            && other.y >= self.y - EPS
            && other.right() <= self.right() + EPS
            && other.bottom() <= self.bottom() + EPS
    }

    /// Linear interpolation between two rectangles.
    #[must_use]
    pub fn lerp(self, other: Rect, t: f32) -> Rect {
        let f = |a: f32, b: f32| a + (b - a) * t;
        Rect::new(
            f(self.x, other.x),
            f(self.y, other.y),
            f(self.w, other.w),
            f(self.h, other.h),
        )
    }
}

/// Which font family to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The system's default sans-serif font.
    Sans,
    /// The system's default monospaced font.
    Mono,
}

/// A font request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font {
    /// Family.
    pub family: FontFamily,
    /// Size in logical pixels.
    pub size: f32,
    /// Bold weight instead of regular.
    pub bold: bool,
}

/// Horizontal text alignment relative to the anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    /// The text starts at the anchor.
    Left,
    /// The text is centered on the anchor.
    Center,
    /// The text ends at the anchor.
    Right,
}

/// Drawing primitives, implemented once per platform.
///
/// Coordinates are logical pixels with the origin in the top-left corner;
/// the shell applies the screen's scale factor. Shapes are antialiased.
/// Every call draws over what is already there, in call order.
pub trait Painter {
    /// Fills a rectangle whose corners are rounded with `radius` (0 for
    /// square corners; never more than half the shorter side).
    fn fill_rect(&mut self, rect: Rect, radius: f32, color: Rgba);

    /// Strokes the outline of a rounded rectangle with a line of `width`
    /// centered on the edge.
    fn stroke_rect(&mut self, rect: Rect, radius: f32, width: f32, color: Rgba);

    /// Draws a straight line with butt ends.
    fn line(&mut self, from: Point, to: Point, width: f32, color: Rgba);

    /// Fills a circle.
    fn fill_circle(&mut self, center: Point, radius: f32, color: Rgba);

    /// Strokes a circle with a line of `width` centered on it.
    fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, color: Rgba);

    /// Fills a closed polygon (non-zero winding).
    fn fill_polygon(&mut self, points: &[Point], color: Rgba);

    /// Draws one line of text. `anchor.y` is the vertical middle of the text;
    /// `align` says where `anchor.x` is.
    fn text(&mut self, text: &str, anchor: Point, font: Font, align: Align, color: Rgba);

    /// Width of `text` in logical pixels when drawn with `font`.
    fn text_width(&mut self, text: &str, font: Font) -> f32;

    /// Restricts drawing to `rect` intersected with the current clip, until
    /// the matching [`pop_clip`](Painter::pop_clip).
    fn push_clip(&mut self, rect: Rect);

    /// Restores the clip that was in effect before the last `push_clip`.
    fn pop_clip(&mut self);
}
