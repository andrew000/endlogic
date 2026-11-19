//! Tests of the structural diagnosis.

use endlogic_core::diagnose::{Problem, diagnose};
use endlogic_core::format::parse;
use endlogic_core::{Axis, Color, Puzzle};

const C: Color = Color::DEFAULT;

fn puzzle(text: &str) -> Puzzle {
    parse(&format!("endlogic 1\n{text}")).unwrap()
}

fn sample(name: &str) -> Puzzle {
    let path = format!("{}/../../puzzles/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn sound_puzzles_have_no_problems() {
    for name in [
        "wln006-2.txt",
        "wlh9000-1.txt",
        "dead-cells.txt",
        "permutations-4.txt",
    ] {
        assert_eq!(diagnose(&sample(name)), vec![], "{name}");
    }
}

#[test]
fn line_sums_that_differ() {
    let p = puzzle("grid\n..\n..\nrows 2 1\ncols 1 1\npiece\n##\n");
    assert_eq!(
        diagnose(&p),
        vec![
            Problem::LineSums {
                color: C,
                rows: 3,
                cols: 2
            },
            Problem::Area {
                color: C,
                clues: 3,
                cells: 2
            },
        ]
    );
}

#[test]
fn area_that_differs_from_the_clues() {
    // The clues ask for 3 cells; the lock and the two pieces give 4.
    let p = puzzle("grid\n#.\n..\nrows 2 1\ncols 2 1\npiece\n##\npiece\n#\n");
    assert_eq!(
        diagnose(&p),
        vec![Problem::Area {
            color: C,
            clues: 3,
            cells: 4
        }]
    );
}

#[test]
fn a_line_asking_for_more_than_fits() {
    // Row 0 has one dead cell, so only two of its cells can be filled.
    let p = puzzle("grid\nx..\n...\nrows 3 0\ncols 1 1 1\npiece\n###\n");
    assert_eq!(
        diagnose(&p),
        vec![Problem::LineTooLong {
            color: C,
            axis: Axis::Row,
            index: 0,
            clue: 3,
            room: 2
        }]
    );
    // A column, with a lock counting as room.
    let p = puzzle("grid\n#.\nx.\nrows 1 1\ncols 2 0\npiece\n#\n");
    assert_eq!(
        diagnose(&p),
        vec![Problem::LineTooLong {
            color: C,
            axis: Axis::Column,
            index: 0,
            clue: 2,
            room: 1
        }]
    );
}

#[test]
fn a_line_with_more_locks_than_its_clue() {
    let p = puzzle("grid\n##.\n...\nrows 1 2\ncols 1 1 1\npiece\n#\n");
    assert_eq!(
        diagnose(&p),
        vec![Problem::TooManyLocks {
            color: C,
            axis: Axis::Row,
            index: 0,
            clue: 1,
            locks: 2
        }]
    );
    let p = puzzle("grid\n#.\n#.\nrows 1 2\ncols 1 2\npiece\n#\n");
    assert_eq!(
        diagnose(&p),
        vec![Problem::TooManyLocks {
            color: C,
            axis: Axis::Column,
            index: 0,
            clue: 1,
            locks: 2
        }]
    );
}

#[test]
fn no_pieces() {
    let p = puzzle("grid\n..\n..\nrows 0 0\ncols 0 0\n");
    assert_eq!(diagnose(&p), vec![Problem::NoPieces]);
}
