//! Windows shell for the endlogic trainer: a Win32 window drawn with
//! Direct2D. The game itself (state, layout, colors, texts and the font)
//! lives in `endlogic-ui`; this program passes events in, draws frames and
//! carries out effects.
//!
//! On other platforms it only prints that it runs on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg_attr(not(windows), allow(dead_code))]
mod args;
#[cfg_attr(not(windows), allow(dead_code))]
mod keys;
#[cfg_attr(not(windows), allow(dead_code))]
mod link;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    win::main()
}

#[cfg(not(windows))]
fn main() {
    println!("endlogic-win runs on Windows only.");
}
