# endlogic

Solver (and, later, generator and trainer) for the "mechanism repair" puzzles
from Arknights: Endfield.

A puzzle is a rectangular grid of free, locked and dead cells, a set of
polyominoes, and a target count of filled cells for every row and column.
Every piece must be placed exactly once on free cells, without overlaps;
pieces may be rotated but not mirrored. Locked cells count as filled, dead
cells do not.

## Layout

- `crates/endlogic-core` — dependency-free library: model, text format,
  solver, solution verifier. Builds for `wasm32-unknown-unknown`.
- `crates/endlogic-cli` — the `endlogic` command-line tool.
- `puzzles/` — sample puzzles.
- `docs/FORMAT.md` — the puzzle file format.

## Building

Requires [rustup](https://rustup.rs) (the nightly toolchain is selected by
`rust-toolchain.toml`) and [just](https://github.com/casey/just).

```sh
just ci
```

runs formatting, clippy, tests and the wasm build of the core.

## Running the solver

```sh
just solve puzzles/wln006-2.txt
```

```sh
cargo run --release -p endlogic-cli -- solve puzzles/wln006-2.txt --count 100
```

`solve FILE` prints the first solution, with each piece drawn as a letter
(`A` for the first piece in the file, `B` for the second, ...). `--count [N]`
counts solutions up to `N` (default 1000).
