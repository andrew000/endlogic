//! Tests for the text format parser and serializer.

use endlogic_core::format::{ParseError, ParseErrorKind, SerializeError, parse, serialize};
use endlogic_core::{Cell, Color, Grid, Piece, Puzzle, Shape};

const REFERENCE: &str = include_str!("../../../puzzles/wln006-2.txt");

fn parse_err(text: &str) -> ParseError {
    parse(text).expect_err("input should be rejected")
}

/// Asserts that `text` fails on `line` with an error matching `pattern`.
macro_rules! assert_parse_error {
    ($text:expr, $line:expr, $pattern:pat) => {{
        let err = parse_err($text);
        assert_eq!(err.line, $line, "wrong line for {err}");
        assert!(matches!(err.kind, $pattern), "unexpected error {err}");
    }};
}

#[test]
fn parses_reference_puzzle() {
    let puzzle = parse(REFERENCE).unwrap();
    assert_eq!(puzzle.grid().width(), 7);
    assert_eq!(puzzle.grid().height(), 7);
    assert_eq!(puzzle.pieces().len(), 5);
    assert_eq!(puzzle.grid().get(0, 1), Some(Cell::Lock(Color::DEFAULT)));
    assert_eq!(puzzle.grid().get(0, 0), Some(Cell::Free));
    let clues = puzzle.clues_for(Color::DEFAULT).unwrap();
    assert_eq!(clues.rows, [5, 7, 3, 5, 5, 3, 1]);
    assert_eq!(clues.cols, [2, 4, 5, 7, 5, 4, 2]);
    assert_eq!(puzzle.pieces()[4].shape.area(), 9);
}

#[test]
fn reference_text_is_canonical() {
    let puzzle = parse(REFERENCE).unwrap();
    assert_eq!(
        serialize(&puzzle).unwrap().replace("\r\n", "\n"),
        REFERENCE.replace("\r\n", "\n")
    );
}

#[test]
fn round_trip_with_comments_dead_cells_and_padding() {
    let text = "\
// leading comment

  endlogic 1
grid
x#..
.x..
// comment inside the grid block
....
rows 1 0 2
cols 0 1 1 1

piece
....
.##.

piece
#
";
    let puzzle = parse(text).unwrap();
    assert_eq!(puzzle.grid().get(0, 0), Some(Cell::Dead));
    assert_eq!(puzzle.grid().get(1, 1), Some(Cell::Dead));
    assert_eq!(puzzle.pieces()[0].shape.cells(), [(0, 0), (0, 1)]);
    let canonical = serialize(&puzzle).unwrap();
    assert_eq!(
        canonical,
        "endlogic 1\ngrid\nx#..\n.x..\n....\nrows 1 0 2\ncols 0 1 1 1\npiece\n##\npiece\n#\n"
    );
    assert_eq!(parse(&canonical).unwrap(), puzzle);
}

#[test]
fn round_trip_of_constructed_puzzle() {
    let mut grid = Grid::new(3, 2).unwrap();
    grid.set(1, 2, Cell::Dead);
    grid.set(0, 0, Cell::Lock(Color::DEFAULT));
    let pieces = vec![Piece::new(
        Color::DEFAULT,
        Shape::new([(0, 1), (1, 1), (1, 0)]).unwrap(),
    )];
    let puzzle = Puzzle::single_color(grid, pieces, vec![2, 2], vec![2, 2, 0]).unwrap();
    assert_eq!(parse(&serialize(&puzzle).unwrap()).unwrap(), puzzle);
}

#[test]
fn accepts_crlf_and_zero_pieces() {
    let puzzle = parse("endlogic 1\r\ngrid\r\n#.\r\nrows 1\r\ncols 1 0\r\n").unwrap();
    assert_eq!(puzzle.pieces(), []);
}

#[test]
fn serializer_rejects_other_colors() {
    let grid = Grid::from_cells(1, 1, vec![Cell::Lock(Color(1))]).unwrap();
    let puzzle = Puzzle::single_color(grid, Vec::new(), vec![0], vec![0]).unwrap();
    assert_eq!(
        serialize(&puzzle),
        Err(SerializeError::UnsupportedColor(Color(1)))
    );
}

#[test]
fn error_bad_header() {
    assert_parse_error!("\n// c\nendlogik 1\ngrid\n", 3, ParseErrorKind::BadHeader);
    assert_parse_error!("endlogic\n", 1, ParseErrorKind::BadHeader);
}

#[test]
fn error_unsupported_version() {
    assert_parse_error!(
        "endlogic 2\ngrid\n.\n",
        1,
        ParseErrorKind::UnsupportedVersion(_)
    );
}

#[test]
fn error_unexpected_end() {
    assert_parse_error!(
        "",
        1,
        ParseErrorKind::UnexpectedEnd {
            expected: "endlogic"
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 0\n",
        5,
        ParseErrorKind::UnexpectedEnd { expected: "cols" }
    );
}

#[test]
fn error_unexpected_line() {
    assert_parse_error!(
        "endlogic 1\nrows 1\n",
        2,
        ParseErrorKind::UnexpectedLine {
            expected: "grid",
            ..
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n.\ncols 0\nrows 0\n",
        4,
        ParseErrorKind::UnexpectedLine {
            expected: "rows",
            ..
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n.\nrows 0\ncols 0\npiece\n#\ngrid\n",
        8,
        ParseErrorKind::UnexpectedLine {
            expected: "piece",
            ..
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n.\nrows 0\ncols 0\npiece 2\n#\n",
        6,
        ParseErrorKind::UnexpectedLine {
            expected: "piece",
            ..
        }
    );
}

#[test]
fn error_empty_grid() {
    assert_parse_error!(
        "endlogic 1\n\ngrid\nrows\ncols\n",
        3,
        ParseErrorKind::EmptyGrid
    );
}

#[test]
fn error_invalid_grid_char() {
    assert_parse_error!(
        "endlogic 1\ngrid\n...\n.o.\n",
        4,
        ParseErrorKind::InvalidGridChar { ch: 'o', column: 2 }
    );
}

#[test]
fn error_ragged_grid() {
    assert_parse_error!(
        "endlogic 1\ngrid\n...\n....\n",
        4,
        ParseErrorKind::RaggedGrid {
            expected: 3,
            found: 4
        }
    );
}

#[test]
fn error_grid_too_large() {
    let wide = format!("endlogic 1\ngrid\n{}\n", ".".repeat(17));
    assert_parse_error!(&wide, 3, ParseErrorKind::GridTooLarge);
    let tall = format!("endlogic 1\ngrid\n{}", ".\n".repeat(17));
    assert_parse_error!(&tall, 19, ParseErrorKind::GridTooLarge);
}

#[test]
fn error_invalid_number() {
    let err = parse_err("endlogic 1\ngrid\n..\nrows 1\ncols 1 -1\n");
    assert_eq!(err.line, 5);
    assert_eq!(err.kind, ParseErrorKind::InvalidNumber("-1".to_owned()));
}

#[test]
fn error_wrong_clue_count() {
    assert_parse_error!(
        "endlogic 1\ngrid\n..\n..\nrows 1\n",
        5,
        ParseErrorKind::WrongClueCount {
            section: "rows",
            expected: 2,
            found: 1
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0 0\n",
        5,
        ParseErrorKind::WrongClueCount {
            section: "cols",
            expected: 2,
            found: 3
        }
    );
}

#[test]
fn error_invalid_piece_char() {
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece\n#\n#x\n",
        8,
        ParseErrorKind::InvalidPieceChar { ch: 'x', column: 2 }
    );
}

#[test]
fn error_empty_piece() {
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece\n..\npiece\n#\n",
        6,
        ParseErrorKind::EmptyPiece
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece\n#\npiece\n",
        8,
        ParseErrorKind::EmptyPiece
    );
}

#[test]
fn error_disconnected_piece() {
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece\n#\npiece\n#.\n.#\n",
        8,
        ParseErrorKind::DisconnectedPiece
    );
}

#[test]
fn error_piece_too_large() {
    let text = format!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece\n{}\n",
        "#".repeat(17)
    );
    assert_parse_error!(&text, 6, ParseErrorKind::PieceTooLarge);
}

#[test]
fn error_too_many_pieces() {
    let mut text = String::from("endlogic 1\ngrid\n..\nrows 1\ncols 1 0\n");
    for _ in 0..33 {
        text.push_str("piece\n#\n");
    }
    // Pieces start on line 6 and take two lines each; the 33rd is on 6 + 2 * 32.
    assert_parse_error!(&text, 70, ParseErrorKind::TooManyPieces);
}

#[test]
fn errors_display_line_numbers() {
    let err = parse_err("endlogic 1\ngrid\n.?\n");
    assert_eq!(
        err.to_string(),
        "line 3: invalid grid character `?` at column 2, expected `.`, `#` or `x`"
    );
}
