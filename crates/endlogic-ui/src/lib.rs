//! Platform-independent trainer for endlogic puzzles.
//!
//! This crate holds the whole look and behaviour of the trainer: game state,
//! input handling, layout, colors, texts and drawing. It knows nothing about
//! any platform. A shell (the browser page, a Win32 window) creates an
//! [`App`] from a [`Config`], forwards [`Event`]s, calls [`App::frame`] with
//! its [`Painter`] whenever a frame is needed, and carries out the
//! [`Effect`]s the app hands back. Time and randomness come from the shell.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod app;
mod board;
mod counters;
mod draw;
mod editor;
mod font;
mod hint;
mod ink;
mod layout;
mod link;
mod math;
mod painter;
mod shell;
mod text;
mod theme;

pub use app::{
    App, Control, CounterMode, DRAG_THRESHOLD, MAX_PLAY_COLORS, MIN_NODES_PER_FRAME,
    NODES_PER_FRAME, SMALL_FIELD, TOUCH_DRAG_THRESHOLD, TOUCH_LIFT, TOUCH_SLACK, Toast,
    frame_nodes,
};
pub use board::{Board, Line, LineState, PieceState, Turned};
pub use counters::{
    ACROSS, Counter, CounterView, EMPTY_DEPTH, LineCounters, MIN_TICK, TICK_LONG, TICK_PITCH,
    TICK_THICK, band_depth, line_counters, ticks_fit,
};
pub use draw::draw_icon;
pub use editor::{
    ClueRef, DRAFT_SIDE, Draft, DraftError, EditControl, Editor, SOLUTION_LIMIT, SolveState,
};
/// What a screenshot reading is not sure about, as the editor marks it.
pub use endlogic_vision::{Doubt, Place, Reason};
pub use font::{
    Align, FONT_SAMPLE_SIZE, Font, MIN_CAP_HEIGHT, MIN_FONT_SIZE, draw_font_sample, draw_text,
    has_glyph, repertoire, text_width,
};
pub use layout::{
    Arrangement, BAND, COUNTER_GAP, EditLayout, HelpRow, LOW_WIDE_HEIGHT, Layout, LayoutInput,
    MAX_TRAY_SHARE, MIN_TARGET, MIN_TRAY_CELL, Mode, Pager, PieceEditor, Prompt, ResultPanel,
    Stepper, TRAY_MIN_SCALE, TRAY_SCALE, TextBlock, tab_label,
};
pub use link::parse_link;
pub use painter::{Painter, Point, Rect, Rgba};
pub use shell::{
    Budgets, CodeKind, Config, Cursor, Effect, Event, Insets, Key, Pointer, PointerKind,
    SETTING_COLORS, SETTING_COUNTERS, SETTING_DIFFICULTY, SETTING_EDITOR, SETTING_KEYS,
    SETTING_LANG,
};
pub use text::{
    Lang, Text, color_name, difficulty_label, problem_line, problem_text, recognized, solution_of,
    text,
};
pub use theme::{BACKGROUND, PALETTES, Palette, palette};
