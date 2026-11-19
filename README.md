# endlogic

**Play it in the browser: https://andrew000.github.io/endlogic/**

Solver, generator and trainer for the "mechanism repair" puzzles from
Arknights: Endfield.

This is a fan project. It is not affiliated with or endorsed by the
developers or publishers of Arknights: Endfield.

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
- `crates/endlogic-ui` — the trainer itself: state, rules, layout and
  drawing, independent of any platform.
- `crates/endlogic-web` — the browser shell for the trainer.
- `web/` — the web page.
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

`--allow-multiple` also accepts puzzles with more than one solution.

Convert between puzzle files and shareable codes:

```sh
cargo run --release -p endlogic-cli -- encode puzzles/wln006-2.txt
```

```sh
cargo run --release -p endlogic-cli -- decode AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ
```

`endlogic --help` lists all options and exit codes.

## Web trainer

Drag pieces onto the board until every row and column count matches; `R`
or a tap rotates, `Tab` switches the counters, `H` gives a hint, `Esc`
cancels a drag. The first hint shows the cells the solution covers; each
next one puts a piece in place or points out a misplaced one.

The custom tab copies a puzzle from the game: set the board size, tap cells
to cycle free, locked and dead, tap a counter to type the clues, and draw
the pieces. "Solve" lists up to 50 solutions; "Play" plays the puzzle.

A puzzle's link (`#p=<code>`) opens the same puzzle; an editor link
(`#e=<code>`) opens it in the custom tab.

Building needs the `wasm32-unknown-unknown` target (installed by
`rust-toolchain.toml`) and `wasm-bindgen-cli` of the same version as the
`wasm-bindgen` crate:

```sh
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

Build into `web/dist`:

```sh
just web-build
```

Serve it at http://localhost:8080 (needs Python 3; stop with Ctrl+C):

```sh
just web-serve
```

`web/dist` is a plain static site and can be served by any web server.

The `Deploy to GitHub Pages` workflow builds the same site and publishes it;
it and the `CI` workflow run only when started by hand from the Actions tab.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
