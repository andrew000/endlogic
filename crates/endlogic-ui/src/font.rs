//! The trainer's own font: every letter is a few polylines on a small grid,
//! drawn with [`Painter::polyline`], so text looks the same everywhere.
//!
//! Glyphs live on a grid whose capital letters are 6 units high and
//! usually 4 wide; points sit on whole or half units. `y = 0` is the top of
//! a capital, `y = 6` the baseline; descenders and marks above reach at most
//! 1.5 units further. Corners are cut at 45° instead of rounded. Lower-case
//! letters are drawn as capitals. Regular and bold differ only in the
//! thickness of the line.

use alloc::string::String;

use crate::painter::{Painter, Point, Rect, Rgba};

/// Grid units in the height of a capital letter.
const CAP_UNITS: f32 = 6.0;
/// Cap height as a share of the font size, as in common system fonts.
const CAP_SHARE: f32 = 0.7;
/// Smallest cap height, in logical pixels.
pub const MIN_CAP_HEIGHT: f32 = 8.0;
/// Smallest font size: the one whose capitals are [`MIN_CAP_HEIGHT`] high.
pub const MIN_FONT_SIZE: f32 = MIN_CAP_HEIGHT / CAP_SHARE;
/// Space after every glyph, in grid units.
const GAP: f32 = 1.4;
/// Line thickness as a share of the cap height.
const REGULAR: f32 = 0.10;
/// Line thickness of bold text as a share of the cap height.
const BOLD: f32 = 0.16;
/// Side of a dot as a multiple of the line thickness.
const DOT: f32 = 1.5;

/// A font: a size and a weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font {
    /// Size in logical pixels; capitals are 0.7 of it high, and at least
    /// [`MIN_CAP_HEIGHT`].
    pub size: f32,
    /// A thicker line instead of the regular one.
    pub bold: bool,
}

impl Font {
    /// A font of `size`, raised to [`MIN_FONT_SIZE`] if smaller.
    #[must_use]
    pub fn new(size: f32, bold: bool) -> Font {
        Font {
            size: size.max(MIN_FONT_SIZE),
            bold,
        }
    }

    /// Height of a capital letter in logical pixels.
    #[must_use]
    pub fn cap_height(self) -> f32 {
        (self.size * CAP_SHARE).max(MIN_CAP_HEIGHT)
    }

    /// Thickness of the line in logical pixels.
    #[must_use]
    pub fn stroke(self) -> f32 {
        let share = if self.bold { BOLD } else { REGULAR };
        (self.cap_height() * share).max(1.0)
    }

    /// Logical pixels per grid unit.
    fn unit(self) -> f32 {
        self.cap_height() / CAP_UNITS
    }
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

/// A point of a glyph in half grid units.
type Half = (i8, i8);

/// One character's drawing.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    /// Width in half units, without the gap after it.
    width: u8,
    /// Polylines.
    strokes: &'static [&'static [Half]],
    /// Centers of square dots.
    dots: &'static [Half],
}

/// Converts grid units to half units.
#[allow(clippy::cast_possible_truncation)]
const fn half(v: f32) -> i8 {
    (v * 2.0) as i8
}

/// `glyph!(width; [(x, y), ...], ...; (dot x, dot y), ...)`, in grid units.
macro_rules! glyph {
    ($w:expr; $([$(($x:expr, $y:expr)),* $(,)?]),* $(; $(($dx:expr, $dy:expr)),*)? ) => {
        const {
            Glyph {
                width: ($w as f32 * 2.0) as u8,
                strokes: &[$(&[$((half($x as f32), half($y as f32))),*]),*],
                dots: &[$($((half($dx as f32), half($dy as f32))),*)?],
            }
        }
    };
}

/// The glyph drawn for an unknown character: an empty box.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
const MISSING: Glyph = glyph!(3; [(1.5, 0), (3, 0), (3, 6), (0, 6), (0, 0), (1.5, 0)]);

/// The capital that a lower-case letter is drawn as.
fn capital(c: char) -> char {
    match c {
        'a'..='z' | 'а'..='я' => char::from_u32(c as u32 - 0x20).unwrap_or(c),
        'і' => 'І',
        'ї' => 'Ї',
        'є' => 'Є',
        'ґ' => 'Ґ',
        _ => c,
    }
}

/// The glyph of `c`, if the font has one. The table is data: one line per
/// character, coordinates in grid units.
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
#[rustfmt::skip]
fn glyph(c: char) -> Option<Glyph> {
    Some(match capital(c) {
        ' ' => glyph!(1.6;),
        'A' | 'А' => glyph!(4; [(0, 6), (0, 1), (1, 0), (3, 0), (4, 1), (4, 6)], [(0, 3.5), (4, 3.5)]),
        'B' | 'В' => glyph!(4; [(0, 3), (3, 3), (4, 4), (4, 5), (3, 6), (0, 6), (0, 0), (3, 0), (4, 1), (4, 2), (3, 3)]),
        'C' | 'С' => glyph!(4; [(4, 1), (3, 0), (1, 0), (0, 1), (0, 5), (1, 6), (3, 6), (4, 5)]),
        'D' => glyph!(4; [(0, 3), (0, 0), (3, 0), (4, 1), (4, 5), (3, 6), (0, 6), (0, 3)]),
        'E' | 'Е' => glyph!(4; [(4, 0), (0, 0), (0, 6), (4, 6)], [(0, 3), (3, 3)]),
        'F' => glyph!(4; [(4, 0), (0, 0), (0, 6)], [(0, 3), (3, 3)]),
        'G' => glyph!(4; [(4, 1), (3, 0), (1, 0), (0, 1), (0, 5), (1, 6), (3, 6), (4, 5), (4, 3), (2, 3)]),
        'H' | 'Н' => glyph!(4; [(0, 0), (0, 6)], [(4, 0), (4, 6)], [(0, 3), (4, 3)]),
        'I' | 'І' => glyph!(2; [(0, 0), (2, 0)], [(1, 0), (1, 6)], [(0, 6), (2, 6)]),
        'J' => glyph!(4; [(1, 0), (4, 0), (4, 5), (3, 6), (1, 6), (0, 5)]),
        'K' | 'К' => glyph!(4; [(0, 0), (0, 6)], [(4, 0), (1, 3), (4, 6)], [(0, 3), (1, 3)]),
        'L' => glyph!(4; [(0, 0), (0, 6), (4, 6)]),
        'M' | 'М' => glyph!(5; [(0, 6), (0, 0), (2.5, 2.5), (5, 0), (5, 6)]),
        'N' => glyph!(4; [(0, 6), (0, 0), (4, 6), (4, 0)]),
        'O' | 'О' => glyph!(4; [(0, 3), (0, 1), (1, 0), (3, 0), (4, 1), (4, 5), (3, 6), (1, 6), (0, 5), (0, 3)]),
        'P' | 'Р' => glyph!(4; [(0, 6), (0, 0), (3, 0), (4, 1), (4, 2), (3, 3), (0, 3)]),
        'Q' => glyph!(4; [(0, 3), (0, 1), (1, 0), (3, 0), (4, 1), (4, 5), (3, 6), (1, 6), (0, 5), (0, 3)], [(2, 4), (4.5, 6.5)]),
        'R' => glyph!(4; [(0, 6), (0, 0), (3, 0), (4, 1), (4, 2), (3, 3), (0, 3)], [(2, 3), (4, 5), (4, 6)]),
        'S' => glyph!(4; [(4, 1), (3, 0), (1, 0), (0, 1), (0, 2), (1, 3), (3, 3), (4, 4), (4, 5), (3, 6), (1, 6), (0, 5)]),
        'T' | 'Т' => glyph!(4; [(0, 0), (4, 0)], [(2, 0), (2, 6)]),
        'U' => glyph!(4; [(0, 0), (0, 5), (1, 6), (3, 6), (4, 5), (4, 0)]),
        'V' => glyph!(4; [(0, 0), (2, 6), (4, 0)]),
        'W' => glyph!(5; [(0, 0), (1, 6), (2.5, 3), (4, 6), (5, 0)]),
        'X' | 'Х' => glyph!(4; [(0, 0), (4, 6)], [(4, 0), (0, 6)]),
        'Y' => glyph!(4; [(0, 0), (2, 3), (4, 0)], [(2, 3), (2, 6)]),
        'Z' => glyph!(4; [(0, 0), (4, 0), (0, 6), (4, 6)]),
        'Б' => glyph!(4; [(4, 0), (0, 0), (0, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)]),
        'Г' => glyph!(4; [(4, 0), (0, 0), (0, 6)]),
        'Ґ' => glyph!(4; [(4, -1.5), (4, 0), (0, 0), (0, 6)]),
        'Д' => glyph!(5; [(1, 6), (1, 1), (2, 0), (4, 0), (4, 6)], [(0, 7.5), (0, 6), (5, 6), (5, 7.5)]),
        'Є' => glyph!(4; [(4, 1), (3, 0), (1, 0), (0, 1), (0, 5), (1, 6), (3, 6), (4, 5)], [(0, 3), (3, 3)]),
        'Ж' => glyph!(6; [(0, 0), (2.5, 3), (0, 6)], [(6, 0), (3.5, 3), (6, 6)], [(3, 0), (3, 6)], [(2.5, 3), (3.5, 3)]),
        'З' => glyph!(4; [(0, 1), (1, 0), (3, 0), (4, 1), (4, 2), (3, 3), (4, 4), (4, 5), (3, 6), (1, 6), (0, 5)], [(1.5, 3), (3, 3)]),
        'И' => glyph!(4; [(0, 0), (0, 6), (4, 0), (4, 6)]),
        'Й' => glyph!(4; [(0, 0), (0, 6), (4, 0), (4, 6)], [(1, -1.5), (1.5, -1), (2.5, -1), (3, -1.5)]),
        'Ї' => glyph!(2; [(0, 0), (2, 0)], [(1, 0), (1, 6)], [(0, 6), (2, 6)]; (0, -1), (2, -1)),
        'Л' => glyph!(4; [(0, 6), (1, 5), (1, 1), (2, 0), (4, 0), (4, 6)]),
        'П' => glyph!(4; [(0, 6), (0, 0), (4, 0), (4, 6)]),
        'У' => glyph!(4; [(0, 0), (0, 2), (1, 3), (4, 3)], [(4, 0), (4, 5), (3, 6), (0, 6)]),
        'Ф' => glyph!(6; [(3, 0), (3, 6)], [(3, 1), (1, 1), (0, 2), (0, 4), (1, 5), (5, 5), (6, 4), (6, 2), (5, 1), (3, 1)]),
        'Ц' => glyph!(5; [(0, 0), (0, 6), (5, 6), (5, 7.5)], [(4, 0), (4, 6)]),
        'Ч' => glyph!(4; [(0, 0), (0, 2), (1, 3), (4, 3)], [(4, 0), (4, 6)]),
        'Ш' => glyph!(6; [(0, 0), (0, 6), (6, 6), (6, 0)], [(3, 0), (3, 6)]),
        'Щ' => glyph!(7; [(0, 0), (0, 6), (7, 6), (7, 7.5)], [(3, 0), (3, 6)], [(6, 0), (6, 6)]),
        'Ь' => glyph!(4; [(0, 0), (0, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)]),
        'Ю' => glyph!(6; [(0, 0), (0, 6)], [(0, 3), (2, 3)], [(2, 3), (2, 1), (3, 0), (5, 0), (6, 1), (6, 5), (5, 6), (3, 6), (2, 5), (2, 3)]),
        'Я' => glyph!(4; [(4, 6), (4, 0), (1, 0), (0, 1), (0, 2), (1, 3), (4, 3)], [(2, 3), (0, 5), (0, 6)]),
        '0' => glyph!(4; [(0, 3), (0, 1), (1, 0), (3, 0), (4, 1), (4, 5), (3, 6), (1, 6), (0, 5), (0, 3)], [(3.5, 1.5), (0.5, 4.5)]),
        '1' => glyph!(4; [(1, 1), (2, 0), (2, 6)], [(1, 6), (3, 6)]),
        '2' => glyph!(4; [(0, 1), (1, 0), (3, 0), (4, 1), (4, 2), (0, 6), (4, 6)]),
        '3' => glyph!(4; [(0, 0), (4, 0), (2, 2.5), (3, 2.5), (4, 3.5), (4, 5), (3, 6), (1, 6), (0, 5)]),
        '4' => glyph!(4; [(3, 6), (3, 0), (0, 4), (4, 4)]),
        '5' => glyph!(4; [(4, 0), (0, 0), (0, 2.5), (3, 2.5), (4, 3.5), (4, 5), (3, 6), (1, 6), (0, 5)]),
        '6' => glyph!(4; [(4, 1), (3, 0), (1, 0), (0, 1), (0, 5), (1, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)]),
        '7' => glyph!(4; [(0, 0), (4, 0), (4, 1), (1.5, 6)]),
        '8' => glyph!(4; [(3, 3), (1, 3), (0, 2), (0, 1), (1, 0), (3, 0), (4, 1), (4, 2), (3, 3), (4, 4), (4, 5), (3, 6), (1, 6), (0, 5), (0, 4), (1, 3)]),
        '9' => glyph!(4; [(0, 5), (1, 6), (3, 6), (4, 5), (4, 1), (3, 0), (1, 0), (0, 1), (0, 2), (1, 3), (4, 3)]),
        '.' => glyph!(1;; (0.5, 5.5)),
        ',' => glyph!(1; [(0.5, 5.5), (0.5, 6), (0, 7)]; (0.5, 5.5)),
        ':' => glyph!(1;; (0.5, 2), (0.5, 5.5)),
        ';' => glyph!(1; [(0.5, 5.5), (0.5, 6), (0, 7)]; (0.5, 2), (0.5, 5.5)),
        '!' => glyph!(1; [(0.5, 0), (0.5, 4)]; (0.5, 5.5)),
        '?' => glyph!(4; [(0, 1), (1, 0), (3, 0), (4, 1), (4, 2), (2, 4)]; (2, 5.5)),
        '\'' => glyph!(1; [(0.5, 0), (0.5, 1.5)]),
        '’' => glyph!(1; [(1, 0), (1, 1), (0, 2)]),
        '‘' => glyph!(1; [(0, 2), (0, 1), (1, 0)]),
        '"' => glyph!(2; [(0, 0), (0, 1.5)], [(2, 0), (2, 1.5)]),
        '“' => glyph!(3; [(0, 2), (0, 1), (1, 0)], [(2, 2), (2, 1), (3, 0)]),
        '”' => glyph!(3; [(1, 0), (1, 1), (0, 2)], [(3, 0), (3, 1), (2, 2)]),
        '„' => glyph!(3; [(1, 5), (1, 6), (0, 7)], [(3, 5), (3, 6), (2, 7)]),
        '«' => glyph!(4; [(2, 1), (0, 3), (2, 5)], [(4, 1), (2, 3), (4, 5)]),
        '»' => glyph!(4; [(0, 1), (2, 3), (0, 5)], [(2, 1), (4, 3), (2, 5)]),
        '-' => glyph!(3; [(0.5, 3), (2.5, 3)]),
        '–' | '−' => glyph!(4; [(0, 3), (4, 3)]),
        '—' => glyph!(6; [(0, 3), (6, 3)]),
        '_' => glyph!(4; [(0, 7), (4, 7)]),
        '/' => glyph!(3; [(0, 6), (3, 0)]),
        '\\' => glyph!(3; [(0, 0), (3, 6)]),
        '|' => glyph!(1; [(0.5, -1), (0.5, 7)]),
        '(' => glyph!(2; [(2, -1), (0.5, 0.5), (0.5, 5.5), (2, 7)]),
        ')' => glyph!(2; [(0, -1), (1.5, 0.5), (1.5, 5.5), (0, 7)]),
        '+' => glyph!(4; [(2, 1), (2, 5)], [(0, 3), (4, 3)]),
        '×' => glyph!(4; [(0.5, 1.5), (3.5, 4.5)], [(3.5, 1.5), (0.5, 4.5)]),
        '=' => glyph!(4; [(0, 2), (4, 2)], [(0, 4), (4, 4)]),
        '·' => glyph!(1;; (0.5, 3)),
        '…' => glyph!(4;; (0.5, 5.5), (2, 5.5), (3.5, 5.5)),
        '%' => glyph!(5; [(0.5, 6), (4.5, 0)], [(1, 0), (2, 0), (2, 2), (0, 2), (0, 0), (1, 0)], [(4, 4), (5, 4), (5, 6), (3, 6), (3, 4), (4, 4)]),
        '#' => glyph!(5; [(1.5, 0.5), (1.5, 5.5)], [(3.5, 0.5), (3.5, 5.5)], [(0, 2), (5, 2)], [(0, 4), (5, 4)]),
        '<' => glyph!(3; [(3, 0), (0, 3), (3, 6)]),
        '>' => glyph!(3; [(0, 0), (3, 3), (0, 6)]),
        '‹' => glyph!(2; [(2, 1), (0, 3), (2, 5)]),
        '›' => glyph!(2; [(0, 1), (2, 3), (0, 5)]),
        'Δ' => glyph!(5; [(2.5, 6), (0, 6), (2.5, 0), (5, 6), (2.5, 6)]),
        '∅' => glyph!(5; [(0.5, 3), (0.5, 1.5), (1.5, 0.5), (3.5, 0.5), (4.5, 1.5), (4.5, 4.5), (3.5, 5.5), (1.5, 5.5), (0.5, 4.5), (0.5, 3)], [(5, 0), (0, 6)]),
        _ => return None,
    })
}

/// Whether the font has a glyph of its own for `c` (lower-case letters
/// count as their capitals).
#[must_use]
pub fn has_glyph(c: char) -> bool {
    glyph(c).is_some()
}

/// Advance of `c` in grid units: its width and the gap after it.
fn advance(c: char) -> f32 {
    f32::from(glyph(c).unwrap_or(MISSING).width) / 2.0 + GAP
}

/// Width of `text` in logical pixels: from the left edge of the first glyph
/// to the right edge of the last (the line's own thickness not counted). A
/// trailing space counts in full, so text that follows it keeps its gap.
#[must_use]
pub fn text_width(text: &str, font: Font) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let last_gap = if text.ends_with(' ') { 0.0 } else { GAP };
    (text.chars().map(advance).sum::<f32>() - last_gap) * font.unit()
}

/// Draws one line of text: `anchor.y` is the vertical middle of the
/// capitals, `align` says where `anchor.x` is.
pub fn draw_text(
    p: &mut dyn Painter,
    text: &str,
    anchor: Point,
    font: Font,
    align: Align,
    color: Rgba,
) {
    let width = text_width(text, font);
    let mut x = match align {
        Align::Left => anchor.x,
        Align::Center => anchor.x - width / 2.0,
        Align::Right => anchor.x - width,
    };
    let top = anchor.y - font.cap_height() / 2.0;
    let unit = font.unit();
    let stroke = font.stroke();
    let dot = stroke * DOT;
    let mut points: alloc::vec::Vec<Point> = alloc::vec::Vec::new();
    for c in text.chars() {
        let g = glyph(c).unwrap_or(MISSING);
        let at = |(hx, hy): Half| {
            Point::new(
                x + f32::from(hx) / 2.0 * unit,
                top + f32::from(hy) / 2.0 * unit,
            )
        };
        for stroke_points in g.strokes {
            points.clear();
            points.extend(stroke_points.iter().map(|&h| at(h)));
            p.polyline(&points, stroke, color);
        }
        for &d in g.dots {
            p.fill_rect(Rect::centered(at(d), dot, dot), 0.0, color);
        }
        x += (f32::from(g.width) / 2.0 + GAP) * unit;
    }
}

/// Every character the font draws, capitals only, for samples and tests.
#[must_use]
pub fn repertoire() -> String {
    let mut all = String::new();
    all.extend('A'..='Z');
    all.push_str("АБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ");
    all.extend('0'..='9');
    all.push_str(" .,:;!?'’‘\"“”„«»-–—_/\\|()+−×=·…%#<>‹›Δ∅");
    all
}

/// Size of the font sample in logical pixels.
pub const FONT_SAMPLE_SIZE: (f32, f32) = (2400.0, 1980.0);

/// Draws a sample of the font: every glyph and some real lines of the
/// interface, regular and bold, with capitals 8, 10, 12, 16, 24 and 48
/// logical pixels high. It fills [`FONT_SAMPLE_SIZE`]; `width` and
/// `height` only size the background.
pub fn draw_font_sample(p: &mut dyn Painter, width: f32, height: f32) {
    use crate::theme;
    p.fill_rect(Rect::new(0.0, 0.0, width, height), 0.0, theme::BACKGROUND);
    let lines = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz",
        "АБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ абвгґдеєжзиіїйклмнопрстуфхцчшщьюя",
        "0123456789 .,:;!?'’‘\"“”„«»-–—_/\\|()+−×=·…%#<>‹›Δ∅ 0O 1I| 3З 6Б 8В 4Ч ЬБ",
        "Δ-H-AWYAG9 · Hint · 3 · Solution 2 of 5 · ЕКСПЕРТ · Поділитися · Розв'язати · Ще раз ‹ 2/3 ›",
    ];
    let label = Font::new(MIN_FONT_SIZE, false);
    let mut y = 24.0;
    for cap in [8_u8, 10, 12, 16, 24, 48] {
        let px = f32::from(cap);
        for bold in [false, true] {
            let font = Font::new(px / CAP_SHARE, bold);
            let weight = if bold { "BOLD" } else { "REGULAR" };
            let title = alloc::format!("{cap} PX {weight}");
            draw_text(
                p,
                &title,
                Point::new(24.0, y),
                label,
                Align::Left,
                theme::MUTED,
            );
            y += 16.0 + px * 0.9;
            for line in lines {
                draw_text(p, line, Point::new(24.0, y), font, Align::Left, theme::TEXT);
                y += px * 1.9;
            }
            y += 4.0;
        }
    }
}
