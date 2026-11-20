//! Golden results of the generator, the solver and the diagnosis.
//!
//! The files live in `tests/golden`; `just bless` rewrites them. The solver
//! results come in two files: `solver-solutions.txt` holds what any correct
//! solver must find (it never changes), `solver-nodes.txt` holds how much
//! work this solver needed (a faster search changes it on purpose). The
//! generated puzzles the solver is checked on are a fixed list,
//! `solver-puzzles.txt`, written once and never blessed, so a change in the
//! generator does not change the solver's inputs.

mod support;

use std::fmt::Write as _;

use endlogic_core::code::{decode, encode};
use endlogic_core::diagnose::diagnose;
use endlogic_core::format::{parse, piece_letter};
use endlogic_core::generator::{Difficulty, generate};
use endlogic_core::solver::{Progress, Search, SearchResult, StopReason, search};
use endlogic_core::{Cell, Placement, Puzzle, Rotation, Solution};

/// Seeds per level for the generator.
const SEEDS: u64 = 20;
/// Solutions are counted up to this many.
const LIMIT: usize = 1000;
/// Node budget for one search; every puzzle here needs far less.
const BUDGET: u64 = 50_000_000;
/// The fixed list of generated puzzles the solver is checked on.
const INPUTS: &str = "solver-puzzles.txt";

fn samples() -> Vec<(String, Puzzle)> {
    let dir = format!("{}/../../puzzles", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let puzzle = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
            (name, puzzle)
        })
        .collect()
}

/// The generated puzzles of every level and seed, by name, e.g. `hard-07`.
fn generated() -> Vec<(String, Puzzle)> {
    let path = support::golden_path(INPUTS);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && support::blessing() => {
            let mut text = String::new();
            for level in Difficulty::ALL {
                for seed in 0..SEEDS {
                    let puzzle = generate(level.params(), seed).unwrap().puzzle;
                    let code = encode(&puzzle).unwrap();
                    writeln!(text, "{}-{seed:02} {code}", level.name()).unwrap();
                }
            }
            std::fs::write(&path, &text).unwrap();
            text
        }
        Err(err) => panic!("{}: {err}; `just bless` creates it once", path.display()),
    };
    text.lines()
        .map(|line| {
            let (name, code) = line.split_once(' ').unwrap();
            (name.to_owned(), decode(code.trim()).unwrap())
        })
        .collect()
}

fn all_puzzles() -> Vec<(String, Puzzle)> {
    let mut all = samples();
    all.extend(generated());
    all
}

/// The cells piece `i` covers at `at`, sorted.
fn cells(puzzle: &Puzzle, i: usize, at: Placement) -> Vec<(usize, usize)> {
    let mut cells: Vec<(usize, usize)> = puzzle.pieces()[i]
        .shape
        .rotated(at.rotation)
        .cells()
        .iter()
        .map(|&(r, c)| (at.row + r, at.col + c))
        .collect();
    cells.sort_unstable();
    cells
}

/// Whether pieces `a` and `b` are interchangeable: same color, same shape
/// up to rotation.
fn identical(puzzle: &Puzzle, a: usize, b: usize) -> bool {
    let (pa, pb) = (&puzzle.pieces()[a], &puzzle.pieces()[b]);
    pa.color == pb.color
        && Rotation::ALL
            .iter()
            .any(|&r| pa.shape.rotated(r) == pb.shape)
}

/// For every piece, the cells it covers in `solution`, with identical
/// pieces given their cell sets in sorted order. The result does not depend
/// on which of the identical pieces the solver put where.
fn canonical_cells(puzzle: &Puzzle, solution: &Solution) -> Vec<Vec<(usize, usize)>> {
    let count = puzzle.pieces().len();
    let mut out: Vec<Vec<(usize, usize)>> = (0..count)
        .map(|i| cells(puzzle, i, solution.placements[i]))
        .collect();
    let mut done = vec![false; count];
    for i in 0..count {
        if done[i] {
            continue;
        }
        let group: Vec<usize> = (i..count).filter(|&j| identical(puzzle, i, j)).collect();
        let mut sets: Vec<Vec<(usize, usize)>> = group.iter().map(|&j| out[j].clone()).collect();
        sets.sort();
        for (&j, set) in group.iter().zip(sets) {
            out[j] = set;
            done[j] = true;
        }
    }
    out
}

/// The solution drawn as a grid: piece letters, `#` locked, `x` dead and
/// `.` empty.
fn render(puzzle: &Puzzle, cells: &[Vec<(usize, usize)>]) -> String {
    let grid = puzzle.grid();
    let mut text = String::new();
    for r in 0..grid.height() {
        for c in 0..grid.width() {
            let owner = cells.iter().position(|set| set.contains(&(r, c)));
            text.push(match (owner, grid.get(r, c).unwrap()) {
                (Some(i), _) => piece_letter(i),
                (None, Cell::Lock(_)) => '#',
                (None, Cell::Dead) => 'x',
                (None, Cell::Free) => '.',
            });
        }
        text.push('\n');
    }
    text
}

/// Where piece `i` stands when it covers `cells`: the first rotation (in
/// [`Rotation::ALL`] order) that fits.
fn placement_of(puzzle: &Puzzle, i: usize, cells_wanted: &[(usize, usize)]) -> Placement {
    let row = cells_wanted.iter().map(|c| c.0).min().unwrap();
    let col = cells_wanted.iter().map(|c| c.1).min().unwrap();
    Rotation::ALL
        .iter()
        .map(|&rotation| Placement { rotation, row, col })
        .find(|&at| cells(puzzle, i, at) == cells_wanted)
        .unwrap()
}

/// Every solution in canonical form, sorted.
fn canonical_solutions(puzzle: &Puzzle, solutions: &[Solution]) -> Vec<Vec<Vec<(usize, usize)>>> {
    let mut all: Vec<_> = solutions
        .iter()
        .map(|s| canonical_cells(puzzle, s))
        .collect();
    all.sort();
    all
}

/// The part of a search result that does not depend on the search order.
fn describe_solutions(out: &mut String, title: &str, puzzle: &Puzzle, result: &SearchResult) {
    writeln!(out, "== {title}").unwrap();
    match result.stop {
        StopReason::Exhausted => {
            writeln!(out, "solutions: {}, all found", result.solutions.len()).unwrap();
            for cells in canonical_solutions(puzzle, &result.solutions) {
                out.push('\n');
                out.push_str(&render(puzzle, &cells));
            }
        }
        StopReason::SolutionLimit => writeln!(out, "solutions: at least {LIMIT}").unwrap(),
        StopReason::NodeBudget => panic!("{title}: {BUDGET} nodes were not enough"),
    }
    out.push('\n');
}

fn run(mut search: Search) -> SearchResult {
    match search.step(BUDGET) {
        Progress::Done(result) => result,
        Progress::Working => unreachable!("one step of the whole budget ends the search"),
    }
}

#[test]
fn generator() {
    let mut out = String::new();
    for level in Difficulty::ALL {
        for seed in 0..SEEDS {
            let name = level.name();
            match generate(level.params(), seed) {
                Ok(g) => writeln!(
                    out,
                    "{name} {seed:2}: attempt {}, nodes {}, all attempts {}, code {}",
                    g.attempt,
                    g.nodes,
                    g.total_nodes,
                    encode(&g.puzzle).unwrap()
                ),
                Err(err) => writeln!(out, "{name} {seed:2}: {err}"),
            }
            .unwrap();
        }
    }
    support::check("generator.txt", &out);
}

/// Pieces fixed in place for [`fixed_searches`]: the puzzle, a title and
/// the pieces to fix, each where the first canonical solution has it, or at
/// an explicit placement.
fn fixed_cases() -> Vec<(String, Puzzle, Vec<Option<Placement>>)> {
    let named = |name: &str| {
        all_puzzles()
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap()
            .1
    };
    let from_solution = |puzzle: &Puzzle, pieces: &[usize]| {
        let result = search(puzzle, LIMIT, BUDGET);
        let first = canonical_solutions(puzzle, &result.solutions).remove(0);
        let mut fixed = vec![None; puzzle.pieces().len()];
        for &i in pieces {
            fixed[i] = Some(placement_of(puzzle, i, &first[i]));
        }
        fixed
    };
    let mut cases = Vec::new();
    for (name, pieces) in [
        ("wln006-2", &[0][..]),
        ("wlh9000-1", &[0]),
        ("wlh9000-1", &[1, 2]),
        ("permutations-4", &[0]),
        ("hard-00", &[0, 1]),
        ("expert-00", &[0]),
        ("expert-07", &[2, 3, 4]),
    ] {
        let puzzle = named(name);
        let fixed = from_solution(&puzzle, pieces);
        let list: Vec<String> = pieces.iter().map(ToString::to_string).collect();
        let title = format!("{name}, pieces {} fixed", list.join(" "));
        cases.push((title, puzzle, fixed));
    }
    // A piece fixed where no solution has it.
    let puzzle = named("wln006-2");
    let mut fixed = vec![None; puzzle.pieces().len()];
    fixed[3] = Some(Placement {
        rotation: Rotation::R0,
        row: 5,
        col: 2,
    });
    cases.push(("wln006-2, piece 3 fixed at 5 2".to_owned(), puzzle, fixed));
    cases
}

#[test]
fn solver() {
    let mut solutions = String::new();
    let mut nodes = String::new();
    for (name, puzzle) in all_puzzles() {
        let first = search(&puzzle, 1, BUDGET);
        let full = search(&puzzle, LIMIT, BUDGET);
        describe_solutions(&mut solutions, &name, &puzzle, &full);
        writeln!(
            nodes,
            "{name}: first solution {} nodes, all solutions {} nodes",
            first.nodes, full.nodes
        )
        .unwrap();
    }
    for (title, puzzle, fixed) in fixed_cases() {
        let first = run(Search::with_fixed(&puzzle, 1, &fixed));
        let full = run(Search::with_fixed(&puzzle, LIMIT, &fixed));
        for solution in &full.solutions {
            for (at, want) in solution.placements.iter().zip(&fixed) {
                if let Some(want) = want {
                    assert_eq!(at, want, "{title}: a fixed piece moved");
                }
            }
        }
        describe_solutions(&mut solutions, &title, &puzzle, &full);
        writeln!(
            nodes,
            "{title}: first solution {} nodes, all solutions {} nodes",
            first.nodes, full.nodes
        )
        .unwrap();
    }
    support::check("solver-solutions.txt", &solutions);
    support::check("solver-nodes.txt", &nodes);
}

#[test]
fn diagnosis() {
    let cases = [
        (
            "row and column sums differ",
            "grid\n...\n...\n...\nrows 2 2 1\ncols 2 1 1\npiece\n##\npiece\n##\n",
        ),
        (
            "pieces cover more than the clues ask",
            "grid\n#..\n...\nrows 2 1\ncols 2 1 0\npiece\n##\npiece\n#\n",
        ),
        (
            "a row asks for more than fits",
            "grid\nx..\n...\nrows 3 0\ncols 1 1 1\npiece\n###\n",
        ),
        (
            "a column asks for more than fits",
            "grid\n#.\nx.\nrows 1 1\ncols 2 0\npiece\n#\n",
        ),
        (
            "too many locks in a row and a column",
            "grid\n##.\n#..\nrows 1 1\ncols 1 1 0\npiece\n#\n",
        ),
        ("no pieces", "grid\n..\n..\nrows 0 0\ncols 0 0\n"),
        (
            "everything wrong at once",
            "grid\n##x\nx..\n...\nrows 3 2 0\ncols 0 1 5\n",
        ),
        (
            "wln006-2 with its last row clue raised",
            "grid\n.......\n.#.....\n.#.....\n.......\n.....#.\n.......\n.......\n\
             rows 4 3 6 6 6 6 6\ncols 3 5 4 7 5 5 7\n\
             piece\n###\n##.\npiece\n#.\n##\n#.\npiece\n##.\n###\n..#\n\
             piece\n.##\n###\npiece\n.##..\n.##..\n#####\npiece\n.#\n.#\n##\n",
        ),
        (
            "a sound puzzle",
            "grid\n..\n..\nrows 1 1\ncols 1 1\npiece\n#\npiece\n#\n",
        ),
    ];
    let mut out = String::new();
    for (title, text) in cases {
        let puzzle = parse(&format!("endlogic 1\n{text}")).unwrap();
        writeln!(out, "== {title}").unwrap();
        let problems = diagnose(&puzzle);
        if problems.is_empty() {
            out.push_str("no problems\n");
        }
        for problem in problems {
            writeln!(out, "{problem:?}").unwrap();
        }
        out.push('\n');
    }
    support::check("diagnosis.txt", &out);
}
