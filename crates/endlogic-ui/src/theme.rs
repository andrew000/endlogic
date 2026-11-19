//! Colors and timings.

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
/// Hint silhouette: cells the pieces cover in the solution.
pub const SILHOUETTE: Rgba = Rgba::hex(0x19_E3C0);
/// Dead cell icon.
pub const DEAD_ICON: Rgba = Rgba::hex(0x7A_7D7C);
/// Errors and over-filled lines.
pub const ERROR: Rgba = Rgba::hex(0xFF_4D5E);
/// The yellow marker next to the puzzle id.
pub const MARKER: Rgba = Rgba::hex(0xFF_D51F);
/// Main text.
pub const TEXT: Rgba = Rgba::hex(0xE6_ECEA);
/// Secondary text.
pub const MUTED: Rgba = Rgba::hex(0x7E_8886);

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
