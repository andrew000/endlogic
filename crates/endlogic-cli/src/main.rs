//! Command-line front end for the endlogic solver.
#![forbid(unsafe_code)]

use std::process::ExitCode;

use endlogic_core::format::{parse, render_solution};
use endlogic_core::solver::{Count, count_solutions, solve};

const USAGE: &str = "\
endlogic - solver for Arknights: Endfield mechanism repair puzzles

Usage:
  endlogic solve <FILE>              Print the first solution
  endlogic solve <FILE> --count [N]  Count solutions, stopping at N (default 1000)
  endlogic --help                    Show this help

In a printed solution, pieces are letters in file order (A, B, ...),
`#` is a locked cell, `x` a dead cell and `.` an empty free cell.

Exit codes: 0 success, 1 no solution, 2 usage, file or parse error.
";

const DEFAULT_COUNT_LIMIT: usize = 1000;

/// Exit code when `solve` finds no solution.
const EXIT_NO_SOLUTION: u8 = 1;
/// Exit code for usage, file and parse errors.
const EXIT_ERROR: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Solve {
        path: String,
        count_limit: Option<usize>,
    },
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let Some((command, rest)) = args.split_first() else {
        return Err("missing command".to_owned());
    };
    match command.as_str() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "solve" => parse_solve_args(rest),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_solve_args(args: &[String]) -> Result<Command, String> {
    let mut path = None;
    let mut count_limit = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--count" => {
                let mut limit = DEFAULT_COUNT_LIMIT;
                if let Some(next) = iter.peek()
                    && next.starts_with(|c: char| c.is_ascii_digit())
                {
                    limit = next
                        .parse()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or_else(|| format!("invalid count limit `{next}`"))?;
                    iter.next();
                }
                count_limit = Some(limit);
            }
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
    Ok(Command::Solve { path, count_limit })
}

fn run_solve(path: &str, count_limit: Option<usize>) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: cannot read `{path}`: {err}");
            return ExitCode::from(EXIT_ERROR);
        }
    };
    let puzzle = match parse(&text) {
        Ok(puzzle) => puzzle,
        Err(err) => {
            eprintln!("error: {path}: {err}");
            return ExitCode::from(EXIT_ERROR);
        }
    };

    if let Some(limit) = count_limit {
        match count_solutions(&puzzle, limit) {
            Count::Exact(n) => println!("solutions: {n}"),
            Count::LimitReached(n) => {
                println!("solutions: at least {n} (stopped at limit {limit})");
            }
        }
        return ExitCode::SUCCESS;
    }

    let Some(solution) = solve(&puzzle) else {
        println!("no solution");
        return ExitCode::from(EXIT_NO_SOLUTION);
    };
    let board = render_solution(&puzzle, &solution)
        .expect("solver returned a geometrically invalid solution");
    print!("{board}");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Solve { path, count_limit }) => run_solve(&path, count_limit),
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

    fn solve_cmd(path: &str, count_limit: Option<usize>) -> Command {
        Command::Solve {
            path: path.to_owned(),
            count_limit,
        }
    }

    #[test]
    fn parses_commands() {
        assert_eq!(parse_args(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse_args(&args(&["solve", "-h"])), Ok(Command::Help));
        assert_eq!(
            parse_args(&args(&["solve", "p.txt"])),
            Ok(solve_cmd("p.txt", None))
        );
        assert_eq!(
            parse_args(&args(&["solve", "p.txt", "--count"])),
            Ok(solve_cmd("p.txt", Some(DEFAULT_COUNT_LIMIT)))
        );
        assert_eq!(
            parse_args(&args(&["solve", "p.txt", "--count", "7"])),
            Ok(solve_cmd("p.txt", Some(7)))
        );
        assert_eq!(
            parse_args(&args(&["solve", "--count", "7", "p.txt"])),
            Ok(solve_cmd("p.txt", Some(7)))
        );
    }

    #[test]
    fn rejects_bad_usage() {
        assert!(parse_args(&args(&[])).is_err());
        assert!(parse_args(&args(&["frobnicate"])).is_err());
        assert!(parse_args(&args(&["solve"])).is_err());
        assert!(parse_args(&args(&["solve", "a", "b"])).is_err());
        assert!(parse_args(&args(&["solve", "a", "--verbose"])).is_err());
        assert!(parse_args(&args(&["solve", "a", "--count", "0"])).is_err());
        assert!(parse_args(&args(&["solve", "a", "--count", "9x"])).is_err());
    }
}
