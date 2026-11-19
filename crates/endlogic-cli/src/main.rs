//! Command-line front end for endlogic: solver, generator and puzzle codes.
#![forbid(unsafe_code)]

use std::process::ExitCode;

use endlogic_core::code::{decode, encode};
use endlogic_core::format::{parse, render_solution, serialize};
use endlogic_core::generator::{Difficulty, generate};
use endlogic_core::solver::{DEFAULT_NODE_BUDGET, StopReason, search};
use endlogic_core::{Puzzle, Solution};

const USAGE: &str = "\
endlogic - solver and generator for Arknights: Endfield mechanism repair puzzles

Usage:
  endlogic solve <FILE> [--count [N]] [--budget N]
  endlogic gen --difficulty <easy|normal|hard|expert> [--seed N] [--solution] [--code]
  endlogic encode <FILE>
  endlogic decode <CODE>
  endlogic --help

Commands:
  solve    Print the first solution of a puzzle file.
             --count [N]   Count solutions instead, stopping at N (default 1000)
             --budget N    Give up after trying N piece placements (default 200000)
  gen      Generate a puzzle with a unique solution and print it in the text
           format. Seed, attempt and solver nodes go to stderr.
             --seed N      Use this seed (default: taken from the clock)
             --solution    Also print the solution, as comment lines
             --code        Also print the puzzle code, as a comment line
  encode   Print the code of a puzzle file.
  decode   Print the puzzle stored in a code, in the text format.

In a printed solution, pieces are letters in file order (A, B, ...),
`#` is a locked cell, `x` a dead cell and `.` an empty free cell.

Exit codes: 0 success, 1 no solution, 2 usage, file, parse or code error,
3 search budget exhausted or generator gave up.
";

const DEFAULT_COUNT_LIMIT: usize = 1000;

/// Exit code when `solve` finds no solution.
const EXIT_NO_SOLUTION: u8 = 1;
/// Exit code for usage, file, parse and code errors.
const EXIT_ERROR: u8 = 2;
/// Exit code when the node budget runs out or the generator gives up.
const EXIT_BUDGET: u8 = 3;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Solve {
        path: String,
        count_limit: Option<usize>,
        budget: u64,
    },
    Gen {
        difficulty: Difficulty,
        seed: Option<u64>,
        solution: bool,
        code: bool,
    },
    Encode {
        path: String,
    },
    Decode {
        code: String,
    },
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let Some((command, rest)) = args.split_first() else {
        return Err("missing command".to_owned());
    };
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        return Ok(Command::Help);
    }
    match command.as_str() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "solve" => parse_solve_args(rest),
        "gen" => parse_gen_args(rest),
        "encode" => Ok(Command::Encode {
            path: single_positional(rest, "puzzle file")?,
        }),
        "decode" => Ok(Command::Decode {
            code: single_positional(rest, "code")?,
        }),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn single_positional(args: &[String], what: &str) -> Result<String, String> {
    match args {
        [] => Err(format!("missing {what}")),
        [one] if one.starts_with('-') && one.len() > 1 => Err(format!("unknown option `{one}`")),
        [one] => Ok(one.clone()),
        [_, extra, ..] => Err(format!("unexpected argument `{extra}`")),
    }
}

/// Parses an option value that must be a positive integer.
fn positive<T: std::str::FromStr + PartialOrd + Default>(
    option: &str,
    value: Option<&String>,
) -> Result<T, String> {
    let value = value.ok_or_else(|| format!("`{option}` needs a value"))?;
    value
        .parse::<T>()
        .ok()
        .filter(|n| *n > T::default())
        .ok_or_else(|| format!("invalid value `{value}` for `{option}`"))
}

fn parse_solve_args(args: &[String]) -> Result<Command, String> {
    let mut path = None;
    let mut count_limit = None;
    let mut budget = DEFAULT_NODE_BUDGET;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--count" => {
                let mut limit = DEFAULT_COUNT_LIMIT;
                if iter
                    .peek()
                    .is_some_and(|next| next.starts_with(|c: char| c.is_ascii_digit()))
                {
                    limit = positive("--count", iter.next())?;
                }
                count_limit = Some(limit);
            }
            "--budget" => budget = positive("--budget", iter.next())?,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown option `{flag}`"));
            }
            file => {
                if path.is_some() {
                    return Err(format!("unexpected argument `{file}`"));
                }
                path = Some(file.to_owned());
            }
        }
    }
    let path = path.ok_or_else(|| "missing puzzle file".to_owned())?;
    Ok(Command::Solve {
        path,
        count_limit,
        budget,
    })
}

fn parse_gen_args(args: &[String]) -> Result<Command, String> {
    let mut difficulty = None;
    let mut seed = None;
    let mut solution = false;
    let mut code = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--difficulty" => {
                let name = iter.next().ok_or("`--difficulty` needs a value")?;
                difficulty = Some(
                    Difficulty::from_name(name)
                        .ok_or_else(|| format!("unknown difficulty `{name}`"))?,
                );
            }
            "--seed" => {
                let value = iter.next().ok_or("`--seed` needs a value")?;
                seed = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid value `{value}` for `--seed`"))?,
                );
            }
            "--solution" => solution = true,
            "--code" => code = true,
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }
    let difficulty = difficulty.ok_or("missing `--difficulty`")?;
    Ok(Command::Gen {
        difficulty,
        seed,
        solution,
        code,
    })
}

fn read_puzzle(path: &str) -> Result<Puzzle, ExitCode> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        eprintln!("error: cannot read `{path}`: {err}");
        ExitCode::from(EXIT_ERROR)
    })?;
    parse(&text).map_err(|err| {
        eprintln!("error: {path}: {err}");
        ExitCode::from(EXIT_ERROR)
    })
}

fn draw(puzzle: &Puzzle, solution: &Solution) -> String {
    render_solution(puzzle, solution).expect("solutions from the core are geometrically valid")
}

fn run_solve(path: &str, count_limit: Option<usize>, budget: u64) -> ExitCode {
    let puzzle = match read_puzzle(path) {
        Ok(puzzle) => puzzle,
        Err(code) => return code,
    };
    let result = search(&puzzle, count_limit.unwrap_or(1), budget);
    let found = result.solutions.len();
    if result.stop == StopReason::NodeBudget {
        println!("search budget of {budget} nodes exhausted after {found} solution(s)");
        return ExitCode::from(EXIT_BUDGET);
    }

    if let Some(limit) = count_limit {
        if result.stop == StopReason::SolutionLimit {
            println!("solutions: at least {found} (stopped at limit {limit})");
        } else {
            println!("solutions: {found}");
        }
        return ExitCode::SUCCESS;
    }

    let Some(solution) = result.solutions.first() else {
        println!("no solution");
        return ExitCode::from(EXIT_NO_SOLUTION);
    };
    print!("{}", draw(&puzzle, solution));
    ExitCode::SUCCESS
}

/// A seed from the system clock. This is the only place that reads the
/// clock; the core never does.
fn clock_seed() -> u64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    elapsed
        .as_secs()
        .wrapping_mul(1_000_000_000)
        .wrapping_add(u64::from(elapsed.subsec_nanos()))
}

fn run_gen(
    difficulty: Difficulty,
    seed: Option<u64>,
    with_solution: bool,
    with_code: bool,
) -> ExitCode {
    let seed = seed.unwrap_or_else(clock_seed);
    let generated = match generate(difficulty.params(), seed) {
        Ok(generated) => generated,
        Err(err) => {
            eprintln!("error: seed {seed}: {err}");
            return ExitCode::from(EXIT_BUDGET);
        }
    };
    let puzzle = &generated.puzzle;
    let text = serialize(puzzle).expect("generated puzzles are single-color");
    let mut out = text;
    if with_solution {
        out.push_str("// solution:\n");
        for line in draw(puzzle, &generated.solution).lines() {
            out.push_str("// ");
            out.push_str(line);
            out.push('\n');
        }
    }
    if with_code {
        let code = encode(puzzle).expect("generated puzzles fit in a code");
        out.push_str("// code: ");
        out.push_str(&code);
        out.push('\n');
    }
    print!("{out}");
    eprintln!(
        "seed {seed}, attempt {}, nodes {} (all attempts: {})",
        generated.attempt, generated.nodes, generated.total_nodes
    );
    ExitCode::SUCCESS
}

fn run_encode(path: &str) -> ExitCode {
    let puzzle = match read_puzzle(path) {
        Ok(puzzle) => puzzle,
        Err(code) => return code,
    };
    match encode(&puzzle) {
        Ok(code) => {
            println!("{code}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {path}: {err}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

fn run_decode(code: &str) -> ExitCode {
    let puzzle = match decode(code) {
        Ok(puzzle) => puzzle,
        Err(err) => {
            eprintln!("error: cannot decode: {err}");
            return ExitCode::from(EXIT_ERROR);
        }
    };
    match serialize(&puzzle) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Solve {
            path,
            count_limit,
            budget,
        }) => run_solve(&path, count_limit, budget),
        Ok(Command::Gen {
            difficulty,
            seed,
            solution,
            code,
        }) => run_gen(difficulty, seed, solution, code),
        Ok(Command::Encode { path }) => run_encode(&path),
        Ok(Command::Decode { code }) => run_decode(&code),
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn solve_cmd(path: &str, count_limit: Option<usize>, budget: u64) -> Command {
        Command::Solve {
            path: path.to_owned(),
            count_limit,
            budget,
        }
    }

    #[test]
    fn parses_solve() {
        let budget = DEFAULT_NODE_BUDGET;
        assert_eq!(parse_args(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse_args(&args(&["solve", "-h"])), Ok(Command::Help));
        assert_eq!(
            parse_args(&args(&["solve", "p.txt"])),
            Ok(solve_cmd("p.txt", None, budget))
        );
        assert_eq!(
            parse_args(&args(&["solve", "p.txt", "--count"])),
            Ok(solve_cmd("p.txt", Some(DEFAULT_COUNT_LIMIT), budget))
        );
        assert_eq!(
            parse_args(&args(&["solve", "--count", "7", "p.txt"])),
            Ok(solve_cmd("p.txt", Some(7), budget))
        );
        assert_eq!(
            parse_args(&args(&["solve", "p.txt", "--budget", "42"])),
            Ok(solve_cmd("p.txt", None, 42))
        );
    }

    #[test]
    fn parses_gen_encode_decode() {
        assert_eq!(
            parse_args(&args(&[
                "gen",
                "--difficulty",
                "hard",
                "--seed",
                "1",
                "--code"
            ])),
            Ok(Command::Gen {
                difficulty: Difficulty::Hard,
                seed: Some(1),
                solution: false,
                code: true,
            })
        );
        assert_eq!(
            parse_args(&args(&["gen", "--solution", "--difficulty", "easy"])),
            Ok(Command::Gen {
                difficulty: Difficulty::Easy,
                seed: None,
                solution: true,
                code: false,
            })
        );
        assert_eq!(
            parse_args(&args(&["encode", "p.txt"])),
            Ok(Command::Encode {
                path: "p.txt".to_owned()
            })
        );
        assert_eq!(
            parse_args(&args(&["decode", "AQID"])),
            Ok(Command::Decode {
                code: "AQID".to_owned()
            })
        );
    }

    #[test]
    fn rejects_bad_usage() {
        for bad in [
            &[][..],
            &["frobnicate"],
            &["solve"],
            &["solve", "a", "b"],
            &["solve", "a", "--verbose"],
            &["solve", "a", "--count", "0"],
            &["solve", "a", "--count", "9x"],
            &["solve", "a", "--budget"],
            &["solve", "a", "--budget", "0"],
            &["gen"],
            &["gen", "--difficulty"],
            &["gen", "--difficulty", "insane"],
            &["gen", "--difficulty", "easy", "--seed", "-1"],
            &["gen", "--difficulty", "easy", "extra"],
            &["encode"],
            &["encode", "a", "b"],
            &["decode"],
            &["decode", "--x"],
        ] {
            assert!(parse_args(&args(bad)).is_err(), "{bad:?} accepted");
        }
    }
}
