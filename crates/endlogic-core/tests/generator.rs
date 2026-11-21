//! Generator tests. Boards stay at 8×8 or smaller.

use endlogic_core::diagnose::diagnose;
use endlogic_core::generator::{
    Difficulty, GenerateError, Generated, Generator, GeneratorParams, ParamsError, SizeRange, Step,
    generate,
};
use endlogic_core::solver::{Uniqueness, check_uniqueness};
use endlogic_core::{Cell, Color, verify};

const SEEDS: u64 = 20;

#[test]
fn same_seed_same_puzzle_different_seeds_differ() {
    let params = Difficulty::Normal.params();
    let a = generate(params, 7).unwrap();
    let b = generate(params, 7).unwrap();
    assert_eq!(a, b);
    let c = generate(params, 8).unwrap();
    assert_ne!(a.puzzle, c.puzzle);
}

#[test]
fn stepwise_and_convenience_agree() {
    for difficulty in Difficulty::ALL {
        let params = difficulty.params();
        let mut generator = Generator::new(params, 3).unwrap();
        let mut steps = 0;
        let generated = loop {
            steps += 1;
            match generator.step(u64::MAX) {
                Step::Working => assert_eq!(generator.attempts(), steps),
                Step::Done(generated) => break generated,
                Step::GaveUp { .. } => panic!("{} gave up", difficulty.name()),
            }
        };
        assert_eq!(generated.attempt, steps);
        assert_eq!(generated.total_nodes, generator.total_nodes());
        assert_eq!(generate(params, 3).unwrap(), generated);
        // Finished generators keep returning their result.
        assert_eq!(generator.step(u64::MAX), Step::Done(generated));
        assert_eq!(generator.attempts(), steps);
    }
}

#[test]
fn every_level_gives_unique_valid_puzzles_within_parameters() {
    println!(
        "{:<7} {:>6} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>6}",
        "level",
        "seeds",
        "att avg",
        "att max",
        "nodes avg",
        "nodes max",
        "total avg",
        "total max",
        "failed"
    );
    for difficulty in Difficulty::ALL {
        let params = difficulty.params();
        let (mut attempts, mut max_attempts, mut failed) = (0u64, 0u32, 0u32);
        let (mut nodes, mut max_nodes, mut total, mut max_total) = (0u64, 0u64, 0u64, 0u64);
        for seed in 0..SEEDS {
            let generated = match generate(params, seed) {
                Ok(generated) => generated,
                Err(err) => {
                    println!("{} seed {seed}: {err}", difficulty.name());
                    failed += 1;
                    continue;
                }
            };
            attempts += u64::from(generated.attempt);
            max_attempts = max_attempts.max(generated.attempt);
            nodes += generated.nodes;
            max_nodes = max_nodes.max(generated.nodes);
            total += generated.total_nodes;
            max_total = max_total.max(generated.total_nodes);

            let puzzle = &generated.puzzle;
            assert_eq!(verify(puzzle, &generated.solution), Ok(()));
            let check = check_uniqueness(puzzle, params.node_budget);
            assert!(
                matches!(check.uniqueness, Uniqueness::Unique(_)),
                "{} seed {seed} is not unique",
                difficulty.name()
            );
            assert_eq!(check.nodes, generated.nodes);

            let grid = puzzle.grid();
            assert_eq!((grid.width(), grid.height()), (params.width, params.height));
            assert!(params.pieces.contains(puzzle.pieces().len()));
            for piece in puzzle.pieces() {
                assert!(params.piece_size.contains(piece.shape.area()));
                let side = piece.shape.width().max(piece.shape.height());
                assert!(side <= params.max_piece_side, "a piece {side} cells long");
            }
            let locks = grid
                .cells()
                .iter()
                .filter(|c| matches!(c, Cell::Lock(_)))
                .count();
            let dead = grid.cells().iter().filter(|c| **c == Cell::Dead).count();
            assert!(params.locks.contains(locks), "{locks} locks");
            assert!(params.dead.contains(dead), "{dead} dead cells");
        }
        let ok = SEEDS - u64::from(failed);
        println!(
            "{:<7} {:>6} {:>9.1} {:>9} {:>9.0} {:>9} {:>9.0} {:>9} {:>6}",
            difficulty.name(),
            SEEDS,
            ratio(attempts, ok),
            max_attempts,
            ratio(nodes, ok),
            max_nodes,
            ratio(total, ok),
            max_total,
            failed
        );
        assert_eq!(failed, 0, "{} failed on {failed} seeds", difficulty.name());
    }
}

fn ratio(sum: u64, count: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let (sum, count) = (sum as f64, count.max(1) as f64);
    sum / count
}

#[test]
fn without_uniqueness_any_solvable_attempt_is_accepted() {
    let params = GeneratorParams {
        require_unique: false,
        ..Difficulty::Hard.params()
    };
    for seed in 0..5 {
        let generated = generate(params, seed).unwrap();
        assert_eq!(verify(&generated.puzzle, &generated.solution), Ok(()));
    }
}

#[test]
fn gives_up_after_max_attempts() {
    // Pieces larger than the board can never be grown.
    let params = GeneratorParams {
        piece_size: SizeRange::new(30, 30),
        max_attempts: 3,
        ..Difficulty::Easy.params()
    };
    let mut generator = Generator::new(params, 1).unwrap();
    assert_eq!(generator.step(u64::MAX), Step::Working);
    assert_eq!(generator.step(u64::MAX), Step::Working);
    let gave_up = Step::GaveUp {
        attempts: 3,
        total_nodes: 0,
    };
    assert_eq!(generator.step(u64::MAX), gave_up);
    assert_eq!(generator.step(u64::MAX), gave_up);
    assert_eq!(
        generate(params, 1),
        Err(GenerateError::GaveUp {
            attempts: 3,
            total_nodes: 0
        })
    );
}

#[test]
fn invalid_params_are_rejected() {
    let base = Difficulty::Easy.params();
    let cases = [
        (GeneratorParams { width: 0, ..base }, ParamsError::GridSize),
        (
            GeneratorParams { height: 17, ..base },
            ParamsError::GridSize,
        ),
        (
            GeneratorParams {
                locks: SizeRange::new(3, 2),
                ..base
            },
            ParamsError::EmptyRange("locks"),
        ),
        (
            GeneratorParams {
                pieces: SizeRange::new(1, 33),
                ..base
            },
            ParamsError::TooManyPieces,
        ),
        (
            GeneratorParams {
                piece_size: SizeRange::new(0, 3),
                ..base
            },
            ParamsError::EmptyPieces,
        ),
        (
            GeneratorParams {
                max_attempts: 0,
                ..base
            },
            ParamsError::NoAttempts,
        ),
        (
            GeneratorParams {
                max_piece_side: 0,
                ..base
            },
            ParamsError::NoPieceSide,
        ),
        (base.with_colors(0), ParamsError::Colors),
        (base.with_colors(5), ParamsError::Colors),
        (
            GeneratorParams {
                pieces: SizeRange::new(1, 2),
                ..base.with_colors(3)
            },
            ParamsError::TooFewPieces,
        ),
    ];
    for (params, expected) in cases {
        assert_eq!(Generator::new(params, 0).unwrap_err(), expected);
        assert_eq!(generate(params, 0), Err(GenerateError::Params(expected)));
    }
}

#[test]
fn difficulty_names_round_trip() {
    for difficulty in Difficulty::ALL {
        assert_eq!(Difficulty::from_name(difficulty.name()), Some(difficulty));
    }
    assert_eq!(Difficulty::from_name("impossible"), None);
}

/// Runs a generator to the end, `budget` nodes per step.
fn run_in_steps(params: GeneratorParams, seed: u64, budget: u64) -> (Generated, u64) {
    let mut generator = Generator::new(params, seed).unwrap();
    let mut calls = 0;
    loop {
        calls += 1;
        let before = generator.total_nodes();
        match generator.step(budget) {
            Step::Working => assert!(generator.total_nodes() - before <= budget),
            Step::Done(generated) => return (generated, calls),
            Step::GaveUp { .. } => panic!("gave up"),
        }
    }
}

#[test]
fn the_step_budget_does_not_change_the_result() {
    for difficulty in Difficulty::ALL {
        let params = difficulty.params();
        for seed in 0..3 {
            let expected = generate(params, seed).unwrap();
            for budget in [1, 7, 100, 20_000, u64::MAX] {
                let (generated, calls) = run_in_steps(params, seed, budget);
                assert_eq!(
                    generated, expected,
                    "{difficulty:?} seed {seed} budget {budget}"
                );
                if budget == 1 {
                    // About one call per node, plus one per attempt.
                    assert!(calls >= expected.total_nodes, "{calls} calls");
                }
            }
        }
    }
}

#[test]
fn a_small_step_budget_keeps_working() {
    let params = Difficulty::Hard.params();
    let mut generator = Generator::new(params, 1).unwrap();
    assert_eq!(generator.step(1), Step::Working);
    assert_eq!(generator.attempts(), 1);
    assert_eq!(generator.total_nodes(), 1);
    assert_eq!(generator.step(10), Step::Working);
    assert_eq!(generator.total_nodes(), 11);
    // The CLI's reference puzzle is unchanged.
    let generated = generate(params, 1).unwrap();
    assert_eq!(
        endlogic_core::code::encode(&generated.puzzle).unwrap(),
        "AWYAGPbauz28AgIAAEAABiH4SuItyQvoTGfEl628"
    );
    assert_eq!((generated.attempt, generated.nodes), (1, 323));
}

/// Two-color puzzles of every level: every color has a piece, the clues of
/// every color add up, and the solution is unique.
#[test]
fn two_colors_give_every_color_a_piece_and_a_unique_solution() {
    for level in Difficulty::ALL {
        for seed in 0..20 {
            let generated = generate(level.params().with_colors(2), seed).unwrap();
            check_colored(&generated, 2, &format!("{} {seed}", level.name()));
        }
    }
}

/// Three and four colors work on the larger levels.
#[test]
fn three_and_four_colors() {
    for colors in [3, 4] {
        for level in [Difficulty::Hard, Difficulty::Expert] {
            for seed in 0..4 {
                let generated = generate(level.params().with_colors(colors), seed).unwrap();
                check_colored(
                    &generated,
                    colors,
                    &format!("{colors} {} {seed}", level.name()),
                );
            }
        }
    }
}

fn check_colored(generated: &Generated, colors: usize, name: &str) {
    let puzzle = &generated.puzzle;
    assert_eq!(verify(puzzle, &generated.solution), Ok(()), "{name}");
    assert_eq!(puzzle.colors(), Color::ALL[..colors], "{name}");
    assert!(
        diagnose(puzzle).is_empty(),
        "{name}: {:?}",
        diagnose(puzzle)
    );
    let mut areas = vec![0; colors];
    for piece in puzzle.pieces() {
        areas[piece.color.index()] += piece.shape.area();
    }
    assert!(areas.iter().all(|&a| a > 0), "{name}: {areas:?}");
    let largest = puzzle
        .pieces()
        .iter()
        .map(|p| p.shape.area())
        .max()
        .unwrap();
    let (least, most) = (areas.iter().min().unwrap(), areas.iter().max().unwrap());
    assert!(most - least <= largest, "{name}: areas {areas:?}");
    for color in &Color::ALL[..colors] {
        let clues = puzzle.clues_for(*color).unwrap();
        let locks = puzzle
            .grid()
            .cells()
            .iter()
            .filter(|&&c| c == Cell::Lock(*color))
            .count();
        let rows: usize = clues.rows.iter().sum();
        assert_eq!(rows, clues.cols.iter().sum::<usize>(), "{name}");
        assert_eq!(rows, areas[color.index()] + locks, "{name}");
    }
    let check = check_uniqueness(puzzle, 10_000_000);
    assert!(matches!(check.uniqueness, Uniqueness::Unique(_)), "{name}");
}

/// With one color the generator gives what `with_colors(1)` gives, and
/// the same as before colors (the golden file `generator.txt` holds that).
#[test]
fn one_color_is_the_default() {
    for level in Difficulty::ALL {
        assert_eq!(level.params().colors, 1);
        assert_eq!(
            generate(level.params().with_colors(1), 3),
            generate(level.params(), 3)
        );
    }
}
