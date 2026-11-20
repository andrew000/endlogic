//! [`Painter`] on a Direct2D render target.

use endlogic_ui::{Painter, Point, Rect, Rgba};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_SQUARE,
    D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE, D2D1_LINE_JOIN_MITER, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, ID2D1Factory, ID2D1PathGeometry, ID2D1RenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::core::Result;

/// The stroke style of [`Painter::polyline`]: mitered joins up to 10 line
/// widths, square ends. It belongs to the factory, so it outlives lost
/// render targets.
///
/// # Errors
///
/// When Direct2D cannot make it.
pub fn polyline_style(factory: &ID2D1Factory) -> Result<ID2D1StrokeStyle> {
    let props = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: D2D1_CAP_STYLE_SQUARE,
        endCap: D2D1_CAP_STYLE_SQUARE,
        dashCap: D2D1_CAP_STYLE_FLAT,
        lineJoin: D2D1_LINE_JOIN_MITER,
        miterLimit: 10.0,
        dashStyle: D2D1_DASH_STYLE_SOLID,
        dashOffset: 0.0,
    };
    // SAFETY: `props` lives through the call; no dashes.
    unsafe { factory.CreateStrokeStyle(&raw const props, None) }
}

fn color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
}

/// An ellipse of no size at `p`. Its `point` field is the Direct2D point
/// type, which lives in a crate this one does not name.
fn at(p: Point) -> D2D1_ELLIPSE {
    let mut e = D2D1_ELLIPSE::default();
    e.point.X = p.x;
    e.point.Y = p.y;
    e
}

fn rect_f(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x,
        top: r.y,
        right: r.right(),
        bottom: r.bottom(),
    }
}

fn rounded(r: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    let radius = radius.clamp(0.0, r.w.min(r.h) / 2.0);
    D2D1_ROUNDED_RECT {
        rect: rect_f(r),
        radiusX: radius,
        radiusY: radius,
    }
}

/// Draws on one render target between its `BeginDraw` and `EndDraw`.
pub struct D2dPainter<'a> {
    target: &'a ID2D1RenderTarget,
    factory: &'a ID2D1Factory,
    brush: &'a ID2D1SolidColorBrush,
    polyline: &'a ID2D1StrokeStyle,
}

impl<'a> D2dPainter<'a> {
    /// A painter for `target`, whose solid brush is `brush`; `polyline`
    /// comes from [`polyline_style`].
    pub fn new(
        target: &'a ID2D1RenderTarget,
        factory: &'a ID2D1Factory,
        brush: &'a ID2D1SolidColorBrush,
        polyline: &'a ID2D1StrokeStyle,
    ) -> D2dPainter<'a> {
        D2dPainter {
            target,
            factory,
            brush,
            polyline,
        }
    }

    fn brush(&self, c: Rgba) -> &ID2D1SolidColorBrush {
        // SAFETY: a plain setter on a live brush.
        unsafe { self.brush.SetColor(&color(c)) };
        self.brush
    }

    fn ellipse(center: Point, radius: f32) -> D2D1_ELLIPSE {
        let radius = radius.max(0.0);
        D2D1_ELLIPSE {
            radiusX: radius,
            radiusY: radius,
            ..at(center)
        }
    }

    /// A path through `points`, closed and filled or open and hollow.
    fn path(&self, points: &[Point], closed: bool) -> Result<ID2D1PathGeometry> {
        // SAFETY: plain factory call.
        let geometry = unsafe { self.factory.CreatePathGeometry()? };
        // SAFETY: the geometry is new and not yet opened.
        let sink = unsafe { geometry.Open()? };
        let (first, rest) = points
            .split_first()
            .ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let rest: Vec<_> = rest.iter().map(|&p| at(p).point).collect();
        // SAFETY: the sink is open; calls follow the documented order.
        unsafe { sink.SetFillMode(D2D1_FILL_MODE_WINDING) };
        let (begin, end) = if closed {
            (D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED)
        } else {
            (D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN)
        };
        // SAFETY: as above.
        unsafe { sink.BeginFigure(at(*first).point, begin) };
        // SAFETY: `rest` lives through the call.
        unsafe { sink.AddLines(&rest) };
        // SAFETY: ends the figure begun above.
        unsafe { sink.EndFigure(end) };
        // SAFETY: closes the sink once.
        unsafe { sink.Close()? };
        Ok(geometry)
    }
}

impl Painter for D2dPainter<'_> {
    fn fill_rect(&mut self, rect: Rect, radius: f32, c: Rgba) {
        let brush = self.brush(c);
        if radius <= 0.0 {
            // SAFETY: drawing between BeginDraw and EndDraw with a live brush.
            unsafe { self.target.FillRectangle(&rect_f(rect), brush) };
        } else {
            // SAFETY: as above.
            unsafe {
                self.target
                    .FillRoundedRectangle(&rounded(rect, radius), brush);
            };
        }
    }

    fn stroke_rect(&mut self, rect: Rect, radius: f32, width: f32, c: Rgba) {
        let brush = self.brush(c);
        // SAFETY: drawing between BeginDraw and EndDraw with a live brush.
        unsafe {
            self.target
                .DrawRoundedRectangle(&rounded(rect, radius), brush, width, None);
        };
    }

    fn line(&mut self, from: Point, to: Point, width: f32, c: Rgba) {
        let brush = self.brush(c);
        // SAFETY: as above; the default stroke style has flat (butt) caps.
        unsafe {
            self.target
                .DrawLine(at(from).point, at(to).point, brush, width, None);
        };
    }

    fn fill_circle(&mut self, center: Point, radius: f32, c: Rgba) {
        let brush = self.brush(c);
        // SAFETY: drawing between BeginDraw and EndDraw with a live brush.
        unsafe {
            self.target
                .FillEllipse(&Self::ellipse(center, radius), brush);
        };
    }

    fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, c: Rgba) {
        let brush = self.brush(c);
        // SAFETY: as above.
        unsafe {
            self.target
                .DrawEllipse(&Self::ellipse(center, radius), brush, width, None);
        };
    }

    fn fill_polygon(&mut self, points: &[Point], c: Rgba) {
        let Ok(geometry) = self.path(points, true) else {
            return;
        };
        let brush = self.brush(c);
        // SAFETY: drawing a closed geometry with a live brush.
        unsafe { self.target.FillGeometry(&geometry, brush, None) };
    }

    fn polyline(&mut self, points: &[Point], width: f32, c: Rgba) {
        let Ok(geometry) = self.path(points, false) else {
            return;
        };
        let brush = self.brush(c);
        // SAFETY: drawing an open geometry with a live brush and the
        // factory's stroke style.
        unsafe {
            self.target
                .DrawGeometry(&geometry, brush, width, self.polyline);
        };
    }

    fn push_clip(&mut self, rect: Rect) {
        // SAFETY: paired with `pop_clip`, as the Painter contract requires.
        unsafe {
            self.target
                .PushAxisAlignedClip(&rect_f(rect), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        };
    }

    fn pop_clip(&mut self) {
        // SAFETY: matches an earlier `push_clip`.
        unsafe { self.target.PopAxisAlignedClip() };
    }
}
