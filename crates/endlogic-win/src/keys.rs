//! Physical keys: keyboard scan codes, so the keys work in every layout.

use endlogic_ui::Key;

/// Scan code of `Esc`.
pub const SCAN_ESCAPE: u32 = 0x01;
/// Scan code of `Tab`.
pub const SCAN_TAB: u32 = 0x0F;
/// Scan code of the key labelled `R` on a US keyboard.
pub const SCAN_R: u32 = 0x13;
/// Scan code of the key labelled `H` on a US keyboard.
pub const SCAN_H: u32 = 0x23;
/// Scan code of the key labelled `V` on a US keyboard, for `Ctrl+V`.
pub const SCAN_V: u32 = 0x2F;

/// The game key at scan code `scan` (set 1, without the extended flag).
#[must_use]
pub fn key_for_scan(scan: u32) -> Option<Key> {
    match scan {
        SCAN_R => Some(Key::R),
        SCAN_H => Some(Key::H),
        SCAN_TAB => Some(Key::Tab),
        SCAN_ESCAPE => Some(Key::Escape),
        _ => None,
    }
}

/// What a key press means, from its scan code, whether it repeats a key
/// still held down, and which modifiers are down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// A game key.
    Key(Key),
    /// `Ctrl+V`: open the puzzle on the clipboard.
    Paste,
    /// Not for the app; the system handles it.
    Ignore,
}

/// Modifier keys held during a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
    /// Either `Ctrl`.
    pub ctrl: bool,
    /// Either `Alt`.
    pub alt: bool,
    /// Either `Shift`.
    pub shift: bool,
    /// Either Windows key.
    pub win: bool,
}

/// Decides what a key press does: game keys without modifiers and without
/// auto-repeat, and `Ctrl+V`; nothing else.
#[must_use]
pub fn press(scan: u32, repeat: bool, mods: Modifiers) -> Press {
    if repeat {
        return Press::Ignore;
    }
    if mods.ctrl && !mods.alt && !mods.win && !mods.shift && scan == SCAN_V {
        return Press::Paste;
    }
    if mods.ctrl || mods.alt || mods.win {
        return Press::Ignore;
    }
    key_for_scan(scan).map_or(Press::Ignore, Press::Key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_codes_map_to_game_keys() {
        assert_eq!(key_for_scan(0x13), Some(Key::R));
        assert_eq!(key_for_scan(0x23), Some(Key::H));
        assert_eq!(key_for_scan(0x0F), Some(Key::Tab));
        assert_eq!(key_for_scan(0x01), Some(Key::Escape));
        // Neighbours and other keys do nothing.
        for scan in [0x12, 0x14, 0x22, 0x24, 0x10, 0x1C, 0x39, 0x2F, 0x00, 0xFF] {
            assert_eq!(key_for_scan(scan), None, "{scan:#x}");
        }
    }

    #[test]
    fn presses_respect_repeat_and_modifiers() {
        let none = Modifiers::default();
        let ctrl = Modifiers { ctrl: true, ..none };
        assert_eq!(press(SCAN_R, false, none), Press::Key(Key::R));
        assert_eq!(press(SCAN_R, true, none), Press::Ignore);
        // Shift does not change the physical key.
        let shift = Modifiers {
            shift: true,
            ..none
        };
        assert_eq!(press(SCAN_H, false, shift), Press::Key(Key::H));
        assert_eq!(press(SCAN_V, false, ctrl), Press::Paste);
        assert_eq!(press(SCAN_V, true, ctrl), Press::Ignore);
        assert_eq!(press(SCAN_V, false, none), Press::Ignore);
        for mods in [
            ctrl,
            Modifiers { alt: true, ..none },
            Modifiers { win: true, ..none },
        ] {
            assert_eq!(press(SCAN_R, false, mods), Press::Ignore, "{mods:?}");
            assert_eq!(press(SCAN_TAB, false, mods), Press::Ignore, "{mods:?}");
        }
        let ctrl_alt = Modifiers { alt: true, ..ctrl };
        assert_eq!(press(SCAN_V, false, ctrl_alt), Press::Ignore);
    }
}
