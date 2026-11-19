//! [`Painter`] on a Direct2D render target, with DirectWrite text.

use std::collections::HashMap;

use endlogic_ui::{Align, Font, FontFamily, Painter, Point, Rect, Rgba};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FILL_MODE_WINDING,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE,
    D2D1_ROUNDED_RECT, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_METRICS, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::{BOOL, HSTRING, Result, w};

/// The sans-serif family.
const SANS: &str = "Segoe UI";
/// The monospaced family, and the one used when it is missing.
const MONO: &str = "Cascadia Mono";
const MONO_FALLBACK: &str = "Consolas";
/// Room given to a line of text; it never wraps.
const NO_LIMIT: f32 = 100_000.0;

/// A text format with what is needed to place its lines.
struct Format {
    format: IDWriteTextFormat,
    /// `(ascent - descent) / 2` in DIPs: how far above the baseline the
    /// vertical middle of the text is.
    middle: f32,
}

/// DirectWrite and the text formats made so far; independent of any render
/// target, so it outlives lost targets.
pub struct Fonts {
    dwrite: IDWriteFactory,
    collection: IDWriteFontCollection,
    mono: HSTRING,
    formats: HashMap<(FontFamily, u32, bool), Format>,
}

impl Fonts {
    /// Opens DirectWrite and picks the monospaced family.
    ///
    /// # Errors
    ///
    /// When DirectWrite cannot be started.
    pub fn new() -> Result<Fonts> {
        // SAFETY: plain factory creation; the result is an owned interface.
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let mut collection = None;
        // SAFETY: `collection` is a valid out pointer for the call.
        unsafe { dwrite.GetSystemFontCollection(&raw mut collection, false)? };
        let collection = collection.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let mono = if family_exists(&collection, MONO) {
            MONO
        } else {
            MONO_FALLBACK
        };
        Ok(Fonts {
            dwrite,
            collection,
            mono: HSTRING::from(mono),
            formats: HashMap::new(),
        })
    }

    fn format(&mut self, font: Font) -> Result<&Format> {
        let key = (font.family, font.size.to_bits(), font.bold);
        if !self.formats.contains_key(&key) {
            let family = match font.family {
                FontFamily::Sans => HSTRING::from(SANS),
                FontFamily::Mono => self.mono.clone(),
            };
            let weight = if font.bold {
                DWRITE_FONT_WEIGHT_BOLD
            } else {
                DWRITE_FONT_WEIGHT_REGULAR
            };
            let size = font.size.max(0.5);
            // SAFETY: the strings are valid for the call; the collection is
            // the system one.
            let format = unsafe {
                self.dwrite.CreateTextFormat(
                    &family,
                    &self.collection,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-us"),
                )?
            };
            // SAFETY: a plain setter on the new format.
            unsafe { format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)? };
            let metrics = font_metrics(&self.collection, &family, weight)?;
            let em = f32::from(metrics.designUnitsPerEm.max(1));
            let middle = (f32::from(metrics.ascent) - f32::from(metrics.descent)) / 2.0 * size / em;
            self.formats.insert(key, Format { format, middle });
        }
        self.formats
            .get(&key)
            .ok_or_else(|| windows::core::Error::from(E_FAIL))
    }

    /// A one-line layout of `text`.
    fn layout(&mut self, text: &str, font: Font) -> Result<(IDWriteTextLayout, f32)> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let dwrite = self.dwrite.clone();
        let format = self.format(font)?;
        let middle = format.middle;
        // SAFETY: `wide` and the format live through the call.
        let layout = unsafe { dwrite.CreateTextLayout(&wide, &format.format, NO_LIMIT, NO_LIMIT)? };
        Ok((layout, middle))
    }

    /// Width of `text` in `font`, trailing spaces included, as drawn.
    pub fn width(&mut self, text: &str, font: Font) -> f32 {
        self.layout(text, font)
            .and_then(|(layout, _)| layout_width(&layout))
            .unwrap_or(0.0)
    }
}

fn layout_width(layout: &IDWriteTextLayout) -> Result<f32> {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    // SAFETY: `metrics` is a valid out pointer.
    unsafe { layout.GetMetrics(&raw mut metrics)? };
    Ok(metrics.widthIncludingTrailingWhitespace)
}

/// Distance from the top of the first line to its baseline.
fn baseline(layout: &IDWriteTextLayout) -> Result<f32> {
    let mut lines = [DWRITE_LINE_METRICS::default(); 1];
    let mut count = 0;
    // SAFETY: the buffer holds one line, which is all a layout without
    // wrapping or line breaks has; `count` is a valid out pointer.
    unsafe { layout.GetLineMetrics(Some(&mut lines), &raw mut count)? };
    Ok(lines[0].baseline)
}

fn family_exists(collection: &IDWriteFontCollection, name: &str) -> bool {
    let mut index = 0;
    let mut exists = BOOL::default();
    // SAFETY: both out pointers are valid locals.
    let found =
        unsafe { collection.FindFamilyName(&HSTRING::from(name), &raw mut index, &raw mut exists) };
    found.is_ok() && exists.as_bool()
}

/// Design metrics of the font of `family` at `weight`, or of the first
/// family when it is missing.
fn font_metrics(
    collection: &IDWriteFontCollection,
    family: &HSTRING,
    weight: DWRITE_FONT_WEIGHT,
) -> Result<DWRITE_FONT_METRICS> {
    let mut index = 0;
    let mut exists = BOOL::default();
    // SAFETY: both out pointers are valid locals.
    unsafe { collection.FindFamilyName(family, &raw mut index, &raw mut exists)? };
    if !exists.as_bool() {
        index = 0;
    }
    // SAFETY: `index` is below the family count: found, or the first.
    let family = unsafe { collection.GetFontFamily(index)? };
    // SAFETY: plain lookup with valid enum values.
    let font = unsafe {
        family.GetFirstMatchingFont(weight, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL)?
    };
    let mut metrics = DWRITE_FONT_METRICS::default();
    // SAFETY: `metrics` is a valid out pointer.
    unsafe { font.GetMetrics(&raw mut metrics) };
    Ok(metrics)
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
    fonts: &'a mut Fonts,
}

impl<'a> D2dPainter<'a> {
    /// A painter for `target`, whose solid brush is `brush`.
    pub fn new(
        target: &'a ID2D1RenderTarget,
        factory: &'a ID2D1Factory,
        brush: &'a ID2D1SolidColorBrush,
        fonts: &'a mut Fonts,
    ) -> D2dPainter<'a> {
        D2dPainter {
            target,
            factory,
            brush,
            fonts,
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

    fn polygon(
        &self,
        points: &[Point],
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1PathGeometry> {
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
        // SAFETY: as above.
        unsafe { sink.BeginFigure(at(*first).point, D2D1_FIGURE_BEGIN_FILLED) };
        // SAFETY: `rest` lives through the call.
        unsafe { sink.AddLines(&rest) };
        // SAFETY: closes the figure begun above.
        unsafe { sink.EndFigure(D2D1_FIGURE_END_CLOSED) };
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
        let Ok(geometry) = self.polygon(points) else {
            return;
        };
        let brush = self.brush(c);
        // SAFETY: drawing a closed geometry with a live brush.
        unsafe { self.target.FillGeometry(&geometry, brush, None) };
    }

    fn text(&mut self, text: &str, anchor: Point, font: Font, align: Align, c: Rgba) {
        let Ok((layout, middle)) = self.fonts.layout(text, font) else {
            return;
        };
        let (Ok(width), Ok(base)) = (layout_width(&layout), baseline(&layout)) else {
            return;
        };
        let x = match align {
            Align::Left => anchor.x,
            Align::Center => anchor.x - width / 2.0,
            Align::Right => anchor.x - width,
        };
        // The middle between ascent and descent goes on the anchor, as with
        // the canvas `middle` baseline.
        let y = anchor.y - base + middle;
        let brush = self.brush(c);
        // SAFETY: drawing a live layout with a live brush.
        unsafe {
            self.target.DrawTextLayout(
                at(Point::new(x, y)).point,
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        };
    }

    fn text_width(&mut self, text: &str, font: Font) -> f32 {
        self.fonts.width(text, font)
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
