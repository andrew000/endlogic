//! Platform-independent trainer for endlogic puzzles.
//!
//! This crate holds the whole look and behaviour of the trainer: game state,
//! input handling, layout, colors, texts and drawing. It knows nothing about
//! any platform. A shell (the browser page, a Win32 window) creates an
//! [`App`] from a [`Config`], forwards [`Event`]s, calls [`App::frame`] with
//! its [`Painter`] whenever a frame is needed, and carries out the
//! [`Effect`]s the app hands back. Time and randomness come from the shell.
#![forbid(unsafe_code)]

mod app;
mod board;
mod draw;
mod layout;
mod painter;
mod shell;
mod text;
mod theme;

pub use app::{App, Control, CounterMode, DRAG_THRESHOLD, TOUCH_LIFT, Toast};
pub use board::{Board, Line, LineState, PieceState, Turned};
pub use layout::{Layout, LayoutInput, MIN_TARGET};
pub use painter::{Align, Font, FontFamily, Painter, Point, Rect, Rgba};
pub use shell::{
    Config, Cursor, Effect, Event, Key, Pointer, PointerKind, SETTING_COUNTERS, SETTING_DIFFICULTY,
    SETTING_KEYS, SETTING_LANG,
};
pub use text::{Lang, Text, difficulty_label, text};
