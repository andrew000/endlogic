//! Solver tests.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use endlogic_core::format::{parse, render_solution};
use endlogic_core::solver::{
    DEFAULT_NODE_BUDGET, Progress, Search, SearchResult, StopReason, Uniqueness, check_uniqueness,
    search,
};
use endlogic_core::{Cell, Color, Grid, LineClues, Piece, Puzzle, Shape, Solution, verify};

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

/// All solutions; panics if the default budget does not suffice.
fn all(p: &Puzzle) -> Vec<Solution> {
    let result = search(p, usize::MAX, DEFAULT_NODE_BUDGET);
    assert_eq!(result.stop, StopReason::Exhausted);
    result.solutions
}

fn count(p: &Puzzle) -> usize {
    all(p).len()
}

/// The first solution, if any; panics if the default budget does not suffice.
fn solve(p: &Puzzle) -> Option<Solution> {
    let result = search(p, 1, DEFAULT_NODE_BUDGET);
    assert_ne!(result.stop, StopReason::NodeBudget);
    result.solutions.into_iter().next()
}

fn sample(name: &str) -> Puzzle {
    let path = format!("{}/../../puzzles/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
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
    assert_eq!(count(&puzzle), 1);
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
    assert_eq!(count(&puzzle), 0);
}

#[test]
fn no_solution_when_sums_match_but_nothing_fits() {
    // Sums agree (2 = 2 = area), but the domino would need both its cells in
    // row 0 and both in column 0 at once.
    let p = puzzle(&["..", ".."], &[2, 0], &[2, 0], &[&["##"]]);
    assert_eq!(solve(&p), None);
    assert_eq!(count(&p), 0);
}

#[test]
fn locks_count_towards_clues() {
    // The lock fills row 0 and column 0; the monomino must go to (1, 1).
    let p = puzzle(&["#.", ".."], &[1, 1], &[1, 1], &[&["#"]]);
    let solutions = all(&p);
    assert_eq!(solutions.len(), 1);
    assert_eq!(render_solution(&p, &solutions[0]).unwrap(), "#.\n.A\n");
}

#[test]
fn locks_exceeding_a_clue_mean_no_solution() {
    let p = puzzle(&["##", ".."], &[1, 1], &[1, 1], &[]);
    assert_eq!(count(&p), 0);
}

#[test]
fn solution_limit_stops_the_search() {
    let p = permutations(4);
    let full = search(&p, 1000, DEFAULT_NODE_BUDGET);
    assert_eq!(full.stop, StopReason::Exhausted);
    assert_eq!(full.solutions.len(), 24);
    let at_24 = search(&p, 24, DEFAULT_NODE_BUDGET);
    assert_eq!(at_24.stop, StopReason::SolutionLimit);
    assert_eq!(at_24.solutions, full.solutions);
    let some = search(&p, 5, DEFAULT_NODE_BUDGET);
    assert_eq!(some.stop, StopReason::SolutionLimit);
    assert_eq!(some.solutions[..], full.solutions[..5]);
    assert!(some.nodes < full.nodes);
    let none = search(&p, 0, DEFAULT_NODE_BUDGET);
    assert_eq!(none.stop, StopReason::SolutionLimit);
    assert_eq!((none.nodes, none.solutions.len()), (0, 0));
}

#[test]
fn small_budget_stops_with_node_budget() {
    let p = parse(REFERENCE).unwrap();
    let result = search(&p, 1, 5);
    assert_eq!(result.stop, StopReason::NodeBudget);
    assert_eq!(result.nodes, 5);
    assert_eq!(result.solutions, []);
}

#[test]
fn budget_boundary_is_exact() {
    let p = permutations(4);
    let full = search(&p, usize::MAX, DEFAULT_NODE_BUDGET);
    assert_eq!(full.stop, StopReason::Exhausted);
    let just_enough = search(&p, usize::MAX, full.nodes);
    assert_eq!(just_enough, full);
    let one_short = search(&p, usize::MAX, full.nodes - 1);
    assert_eq!(one_short.stop, StopReason::NodeBudget);
    assert_eq!(one_short.nodes, full.nodes - 1);
}

#[test]
fn search_is_deterministic() {
    let puzzles = [
        parse(REFERENCE).unwrap(),
        permutations(4),
        sample("wlh9000-1.txt"),
    ];
    for puzzle in &puzzles {
        for (limit, budget) in [
            (1, DEFAULT_NODE_BUDGET),
            (usize::MAX, DEFAULT_NODE_BUDGET),
            (usize::MAX, 7),
        ] {
            let a = search(puzzle, limit, budget);
            let b = search(puzzle, limit, budget);
            assert_eq!(a, b);
            assert!(a.nodes <= budget);
        }
    }
}

#[test]
fn uniqueness_has_four_answers() {
    let reference = parse(REFERENCE).unwrap();
    let check = check_uniqueness(&reference, DEFAULT_NODE_BUDGET);
    let Uniqueness::Unique(solution) = check.uniqueness else {
        panic!("expected a unique solution, got {:?}", check.uniqueness);
    };
    assert_eq!(
        render_solution(&reference, &solution).unwrap(),
        REFERENCE_SOLUTION
    );
    assert!(check.nodes > 0);

    let none = check_uniqueness(&sample("no-solution.txt"), DEFAULT_NODE_BUDGET);
    assert_eq!(none.uniqueness, Uniqueness::NoSolution);

    let many = check_uniqueness(&sample("wlh9000-1.txt"), DEFAULT_NODE_BUDGET);
    assert_eq!(many.uniqueness, Uniqueness::Multiple);

    let unknown = check_uniqueness(&reference, 3);
    assert_eq!(unknown.uniqueness, Uniqueness::Unknown);
    assert_eq!(unknown.nodes, 3);
}

#[test]
fn wlh9000_1_has_five_solutions() {
    let p = sample("wlh9000-1.txt");
    let result = search(&p, usize::MAX, DEFAULT_NODE_BUDGET);
    assert_eq!(result.stop, StopReason::Exhausted);
    assert_eq!(result.solutions.len(), 5);
    for solution in &result.solutions {
        assert_eq!(verify(&p, solution), Ok(()));
        let owners = solution.cell_owners(&p).unwrap();
        assert_eq!(owners[2], None, "top middle cell must stay empty");
    }
}

#[test]
fn zero_clue_keeps_line_empty() {
    // Row 0 and column 2 have free cells but a clue of 0.
    let p = puzzle(&["...", "..."], &[0, 2], &[1, 1, 0], &[&["##"]]);
    let solutions = all(&p);
    assert_eq!(solutions.len(), 1);
    assert_eq!(render_solution(&p, &solutions[0]).unwrap(), "...\nAA.\n");
    // A vertical domino would have to use the zero row: no solution.
    let p = puzzle(&["...", "..."], &[0, 2], &[2, 0, 0], &[&["#", "#"]]);
    assert_eq!(count(&p), 0);
}

#[test]
fn samples_fit_the_default_budget_a_hundred_times_over() {
    let dir = format!("{}/../../puzzles", env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    println!(
        "{:<22} {:>9} {:>11} {:>9}",
        "sample", "solutions", "first-only", "all"
    );
    for name in names {
        let p = sample(&name);
        let first = search(&p, 1, DEFAULT_NODE_BUDGET);
        let full = search(&p, usize::MAX, DEFAULT_NODE_BUDGET);
        assert_eq!(full.stop, StopReason::Exhausted, "{name}");
        println!(
            "{name:<22} {:>9} {:>11} {:>9}",
            full.solutions.len(),
            first.nodes,
            full.nodes
        );
        assert!(
            full.nodes * 100 <= DEFAULT_NODE_BUDGET,
            "{name}: {} nodes",
            full.nodes
        );
    }
}

#[test]
fn identical_pieces_do_not_produce_duplicates() {
    // Two monominoes on a diagonal: two boards, not four.
    let p = puzzle(&["..", ".."], &[1, 1], &[1, 1], &[&["#"], &["#"]]);
    assert_eq!(count(&p), 2);

    // Two dominoes filling a 2x2 board: both horizontal or both vertical.
    let p = puzzle(&["..", ".."], &[2, 2], &[2, 2], &[&["##"], &["##"]]);
    assert_eq!(count(&p), 2);

    // The same L tromino given in two different rotations is still one type.
    let p = puzzle(
        &["...", "...", "..."],
        &[3, 3, 2],
        &[3, 2, 3],
        &[&["#.", "##"], &["##", "#."], &["#"], &["#"]],
    );
    let solutions = all(&p);
    assert!(solutions.len() > 1, "{} solutions", solutions.len());
    // Pieces 0 and 1 are one type, 2 and 3 another. Describe each solution by
    // the sorted cell sets of each type, which ignores piece identity.
    let mut layouts: Vec<Vec<Vec<usize>>> = solutions
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
    assert_eq!(count(&square), 1);
    let domino = puzzle(&[".."], &[2], &[1, 1], &[&["##"]]);
    assert_eq!(count(&domino), 1);
}

#[test]
fn dead_cells_are_never_covered_or_counted() {
    // The L tromino must avoid the dead corner; the dead cell does not count.
    let p = puzzle(&["x.", ".."], &[1, 2], &[1, 2], &[&["#.", "##"]]);
    let solution = solve(&p).unwrap();
    assert_eq!(render_solution(&p, &solution).unwrap(), "xA\nAA\n");
    assert_eq!(count(&p), 1);

    // Clues that count the dead cell as filled cannot be met.
    let p = puzzle(&["x.", ".."], &[2, 2], &[2, 2], &[&["#.", "##"]]);
    assert_eq!(count(&p), 0);

    // A dead cell splits the row; the tromino has nowhere to go.
    let p = puzzle(&[".x.."], &[3], &[1, 0, 1, 1], &[&["###"]]);
    assert_eq!(count(&p), 0);
}

#[test]
fn empty_puzzle_with_zero_clues_is_solved() {
    let p = puzzle(&[".."], &[0], &[0, 0], &[]);
    let solution = solve(&p).unwrap();
    assert_eq!(solution.placements, []);
    assert_eq!(count(&p), 1);
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
    assert_eq!(count(&p), 1);
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
    assert_eq!(count(&p), 0);
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

fn sample_names() -> Vec<String> {
    let dir = format!("{}/../../puzzles", env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Runs a [`Search`] to the end in steps of `step` nodes.
fn stepped(p: &Puzzle, max_solutions: usize, step: u64) -> SearchResult {
    let mut search = Search::new(p, max_solutions);
    let mut calls = 0u64;
    loop {
        calls += 1;
        assert!(calls <= DEFAULT_NODE_BUDGET, "search never finished");
        let before = search.nodes();
        match search.step(step) {
            Progress::Working => assert_eq!(search.nodes() - before, step),
            Progress::Done(result) => {
                assert!(result.nodes - before <= step);
                // Finished searches keep returning the same result.
                assert_eq!(search.step(step), Progress::Done(result.clone()));
                return result;
            }
        }
    }
}

#[test]
fn stepwise_search_matches_search_for_any_step() {
    let mut puzzles: Vec<(String, Puzzle)> = sample_names()
        .into_iter()
        .map(|name| {
            let p = sample(&name);
            (name, p)
        })
        .collect();
    puzzles.push(("permutations-3".into(), permutations(3)));
    puzzles.push(("empty".into(), puzzle(&["."], &[0], &[0], &[])));
    for (name, p) in &puzzles {
        for max_solutions in [0, 1, 2, 3, usize::MAX] {
            let expected = search(p, max_solutions, DEFAULT_NODE_BUDGET);
            assert_ne!(expected.stop, StopReason::NodeBudget, "{name}");
            for step in [1, 7, 1000] {
                assert_eq!(
                    stepped(p, max_solutions, step),
                    expected,
                    "{name}, {max_solutions} solutions, step {step}"
                );
            }
        }
    }
}

#[test]
fn stepwise_search_reports_progress() {
    let p = sample("wlh9000-1.txt");
    let mut stepper = Search::new(&p, usize::MAX);
    assert_eq!(stepper.nodes(), 0);
    assert_eq!(stepper.step(0), Progress::Working);
    assert_eq!(stepper.nodes(), 0);
    while stepper.solutions().is_empty() {
        assert_eq!(stepper.step(1), Progress::Working);
    }
    let first = stepper.solutions()[0].clone();
    assert_eq!(verify(&p, &first), Ok(()));
    assert_eq!(stepper.nodes(), search(&p, 1, DEFAULT_NODE_BUDGET).nodes);
}
