//! Text format, version 1 (single color). See `docs/FORMAT.md`.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use crate::color::Color;
use crate::grid::{Cell, Grid, MAX_GRID_SIZE};
use crate::puzzle::{MAX_PIECES, Piece, Puzzle};
use crate::shape::Shape;
use crate::solution::{Solution, VerifyError};

/// The format version this module reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// A structural problem in puzzle text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// One-based line number. For [`ParseErrorKind::UnexpectedEnd`] it is the
    /// number of lines in the input plus one.
    pub line: usize,
    /// What went wrong.
    pub kind: ParseErrorKind,
}

/// The kinds of [`ParseError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// The first significant line is not `endlogic <version>`.
    BadHeader,
    /// The header names a version other than [`FORMAT_VERSION`].
    UnsupportedVersion(String),
    /// The input ended before a required section.
    UnexpectedEnd {
        /// The section that was expected.
        expected: &'static str,
    },
    /// A line does not start the section expected at this point.
    UnexpectedLine {
        /// The section that was expected.
        expected: &'static str,
        /// The line that was found instead.
        found: String,
    },
    /// The `grid` section has no rows.
    EmptyGrid,
    /// A grid row contains a character other than `.`, `#` or `x`.
    InvalidGridChar {
        /// The offending character.
        ch: char,
        /// One-based column of the character within the row.
        column: usize,
    },
    /// A grid row's length differs from the first row's.
    RaggedGrid {
        /// Length of the first row.
        expected: usize,
        /// Length of this row.
        found: usize,
    },
    /// The grid is wider or taller than [`MAX_GRID_SIZE`].
    GridTooLarge,
    /// A clue is not a non-negative integer.
    InvalidNumber(String),
    /// A `rows` or `cols` line has the wrong number of values.
    WrongClueCount {
        /// `"rows"` or `"cols"`.
        section: &'static str,
        /// Grid height or width.
        expected: usize,
        /// Number of values on the line.
        found: usize,
    },
    /// A piece row contains a character other than `#` or `.`.
    InvalidPieceChar {
        /// The offending character.
        ch: char,
        /// One-based column of the character within the row.
        column: usize,
    },
    /// A piece block has no `#` cells.
    EmptyPiece,
    /// A piece's cells are not 4-connected.
    DisconnectedPiece,
    /// A piece is wider or taller than [`MAX_GRID_SIZE`].
    PieceTooLarge,
    /// More than [`MAX_PIECES`] pieces.
    TooManyPieces,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.kind)
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::BadHeader => f.write_str("expected header `endlogic <version>`"),
            ParseErrorKind::UnsupportedVersion(v) => write!(
                f,
                "unsupported format version `{v}`, expected {FORMAT_VERSION}"
            ),
            ParseErrorKind::UnexpectedEnd { expected } => {
                write!(f, "unexpected end of input, expected `{expected}`")
            }
            ParseErrorKind::UnexpectedLine { expected, found } => {
                write!(f, "expected `{expected}`, found `{found}`")
            }
            ParseErrorKind::EmptyGrid => f.write_str("grid has no rows"),
            ParseErrorKind::InvalidGridChar { ch, column } => write!(
                f,
                "invalid grid character `{ch}` at column {column}, expected `.`, `#` or `x`"
            ),
            ParseErrorKind::RaggedGrid { expected, found } => write!(
                f,
                "grid row has {found} cells, expected {expected} like the first row"
            ),
            ParseErrorKind::GridTooLarge => {
                write!(f, "grid is larger than {MAX_GRID_SIZE}x{MAX_GRID_SIZE}")
            }
            ParseErrorKind::InvalidNumber(s) => {
                write!(f, "`{s}` is not a non-negative integer")
            }
            ParseErrorKind::WrongClueCount {
                section,
                expected,
                found,
            } => write!(f, "`{section}` has {found} values, expected {expected}"),
            ParseErrorKind::InvalidPieceChar { ch, column } => write!(
                f,
                "invalid piece character `{ch}` at column {column}, expected `#` or `.`"
            ),
            ParseErrorKind::EmptyPiece => f.write_str("piece has no cells"),
            ParseErrorKind::DisconnectedPiece => f.write_str("piece is not connected"),
            ParseErrorKind::PieceTooLarge => {
                write!(f, "piece is larger than {MAX_GRID_SIZE}x{MAX_GRID_SIZE}")
            }
            ParseErrorKind::TooManyPieces => write!(f, "more than {MAX_PIECES} pieces"),
        }
    }
}

impl core::error::Error for ParseError {}

/// A puzzle that cannot be written in format version 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializeError {
    /// The puzzle uses a color other than [`Color::DEFAULT`]; version 1 is
    /// single-color.
    UnsupportedColor(Color),
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializeError::UnsupportedColor(color) => write!(
                f,
                "color {} cannot be written in format version {FORMAT_VERSION}",
                color.0
            ),
        }
    }
}

impl core::error::Error for SerializeError {}

const KEYWORDS: [&str; 5] = ["endlogic", "grid", "rows", "cols", "piece"];

/// A significant line: one-based number and trimmed text.
type Line<'a> = (usize, &'a str);

struct Lines<'a> {
    items: Vec<Line<'a>>,
    pos: usize,
    end_line: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Lines<'a> {
        let mut total = 0;
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            total = index + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            lines.push((index + 1, line));
        }
        Lines {
            items: lines,
            pos: 0,
            end_line: total + 1,
        }
    }

    fn peek(&self) -> Option<Line<'a>> {
        self.items.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Line<'a>> {
        let line = self.peek();
        self.pos += usize::from(line.is_some());
        line
    }

    fn end_error(&self, expected: &'static str) -> ParseError {
        ParseError {
            line: self.end_line,
            kind: ParseErrorKind::UnexpectedEnd { expected },
        }
    }

    /// Consumes a line consisting of exactly `keyword`, followed by arguments.
    fn expect_keyword(&mut self, keyword: &'static str) -> Result<(usize, &'a str), ParseError> {
        let (line, text) = self.next().ok_or_else(|| self.end_error(keyword))?;
        let mut parts = text.splitn(2, char::is_whitespace);
        if parts.next() != Some(keyword) {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::UnexpectedLine {
                    expected: keyword,
                    found: text.to_owned(),
                },
            });
        }
        Ok((line, parts.next().unwrap_or("").trim()))
    }

    /// Consumes lines up to the next keyword line.
    fn take_block(&mut self) -> Vec<Line<'a>> {
        let mut block = Vec::new();
        while let Some((line, text)) = self.peek() {
            let first = text.split_whitespace().next().unwrap_or("");
            if KEYWORDS.contains(&first) {
                break;
            }
            block.push((line, text));
            self.pos += 1;
        }
        block
    }
}

/// Parses puzzle text in format version 1.
///
/// All pieces and locks get [`Color::DEFAULT`]; the clues are stored for that
/// color.
///
/// # Errors
///
/// A [`ParseError`] with the line number of the first structural problem.
/// Logical inconsistencies (for example clue sums that do not match the piece
/// area) are not errors.
///
/// # Panics
///
/// Does not panic on any input: the internal `expect`s only restate checks
/// the parser has already made.
pub fn parse(text: &str) -> Result<Puzzle, ParseError> {
    let mut lines = Lines::new(text);

    let (line, header) = lines.next().ok_or_else(|| lines.end_error("endlogic"))?;
    let mut parts = header.split_whitespace();
    let (Some("endlogic"), Some(version), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(ParseError {
            line,
            kind: ParseErrorKind::BadHeader,
        });
    };
    if version.parse::<u32>().ok() != Some(FORMAT_VERSION) {
        return Err(ParseError {
            line,
            kind: ParseErrorKind::UnsupportedVersion(version.to_owned()),
        });
    }

    let grid = parse_grid(&mut lines)?;
    let rows = parse_clues(&mut lines, "rows", grid.height())?;
    let cols = parse_clues(&mut lines, "cols", grid.width())?;

    let mut pieces = Vec::new();
    while lines.peek().is_some() {
        let (line, args) = lines.expect_keyword("piece")?;
        reject_args(line, "piece", args)?;
        if pieces.len() == MAX_PIECES {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::TooManyPieces,
            });
        }
        pieces.push(parse_piece(line, &lines.take_block())?);
    }

    let puzzle = Puzzle::single_color(grid, pieces, rows, cols)
        .expect("parser checks clue counts and piece count");
    Ok(puzzle)
}

fn reject_args(line: usize, keyword: &'static str, args: &str) -> Result<(), ParseError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(ParseError {
            line,
            kind: ParseErrorKind::UnexpectedLine {
                expected: keyword,
                found: format!("{keyword} {args}"),
            },
        })
    }
}

fn parse_grid(lines: &mut Lines<'_>) -> Result<Grid, ParseError> {
    let (grid_line, args) = lines.expect_keyword("grid")?;
    reject_args(grid_line, "grid", args)?;
    let block = lines.take_block();
    let Some(&(_, first)) = block.first() else {
        return Err(ParseError {
            line: grid_line,
            kind: ParseErrorKind::EmptyGrid,
        });
    };
    let width = first.chars().count();
    let mut cells = Vec::new();
    for (index, &(line, text)) in block.iter().enumerate() {
        let error = |kind| ParseError { line, kind };
        let mut row_len = 0;
        for (column, ch) in text.chars().enumerate() {
            cells.push(match ch {
                '.' => Cell::Free,
                '#' => Cell::Lock(Color::DEFAULT),
                'x' => Cell::Dead,
                _ => {
                    return Err(error(ParseErrorKind::InvalidGridChar {
                        ch,
                        column: column + 1,
                    }));
                }
            });
            row_len += 1;
        }
        if row_len != width {
            return Err(error(ParseErrorKind::RaggedGrid {
                expected: width,
                found: row_len,
            }));
        }
        if width > MAX_GRID_SIZE || index >= MAX_GRID_SIZE {
            return Err(error(ParseErrorKind::GridTooLarge));
        }
    }
    Ok(Grid::from_cells(width, block.len(), cells).expect("parser checks grid dimensions"))
}

fn parse_clues(
    lines: &mut Lines<'_>,
    section: &'static str,
    expected: usize,
) -> Result<Vec<usize>, ParseError> {
    let (line, args) = lines.expect_keyword(section)?;
    let error = |kind| ParseError { line, kind };
    let values = args
        .split_whitespace()
        .map(|token| {
            token
                .parse::<usize>()
                .map_err(|_| error(ParseErrorKind::InvalidNumber(token.to_owned())))
        })
        .collect::<Result<Vec<usize>, ParseError>>()?;
    if values.len() != expected {
        return Err(error(ParseErrorKind::WrongClueCount {
            section,
            expected,
            found: values.len(),
        }));
    }
    Ok(values)
}

fn parse_piece(piece_line: usize, block: &[Line<'_>]) -> Result<Piece, ParseError> {
    let mut cells = Vec::new();
    for (row, &(line, text)) in block.iter().enumerate() {
        for (col, ch) in text.chars().enumerate() {
            match ch {
                '#' => cells.push((row, col)),
                '.' => {}
                _ => {
                    return Err(ParseError {
                        line,
                        kind: ParseErrorKind::InvalidPieceChar {
                            ch,
                            column: col + 1,
                        },
                    });
                }
            }
        }
    }
    let error = |kind| ParseError {
        line: piece_line,
        kind,
    };
    let shape = Shape::new(cells).map_err(|e| {
        error(match e {
            crate::ModelError::DisconnectedShape => ParseErrorKind::DisconnectedPiece,
            _ => ParseErrorKind::EmptyPiece,
        })
    })?;
    if shape.width() > MAX_GRID_SIZE || shape.height() > MAX_GRID_SIZE {
        return Err(error(ParseErrorKind::PieceTooLarge));
    }
    Ok(Piece::new(Color::DEFAULT, shape))
}

impl core::str::FromStr for Puzzle {
    type Err = ParseError;

    /// Same as [`parse`].
    fn from_str(text: &str) -> Result<Puzzle, ParseError> {
        parse(text)
    }
}

/// Writes a puzzle in format version 1, in canonical form: no comments or
/// blank lines, each piece as its bounding box.
///
/// `parse(&serialize(p)?)` returns a puzzle equal to `p`.
///
/// # Errors
///
/// [`SerializeError::UnsupportedColor`] if any piece, lock or clue uses a
/// color other than [`Color::DEFAULT`].
pub fn serialize(puzzle: &Puzzle) -> Result<String, SerializeError> {
    if let Some(&color) = puzzle.colors().iter().find(|&&c| c != Color::DEFAULT) {
        return Err(SerializeError::UnsupportedColor(color));
    }
    let grid = puzzle.grid();
    let mut out = String::new();
    let _ = writeln!(out, "endlogic {FORMAT_VERSION}");
    out.push_str("grid\n");
    for row in 0..grid.height() {
        for &cell in grid.row(row) {
            out.push(match cell {
                Cell::Free => '.',
                Cell::Lock(_) => '#',
                Cell::Dead => 'x',
            });
        }
        out.push('\n');
    }
    let line = |label: &str, values: &[usize]| {
        let mut s = String::from(label);
        for v in values {
            let _ = write!(s, " {v}");
        }
        s.push('\n');
        s
    };
    let rows: Vec<usize> = (0..grid.height())
        .map(|r| puzzle.row_clue(Color::DEFAULT, r))
        .collect();
    let cols: Vec<usize> = (0..grid.width())
        .map(|c| puzzle.col_clue(Color::DEFAULT, c))
        .collect();
    out.push_str(&line("rows", &rows));
    out.push_str(&line("cols", &cols));
    for piece in puzzle.pieces() {
        out.push_str("piece\n");
        out.push_str(&shape_to_text(&piece.shape));
    }
    Ok(out)
}

/// Draws a shape as rows of `#` and `.`, one line per row.
#[must_use]
pub fn shape_to_text(shape: &Shape) -> String {
    let mut out = String::new();
    for row in 0..shape.height() {
        for col in 0..shape.width() {
            out.push(if shape.contains(row, col) { '#' } else { '.' });
        }
        out.push('\n');
    }
    out
}

/// The letter used for piece `index` when drawing a solution: `A`–`Z` for the
/// first 26 pieces, then `a`–`f`.
///
/// # Panics
///
/// Panics if `index >= MAX_PIECES`.
#[must_use]
pub fn piece_letter(index: usize) -> char {
    assert!(index < MAX_PIECES, "piece index {index} out of range");
    let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef";
    char::from(letters[index])
}

/// Draws a solved grid: each piece's cells as [`piece_letter`], `#` for locks,
/// `x` for dead cells, `.` for empty free cells. One line per row, each ending
/// in `\n`.
///
/// # Errors
///
/// Any geometric [`VerifyError`] from [`Solution::cell_owners`].
pub fn render_solution(puzzle: &Puzzle, solution: &Solution) -> Result<String, VerifyError> {
    let owners = solution.cell_owners(puzzle)?;
    let grid = puzzle.grid();
    let mut out = String::new();
    for row in 0..grid.height() {
        for (col, &cell) in grid.row(row).iter().enumerate() {
            out.push(match (cell, owners[row * grid.width() + col]) {
                (_, Some(piece)) => piece_letter(piece),
                (Cell::Lock(_), None) => '#',
                (Cell::Dead, None) => 'x',
                (Cell::Free, None) => '.',
            });
        }
        out.push('\n');
    }
    Ok(out)
}
