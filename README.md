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
- `crates/endlogic-win` — the Windows shell for the trainer.
- `web/` — the web page.
- `puzzles/` — sample puzzles.
- `docs/FORMAT.md` — the puzzle file format.
- `docs/CODE.md` — the puzzle code format.
- `docs/BASELINE.md` — benchmark results and binary sizes to compare
  against.

## Building

Requires [rustup](https://rustup.rs) (the nightly toolchain is selected by
`rust-toolchain.toml`) and [just](https://github.com/casey/just).

```sh
just ci
```

checks formatting, runs clippy and the tests, and builds the core for wasm.

## Golden tests

Some tests compare their results with files kept next to them in
`tests/golden`:

| Crate | Golden file | What it holds |
|---|---|---|
| `endlogic-core` | `generator.txt` | code, accepted attempt and solver nodes for every level and seeds 0–19 |
| `endlogic-core` | `solver-solutions.txt` | every solution of the samples, the generated puzzles and searches with fixed pieces, in an order-independent form |
| `endlogic-core` | `solver-nodes.txt` | solver nodes to the first solution and for the whole search |
| `endlogic-core` | `diagnosis.txt` | the problems found in broken puzzles |
| `endlogic-ui` | `hints.txt` | the silhouette, the piece placed and the piece blamed by hints in given positions |
| `endlogic-ui` | `draw.txt` | a fingerprint of the drawing commands of each scene |
| `endlogic-win` | `*.png` | screenshots (Windows only) |

`solver-solutions.txt` must never change: any correct solver finds the same
solutions. `solver-nodes.txt` changes when the search order or pruning
changes, and so may `generator.txt`, `hints.txt` and the pictures. The
generated puzzles the solver is checked on are a fixed list,
`solver-puzzles.txt`; it is written only when missing.

A failing golden test says what differs and where it wrote the actual
result (under `target/tmp/golden`; for `draw.txt`, the full list of drawing
commands of each differing scene). Screenshots may differ by 16 per channel,
and up to 200 pixels in a million may differ by more. When a change is
intended, rewrite the golden files with

```sh
just bless
```

and review `git diff` before committing. The screenshots depend on the
fonts installed; they were made on Windows 11 with Segoe UI and Cascadia
Mono.

## Benchmarks and sizes

```sh
just bench
```

builds in release mode and prints tables: the solver on the samples, on
generated puzzles of every level and on large fields, the generator, puzzle
codes, and the trainer's frame and layout times (drawing into a painter that
does nothing). Inputs are fixed, so runs can be compared.

```sh
just size
```

builds the release artifacts and prints their sizes. `docs/BASELINE.md`
records both as a baseline, with what the binaries are made of and how
build settings change their size.

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
(`#e=<code>`) opens it in the custom tab's editor, and a custom link
(`#c=<code>`) plays it in the custom tab.

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
like the `CI` and `Release` workflows, it runs only when started by hand
from the Actions tab.

## Windows app

`endlogic-win` is the same trainer in a native window (Win32, Direct2D,
DirectWrite), for Windows 10 and 11 on x64. Build it on Windows with the
toolchain from `rust-toolchain.toml`:

```sh
just win-build
```

The result, `target/release/endlogic-win.exe`, is a single file: the C
runtime is linked in, so it runs on a clean system with nothing next to it.
Run it directly, or with a puzzle link or code as the only argument:

```sh
just win-run "https://andrew000.github.io/endlogic/#p=<code>"
```

- "Share" copies a web link to the puzzle, so anyone can open it in a
  browser.
- `Ctrl+V` opens the puzzle in a link or code on the clipboard.
- The app writes nothing to disk and keeps no settings between runs.
- A downloaded `.exe` is not signed, so Windows SmartScreen may warn before
  the first start; "More info", then "Run anyway" starts it.

For checks without a screen, `--screenshot` draws one settled frame into a
PNG file instead of opening the window (the seed is fixed, so the same
arguments give the same picture):

```sh
just win-shot out.png 1366x768 --code "<link or code>" --lang uk --touch
```

The `Release` workflow, started by hand with a version number, builds
`endlogic-win-x64.exe` with its SHA-256 file and publishes both as a GitHub
release.

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
