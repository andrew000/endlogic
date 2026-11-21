//! What a shell passes in and gets back: start-up data, events and effects.

use alloc::string::String;
use alloc::vec::Vec;

/// Keys of the settings the UI saves through [`Effect::SaveSetting`].
pub const SETTING_KEYS: [&str; 5] = [
    SETTING_LANG,
    SETTING_COUNTERS,
    SETTING_DIFFICULTY,
    SETTING_EDITOR,
    SETTING_COLORS,
];

/// Setting key for the language: `en` or `ua`.
pub const SETTING_LANG: &str = "lang";
/// Setting key for the counter display: `grid` or `numbers`.
pub const SETTING_COUNTERS: &str = "counters";
/// Setting key for the difficulty: `easy`, `normal`, `hard` or `expert`.
pub const SETTING_DIFFICULTY: &str = "difficulty";
/// Setting key for the puzzle in the editor of the custom tab, as a code.
pub const SETTING_EDITOR: &str = "editor";
/// Setting key for the number of colors of generated puzzles: `1` or `2`.
pub const SETTING_COLORS: &str = "colors";

/// What a puzzle code in the address stands for. The shell keeps the two
/// apart (in the browser with different address prefixes); the UI only
/// sees the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeKind {
    /// A puzzle to play: a level if its size is one, else in the custom
    /// tab.
    Play,
    /// A puzzle open in the editor of the custom tab.
    Edit,
    /// A puzzle to play in the custom tab, whatever its size.
    Custom,
}

/// Everything the UI needs at start-up.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Width of the drawing area in logical pixels.
    pub width: f32,
    /// Height of the drawing area in logical pixels.
    pub height: f32,
    /// Parts of the area covered by notches, rounded corners or system
    /// bars; no control is placed there.
    pub insets: Insets,
    /// Random bits for puzzle seeds. The UI has no other source of
    /// randomness, so the same entropy and the same events give the same
    /// puzzles.
    pub entropy: u64,
    /// Settings saved earlier, as `(key, value)` pairs with keys from
    /// [`SETTING_KEYS`]. Unknown keys and values are ignored.
    pub settings: Vec<(String, String)>,
    /// The system language as a BCP 47 tag, e.g. `uk-UA` or `en-US`.
    pub system_language: String,
    /// A puzzle code to open instead of generating a puzzle.
    pub code: Option<(CodeKind, String)>,
    /// Whether the main input is a touch screen. This hides the keyboard
    /// help; how each pointer behaves depends on its own [`PointerKind`].
    pub touch: bool,
    /// How much solver work the UI may do.
    pub budgets: Budgets,
}

/// Solver work, in nodes (piece placements tried). The UI cannot measure
/// time, so a shell on a slow device may lower these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Budgets {
    /// Nodes per frame for anything that searches: the generator, hints and
    /// the solver of the custom tab.
    pub per_frame: u64,
    /// Nodes one hint may use in total.
    pub hint: u64,
    /// Nodes the solver of the custom tab may use before it stops and
    /// offers to continue; each continuation adds as many again.
    pub solve: u64,
}

impl Default for Budgets {
    fn default() -> Budgets {
        Budgets {
            per_frame: crate::app::NODES_PER_FRAME,
            hint: 1_000_000,
            solve: 2_000_000,
        }
    }
}

/// Safe-area insets: how far in from each edge the area is fully usable,
/// in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// From the top edge.
    pub top: f32,
    /// From the right edge.
    pub right: f32,
    /// From the bottom edge.
    pub bottom: f32,
    /// From the left edge.
    pub left: f32,
}

/// The kind of device behind a pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// A mouse or touchpad: the piece is held under the cursor.
    Mouse,
    /// A finger or pen: the dragged piece floats above the contact point and
    /// a short tap rotates.
    Touch,
}

/// A pointer position report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// Identifies one finger or the mouse for the duration of a press.
    pub id: u32,
    /// Horizontal position in logical pixels.
    pub x: f32,
    /// Vertical position in logical pixels.
    pub y: f32,
    /// Device kind.
    pub kind: PointerKind,
}

/// The keys the UI reacts to. Shells map them by physical key, so `R` works
/// in every keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Rotate.
    R,
    /// Switch the counter display.
    Tab,
    /// Cancel a drag.
    Escape,
    /// Ask for a hint.
    H,
}

/// Input from the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The drawing area changed size or safe-area insets (logical pixels).
    Resize {
        /// New width.
        width: f32,
        /// New height.
        height: f32,
        /// New safe-area insets.
        insets: Insets,
    },
    /// The primary button went down or a finger touched. Shells send only
    /// the primary mouse button.
    PointerDown(Pointer),
    /// A pointer moved, pressed or not.
    PointerMove(Pointer),
    /// The button or finger was released.
    PointerUp(Pointer),
    /// The platform took the pointer away (e.g. a system gesture).
    PointerCancel(Pointer),
    /// A key was pressed.
    Key(Key),
    /// The puzzle code in the address changed (e.g. a pasted link).
    CodeChanged(CodeKind, String),
    /// The result of an [`Effect::CopyLink`]: `true` if the link reached
    /// the clipboard.
    CopyFinished(bool),
}

/// The mouse cursor the UI wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cursor {
    /// The normal arrow.
    #[default]
    Default,
    /// A pointing hand, over buttons.
    Pointer,
    /// An open hand, over a piece that can be picked up.
    Grab,
    /// A closed hand, while dragging.
    Grabbing,
}

/// Something the shell must do on the UI's behalf.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Show this puzzle code in the address, without adding history.
    SetCode(CodeKind, String),
    /// Copy a shareable link to this puzzle code to the clipboard, then
    /// report the result with [`Event::CopyFinished`].
    CopyLink(CodeKind, String),
    /// Store a setting so it can be passed back in [`Config::settings`].
    SaveSetting {
        /// One of [`SETTING_KEYS`].
        key: &'static str,
        /// The value.
        value: String,
    },
    /// Change the mouse cursor.
    SetCursor(Cursor),
}
