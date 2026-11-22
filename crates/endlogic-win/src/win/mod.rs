//! The parts that talk to Windows. Calling Win32 and COM needs `unsafe`;
//! each block is as small as it can be and says why it is sound.
#![allow(unsafe_code)]

mod image;
mod painter;
mod shot;
mod system;
mod window;

use std::process::ExitCode;

use crate::args::{self, Command, USAGE};

/// Runs the program with its command line.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    match args::parse(&args) {
        Ok(Command::Window { start }) => window::run(start.as_deref()),
        Ok(Command::Screenshot(shot)) => shot::run(&shot),
        Err(message) => {
            eprintln!("endlogic-win: {message}\nusage: {USAGE}");
            ExitCode::from(2)
        }
    }
}
