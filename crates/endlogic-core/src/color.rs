//! Color identifiers.

/// Identifies the color of a piece or a locked cell.
///
/// Clues are kept separately for every color, and a filled cell only counts
/// towards the clues of its own color. The text format and all current
/// puzzles use a single color, [`Color::DEFAULT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Color(pub u8);

impl Color {
    /// The only color used by single-color puzzles.
    pub const DEFAULT: Color = Color(0);
}
