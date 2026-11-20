//! Benchmarks of the core: solver, generator and puzzle codes.
//!
//! Run with `just bench`; prints Markdown tables. Every input is fixed
//! (sample files, fixed seeds), so runs are comparable.

mod measure;

use std::time::Instant;

use endlogic_core::Puzzle;
use endlogic_core::code::{decode, encode};
use endlogic_core::format::parse;
use endlogic_core::generator::{Difficulty, GeneratorParams, SizeRange, generate};
use endlogic_core::solver::{
    DEFAULT_NODE_BUDGET, StopReason, Uniqueness, check_uniqueness, search,
};
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

fn main() {
    let start = Instant::now();
    println!("## endlogic-core");
    full_search();
    uniqueness();
    let fields = large_fields();
    large_search(&fields);
    generator();
    codes(&fields[2].1);
    println!("\nendlogic-core benchmarks took {}", show(start.elapsed()));
}
