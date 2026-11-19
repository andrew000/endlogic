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
mod counters;
mod draw;
mod editor;
mod hint;
mod layout;
mod painter;
mod shell;
mod text;
mod theme;

pub use app::{
    App, Control, CounterMode, DRAG_THRESHOLD, NODES_PER_FRAME, TOUCH_DRAG_THRESHOLD, TOUCH_LIFT,
    TOUCH_SLACK, Toast,
};
pub use board::{Board, Line, LineState, PieceState, Turned};
pub use counters::{CounterView, MAX_TICKS, MAX_TWO_LINES, MIN_SQUARE, counter_view};
pub use editor::{
    ClueRef, DRAFT_SIDE, Draft, DraftError, EditControl, Editor, SOLUTION_LIMIT, SolveState,
};
pub use layout::{
    Arrangement, BAND, COUNTER_GAP, EditLayout, HelpRow, LOW_WIDE_HEIGHT, Layout, LayoutInput,
    MAX_TRAY_SHARE, MIN_TARGET, MIN_TRAY_CELL, Mode, Pager, PieceEditor, ResultPanel, Stepper,
    TRAY_MIN_SCALE, TRAY_SCALE, TextBlock, tab_label,
};
pub use painter::{Align, Font, FontFamily, Painter, Point, Rect, Rgba};
pub use shell::{
    Budgets, CodeKind, Config, Cursor, Effect, Event, Insets, Key, Pointer, PointerKind,
    SETTING_COUNTERS, SETTING_DIFFICULTY, SETTING_EDITOR, SETTING_KEYS, SETTING_LANG,
};
pub use text::{Lang, Text, difficulty_label, problem_text, solution_of, text};
