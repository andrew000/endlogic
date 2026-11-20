//! What the drawing code draws with: the shell's [`Painter`] plus text in
//! the trainer's own font, optionally noting every line of text drawn.

use alloc::string::String;
use alloc::vec::Vec;

use crate::font::{self, Align, Font};
use crate::painter::{Painter, Point, Rect, Rgba};

/// A painter with text.
pub(crate) struct Ink<'a> {
    painter: &'a mut dyn Painter,
    /// The text drawn so far, when it is being noted.
    texts: Option<Vec<String>>,
}

impl<'a> Ink<'a> {
    /// Draws with `painter`.
    pub(crate) fn new(painter: &'a mut dyn Painter) -> Ink<'a> {
        Ink {
            painter,
            texts: None,
        }
    }

    /// Draws with `painter` and notes every line of text.
    pub(crate) fn noting(painter: &'a mut dyn Painter) -> Ink<'a> {
        Ink {
            painter,
            texts: Some(Vec::new()),
        }
    }

    /// The text noted so far.
    pub(crate) fn into_texts(self) -> Vec<String> {
        self.texts.unwrap_or_default()
    }

    pub(crate) fn fill_rect(&mut self, rect: Rect, radius: f32, color: Rgba) {
        self.painter.fill_rect(rect, radius, color);
    }

    pub(crate) fn stroke_rect(&mut self, rect: Rect, radius: f32, width: f32, color: Rgba) {
        self.painter.stroke_rect(rect, radius, width, color);
    }

    pub(crate) fn line(&mut self, from: Point, to: Point, width: f32, color: Rgba) {
        self.painter.line(from, to, width, color);
    }

    pub(crate) fn fill_circle(&mut self, center: Point, radius: f32, color: Rgba) {
        self.painter.fill_circle(center, radius, color);
    }

    pub(crate) fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, color: Rgba) {
        self.painter.stroke_circle(center, radius, width, color);
    }

    pub(crate) fn fill_polygon(&mut self, points: &[Point], color: Rgba) {
        self.painter.fill_polygon(points, color);
    }

    pub(crate) fn push_clip(&mut self, rect: Rect) {
        self.painter.push_clip(rect);
    }

    pub(crate) fn pop_clip(&mut self) {
        self.painter.pop_clip();
    }

    /// Draws one line of text; see [`font::draw_text`].
    pub(crate) fn text(
        &mut self,
        text: &str,
        anchor: Point,
        font: Font,
        align: Align,
        color: Rgba,
    ) {
        if let Some(texts) = &mut self.texts {
            texts.push(String::from(text));
        }
        font::draw_text(self.painter, text, anchor, font, align, color);
    }

    /// Width of `text` in `font`.
    #[allow(clippy::unused_self)]
    pub(crate) fn text_width(&self, text: &str, font: Font) -> f32 {
        font::text_width(text, font)
    }
}

/// A painter that draws nothing.
pub(crate) struct Nothing;

impl Painter for Nothing {
    fn fill_rect(&mut self, _: Rect, _: f32, _: Rgba) {}
    fn stroke_rect(&mut self, _: Rect, _: f32, _: f32, _: Rgba) {}
    fn line(&mut self, _: Point, _: Point, _: f32, _: Rgba) {}
    fn fill_circle(&mut self, _: Point, _: f32, _: Rgba) {}
    fn stroke_circle(&mut self, _: Point, _: f32, _: f32, _: Rgba) {}
    fn fill_polygon(&mut self, _: &[Point], _: Rgba) {}
    fn polyline(&mut self, _: &[Point], _: f32, _: Rgba) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
}
