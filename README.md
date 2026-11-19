# endlogic

Solver, generator and (later) trainer for the "mechanism repair" puzzles
from Arknights: Endfield.

A puzzle is a rectangular grid of free, locked and dead cells, a set of
polyominoes, and a target count of filled cells for every row and column.
Every piece must be placed exactly once on free cells, without overlaps;
pieces may be rotated but not mirrored. Locked cells count as filled, dead
cells do not.

## Layout

- `crates/endlogic-core` — dependency-free `no_std` library: model, text
  format, solver with a node budget, solution verifier, random number
  generator, puzzle generator and puzzle codes. Builds for
  `wasm32-unknown-unknown`.
- `crates/endlogic-cli` — the `endlogic` command-line tool.
- `puzzles/` — sample puzzles.
- `docs/FORMAT.md` — the puzzle file format.
- `docs/CODE.md` — the puzzle code format.

## Building

Requires [rustup](https://rustup.rs) (the nightly toolchain is selected by
`rust-toolchain.toml`) and [just](https://github.com/casey/just).

```sh
just ci
```

checks formatting, runs clippy and the tests, and builds the core for wasm.

## Using the CLI

Solve a puzzle (pieces are drawn as letters `A`, `B`, ... in file order):

```sh
just solve puzzles/wln006-2.txt
```

Count solutions up to a limit, or give the search a different node budget
(exit code 3 if the budget runs out):

```sh
cargo run --release -p endlogic-cli -- solve puzzles/wlh9000-1.txt --count 100 --budget 50000
```

Generate a puzzle with a unique solution; `--solution` and `--code` append
the solution and the puzzle code as comment lines, so the output is still a
valid puzzle file:

```sh
cargo run --release -p endlogic-cli -- gen --difficulty hard --seed 1 --solution --code
```

Convert between puzzle files and shareable codes:

```sh
cargo run --release -p endlogic-cli -- encode puzzles/wln006-2.txt
```

```sh
cargo run --release -p endlogic-cli -- decode AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ
```

`endlogic --help` lists all options and exit codes.
