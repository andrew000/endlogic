//! Core library for endlogic: puzzle model, text format, solver and verifier.
//!
//! The crate has no dependencies and does no I/O, so it builds for
//! `wasm32-unknown-unknown`.
//!
//! A puzzle ([`Puzzle`]) is a [`Grid`] of free, locked and dead cells, a list
//! of [`Piece`]s and per-color [`LineClues`]. A [`Solution`] gives each piece
//! a [`Rotation`] and a position; [`verify`] checks it independently of the
//! solver.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod color;
mod error;
mod grid;
mod puzzle;
mod shape;
mod solution;

pub use color::Color;
pub use error::{Axis, ModelError};
pub use grid::{Cell, Grid, MAX_GRID_SIZE};
pub use puzzle::{LineClues, MAX_PIECES, Piece, Puzzle};
pub use shape::{Rotation, Shape};
pub use solution::{Placement, Solution, VerifyError, verify};

pub mod format;
pub mod rng;
pub mod solver;
