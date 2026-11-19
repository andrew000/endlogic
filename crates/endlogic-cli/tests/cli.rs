//! End-to-end tests of the `endlogic` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

fn puzzle_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../puzzles")
        .join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_endlogic"))
        .args(args)
        .output()
        .expect("failed to run endlogic")
}

fn solve(name: &str, extra: &[&str]) -> Output {
    let path = puzzle_path(name);
    let mut args = vec!["solve", path.to_str().unwrap()];
    args.extend_from_slice(extra);
    run(&args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn solves_reference_puzzle() {
    let output = solve("wln006-2.txt", &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        ".#####.\nCCCEEEE\nC..E..E\n.DDEAA.\n.DDEEA.\n..BBB..\n...B...\n"
    );
}

#[test]
fn counts_reference_puzzle() {
    let output = solve("wln006-2.txt", &["--count"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "solutions: 1\n");
}

#[test]
fn count_shows_when_limit_is_reached() {
    let output = solve("permutations-4.txt", &["--count", "10"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        "solutions: at least 10 (stopped at limit 10)\n"
    );
    let output = solve("permutations-4.txt", &["--count"]);
    assert_eq!(stdout(&output), "solutions: 24\n");
}

#[test]
fn dead_cells_are_drawn() {
    let output = solve("dead-cells.txt", &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "xAAA\nBBx.\nB.CC\n");
}

#[test]
fn no_solution_exits_with_1() {
    let output = solve("no-solution.txt", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "no solution\n");
    let output = solve("no-solution.txt", &["--count"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "solutions: 0\n");
}

#[test]
fn missing_file_exits_with_2() {
    let output = solve("does-not-exist.txt", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("cannot read"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn parse_error_exits_with_2_and_names_the_line() {
    let path = std::env::temp_dir().join(format!("endlogic-cli-test-{}.txt", std::process::id()));
    std::fs::write(&path, "endlogic 1\ngrid\n..\n.?\n").unwrap();
    let output = run(&["solve", path.to_str().unwrap()]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("line 4: invalid grid character `?`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn help_and_usage_errors() {
    let output = run(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("endlogic solve <FILE>"));
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("missing command"));
    let output = run(&["solve", "a.txt", "--bogus"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn exhausted_budget_exits_with_3() {
    let output = solve("wln006-2.txt", &["--budget", "1"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        stdout(&output),
        "search budget of 1 nodes exhausted after 0 solution(s)\n"
    );
    let output = solve("permutations-4.txt", &["--count", "--budget", "10"]);
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn gen_with_fixed_seed_is_stable() {
    let args = [
        "gen",
        "--difficulty",
        "hard",
        "--seed",
        "1",
        "--solution",
        "--code",
    ];
    let first = run(&args);
    let second = run(&args);
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);

    let text = stdout(&first);
    assert!(text.starts_with("endlogic 1\ngrid\n"), "{text}");
    assert!(text.contains("\n// solution:\n"), "{text}");
    assert!(stderr(&first).starts_with("seed 1, attempt "));

    // The whole output, comments included, is a valid puzzle file whose
    // code line decodes to the same puzzle.
    let puzzle = endlogic_core::format::parse(&text).unwrap();
    let code = text
        .lines()
        .find_map(|line| line.strip_prefix("// code: "))
        .unwrap();
    assert_eq!(endlogic_core::code::decode(code).unwrap(), puzzle);

    let other = run(&["gen", "--difficulty", "hard", "--seed", "2"]);
    assert_ne!(stdout(&other), text);
}

#[test]
fn encode_and_decode_are_inverse() {
    for name in [
        "wln006-2.txt",
        "wlh9000-1.txt",
        "dead-cells.txt",
        "no-solution.txt",
        "permutations-4.txt",
    ] {
        let path = puzzle_path(name);
        let encoded = run(&["encode", path.to_str().unwrap()]);
        assert_eq!(encoded.status.code(), Some(0), "{name}");
        let code = stdout(&encoded).trim().to_owned();
        let decoded = run(&["decode", &code]);
        assert_eq!(decoded.status.code(), Some(0), "{name}");
        let original = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            endlogic_core::format::parse(&stdout(&decoded)).unwrap(),
            endlogic_core::format::parse(&original).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn decode_rejects_bad_codes() {
    let output = run(&["decode", "not*a*code"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("invalid character `*`"),
        "{}",
        stderr(&output)
    );
}
