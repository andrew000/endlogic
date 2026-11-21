//! Compact, URL-safe puzzle codes. See `docs/CODE.md`.
//!
//! A code holds the puzzle itself (grid, clues, pieces, colors), not a
//! generator seed, so it stays valid when the generator changes and works for
//! hand-made puzzles.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::color::Color;
use crate::error::Axis;
use crate::grid::{Cell, Grid, MAX_GRID_SIZE};
use crate::puzzle::{LineClues, MAX_PIECES, Piece, Puzzle};
use crate::shape::Shape;

/// Version byte written at the start of every code.
pub const CODE_VERSION: u8 = 1;

/// Largest number of distinct colors a version 1 code can hold.
pub const MAX_CODE_COLORS: usize = 8;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Why a puzzle cannot be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodeError {
    /// More than [`MAX_CODE_COLORS`] colors.
    TooManyColors(usize),
    /// A clue is larger than the length of its line.
    ClueOutOfRange {
        /// Color of the clue.
        color: Color,
        /// Row or column.
        axis: Axis,
        /// Index of the line.
        index: usize,
        /// The clue value.
        value: usize,
    },
    /// A piece's bounding box is larger than 16×16.
    PieceTooLarge {
        /// Index of the piece.
        piece: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::TooManyColors(n) => {
                write!(f, "{n} colors, a code holds at most {MAX_CODE_COLORS}")
            }
            EncodeError::ClueOutOfRange {
                color,
                axis,
                index,
                value,
            } => write!(
                f,
                "color {}: clue {value} of {axis} {index} is longer than the line",
                color.0
            ),
            EncodeError::PieceTooLarge { piece } => {
                write!(
                    f,
                    "piece {piece} is larger than {MAX_GRID_SIZE}x{MAX_GRID_SIZE}"
                )
            }
        }
    }
}

impl core::error::Error for EncodeError {}

/// Why a code cannot be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeError {
    /// The code is empty.
    Empty,
    /// A character outside the base64url alphabet, or a final character with
    /// stray bits; `position` counts characters from 0.
    InvalidCharacter {
        /// Position in the code.
        position: usize,
        /// The character.
        ch: char,
    },
    /// The length cannot come from base64url without padding.
    InvalidLength,
    /// The code ends before the puzzle does.
    Truncated,
    /// The version byte is not [`CODE_VERSION`].
    UnsupportedVersion(u8),
    /// The checksum does not match: the code was mistyped or cut.
    ChecksumMismatch,
    /// The checksum matches but the content is not a valid puzzle.
    InvalidData(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Empty => f.write_str("the code is empty"),
            DecodeError::InvalidCharacter { position, ch } => {
                write!(f, "invalid character `{ch}` at position {position}")
            }
            DecodeError::InvalidLength => f.write_str("the code has an impossible length"),
            DecodeError::Truncated => f.write_str("the code is too short"),
            DecodeError::UnsupportedVersion(v) => {
                write!(f, "unsupported code version {v}, expected {CODE_VERSION}")
            }
            DecodeError::ChecksumMismatch => {
                f.write_str("checksum mismatch: the code is damaged or incomplete")
            }
            DecodeError::InvalidData(what) => write!(f, "invalid puzzle data: {what}"),
        }
    }
}

impl core::error::Error for DecodeError {}

/// Encodes a puzzle as a base64url code (no padding).
///
/// # Errors
///
/// An [`EncodeError`] if the puzzle has more than [`MAX_CODE_COLORS`]
/// colors, a clue longer than its line, or a piece larger than 16×16.
pub fn encode(puzzle: &Puzzle) -> Result<String, EncodeError> {
    let grid = puzzle.grid();
    let (width, height) = (grid.width(), grid.height());
    let colors = puzzle.colors();
    if colors.len() > MAX_CODE_COLORS {
        return Err(EncodeError::TooManyColors(colors.len()));
    }
    let color_bits = index_bits(colors.len());
    let color_index = |color: Color| colors.binary_search(&color).unwrap_or(0);

    let mut w = BitWriter::default();
    w.write(width - 1, 4);
    w.write(height - 1, 4);
    // A puzzle without pieces, locks or clues still records one color.
    let palette: Vec<Color> = if colors.is_empty() {
        vec![Color::DEFAULT]
    } else {
        colors.clone()
    };
    w.write(palette.len() - 1, 3);
    for color in &palette {
        w.write(usize::from(color.0), 8);
    }
    for &color in &palette {
        let Some(clues) = puzzle.clues_for(color) else {
            w.write(0, 1);
            continue;
        };
        w.write(1, 1);
        for (axis, values, line) in [
            (Axis::Row, &clues.rows, width),
            (Axis::Column, &clues.cols, height),
        ] {
            for (index, &value) in values.iter().enumerate() {
                if value > line {
                    return Err(EncodeError::ClueOutOfRange {
                        color,
                        axis,
                        index,
                        value,
                    });
                }
                w.write(value, value_bits(line));
            }
        }
    }
    for &cell in grid.cells() {
        match cell {
            Cell::Free => w.write(0, 1),
            Cell::Lock(color) => {
                w.write(0b10, 2);
                w.write(color_index(color), color_bits);
            }
            Cell::Dead => w.write(0b11, 2),
        }
    }
    w.write(puzzle.pieces().len(), 6);
    for (index, piece) in puzzle.pieces().iter().enumerate() {
        let shape = &piece.shape;
        if shape.width() > MAX_GRID_SIZE || shape.height() > MAX_GRID_SIZE {
            return Err(EncodeError::PieceTooLarge { piece: index });
        }
        w.write(color_index(piece.color), color_bits);
        w.write(shape.width() - 1, 4);
        w.write(shape.height() - 1, 4);
        for row in 0..shape.height() {
            for col in 0..shape.width() {
                w.write(usize::from(shape.contains(row, col)), 1);
            }
        }
    }

    let mut bytes = vec![CODE_VERSION];
    bytes.extend(w.finish());
    let checksum = crc16(&bytes);
    bytes.extend(checksum.to_be_bytes());
    Ok(base64url_encode(&bytes))
}

/// Decodes a code made by [`encode`]. Surrounding whitespace is ignored.
///
/// # Errors
///
/// A [`DecodeError`] describing the first problem; never panics.
pub fn decode(code: &str) -> Result<Puzzle, DecodeError> {
    let code = code.trim();
    if code.is_empty() {
        return Err(DecodeError::Empty);
    }
    let bytes = base64url_decode(code)?;
    if bytes.len() < 3 {
        return Err(DecodeError::Truncated);
    }
    if bytes[0] != CODE_VERSION {
        return Err(DecodeError::UnsupportedVersion(bytes[0]));
    }
    let (body, checksum) = bytes.split_at(bytes.len() - 2);
    if crc16(body).to_be_bytes() != checksum {
        return Err(DecodeError::ChecksumMismatch);
    }
    read_puzzle(&mut BitReader::new(&body[1..]))
}

fn read_puzzle(r: &mut BitReader<'_>) -> Result<Puzzle, DecodeError> {
    let width = r.read(4)? + 1;
    let height = r.read(4)? + 1;
    let color_count = r.read(3)? + 1;
    let mut palette = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        let id = u8::try_from(r.read(8)?).map_err(|_| DecodeError::InvalidData("color id"))?;
        if usize::from(id) >= crate::MAX_COLORS {
            return Err(DecodeError::InvalidData("color id"));
        }
        palette.push(Color(id));
    }
    if palette.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DecodeError::InvalidData(
            "colors are not strictly increasing",
        ));
    }
    let color_bits = index_bits(color_count);
    let read_color = |r: &mut BitReader<'_>| -> Result<Color, DecodeError> {
        palette
            .get(r.read(color_bits)?)
            .copied()
            .ok_or(DecodeError::InvalidData("color index out of range"))
    };

    let mut clues = BTreeMap::new();
    for &color in &palette {
        if r.read(1)? == 0 {
            continue;
        }
        let mut read_line = |count: usize, line: usize| -> Result<Vec<usize>, DecodeError> {
            (0..count)
                .map(|_| {
                    let value = r.read(value_bits(line))?;
                    if value > line {
                        Err(DecodeError::InvalidData("clue longer than its line"))
                    } else {
                        Ok(value)
                    }
                })
                .collect()
        };
        let rows = read_line(height, width)?;
        let cols = read_line(width, height)?;
        clues.insert(color, LineClues { rows, cols });
    }

    let mut cells = Vec::with_capacity(width * height);
    for _ in 0..width * height {
        let free = r.read(1)? == 0;
        let cell = if free {
            Cell::Free
        } else {
            let dead = r.read(1)? == 1;
            if dead {
                Cell::Dead
            } else {
                Cell::Lock(read_color(r)?)
            }
        };
        cells.push(cell);
    }

    let piece_count = r.read(6)?;
    if piece_count > MAX_PIECES {
        return Err(DecodeError::InvalidData("too many pieces"));
    }
    let mut pieces = Vec::with_capacity(piece_count);
    for _ in 0..piece_count {
        let color = read_color(r)?;
        let shape_width = r.read(4)? + 1;
        let shape_height = r.read(4)? + 1;
        let mut shape_cells = Vec::new();
        for row in 0..shape_height {
            for col in 0..shape_width {
                if r.read(1)? == 1 {
                    shape_cells.push((row, col));
                }
            }
        }
        let shape = Shape::new(shape_cells).map_err(|_| DecodeError::InvalidData("piece shape"))?;
        if shape.width() != shape_width || shape.height() != shape_height {
            return Err(DecodeError::InvalidData(
                "piece has empty border rows or columns",
            ));
        }
        pieces.push(Piece::new(color, shape));
    }
    if !r.only_zero_padding_left() {
        return Err(DecodeError::InvalidData("trailing data"));
    }

    let grid =
        Grid::from_cells(width, height, cells).map_err(|_| DecodeError::InvalidData("grid"))?;
    let puzzle = Puzzle::new(grid, pieces, clues).map_err(|_| DecodeError::InvalidData("clues"))?;
    if puzzle.colors() != palette && !(puzzle.colors().is_empty() && palette == [Color::DEFAULT]) {
        return Err(DecodeError::InvalidData("unused color in palette"));
    }
    Ok(puzzle)
}

/// Bits needed to store an index below `count` (0 for a single value).
fn index_bits(count: usize) -> u32 {
    count.saturating_sub(1).bit_width()
}

/// Bits needed to store a value in `0..=max`.
fn value_bits(max: usize) -> u32 {
    max.bit_width()
}

/// CRC-16/CCITT-FALSE: polynomial 0x1021, initial value 0xFFFF.
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in bytes {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x1021
            };
        }
    }
    crc
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    used: u32,
}

impl BitWriter {
    /// Appends the low `bits` bits of `value`, most significant first.
    fn write(&mut self, value: usize, bits: u32) {
        for shift in (0..bits).rev() {
            if self.used % 8 == 0 {
                self.bytes.push(0);
            }
            if (value >> shift) & 1 == 1 {
                let last = self.bytes.len() - 1;
                self.bytes[last] |= 0x80 >> (self.used % 8);
            }
            self.used += 1;
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> BitReader<'a> {
        BitReader { bytes, position: 0 }
    }

    /// Reads `bits` bits, most significant first.
    fn read(&mut self, bits: u32) -> Result<usize, DecodeError> {
        let mut value = 0;
        for _ in 0..bits {
            let byte = self
                .bytes
                .get(self.position / 8)
                .ok_or(DecodeError::Truncated)?;
            let bit = (byte >> (7 - self.position % 8)) & 1;
            value = (value << 1) | usize::from(bit);
            self.position += 1;
        }
        Ok(value)
    }

    /// Whether everything after the current position is zero bits inside the
    /// current byte.
    fn only_zero_padding_left(&mut self) -> bool {
        let total = self.bytes.len() * 8;
        if total - self.position >= 8 {
            return false;
        }
        while self.position < total {
            if self.read(1) != Ok(0) {
                return false;
            }
        }
        true
    }
}

fn base64url_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &b)| acc | (u32::from(b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            let sextet = (n >> (18 - 6 * i)) & 0x3f;
            out.push(char::from(ALPHABET[sextet as usize]));
        }
    }
    out
}

fn base64url_decode(code: &str) -> Result<Vec<u8>, DecodeError> {
    let mut values = Vec::with_capacity(code.len());
    for (position, ch) in code.chars().enumerate() {
        let value = ALPHABET
            .iter()
            .position(|&a| char::from(a) == ch)
            .ok_or(DecodeError::InvalidCharacter { position, ch })?;
        values.push(u32::try_from(value).unwrap_or(0));
    }
    if values.len() % 4 == 1 {
        return Err(DecodeError::InvalidLength);
    }
    let mut bytes = Vec::with_capacity(values.len() * 3 / 4);
    for chunk in values.chunks(4) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &v)| acc | (v << (18 - 6 * i)));
        let produced = chunk.len() - 1;
        for i in 0..produced {
            bytes.push(n.to_be_bytes()[1 + i]);
        }
        // Bits below the last whole byte must be zero in a canonical code.
        let stray_bits = 24 - 8 * produced;
        if n & ((1 << stray_bits) - 1) != 0 {
            let position = code.chars().count() - 1;
            let ch = code.chars().last().unwrap_or('?');
            return Err(DecodeError::InvalidCharacter { position, ch });
        }
    }
    Ok(bytes)
}
