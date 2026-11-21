//! Tests for the text format parser and serializer.

use endlogic_core::format::{ParseError, ParseErrorKind, parse, render_colors, serialize};
use endlogic_core::solver::search;
use endlogic_core::{Cell, Color, Grid, LineClues, Piece, Puzzle, Shape};
use std::collections::BTreeMap;

const REFERENCE: &str = include_str!("../../../puzzles/wln006-2.txt");
const GAME_1: &str = include_str!("../../../puzzles/wla980-1.txt");
const GAME_2: &str = include_str!("../../../puzzles/wla980-2.txt");

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
fn from_str_matches_parse() {
    let puzzle: Puzzle = REFERENCE.parse().unwrap();
    assert_eq!(puzzle, parse(REFERENCE).unwrap());
    let err = "endlogic 1
grid
"
    .parse::<Puzzle>()
    .unwrap_err();
    assert_eq!(err.line, 2);
}

#[test]
fn reference_text_is_canonical() {
    let puzzle = parse(REFERENCE).unwrap();
    assert_eq!(
        serialize(&puzzle).replace("\r\n", "\n"),
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
    let canonical = serialize(&puzzle);
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
    assert_eq!(parse(&serialize(&puzzle)).unwrap(), puzzle);
}

#[test]
fn accepts_crlf_and_zero_pieces() {
    let puzzle = parse("endlogic 1\r\ngrid\r\n#.\r\nrows 1\r\ncols 1 0\r\n").unwrap();
    assert_eq!(puzzle.pieces(), []);
}

#[test]
fn error_bad_header() {
    assert_parse_error!("\n// c\nendlogik 1\ngrid\n", 3, ParseErrorKind::BadHeader);
    assert_parse_error!("endlogic\n", 1, ParseErrorKind::BadHeader);
}

#[test]
fn error_unsupported_version() {
    assert_parse_error!(
        "endlogic 3\ngrid\n.\n",
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

// ---- version 2 ----

/// A small two-color puzzle in version 2, starting with its header.
const TWO: &str = "endlogic 2\ncolors 2\ngrid\nab.\n.x.\nrows a 1 0\nrows b 1 1\ncols a 1 0 0\ncols b 0 1 1\npiece b\n#\n";

fn game(text: &str) -> Puzzle {
    parse(text).unwrap()
}

#[test]
fn game_samples_are_canonical_version_2() {
    for text in [GAME_1, GAME_2] {
        let puzzle = game(text);
        assert_eq!(puzzle.colors(), [Color(0), Color(1)]);
        assert_eq!(serialize(&puzzle), text.replace("\r\n", "\n"));
        assert_eq!(puzzle.to_string(), serialize(&puzzle));
        assert_eq!(parse(&serialize(&puzzle)).unwrap(), puzzle);
    }
}

#[test]
fn parses_game_sample_colors() {
    let puzzle = game(GAME_1);
    assert_eq!(puzzle.grid().get(3, 4), Some(Cell::Lock(Color(0))));
    assert_eq!(puzzle.grid().get(5, 2), Some(Cell::Lock(Color(0))));
    let orange = puzzle.clues_for(Color(1)).unwrap();
    assert_eq!(orange.rows, [0, 2, 3, 1, 2, 1]);
    assert_eq!(orange.cols, [0, 2, 3, 1, 2, 1]);
    let colors: Vec<Color> = puzzle.pieces().iter().map(|p| p.color).collect();
    assert_eq!(
        colors,
        [Color(1), Color(1), Color(1), Color(0), Color(0), Color(0)]
    );
}

/// Each game sample has exactly one solution, and its cells have the
/// colors seen in the game.
#[test]
fn game_samples_have_one_solution_with_the_game_colors() {
    let expected = [
        (GAME_1, "aaaaaa\nabbaa.\nabbbaa\naabaA.\naaaabb\na.A.b.\n"),
        (GAME_2, "aabb..\naxbaaa\nbbxabb\nbaaxb.\n.abbx.\n.ab..x\n"),
    ];
    for (text, colors) in expected {
        let puzzle = game(text);
        let result = search(&puzzle, 10, 1_000_000);
        assert_eq!(result.solutions.len(), 1);
        assert_eq!(
            render_colors(&puzzle, &result.solutions[0]).unwrap(),
            colors
        );
    }
}

#[test]
fn version_2_round_trips_three_and_four_colors() {
    let mut grid = Grid::new(4, 2).unwrap();
    grid.set(0, 0, Cell::Lock(Color(2)));
    grid.set(0, 1, Cell::Lock(Color(3)));
    grid.set(1, 3, Cell::Dead);
    let mono = Shape::new([(0, 0)]).unwrap();
    let pieces = vec![
        Piece::new(Color(1), mono.clone()),
        Piece::new(Color(0), Shape::new([(0, 0), (1, 0)]).unwrap()),
        Piece::new(Color(3), mono),
    ];
    let mut clues = BTreeMap::new();
    for (color, rows, cols) in [
        (0, vec![1, 1], vec![0, 0, 2, 0]),
        (1, vec![0, 1], vec![1, 0, 0, 0]),
        (2, vec![1, 0], vec![1, 0, 0, 0]),
        (3, vec![2, 0], vec![0, 1, 0, 1]),
    ] {
        clues.insert(Color(color), LineClues { rows, cols });
    }
    let puzzle = Puzzle::new(grid, pieces, clues).unwrap();
    let text = serialize(&puzzle);
    assert_eq!(
        text,
        "endlogic 2\ncolors 4\ngrid\ncd..\n...x\nrows a 1 1\nrows b 0 1\nrows c 1 0\nrows d 2 0\n\
         cols a 0 0 2 0\ncols b 1 0 0 0\ncols c 1 0 0 0\ncols d 0 1 0 1\n\
         piece b\n#\npiece a\n#\n#\npiece d\n#\n"
    );
    assert_eq!(parse(&text).unwrap(), puzzle);
    let three = parse(TWO).unwrap();
    assert_eq!(parse(&serialize(&three)).unwrap(), three);
}

#[test]
fn single_color_puzzles_are_written_in_version_1() {
    let one = parse("endlogic 2\ncolors 1\ngrid\na.\nrows a 1\ncols a 1 0\n").unwrap();
    assert_eq!(serialize(&one), "endlogic 1\ngrid\n#.\nrows 1\ncols 1 0\n");
    let reference = parse(REFERENCE).unwrap();
    assert!(serialize(&reference).starts_with("endlogic 1\n"));
}

#[test]
fn version_2_accepts_clue_lines_in_any_order_and_comments() {
    let text = "// c\nendlogic 2\ncolors 2\n\ngrid\nab.\n.x.\ncols b 0 1 1\n// c\nrows b 1 1\ncols a 1 0 0\nrows a 1 0\npiece b\n#\n";
    assert_eq!(parse(text).unwrap(), parse(TWO).unwrap());
}

#[test]
fn error_version_2_color_count() {
    for (bad, found) in [
        ("colors 0", "0"),
        ("colors 5", "5"),
        ("colors two", "two"),
        ("colors", ""),
    ] {
        let text = format!("endlogic 2\n{bad}\ngrid\n.\n");
        let err = parse_err(&text);
        assert_eq!(err.line, 2, "{bad}");
        assert_eq!(
            err.kind,
            ParseErrorKind::InvalidColorCount(found.to_owned())
        );
    }
    assert_parse_error!(
        "endlogic 2\ngrid\n.\n",
        2,
        ParseErrorKind::UnexpectedLine {
            expected: "colors",
            ..
        }
    );
    assert_parse_error!(
        "endlogic 2\n",
        2,
        ParseErrorKind::UnexpectedEnd { expected: "colors" }
    );
}

#[test]
fn error_version_2_grid() {
    // `#` is a lock only in version 1.
    assert_parse_error!(
        "endlogic 2\ncolors 2\ngrid\n..\n.#\n",
        5,
        ParseErrorKind::InvalidColorGridChar { ch: '#', column: 2 }
    );
    // A lock of a color that was not declared.
    assert_parse_error!(
        "endlogic 2\ncolors 2\ngrid\nc.\n",
        4,
        ParseErrorKind::InvalidColorGridChar { ch: 'c', column: 1 }
    );
    let err = parse_err("endlogic 2\ncolors 1\ngrid\n.b\n");
    assert_eq!(
        err.to_string(),
        "line 4: invalid grid character `b` at column 2, expected `.`, `x` or a declared color letter"
    );
}

#[test]
fn error_version_2_clue_colors() {
    let head = "endlogic 2\ncolors 2\ngrid\n..\n";
    assert_parse_error!(
        &format!("{head}rows 1\n"),
        5,
        ParseErrorKind::InvalidColor(_)
    );
    assert_parse_error!(
        &format!("{head}rows e 1\n"),
        5,
        ParseErrorKind::InvalidColor(_)
    );
    assert_parse_error!(
        &format!("{head}rows a 1\ncols c 1 0\n"),
        6,
        ParseErrorKind::UndeclaredColor('c')
    );
    assert_parse_error!(
        &format!("{head}rows a 1\nrows a 1\n"),
        6,
        ParseErrorKind::DuplicateClues {
            section: "rows",
            color: 'a'
        }
    );
    assert_parse_error!(
        &format!("{head}rows a 1\ncols a 1 0\nrows b 0\npiece a\n#\n"),
        8,
        ParseErrorKind::MissingClues {
            section: "cols",
            color: 'b'
        }
    );
    assert_parse_error!(
        &format!("{head}rows a 1\ncols a 1 0\n"),
        7,
        ParseErrorKind::MissingClues {
            section: "rows",
            color: 'b'
        }
    );
    assert_parse_error!(
        &format!("{head}rows a 1 1\n"),
        5,
        ParseErrorKind::WrongClueCount {
            section: "rows",
            expected: 1,
            found: 2
        }
    );
    assert_parse_error!(
        &format!("{head}cols b 1 x\n"),
        5,
        ParseErrorKind::InvalidNumber(_)
    );
}

#[test]
fn error_version_2_pieces() {
    let head = "endlogic 2\ncolors 2\ngrid\n..\nrows a 1\nrows b 1\ncols a 1 0\ncols b 0 1\n";
    assert_parse_error!(
        &format!("{head}piece\n#\n"),
        9,
        ParseErrorKind::InvalidColor(_)
    );
    assert_parse_error!(
        &format!("{head}piece a\n#\npiece ab\n#\n"),
        11,
        ParseErrorKind::InvalidColor(_)
    );
    assert_parse_error!(
        &format!("{head}piece d\n#\n"),
        9,
        ParseErrorKind::UndeclaredColor('d')
    );
    assert_parse_error!(
        &format!("{head}piece b\n#.\n.#\n"),
        9,
        ParseErrorKind::DisconnectedPiece
    );
    assert_parse_error!(
        &format!("{head}piece b\n#\ncolors 2\n"),
        11,
        ParseErrorKind::UnexpectedLine {
            expected: "piece",
            ..
        }
    );
    let err = parse_err(&format!("{head}piece\n#\n"));
    assert_eq!(err.to_string(), "line 9: missing color letter `a` to `d`");
}

#[test]
fn version_1_does_not_know_colors() {
    // In version 1 `colors` is not a keyword and letters are not locks.
    assert_parse_error!(
        "endlogic 1\ncolors 2\n",
        2,
        ParseErrorKind::UnexpectedLine {
            expected: "grid",
            ..
        }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\na.\n",
        3,
        ParseErrorKind::InvalidGridChar { ch: 'a', column: 1 }
    );
    assert_parse_error!(
        "endlogic 1\ngrid\n..\nrows 1\ncols 1 0\npiece a\n#\n",
        6,
        ParseErrorKind::UnexpectedLine {
            expected: "piece",
            ..
        }
    );
}
