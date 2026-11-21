//! Text format: version 1 (single color) and version 2 (up to four
//! colors). See `docs/FORMAT.md`.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use crate::color::{Color, MAX_COLORS};
use crate::grid::{Cell, Grid, MAX_GRID_SIZE};
use crate::puzzle::{LineClues, MAX_PIECES, Piece, Puzzle};
use crate::shape::Shape;
use crate::solution::{Solution, VerifyError};

/// The newest format version. [`parse`] also reads version 1, and
/// [`serialize`] writes version 1 whenever it can.
pub const FORMAT_VERSION: u32 = 2;

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
    /// The header names a version other than 1 or [`FORMAT_VERSION`].
    UnsupportedVersion(String),
    /// The `colors` line of version 2 does not give a number from 1 to 4.
    InvalidColorCount(String),
    /// A `rows`, `cols` or `piece` line of version 2 has no color letter, or
    /// something other than one of `a` to `d`.
    InvalidColor(String),
    /// A color letter past the number of colors the puzzle declares.
    UndeclaredColor(char),
    /// A second `rows` or `cols` line for the same color.
    DuplicateClues {
        /// `"rows"` or `"cols"`.
        section: &'static str,
        /// The color's letter.
        color: char,
    },
    /// A declared color has no `rows` or no `cols` line.
    MissingClues {
        /// `"rows"` or `"cols"`.
        section: &'static str,
        /// The color's letter.
        color: char,
    },
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
    /// A grid row of version 2 contains a character other than `.`, `x` or
    /// the letter of a declared color.
    InvalidColorGridChar {
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
                "unsupported format version `{v}`, expected 1 or {FORMAT_VERSION}"
            ),
            ParseErrorKind::InvalidColorCount(s) => {
                write!(f, "`{s}` is not a number of colors from 1 to {MAX_COLORS}")
            }
            ParseErrorKind::InvalidColor(s) if s.is_empty() => {
                f.write_str("missing color letter `a` to `d`")
            }
            ParseErrorKind::InvalidColor(s) => {
                write!(f, "`{s}` is not a color letter `a` to `d`")
            }
            ParseErrorKind::UndeclaredColor(letter) => {
                write!(f, "color `{letter}` is past the declared colors")
            }
            ParseErrorKind::DuplicateClues { section, color } => {
                write!(f, "second `{section} {color}` line")
            }
            ParseErrorKind::MissingClues { section, color } => {
                write!(f, "no `{section} {color}` line")
            }
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
            ParseErrorKind::InvalidColorGridChar { ch, column } => write!(
                f,
                "invalid grid character `{ch}` at column {column}, \
                 expected `.`, `x` or a declared color letter"
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

/// Lines that start a section in version 1.
const KEYWORDS_1: [&str; 5] = ["endlogic", "grid", "rows", "cols", "piece"];
/// Lines that start a section in version 2.
const KEYWORDS_2: [&str; 6] = ["endlogic", "colors", "grid", "rows", "cols", "piece"];

/// A significant line: one-based number and trimmed text.
type Line<'a> = (usize, &'a str);

struct Lines<'a> {
    items: Vec<Line<'a>>,
    pos: usize,
    end_line: usize,
    /// Version 2: locks are color letters and some lines carry a color.
    colored: bool,
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
            colored: false,
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

    /// The line number of the next line, or the end-of-input line number.
    fn here(&self) -> usize {
        self.peek().map_or(self.end_line, |(line, _)| line)
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
        let keywords: &[&str] = if self.colored {
            &KEYWORDS_2
        } else {
            &KEYWORDS_1
        };
        let mut block = Vec::new();
        while let Some((line, text)) = self.peek() {
            let first = text.split_whitespace().next().unwrap_or("");
            if keywords.contains(&first) {
                break;
            }
            block.push((line, text));
            self.pos += 1;
        }
        block
    }
}

/// Parses puzzle text in format version 1 or 2.
///
/// Version 1 gives every piece and lock [`Color::DEFAULT`] and stores the
/// clues for that color; version 2 stores clues for every declared color.
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
    match version.parse::<u32>().ok() {
        Some(1) => {}
        Some(2) => return parse_colored(&mut lines),
        _ => {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::UnsupportedVersion(version.to_owned()),
            });
        }
    }

    let grid = parse_grid(&mut lines, 1)?;
    let (line, args) = lines.expect_keyword("rows")?;
    let rows = parse_values(line, args, "rows", grid.height())?;
    let (line, args) = lines.expect_keyword("cols")?;
    let cols = parse_values(line, args, "cols", grid.width())?;
    let pieces = parse_pieces(&mut lines, 1)?;
    let puzzle = Puzzle::single_color(grid, pieces, rows, cols)
        .expect("parser checks clue counts and piece count");
    Ok(puzzle)
}

/// The rest of a version 2 text, after the header.
fn parse_colored(lines: &mut Lines<'_>) -> Result<Puzzle, ParseError> {
    lines.colored = true;
    let (line, args) = lines.expect_keyword("colors")?;
    let colors = match args.parse::<usize>() {
        Ok(n) if (1..=MAX_COLORS).contains(&n) => n,
        _ => {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::InvalidColorCount(args.to_owned()),
            });
        }
    };
    let grid = parse_grid(lines, colors)?;

    // `rows <color> ...` and `cols <color> ...` lines, in any order.
    let mut found: [[Option<Vec<usize>>; MAX_COLORS]; 2] = Default::default();
    while let Some((line, text)) = lines.peek() {
        let (axis, section, expected) = match text.split_whitespace().next() {
            Some("rows") => (0, "rows", grid.height()),
            Some("cols") => (1, "cols", grid.width()),
            _ => break,
        };
        let (_, args) = lines.expect_keyword(section)?;
        let (letter, values) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
        let color = read_color(line, letter, colors)?;
        let slot = &mut found[axis][color.index()];
        if slot.is_some() {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::DuplicateClues {
                    section,
                    color: letter_of(color),
                },
            });
        }
        *slot = Some(parse_values(line, values, section, expected)?);
    }
    let mut clues = BTreeMap::new();
    for color in &Color::ALL[..colors] {
        let rows = found[0][color.index()].take();
        let cols = found[1][color.index()].take();
        let missing = |section| ParseError {
            line: lines.here(),
            kind: ParseErrorKind::MissingClues {
                section,
                color: letter_of(*color),
            },
        };
        let rows = rows.ok_or_else(|| missing("rows"))?;
        let cols = cols.ok_or_else(|| missing("cols"))?;
        clues.insert(*color, LineClues { rows, cols });
    }

    let pieces = parse_pieces(lines, colors)?;
    let puzzle = Puzzle::new(grid, pieces, clues)
        .expect("parser checks clue counts, piece count and colors");
    Ok(puzzle)
}

/// The letter of a color a puzzle can use.
fn letter_of(color: Color) -> char {
    color.letter().unwrap_or('?')
}

/// Reads a color letter, one of the first `colors`.
fn read_color(line: usize, letter: &str, colors: usize) -> Result<Color, ParseError> {
    let mut chars = letter.chars();
    let color = match (chars.next(), chars.next()) {
        (Some(ch), None) => Color::from_letter(ch),
        _ => None,
    };
    let kind = match color {
        Some(color) if color.index() < colors => return Ok(color),
        Some(color) => ParseErrorKind::UndeclaredColor(letter_of(color)),
        None => ParseErrorKind::InvalidColor(letter.to_owned()),
    };
    Err(ParseError { line, kind })
}

/// Reads the `piece` blocks up to the end of the input.
fn parse_pieces(lines: &mut Lines<'_>, colors: usize) -> Result<Vec<Piece>, ParseError> {
    let mut pieces = Vec::new();
    while lines.peek().is_some() {
        let (line, args) = lines.expect_keyword("piece")?;
        let color = if lines.colored {
            read_color(line, args, colors)?
        } else {
            reject_args(line, "piece", args)?;
            Color::DEFAULT
        };
        if pieces.len() == MAX_PIECES {
            return Err(ParseError {
                line,
                kind: ParseErrorKind::TooManyPieces,
            });
        }
        pieces.push(parse_piece(line, &lines.take_block(), color)?);
    }
    Ok(pieces)
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

/// Reads the `grid` block: locks are `#` in version 1 and the letters of
/// the first `colors` colors in version 2.
fn parse_grid(lines: &mut Lines<'_>, colors: usize) -> Result<Grid, ParseError> {
    let (grid_line, args) = lines.expect_keyword("grid")?;
    reject_args(grid_line, "grid", args)?;
    let colored = lines.colored;
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
            let lock = Color::from_letter(ch).filter(|c| colored && c.index() < colors);
            cells.push(match (ch, lock) {
                ('.', _) => Cell::Free,
                ('#', _) if !colored => Cell::Lock(Color::DEFAULT),
                ('x', _) => Cell::Dead,
                (_, Some(color)) => Cell::Lock(color),
                _ if colored => {
                    return Err(error(ParseErrorKind::InvalidColorGridChar {
                        ch,
                        column: column + 1,
                    }));
                }
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

/// Reads the values of a `rows` or `cols` line: `args` is the text after
/// the keyword (and the color, in version 2).
fn parse_values(
    line: usize,
    args: &str,
    section: &'static str,
    expected: usize,
) -> Result<Vec<usize>, ParseError> {
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

fn parse_piece(piece_line: usize, block: &[Line<'_>], color: Color) -> Result<Piece, ParseError> {
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
    Ok(Piece::new(color, shape))
}

impl core::str::FromStr for Puzzle {
    type Err = ParseError;

    /// Same as [`parse`].
    fn from_str(text: &str) -> Result<Puzzle, ParseError> {
        parse(text)
    }
}

/// Writes a puzzle in canonical form: no comments or blank lines, each
/// piece as its bounding box. A puzzle that uses no color but
/// [`Color::DEFAULT`] is written in version 1; any other in version 2, with
/// as many colors as the highest color it uses needs.
///
/// `parse(&serialize(p))` returns a puzzle equal to `p` whenever `p` has
/// clues for every color up to the highest it uses, as every parsed,
/// generated or decoded puzzle does.
#[must_use]
pub fn serialize(puzzle: &Puzzle) -> String {
    let count = puzzle.colors().last().map_or(1, |last| last.index() + 1);
    let colored = count > 1;
    let grid = puzzle.grid();
    let mut out = String::new();
    if colored {
        let _ = writeln!(out, "endlogic {FORMAT_VERSION}");
        let _ = writeln!(out, "colors {count}");
    } else {
        out.push_str("endlogic 1\n");
    }
    out.push_str("grid\n");
    for row in 0..grid.height() {
        for &cell in grid.row(row) {
            out.push(match cell {
                Cell::Free => '.',
                Cell::Lock(color) if colored => letter_of(color),
                Cell::Lock(_) => '#',
                Cell::Dead => 'x',
            });
        }
        out.push('\n');
    }
    for (label, len) in [("rows", grid.height()), ("cols", grid.width())] {
        for &color in &Color::ALL[..count] {
            out.push_str(label);
            if colored {
                out.push(' ');
                out.push(letter_of(color));
            }
            for i in 0..len {
                let value = if label == "rows" {
                    puzzle.row_clue(color, i)
                } else {
                    puzzle.col_clue(color, i)
                };
                let _ = write!(out, " {value}");
            }
            out.push('\n');
        }
    }
    for piece in puzzle.pieces() {
        out.push_str("piece");
        if colored {
            out.push(' ');
            out.push(letter_of(piece.color));
        }
        out.push('\n');
        out.push_str(&shape_to_text(&piece.shape));
    }
    out
}

impl fmt::Display for Puzzle {
    /// The puzzle in canonical text, as [`serialize`] writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serialize(self))
    }
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

/// Draws the colors of a solved grid: the letter of each cell's color (`a`
/// to `d`), upper case for a lock, `x` for a dead cell and `.` for an empty
/// free cell. One line per row, each ending in `\n`.
///
/// # Errors
///
/// Any geometric [`VerifyError`] from [`Solution::cell_owners`].
pub fn render_colors(puzzle: &Puzzle, solution: &Solution) -> Result<String, VerifyError> {
    let owners = solution.cell_owners(puzzle)?;
    let grid = puzzle.grid();
    let mut out = String::new();
    for row in 0..grid.height() {
        for (col, &cell) in grid.row(row).iter().enumerate() {
            out.push(match (cell, owners[row * grid.width() + col]) {
                (_, Some(piece)) => letter_of(puzzle.pieces()[piece].color),
                (Cell::Lock(color), None) => letter_of(color).to_ascii_uppercase(),
                (Cell::Dead, None) => 'x',
                (Cell::Free, None) => '.',
            });
        }
        out.push('\n');
    }
    Ok(out)
}
