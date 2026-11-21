//! Benchmarks of the core: solver, generator and puzzle codes.
//!
//! Run with `just bench`; prints Markdown tables. Every input is fixed
//! (sample files, fixed seeds), so runs are comparable.

mod measure;

use std::time::Instant;

use endlogic_core::code::{decode, encode};
use endlogic_core::format::parse;
use endlogic_core::generator::{Difficulty, GeneratorParams, SizeRange, generate};
use endlogic_core::rng::Rng;
use endlogic_core::solver::{
    DEFAULT_NODE_BUDGET, Progress, Search, StopReason, Uniqueness, check_uniqueness, search,
};
use endlogic_core::{Cell, Color, Grid, Piece, Puzzle, Rotation, Shape};
use measure::{Table, rate, show, time};

/// Seeds per level.
const SEEDS: u64 = 20;
/// Node budget for checking one attempt of a large field.
const GENERATOR_BUDGET: u64 = 200_000;
/// Node budget for the search on large fields.
const LARGE_BUDGET: u64 = 2_000_000;

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
            (
                name,
                parse(&std::fs::read_to_string(path).unwrap()).unwrap(),
            )
        })
        .collect()
}

fn full_search() {
    let mut table = Table::new(
        "Solver: full search of the samples",
        &["puzzle", "solutions", "nodes", "mean", "best", "speed"],
    );
    for (name, puzzle) in samples() {
        let result = search(&puzzle, usize::MAX, u64::MAX);
        assert_eq!(result.stop, StopReason::Exhausted);
        let t = time(|| search(&puzzle, usize::MAX, u64::MAX));
        table.row(vec![
            name,
            result.solutions.len().to_string(),
            result.nodes.to_string(),
            show(t.mean),
            show(t.best),
            rate(result.nodes, t.mean),
        ]);
    }
    table.print();
}

fn uniqueness() {
    let mut table = Table::new(
        &format!(
            "Solver: uniqueness check, generated puzzles of seeds 0-{}",
            SEEDS - 1
        ),
        &[
            "level",
            "puzzles",
            "nodes, mean",
            "nodes, most",
            "time, mean",
            "time, worst",
            "speed",
        ],
    );
    for level in Difficulty::ALL {
        let mut nodes = Vec::new();
        let mut times = Vec::new();
        for seed in 0..SEEDS {
            let puzzle = generate(level.params(), seed).unwrap().puzzle;
            let check = check_uniqueness(&puzzle, DEFAULT_NODE_BUDGET);
            assert!(matches!(check.uniqueness, Uniqueness::Unique(_)));
            nodes.push(check.nodes);
            times.push(time(|| check_uniqueness(&puzzle, DEFAULT_NODE_BUDGET)).mean);
        }
        let total_nodes: u64 = nodes.iter().sum();
        let total_time: std::time::Duration = times.iter().sum();
        table.row(vec![
            level.name().to_owned(),
            nodes.len().to_string(),
            (total_nodes / SEEDS).to_string(),
            nodes.iter().max().unwrap().to_string(),
            show(total_time / u32::try_from(SEEDS).unwrap()),
            show(*times.iter().max().unwrap()),
            rate(total_nodes, total_time),
        ]);
    }
    table.print();
}

/// Parameters for a large field of side `n`.
fn large(n: usize) -> GeneratorParams {
    let (pieces, locks, dead) = match n {
        10 => ((8, 10), (4, 8), (0, 6)),
        12 => ((10, 12), (6, 10), (0, 8)),
        _ => ((9, 11), (8, 12), (0, 8)),
    };
    GeneratorParams {
        width: n,
        height: n,
        pieces: SizeRange::new(pieces.0, pieces.1),
        piece_size: SizeRange::new(4, 9),
        max_piece_side: 5,
        locks: SizeRange::new(locks.0, locks.1),
        dead: SizeRange::new(dead.0, dead.1),
        node_budget: GENERATOR_BUDGET,
        max_attempts: 50,
        require_unique: false,
        colors: 1,
    }
}

/// Seed of the large fields.
const LARGE_SEED: u64 = 3;

/// Large generated fields: 10x10, 12x12 and 16x16.
fn large_fields() -> Vec<(usize, Puzzle)> {
    [10, 12, 16]
        .into_iter()
        .map(|n| (n, generate(large(n), LARGE_SEED).unwrap().puzzle))
        .collect()
}

fn large_search(fields: &[(usize, Puzzle)]) {
    let mut table = Table::new(
        &format!("Solver: large generated fields (seed {LARGE_SEED}), budget {LARGE_BUDGET} nodes"),
        &[
            "field", "pieces", "task", "nodes", "finished", "time", "speed",
        ],
    );
    for (n, puzzle) in fields {
        for (task, wanted) in [("first solution", 1), ("uniqueness (2)", 2)] {
            let start = Instant::now();
            let result = search(puzzle, wanted, LARGE_BUDGET);
            let took = start.elapsed();
            let finished = match result.stop {
                StopReason::NodeBudget => "no, budget spent",
                StopReason::SolutionLimit => "yes, found",
                StopReason::Exhausted => "yes, exhausted",
            };
            table.row(vec![
                format!("{n}x{n}"),
                puzzle.pieces().len().to_string(),
                task.to_owned(),
                result.nodes.to_string(),
                finished.to_owned(),
                show(took),
                rate(result.nodes, took),
            ]);
        }
    }
    table.print();
}

/// Seeds of each crowded 16x16 field.
const CROWDED_SEEDS: u64 = 5;

/// A 16x16 puzzle with `pieces` pieces planted on it from `seed`, the way
/// the generator builds one but without checking it with the solver (the
/// generator cannot confirm such fields within its budget): pieces of 4 to
/// 9 cells at most 5 long grown on free cells, then 10 to 16 locks and up
/// to 8 dead cells on what is left. It has a solution by construction; it
/// need not be unique.
fn crowded(pieces: usize, seed: u64) -> Puzzle {
    const N: usize = 16;
    let mut rng = Rng::seed_from_u64(
        seed.wrapping_mul(1000)
            .wrapping_add(u64::try_from(pieces).unwrap()),
    );
    let mut taken = [[false; N]; N];
    let mut planted: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut tries = 0;
    while planted.len() < pieces && tries < 10_000 {
        tries += 1;
        let size = rng.usize_inclusive(4, 9);
        let start = (rng.index(N), rng.index(N));
        if taken[start.0][start.1] {
            continue;
        }
        let mut cells = vec![start];
        while cells.len() < size {
            let mut next = Vec::new();
            for &(r, c) in &cells {
                for (dr, dc) in [(0isize, 1isize), (1, 0), (0, -1), (-1, 0)] {
                    let (Some(nr), Some(nc)) = (r.checked_add_signed(dr), c.checked_add_signed(dc))
                    else {
                        continue;
                    };
                    if nr >= N || nc >= N || taken[nr][nc] || cells.contains(&(nr, nc)) {
                        continue;
                    }
                    let fold = |(lo, hi): (usize, usize), v: usize| (lo.min(v), hi.max(v));
                    let rows = cells.iter().map(|x| x.0).fold((nr, nr), fold);
                    let cols = cells.iter().map(|x| x.1).fold((nc, nc), fold);
                    if rows.1 - rows.0 < 5 && cols.1 - cols.0 < 5 {
                        next.push((nr, nc));
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            cells.push(next[rng.index(next.len())]);
        }
        if cells.len() < 4 {
            continue;
        }
        for &(r, c) in &cells {
            taken[r][c] = true;
        }
        planted.push(cells);
    }
    let mut grid = vec![Cell::Free; N * N];
    let mut rows = vec![0; N];
    let mut cols = vec![0; N];
    for cells in &planted {
        for &(r, c) in cells {
            rows[r] += 1;
            cols[c] += 1;
        }
    }
    let free: Vec<usize> = (0..N * N).filter(|&i| !taken[i / N][i % N]).collect();
    let locks = rng.usize_inclusive(10, 16);
    let dead = rng.usize_inclusive(0, 8);
    let mut order = free.clone();
    rng.shuffle(&mut order);
    for &i in order.iter().take(locks) {
        grid[i] = Cell::Lock(Color::DEFAULT);
        rows[i / N] += 1;
        cols[i % N] += 1;
    }
    for &i in order.iter().skip(locks).take(dead) {
        grid[i] = Cell::Dead;
    }
    let pieces = planted
        .iter()
        .map(|cells| {
            let r0 = cells.iter().map(|x| x.0).min().unwrap();
            let c0 = cells.iter().map(|x| x.1).min().unwrap();
            let shape = Shape::new(cells.iter().map(|&(r, c)| (r - r0, c - c0))).unwrap();
            Piece::new(Color::DEFAULT, shape.rotated(Rotation::ALL[rng.index(4)]))
        })
        .collect();
    Puzzle::single_color(Grid::from_cells(N, N, grid).unwrap(), pieces, rows, cols).unwrap()
}

fn crowded_search() {
    let mut table = Table::new(
        &format!(
            "Solver: crowded 16x16 fields (planted, seeds 0-{}), first solution, budget {LARGE_BUDGET} nodes",
            CROWDED_SEEDS - 1
        ),
        &["pieces", "seed", "nodes", "finished", "time", "speed"],
    );
    for pieces in [14, 15, 16] {
        for seed in 0..CROWDED_SEEDS {
            let puzzle = crowded(pieces, seed);
            assert_eq!(puzzle.pieces().len(), pieces);
            let start = Instant::now();
            let result = search(&puzzle, 1, LARGE_BUDGET);
            let took = start.elapsed();
            let finished = match result.stop {
                StopReason::NodeBudget => "no, budget spent",
                StopReason::SolutionLimit => "yes, found",
                StopReason::Exhausted => "exhausted (no solution?)",
            };
            table.row(vec![
                pieces.to_string(),
                seed.to_string(),
                result.nodes.to_string(),
                finished.to_owned(),
                show(took),
                rate(result.nodes, took),
            ]);
        }
    }
    table.print();
}

fn generator() {
    let mut table = Table::new(
        &format!("Generator: one accepted puzzle, seeds 0-{}", SEEDS - 1),
        &[
            "level",
            "attempts, mean",
            "time, mean",
            "time, worst",
            "worst seed",
        ],
    );
    for level in Difficulty::ALL {
        let mut times = Vec::new();
        let mut attempts = 0;
        for seed in 0..SEEDS {
            attempts += generate(level.params(), seed).unwrap().attempt;
            times.push((time(|| generate(level.params(), seed)).mean, seed));
        }
        let total: std::time::Duration = times.iter().map(|t| t.0).sum();
        let worst = times.iter().max().unwrap();
        #[allow(clippy::cast_precision_loss)]
        let mean_attempts = f64::from(attempts) / SEEDS as f64;
        table.row(vec![
            level.name().to_owned(),
            format!("{mean_attempts:.2}"),
            show(total / u32::try_from(SEEDS).unwrap()),
            show(worst.0),
            worst.1.to_string(),
        ]);
    }
    table.print();
}

fn codes(big: &Puzzle) {
    let mut table = Table::new(
        "Puzzle codes",
        &["puzzle", "code length", "encode", "decode"],
    );
    let wln = samples()
        .into_iter()
        .find(|(n, _)| n == "wln006-2")
        .unwrap()
        .1;
    for (name, puzzle) in [("wln006-2 (7x7)", &wln), ("generated 16x16", big)] {
        let code = encode(puzzle).unwrap();
        let enc = time(|| encode(puzzle));
        let dec = time(|| decode(&code));
        table.row(vec![
            name.to_owned(),
            code.len().to_string(),
            show(enc.mean),
            show(dec.mean),
        ]);
    }
    table.print();
}

/// Nodes the trainer runs per frame (`NODES_PER_FRAME` in the interface).
const FRAME_NODES: u64 = 20_000;

/// Runs of each stepped search.
const FRAME_RUNS: u32 = 10;

/// A uniqueness check (two solutions wanted) run in steps of `FRAME_NODES`
/// nodes, the way the trainer runs it: the time of each step.
fn frame_steps(fields: &[(usize, Puzzle)]) {
    let mut table = Table::new(
        &format!(
            "Solver: uniqueness check in steps of {FRAME_NODES} nodes, mean of {FRAME_RUNS} runs"
        ),
        &[
            "puzzle",
            "nodes",
            "steps",
            "first step",
            "step, mean",
            "step, slowest",
        ],
    );
    let hardest = (0..SEEDS)
        .map(|seed| generate(Difficulty::Expert.params(), seed).unwrap().puzzle)
        .max_by_key(|puzzle| check_uniqueness(puzzle, DEFAULT_NODE_BUDGET).nodes)
        .unwrap();
    let puzzles = [
        ("expert, most nodes".to_owned(), &hardest),
        ("12x12 field".to_owned(), &fields[1].1),
    ];
    for (name, puzzle) in puzzles {
        let mut steps: Vec<std::time::Duration> = Vec::new();
        let mut nodes = 0;
        for _ in 0..FRAME_RUNS {
            let mut search = Search::new(puzzle, 2);
            for i in 0.. {
                let start = Instant::now();
                let progress = std::hint::black_box(search.step(FRAME_NODES));
                let took = start.elapsed();
                if steps.len() <= i {
                    steps.push(took);
                } else {
                    steps[i] += took;
                }
                if let Progress::Done(result) = progress {
                    nodes = result.nodes;
                    break;
                }
            }
        }
        let steps: Vec<_> = steps.iter().map(|d| *d / FRAME_RUNS).collect();
        let total: std::time::Duration = steps.iter().sum();
        table.row(vec![
            name,
            nodes.to_string(),
            steps.len().to_string(),
            show(steps[0]),
            show(total / u32::try_from(steps.len()).unwrap()),
            show(*steps.iter().max().unwrap()),
        ]);
    }
    table.print();
}

fn main() {
    let start = Instant::now();
    println!("## endlogic-core");
    full_search();
    uniqueness();
    let fields = large_fields();
    large_search(&fields);
    crowded_search();
    frame_steps(&fields);
    generator();
    codes(&fields[2].1);
    println!("\nendlogic-core benchmarks took {}", show(start.elapsed()));
}
