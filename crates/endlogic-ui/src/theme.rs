//! Colors and timings.

use endlogic_core::Color;

use crate::painter::Rgba;

/// Page background; a shell may use it around the drawing area (a window
/// title bar, the page behind the canvas).
pub const BACKGROUND: Rgba = Rgba::hex(0x0B_0D0C);
/// Panels: the left column, banners and toasts.
pub const PANEL: Rgba = Rgba::hex(0x15_1817);
/// Tray background.
pub const TRAY: Rgba = Rgba::hex(0x2F_3231);
/// A piece card in the tray.
pub const CARD: Rgba = Rgba::hex(0x1A_1D1C);
/// A free board cell.
pub const CELL: Rgba = Rgba::hex(0x10_1312);
/// Grid lines.
pub const GRID_LINE: Rgba = Rgba::hex(0x1E_2322);
/// Marks inside a free cell.
pub const CELL_MARK: Rgba = Rgba::hex(0x2C_3331);
/// Accent: outlines, counter ticks, active elements.
pub const ACCENT: Rgba = Rgba::hex(0x19_E3C0);
/// Piece fill.
pub const PIECE: Rgba = Rgba::hex(0x0F_BF9F);
/// Bright accent: piece edges, satisfied counters.
pub const BRIGHT: Rgba = Rgba::hex(0x3F_F5D6);
/// Locked cell fill.
pub const LOCK: Rgba = Rgba::hex(0x12_A88E);
/// Dead cell background.
pub const DEAD: Rgba = Rgba::hex(0x4A_4D4C);
/// Dead cell hatching.
pub const DEAD_HATCH: Rgba = Rgba::hex(0x5E_6160);
/// Dead cell icon.
pub const DEAD_ICON: Rgba = Rgba::hex(0x7A_7D7C);
/// Errors and over-filled lines.
pub const ERROR: Rgba = Rgba::hex(0xFF_4D5E);
/// The yellow marker next to the puzzle id; also the outline of places a
/// screenshot reading is not sure about.
pub const MARKER: Rgba = Rgba::hex(0xFF_D51F);
/// Main text.
pub const TEXT: Rgba = Rgba::hex(0xE6_ECEA);
/// Secondary text.
pub const MUTED: Rgba = Rgba::hex(0x7E_8886);

/// The look of one puzzle color: piece fill, bright edge (also counters
/// that match), counter ticks and outlines, and locked cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Piece fill.
    pub fill: Rgba,
    /// Piece edges, satisfied counters.
    pub bright: Rgba,
    /// Counter ticks and outlines, the drop shadow, the hint silhouette.
    pub accent: Rgba,
    /// Locked cell fill.
    pub lock: Rgba,
}

/// The palettes of the four colors: teal, orange, violet, lime.
pub const PALETTES: [Palette; 4] = [
    Palette {
        fill: PIECE,
        bright: BRIGHT,
        accent: ACCENT,
        lock: LOCK,
    },
    Palette {
        fill: Rgba::hex(0xC9_8A3C),
        bright: Rgba::hex(0xF7_B65A),
        accent: Rgba::hex(0xE8_9B3E),
        lock: Rgba::hex(0xA8_6F2C),
    },
    Palette {
        fill: Rgba::hex(0x8D_6FE0),
        bright: Rgba::hex(0xB8_A2FF),
        accent: Rgba::hex(0xA0_86F5),
        lock: Rgba::hex(0x74_58C4),
    },
    Palette {
        fill: Rgba::hex(0x9D_BF3A),
        bright: Rgba::hex(0xCC_EC5E),
        accent: Rgba::hex(0xB5_D645),
        lock: Rgba::hex(0x80_9E2C),
    },
];

/// The palette of `color`; colors past the fourth look like the first.
#[must_use]
pub fn palette(color: Color) -> Palette {
    PALETTES.get(color.index()).copied().unwrap_or(PALETTES[0])
}

/// Seconds for a dropped piece to settle into its cells.
pub const SNAP_TIME: f32 = 0.12;
/// Seconds for a piece to fly back to the tray.
pub const RETURN_TIME: f32 = 0.18;
/// Seconds for a piece taken from the tray to grow to board scale.
pub const GROW_TIME: f32 = 0.08;
/// Seconds a piece shakes when a turn does not fit.
pub const WOBBLE_TIME: f32 = 0.2;
/// Seconds for a piece placed by a hint to travel to its cells.
pub const HINT_FLY_TIME: f32 = 0.45;
/// Seconds a piece pointed out by a hint stays red.
pub const BLAME_TIME: f32 = 1.5;
/// Seconds the clear button waits for its second tap.
pub const CLEAR_TIME: f32 = 3.0;
/// Seconds for a quarter turn.
pub const ROTATE_TIME: f32 = 0.10;
/// Seconds for the win highlight to fade in.
pub const WIN_TIME: f32 = 0.6;
/// Seconds a toast stays on screen, fade included.
pub const TOAST_TIME: f32 = 2.2;

/// Ease-out cubic: fast start, gentle stop. `t` is clamped to `0..=1`.
#[must_use]
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}
