//! Color identifiers.

/// Largest number of colors in one puzzle.
pub const MAX_COLORS: usize = 4;

/// Identifies the color of a piece or a locked cell.
///
/// Clues are kept separately for every color, and a filled cell only counts
/// towards the clues of its own color. A puzzle uses the colors `Color(0)`
/// to `Color(3)`, written `a` to `d` in the text format; single-color
/// puzzles use only [`Color::DEFAULT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Color(pub u8);

impl Color {
    /// The only color used by single-color puzzles.
    pub const DEFAULT: Color = Color(0);

    /// Every color a puzzle may use, in order.
    pub const ALL: [Color; MAX_COLORS] = [Color(0), Color(1), Color(2), Color(3)];

    /// The color's position in [`Color::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The color's letter in the text format, `a` to `d`, or `None` for a
    /// color a puzzle cannot use.
    #[must_use]
    pub const fn letter(self) -> Option<char> {
        match self.0 {
            0 => Some('a'),
            1 => Some('b'),
            2 => Some('c'),
            3 => Some('d'),
            _ => None,
        }
    }

    /// The color written as `letter`, `a` to `d`.
    #[must_use]
    pub const fn from_letter(letter: char) -> Option<Color> {
        match letter {
            'a' => Some(Color(0)),
            'b' => Some(Color(1)),
            'c' => Some(Color(2)),
            'd' => Some(Color(3)),
            _ => None,
        }
    }
}
