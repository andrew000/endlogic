//! Solver tests.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use endlogic_core::format::{parse, render_solution};
use endlogic_core::solver::{Count, count_solutions, find_solutions, solve};
use endlogic_core::{Cell, Color, Grid, LineClues, Piece, Puzzle, Shape, verify};

const REFERENCE: &str = include_str!("../../../puzzles/wln006-2.txt");

const REFERENCE_SOLUTION: &str = "\
.#####.
CCCEEEE
C..E..E
.DDEAA.
.DDEEA.
..BBB..
...B...
";

/// Builds a single-color puzzle from grid rows, clues and piece pictures.
fn puzzle(grid: &[&str], rows: &[usize], cols: &[usize], pieces: &[&[&str]]) -> Puzzle {
    let mut text = String::from("endlogic 1\ngrid\n");
    for row in grid {
        text.push_str(row);
        text.push('\n');
    }
    let join = |v: &[usize]| {
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    };
    writeln!(text, "rows {}\ncols {}", join(rows), join(cols)).unwrap();
    for piece in pieces {
        text.push_str("piece\n");
        for row in *piece {
            text.push_str(row);
            text.push('\n');
        }
    }
    parse(&text).unwrap()
}

/// `n` monominoes on an empty `n`×`n` board with every clue 1: one solution
/// per permutation, `n!` in total.
fn permutations(n: usize) -> Puzzle {
    let row = ".".repeat(n);
    let grid = vec![row.as_str(); n];
    let ones = vec![1; n];
    let mono: &[&str] = &["#"];
    puzzle(&grid, &ones, &ones, &vec![mono; n])
}

#[test]
fn reference_has_exactly_the_known_solution() {
    let puzzle = parse(REFERENCE).unwrap();
    assert_eq!(count_solutions(&puzzle, 1000), Count::Exact(1));
    let solution = solve(&puzzle).unwrap();
    assert_eq!(verify(&puzzle, &solution), Ok(()));
    let rendered = render_solution(&puzzle, &solution).unwrap();
    assert_eq!(rendered, REFERENCE_SOLUTION);
}

#[test]
fn no_solution_when_sums_disagree() {
    let text = REFERENCE.replace("rows 5 7 3 5 5 3 1", "rows 5 7 3 5 5 3 2");
    let puzzle = parse(&text).unwrap();
    assert_eq!(solve(&puzzle), None);
    assert_eq!(count_solutions(&puzzle, 10), Count::Exact(0));
}

#[test]
fn no_solution_when_sums_match_but_nothing_fits() {
    // Sums agree (2 = 2 = area), but the domino would need both its cells in
    // row 0 and both in column 0 at once.
    let p = puzzle(&["..", ".."], &[2, 0], &[2, 0], &[&["##"]]);
    assert_eq!(solve(&p), None);
    assert_eq!(count_solutions(&p, 10), Count::Exact(0));
}

#[test]
fn locks_count_towards_clues() {
    // The lock fills row 0 and column 0; the monomino must go to (1, 1).
    let p = puzzle(&["#.", ".."], &[1, 1], &[1, 1], &[&["#"]]);
    let solutions = find_solutions(&p, 10);
    assert!(!solutions.limit_reached);
    assert_eq!(solutions.solutions.len(), 1);
    assert_eq!(
        render_solution(&p, &solutions.solutions[0]).unwrap(),
        "#.\n.A\n"
    );
}

#[test]
fn locks_exceeding_a_clue_mean_no_solution() {
    let p = puzzle(&["##", ".."], &[1, 1], &[1, 1], &[]);
    assert_eq!(count_solutions(&p, 10), Count::Exact(0));
}

#[test]
fn counting_respects_the_limit() {
    let p = permutations(4);
    assert_eq!(count_solutions(&p, 1000), Count::Exact(24));
    assert_eq!(count_solutions(&p, 25), Count::Exact(24));
    assert_eq!(count_solutions(&p, 24), Count::LimitReached(24));
    assert_eq!(count_solutions(&p, 10), Count::LimitReached(10));
    assert_eq!(count_solutions(&p, 0), Count::LimitReached(0));
    assert!(!count_solutions(&p, 10).is_exact());
    assert_eq!(count_solutions(&p, 10).found(), 10);
}

#[test]
fn listing_respects_the_limit_and_is_deterministic() {
    let p = permutations(4);
    let some = find_solutions(&p, 5);
    assert!(some.limit_reached);
    assert_eq!(some.solutions.len(), 5);
    let all = find_solutions(&p, 100);
    assert!(!all.limit_reached);
    assert_eq!(all.solutions.len(), 24);
    assert_eq!(all.solutions[..5], some.solutions[..]);
    assert_eq!(find_solutions(&p, 100), all);
    for solution in &all.solutions {
        assert_eq!(verify(&p, solution), Ok(()));
    }
    let mut boards: Vec<String> = all
        .solutions
        .iter()
        .map(|s| render_solution(&p, s).unwrap())
        .collect();
    boards.sort();
    boards.dedup();
    assert_eq!(boards.len(), 24);
}

#[test]
fn identical_pieces_do_not_produce_duplicates() {
    // Two monominoes on a diagonal: two boards, not four.
    let p = puzzle(&["..", ".."], &[1, 1], &[1, 1], &[&["#"], &["#"]]);
    assert_eq!(count_solutions(&p, 100), Count::Exact(2));

    // Two dominoes filling a 2x2 board: both horizontal or both vertical.
    let p = puzzle(&["..", ".."], &[2, 2], &[2, 2], &[&["##"], &["##"]]);
    assert_eq!(count_solutions(&p, 100), Count::Exact(2));

    // The same L tromino given in two different rotations is still one type.
    let p = puzzle(
        &["...", "...", "..."],
        &[3, 3, 2],
        &[3, 2, 3],
        &[&["#.", "##"], &["##", "#."], &["#"], &["#"]],
    );
    let all = find_solutions(&p, 1000);
    assert!(!all.limit_reached);
    assert!(all.solutions.len() > 1, "{} solutions", all.solutions.len());
    // Pieces 0 and 1 are one type, 2 and 3 another. Describe each solution by
    // the sorted cell sets of each type, which ignores piece identity.
    let mut layouts: Vec<Vec<Vec<usize>>> = all
        .solutions
        .iter()
        .map(|s| {
            verify(&p, s).unwrap();
            let owners = s.cell_owners(&p).unwrap();
            let cells_of = |piece: usize| -> Vec<usize> {
                (0..owners.len())
                    .filter(|&i| owners[i] == Some(piece))
                    .collect()
            };
            let mut trominoes = vec![cells_of(0), cells_of(1)];
            let mut monominoes = vec![cells_of(2), cells_of(3)];
            trominoes.sort();
            monominoes.sort();
            trominoes.into_iter().chain(monominoes).collect()
        })
        .collect();
    let total = layouts.len();
    layouts.sort();
    layouts.dedup();
    assert_eq!(layouts.len(), total, "a layout appeared more than once");
}

#[test]
fn symmetric_orientations_do_not_produce_duplicates() {
    let square = puzzle(&["..", ".."], &[2, 2], &[2, 2], &[&["##", "##"]]);
    assert_eq!(count_solutions(&square, 100), Count::Exact(1));
    let domino = puzzle(&[".."], &[2], &[1, 1], &[&["##"]]);
    assert_eq!(count_solutions(&domino, 100), Count::Exact(1));
}

#[test]
fn dead_cells_are_never_covered_or_counted() {
    // The L tromino must avoid the dead corner; the dead cell does not count.
    let p = puzzle(&["x.", ".."], &[1, 2], &[1, 2], &[&["#.", "##"]]);
    let solution = solve(&p).unwrap();
    assert_eq!(render_solution(&p, &solution).unwrap(), "xA\nAA\n");
    assert_eq!(count_solutions(&p, 100), Count::Exact(1));

    // Clues that count the dead cell as filled cannot be met.
    let p = puzzle(&["x.", ".."], &[2, 2], &[2, 2], &[&["#.", "##"]]);
    assert_eq!(count_solutions(&p, 100), Count::Exact(0));

    // A dead cell splits the row; the tromino has nowhere to go.
    let p = puzzle(&[".x.."], &[3], &[1, 0, 1, 1], &[&["###"]]);
    assert_eq!(count_solutions(&p, 100), Count::Exact(0));
}

#[test]
fn empty_puzzle_with_zero_clues_is_solved() {
    let p = puzzle(&[".."], &[0], &[0, 0], &[]);
    let solution = solve(&p).unwrap();
    assert_eq!(solution.placements, []);
    assert_eq!(count_solutions(&p, 10), Count::Exact(1));
}

#[test]
fn colors_are_counted_separately() {
    let grid = Grid::new(2, 1).unwrap();
    let mono = Shape::new([(0, 0)]).unwrap();
    let pieces = vec![
        Piece::new(Color(0), mono.clone()),
        Piece::new(Color(1), mono),
    ];
    let mut clues = BTreeMap::new();
    clues.insert(
        Color(0),
        LineClues {
            rows: vec![1],
            cols: vec![0, 1],
        },
    );
    clues.insert(
        Color(1),
        LineClues {
            rows: vec![1],
            cols: vec![1, 0],
        },
    );
    let p = Puzzle::new(grid.clone(), pieces.clone(), clues).unwrap();
    assert_eq!(count_solutions(&p, 10), Count::Exact(1));
    let solution = solve(&p).unwrap();
    assert_eq!(solution.placements[0].col, 1);
    assert_eq!(solution.placements[1].col, 0);
    assert_eq!(verify(&p, &solution), Ok(()));

    // A color without clues must stay empty.
    let mut clues = BTreeMap::new();
    clues.insert(
        Color(0),
        LineClues {
            rows: vec![2],
            cols: vec![1, 1],
        },
    );
    let p = Puzzle::new(grid, pieces, clues).unwrap();
    assert_eq!(count_solutions(&p, 10), Count::Exact(0));
}

#[test]
fn locked_cell_of_reference_is_kept() {
    let puzzle = parse(REFERENCE).unwrap();
    let solution = solve(&puzzle).unwrap();
    let owners = solution.cell_owners(&puzzle).unwrap();
    for (col, owner) in owners.iter().enumerate().take(6).skip(1) {
        assert_eq!(puzzle.grid().get(0, col), Some(Cell::Lock(Color::DEFAULT)));
        assert_eq!(*owner, None);
    }
}
