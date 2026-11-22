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

/// Scan code of `Backspace`.
pub const SCAN_BACKSPACE: u32 = 0x0E;
/// Scan code of `Enter` (with the extended flag: the keypad's).
pub const SCAN_ENTER: u32 = 0x1C;

/// The game key at scan code `scan` (set 1, without the extended flag):
/// letters, digits of the top row and of the numeric keypad.
#[must_use]
pub fn key_for_scan(scan: u32) -> Option<Key> {
    // Keypad digits by position, whatever Num Lock says.
    let keypad = [
        (0x52, 0),
        (0x4F, 1),
        (0x50, 2),
        (0x51, 3),
        (0x4B, 4),
        (0x4C, 5),
        (0x4D, 6),
        (0x47, 7),
        (0x48, 8),
        (0x49, 9),
    ];
    match scan {
        SCAN_R => Some(Key::R),
        SCAN_H => Some(Key::H),
        SCAN_TAB => Some(Key::Tab),
        SCAN_ESCAPE => Some(Key::Escape),
        SCAN_BACKSPACE => Some(Key::Backspace),
        SCAN_ENTER => Some(Key::Enter),
        // The top row: 1 to 9, then 0.
        0x02..=0x0A => u8::try_from(scan - 1).ok().map(Key::Digit),
        0x0B => Some(Key::Digit(0)),
        _ => keypad
            .iter()
            .find(|&&(s, _)| s == scan)
            .map(|&(_, d)| Key::Digit(d)),
    }
}

/// The game key at an extended scan code: the arrows and the keypad's
/// `Enter`.
#[must_use]
pub fn key_for_extended(scan: u32) -> Option<Key> {
    match scan {
        0x48 => Some(Key::Up),
        0x50 => Some(Key::Down),
        0x4B => Some(Key::Left),
        0x4D => Some(Key::Right),
        SCAN_ENTER => Some(Key::Enter),
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

/// Decides what a key press does: game keys without modifiers, and
/// `Ctrl+V`; nothing else. Held keys repeat only to move between clue
/// fields (arrows and `Backspace`).
#[must_use]
pub fn press(scan: u32, extended: bool, repeat: bool, mods: Modifiers) -> Press {
    let key = if extended {
        key_for_extended(scan)
    } else {
        key_for_scan(scan)
    };
    let moves = matches!(
        key,
        Some(Key::Left | Key::Right | Key::Up | Key::Down | Key::Backspace)
    );
    if repeat && !moves {
        return Press::Ignore;
    }
    if mods.ctrl && !mods.alt && !mods.win && !mods.shift && !extended && scan == SCAN_V {
        return if repeat { Press::Ignore } else { Press::Paste };
    }
    if mods.ctrl || mods.alt || mods.win {
        return Press::Ignore;
    }
    key.map_or(Press::Ignore, Press::Key)
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
        for scan in [0x12, 0x14, 0x22, 0x24, 0x10, 0x39, 0x2F, 0x00, 0xFF] {
            assert_eq!(key_for_scan(scan), None, "{scan:#x}");
        }
    }

    #[test]
    fn digits_of_the_top_row_and_the_keypad() {
        for (scan, d) in [(0x02, 1), (0x0A, 9), (0x0B, 0), (0x06, 5)] {
            assert_eq!(key_for_scan(scan), Some(Key::Digit(d)));
        }
        for (scan, d) in [(0x52, 0), (0x4F, 1), (0x4C, 5), (0x49, 9)] {
            assert_eq!(key_for_scan(scan), Some(Key::Digit(d)));
        }
        assert_eq!(key_for_scan(0x0E), Some(Key::Backspace));
        assert_eq!(key_for_scan(0x1C), Some(Key::Enter));
        // The arrows share scan codes with the keypad, marked extended.
        assert_eq!(key_for_extended(0x48), Some(Key::Up));
        assert_eq!(key_for_extended(0x50), Some(Key::Down));
        assert_eq!(key_for_extended(0x4B), Some(Key::Left));
        assert_eq!(key_for_extended(0x4D), Some(Key::Right));
        assert_eq!(key_for_extended(0x1C), Some(Key::Enter));
        assert_eq!(key_for_extended(0x13), None);
        let none = Modifiers::default();
        // Held arrows repeat, held digits do not.
        assert_eq!(press(0x4B, true, true, none), Press::Key(Key::Left));
        assert_eq!(press(0x0E, false, true, none), Press::Key(Key::Backspace));
        assert_eq!(press(0x02, false, true, none), Press::Ignore);
    }

    #[test]
    fn presses_respect_repeat_and_modifiers() {
        let none = Modifiers::default();
        let ctrl = Modifiers { ctrl: true, ..none };
        assert_eq!(press(SCAN_R, false, false, none), Press::Key(Key::R));
        assert_eq!(press(SCAN_R, false, true, none), Press::Ignore);
        // Shift does not change the physical key.
        let shift = Modifiers {
            shift: true,
            ..none
        };
        assert_eq!(press(SCAN_H, false, false, shift), Press::Key(Key::H));
        assert_eq!(press(SCAN_V, false, false, ctrl), Press::Paste);
        assert_eq!(press(SCAN_V, false, true, ctrl), Press::Ignore);
        assert_eq!(press(SCAN_V, false, false, none), Press::Ignore);
        for mods in [
            ctrl,
            Modifiers { alt: true, ..none },
            Modifiers { win: true, ..none },
        ] {
            assert_eq!(press(SCAN_R, false, false, mods), Press::Ignore, "{mods:?}");
            assert_eq!(
                press(SCAN_TAB, false, false, mods),
                Press::Ignore,
                "{mods:?}"
            );
        }
        let ctrl_alt = Modifiers { alt: true, ..ctrl };
        assert_eq!(press(SCAN_V, false, false, ctrl_alt), Press::Ignore);
    }
}
