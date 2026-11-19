//! Tests for shapes, model constructors and the independent verifier.

use endlogic_core::format::{parse, shape_to_text};
use endlogic_core::{
    Axis, Cell, Color, Grid, ModelError, Piece, Placement, Puzzle, Rotation, Shape, Solution,
    VerifyError, verify,
};

/// Parses a picture of `#` and `.` rows into a shape.
fn shape(rows: &[&str]) -> Shape {
    let cells = rows.iter().enumerate().flat_map(|(r, row)| {
        row.chars()
            .enumerate()
            .filter(|&(_, ch)| ch == '#')
            .map(move |(c, _)| (r, c))
    });
    Shape::new(cells).unwrap()
}

fn orientation_count(rows: &[&str]) -> usize {
    shape(rows).orientations().len()
}

fn at(rotation: Rotation, row: usize, col: usize) -> Placement {
    Placement { rotation, row, col }
}

#[test]
fn orientation_counts() {
    assert_eq!(orientation_count(&["#"]), 1);
    assert_eq!(orientation_count(&["##", "##"]), 1);
    assert_eq!(orientation_count(&["###"]), 2);
    assert_eq!(orientation_count(&["#.", "#.", "##"]), 4);
    assert_eq!(orientation_count(&["###", ".#."]), 4);
    assert_eq!(orientation_count(&[".##", "##."]), 2);
    assert_eq!(orientation_count(&[".#.", "###", ".#."]), 1);
}

#[test]
fn orientations_are_distinct_and_start_with_the_original() {
    let l = shape(&["#.", "#.", "##"]);
    let orientations = l.orientations();
    assert_eq!(orientations[0], (Rotation::R0, l.clone()));
    let rotations: Vec<Rotation> = orientations.iter().map(|(r, _)| *r).collect();
    assert_eq!(rotations, Rotation::ALL);
    for (i, (_, a)) in orientations.iter().enumerate() {
        for (_, b) in &orientations[i + 1..] {
            assert_ne!(a, b);
        }
    }
    let line = shape(&["###"]).orientations();
    assert_eq!(line[0].0, Rotation::R0);
    assert_eq!(line[1].0, Rotation::R90);
}

#[test]
fn mirror_images_never_appear() {
    let l = shape(&["#.", "#.", "##"]);
    let j = shape(&[".#", ".#", "##"]);
    for (_, a) in l.orientations() {
        for (_, b) in j.orientations() {
            assert_ne!(a, b, "L and J share an orientation");
        }
    }
    let s = shape(&[".##", "##."]);
    let z = shape(&["##.", ".##"]);
    for (_, a) in s.orientations() {
        assert!(z.orientations().iter().all(|(_, b)| *b != a));
    }
}

#[test]
fn rotation_is_clockwise() {
    let l = shape(&["#.", "#.", "##"]);
    assert_eq!(shape_to_text(&l.rotated(Rotation::R90)), "###\n#..\n");
    assert_eq!(shape_to_text(&l.rotated(Rotation::R180)), "##\n.#\n.#\n");
    assert_eq!(shape_to_text(&l.rotated(Rotation::R270)), "..#\n###\n");
    assert_eq!(l.rotated(Rotation::R0), l);
}

#[test]
fn shapes_are_normalized() {
    let s = Shape::new([(5, 7), (5, 8), (6, 8), (5, 7)]).unwrap();
    assert_eq!(s.cells(), [(0, 0), (0, 1), (1, 1)]);
    assert_eq!((s.width(), s.height(), s.area()), (2, 2, 3));
    assert!(s.contains(1, 1));
    assert!(!s.contains(1, 0));
}

#[test]
fn shape_errors() {
    assert_eq!(Shape::new([]), Err(ModelError::EmptyShape));
    assert_eq!(
        Shape::new([(0, 0), (1, 1)]),
        Err(ModelError::DisconnectedShape)
    );
}

#[test]
fn max_line_span() {
    assert_eq!(shape(&["#"]).max_line_span(), 1);
    assert_eq!(shape(&["#.", "#.", "##"]).max_line_span(), 3);
    assert_eq!(shape(&["####", "#..#", "...#", "..##"]).max_line_span(), 4);
}

#[test]
fn grid_errors() {
    assert_eq!(
        Grid::new(0, 3),
        Err(ModelError::GridSize {
            width: 0,
            height: 3
        })
    );
    assert!(Grid::new(16, 16).is_ok());
    assert!(Grid::new(17, 1).is_err());
    assert_eq!(
        Grid::from_cells(2, 2, vec![Cell::Free; 3]),
        Err(ModelError::CellCount {
            expected: 4,
            found: 3
        })
    );
}

#[test]
fn puzzle_errors() {
    let grid = Grid::new(2, 3).unwrap();
    assert_eq!(
        Puzzle::single_color(grid.clone(), Vec::new(), vec![0, 0], vec![0, 0]),
        Err(ModelError::ClueLength {
            color: Color::DEFAULT,
            axis: Axis::Row,
            expected: 3,
            found: 2
        })
    );
    let mono = Piece::new(Color::DEFAULT, shape(&["#"]));
    assert_eq!(
        Puzzle::single_color(grid, vec![mono; 33], vec![0; 3], vec![0; 2]),
        Err(ModelError::TooManyPieces(33))
    );
}

/// A 3×3 board with a lock at (0, 0) and a dead cell at (2, 1); pieces are a
/// monomino (0) and a horizontal domino (1).
fn small() -> Puzzle {
    parse("endlogic 1\ngrid\n#..\n...\n.x.\nrows 1 3 0\ncols 2 1 1\npiece\n#\npiece\n##\n").unwrap()
}

#[test]
fn verify_accepts_valid_solution_with_rotation() {
    let p = small();
    let ok = Solution {
        placements: vec![at(Rotation::R0, 1, 0), at(Rotation::R180, 1, 1)],
    };
    assert_eq!(verify(&p, &ok), Ok(()));
}

#[test]
fn verify_rejects_wrong_placement_count() {
    let bad = Solution {
        placements: vec![at(Rotation::R0, 1, 0)],
    };
    assert_eq!(
        verify(&small(), &bad),
        Err(VerifyError::PlacementCount {
            expected: 2,
            found: 1
        })
    );
}

#[test]
fn verify_rejects_out_of_bounds() {
    let p = small();
    let bad = Solution {
        placements: vec![at(Rotation::R0, 1, 0), at(Rotation::R0, 1, 2)],
    };
    assert_eq!(verify(&p, &bad), Err(VerifyError::OutOfBounds { piece: 1 }));
    // Rotating the domino upright at row 2 runs off the bottom.
    let bad = Solution {
        placements: vec![at(Rotation::R0, 1, 0), at(Rotation::R90, 2, 2)],
    };
    assert_eq!(verify(&p, &bad), Err(VerifyError::OutOfBounds { piece: 1 }));
    let bad = Solution {
        placements: vec![at(Rotation::R0, usize::MAX, 0), at(Rotation::R0, 1, 1)],
    };
    assert_eq!(verify(&p, &bad), Err(VerifyError::OutOfBounds { piece: 0 }));
}

#[test]
fn verify_rejects_locked_cell() {
    let bad = Solution {
        placements: vec![at(Rotation::R0, 0, 0), at(Rotation::R0, 1, 1)],
    };
    assert_eq!(
        verify(&small(), &bad),
        Err(VerifyError::OnLockedCell {
            piece: 0,
            row: 0,
            col: 0
        })
    );
}

#[test]
fn verify_rejects_dead_cell() {
    let bad = Solution {
        placements: vec![at(Rotation::R0, 1, 0), at(Rotation::R0, 2, 0)],
    };
    assert_eq!(
        verify(&small(), &bad),
        Err(VerifyError::OnDeadCell {
            piece: 1,
            row: 2,
            col: 1
        })
    );
}

#[test]
fn verify_rejects_overlap() {
    let bad = Solution {
        placements: vec![at(Rotation::R0, 1, 1), at(Rotation::R0, 1, 0)],
    };
    assert_eq!(
        verify(&small(), &bad),
        Err(VerifyError::Overlap {
            first: 0,
            second: 1,
            row: 1,
            col: 1
        })
    );
}

#[test]
fn verify_rejects_wrong_counts() {
    let p = small();
    // Geometrically fine, but row 1 gets only 2 cells and row 0 gets 2.
    let bad = Solution {
        placements: vec![at(Rotation::R0, 0, 1), at(Rotation::R0, 1, 0)],
    };
    assert_eq!(
        verify(&p, &bad),
        Err(VerifyError::LineCount {
            color: Color::DEFAULT,
            axis: Axis::Row,
            index: 0,
            expected: 1,
            found: 2
        })
    );
    // Row counts match but column counts do not.
    let p = parse(
        "endlogic 1
grid
..
..
rows 1 0
cols 0 1
piece
#
",
    )
    .unwrap();
    let bad = Solution {
        placements: vec![at(Rotation::R0, 0, 0)],
    };
    assert_eq!(
        verify(&p, &bad),
        Err(VerifyError::LineCount {
            color: Color::DEFAULT,
            axis: Axis::Column,
            index: 0,
            expected: 0,
            found: 1
        })
    );
}
