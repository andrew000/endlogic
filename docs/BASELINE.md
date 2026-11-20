# Baseline

Speed and size of endlogic before any work on size, speed or `no_std` in
the trainer. Later measurements are compared with this one.

- Date: 2026-11-20.
- Code: commit `e81ff18` (the benchmarks and golden tests were added on top
  of it without changing the application).
- Machine: AMD Ryzen 5 5600G (6 cores, 12 threads, 3.9 GHz), 32 GB RAM,
  Windows 11 Pro build 26300.
- Toolchain: `rustc 1.102.0-nightly (65ce8611f 2026-11-18)`, LLVM 23.1.6,
  `cargo 1.102.0-nightly (f93c676e3 2026-11-15)`; `windows` 0.62.2,
  `wasm-bindgen` 0.2.130.
- Size tools (developer tools, not dependencies): `cargo-bloat` 0.12.1,
  `twiggy` 0.8.0, `wasm-opt` 116 (the `wasm-opt` crate 0.116.1).

Reproduce with `just bench` (about 30 seconds) and `just size`.

## Benchmarks

`just bench` runs `crates/endlogic-core/benches/core.rs` and
`crates/endlogic-ui/benches/ui.rs` in the release profile. Every case runs
in batches of at least 2 ms for at least 150 ms; *mean* is over all batches,
*best* is the best batch. Short cases (below about 0.2 ms) vary by up to
2x between runs on this machine; compare *best* values first. *Speed* is
nodes per second at the mean time.

### Solver: full search of the samples

| puzzle         | solutions | nodes | mean     | best    | speed          |
|----------------|-----------|-------|----------|---------|----------------|
| dead-cells     | 1         | 7     | 8.7 µs   | 8.0 µs  | 0.80 M nodes/s |
| no-solution    | 0         | 0     | 1.9 µs   | 1.8 µs  | 0.00 M nodes/s |
| permutations-4 | 24        | 124   | 35.1 µs  | 27.3 µs | 3.53 M nodes/s |
| wlh9000-1      | 5         | 170   | 67.2 µs  | 55.9 µs | 2.53 M nodes/s |
| wln006-2       | 1         | 30    | 141.8 µs | 97.3 µs | 0.21 M nodes/s |

### Solver: uniqueness check, generated puzzles of seeds 0-19

`check_uniqueness` with the default budget of 200,000 nodes; every puzzle
is unique.

| level  | puzzles | nodes, mean | nodes, most | time, mean | time, worst | speed          |
|--------|---------|-------------|-------------|------------|-------------|----------------|
| easy   | 20      | 22          | 119         | 26.6 µs    | 78.6 µs     | 0.83 M nodes/s |
| normal | 20      | 46          | 170         | 53.4 µs    | 136.7 µs    | 0.87 M nodes/s |
| hard   | 20      | 72          | 323         | 136.9 µs   | 394.8 µs    | 0.53 M nodes/s |
| expert | 20      | 1244        | 15666       | 2.47 ms    | 33.44 ms    | 0.50 M nodes/s |

### Solver: large generated fields

Generated with own parameters, seed 3, one solution required (not a unique
one), pieces of 4 to 9 cells at most 5 long; 10x10: 8 to 10 pieces, 12x12:
10 to 12, 16x16: 9 to 11. Then searched once with a budget of 2,000,000
nodes.

| field | pieces | task           | nodes  | finished       | time      | speed          |
|-------|--------|----------------|--------|----------------|-----------|----------------|
| 10x10 | 8      | first solution | 3306   | yes, found     | 7.06 ms   | 0.47 M nodes/s |
| 10x10 | 8      | uniqueness (2) | 3335   | yes, found     | 7.44 ms   | 0.45 M nodes/s |
| 12x12 | 10     | first solution | 89830  | yes, found     | 338.63 ms | 0.27 M nodes/s |
| 12x12 | 10     | uniqueness (2) | 114132 | yes, exhausted | 424.63 ms | 0.27 M nodes/s |
| 16x16 | 9      | first solution | 24679  | yes, found     | 55.43 ms  | 0.45 M nodes/s |
| 16x16 | 9      | uniqueness (2) | 38241  | yes, exhausted | 86.59 ms  | 0.44 M nodes/s |

A 16x16 field with 14 to 16 pieces could not be generated at all: with
200,000 nodes per attempt, none of 50 attempts confirmed a solution (about
70 s per seed for seeds 1 to 3).

### Generator: one accepted puzzle, seeds 0-19

| level  | attempts, mean | time, mean | time, worst | worst seed |
|--------|----------------|------------|-------------|------------|
| easy   | 1.60           | 45.5 µs    | 114.5 µs    | 13         |
| normal | 1.60           | 150.6 µs   | 425.1 µs    | 10         |
| hard   | 1.60           | 206.4 µs   | 589.6 µs    | 10         |
| expert | 1.45           | 2.67 ms    | 34.40 ms    | 16         |

### Puzzle codes

| puzzle          | code length | encode | decode |
|-----------------|-------------|--------|--------|
| wln006-2 (7x7)  | 38          | 1.2 µs | 2.5 µs |
| generated 16x16 | 108         | 2.4 µs | 6.2 µs |

### Trainer: one idle frame

`App::frame` with a painter that does nothing (text is measured as 0.55 em
per character), after the app has settled; Expert puzzle from the fixed
seed.

| scene                                        | mean    | best    |
|----------------------------------------------|---------|---------|
| expert, wide 1366x768                        | 4.5 µs  | 4.2 µs  |
| expert, wide 1920x1080                       | 4.4 µs  | 4.2 µs  |
| expert, low wide 844x390                     | 4.6 µs  | 4.0 µs  |
| expert, tall 390x844, touch                  | 4.1 µs  | 4.0 µs  |
| editor 16x16, 24 pieces, wide 1366x768       | 30.2 µs | 26.3 µs |
| editor 16x16, 24 pieces, tall 390x844, touch | 25.5 µs | 23.3 µs |

### Trainer: layout alone

`Layout::compute` for the same sizes.

| input                             | mean   | best   |
|-----------------------------------|--------|--------|
| expert, wide 1366x768             | 2.4 µs | 2.3 µs |
| expert, wide 1920x1080            | 2.5 µs | 2.5 µs |
| expert, low wide 844x390          | 247 ns | 232 ns |
| expert, tall 390x844, touch       | 376 ns | 333 ns |
| editor 16x16, wide 1366x768       | 250 ns | 223 ns |
| editor 16x16, tall 390x844, touch | 292 ns | 271 ns |

### The same benchmarks built for size

With `opt-level = "z"`, `lto = "fat"` and `codegen-units = 1` the solver
is about 1.8 times slower (Expert uniqueness 2.47 → 4.46 ms mean; 12x12
first solution 339 → 610 ms; codes 2x slower) and frames about 15 to 30 %
slower (Expert wide 4.5 → 5.2 µs, editor 30.2 → 38.5 µs).

## Sizes

`just size` (release profile as in the repository: `opt-level = 3`, no LTO,
16 codegen units, unwinding panics, static C runtime on Windows).

| artifact | bytes | gzip -9 |
|---|---:|---:|
| `endlogic-win.exe` | 696,320 | |
| `endlogic.exe` (CLI) | 423,424 | |
| `endlogic_web_bg.wasm` | 453,029 | 163,691 |
| `endlogic_web.js` | 29,661 | 5,692 |

### endlogic-win.exe by origin

From `cargo bloat --crates` and the list of all functions. `cargo-bloat`
builds with debug information, so its file is 699,392 bytes; the split
below is of that file. Generic code is counted where `cargo-bloat` puts
it, mostly in the crate that instantiates it. The PE sections of the real
file: `.text` 529,408, `.rdata` 140,288 (constants, import tables, unwind
data), `.pdata` 18,944, `.data` 3,072, `.reloc` 3,072, `.fptable` 512,
headers 1,024.

| part | bytes | share |
|---|---:|---:|
| endlogic-ui | 173,495 | 24.8% |
| std (core, alloc, std) | 114,862 | 16.4% |
| endlogic-core | 112,891 | 16.1% |
| static C runtime and unknown (includes the float math) | 63,723 | 9.1% |
| endlogic-win | 32,190 | 4.6% |
| `windows` crates | 3,796 | 0.5% |
| text not attributed to a function | 31,011 | 4.4% |
| other sections: constants, imports, unwind tables, headers | 167,424 | 23.9% |

std inside the .exe:

| part | bytes | share of file |
|---|---:|---:|
| formatting (`core::fmt`, float printing) | 31,406 | 4.5% |
| runtime: start-up, stderr, environment, paths | 29,151 | 4.2% |
| allocation and collections | 25,962 | 3.7% |
| panics and backtraces | 14,728 | 2.1% |
| strings and chars | 6,862 | 1.0% |
| other | 6,753 | 1.0% |

Most of the runtime part (stderr, environment, paths) is there to print
panic messages and backtraces; panics, backtraces and that runtime together
are about 44 KB. Display and Debug impls of the project's own types are
only 804 bytes.

20 largest functions:

| bytes | crate | function |
|---:|---|---|
| 20,457 | endlogic_ui | `endlogic_ui::draw::draw` |
| 17,891 | endlogic_ui | `<endlogic_ui::layout::Layout>::compute` |
| 17,501 | endlogic_ui | `endlogic_ui::draw::edit::draw` |
| 16,880 | endlogic_core | `<endlogic_core::solver::Search>::new` |
| 15,720 | endlogic_core | `<endlogic_core::generator::Generator>::step` |
| 14,494 | endlogic_ui | `<endlogic_ui::app::App>::handle` |
| 10,261 | endlogic_ui | `endlogic_ui::app::App::control_at` |
| 5,749 | endlogic_win | `endlogic_win::win::window::run` |
| 5,580 | endlogic_core | `endlogic_core::code::read_puzzle` |
| 5,158 | endlogic_core | `endlogic_core::code::encode` |
| 5,110 | endlogic_ui | `endlogic_ui::layout::edit::wide` |
| 4,905 | endlogic_ui | `endlogic_ui::layout::edit::tall` |
| 4,599 | endlogic_core | `<alloc::collections::btree::node::Handle<alloc::collections::btree::node::NodeRef<alloc::collections::btree::n…` |
| 4,564 | endlogic_win | `endlogic_win::args::parse` |
| 4,543 | std | `core::num::imp::flt2dec::strategy::dragon::format_shortest` |
| 3,930 | endlogic_core | `endlogic_core::diagnose::diagnose` |
| 3,736 | std | `core::num::imp::flt2dec::strategy::dragon::format_exact` |
| 3,704 | endlogic_ui | `endlogic_ui::layout::edit::low_wide` |
| 3,488 | endlogic_ui | `endlogic_ui::draw::edit::block` |
| 3,411 | endlogic_ui | `<endlogic_ui::app::App>::frame` |

### endlogic_web_bg.wasm by origin

From `twiggy top` over every item. Functions are counted by the first crate
in their name, so generic std code instantiated for project types (vectors,
B-tree maps, sorting of project types, drop glue) counts as std here.

| part | bytes | share |
|---|---:|---:|
| std (core, alloc, compiler-builtins, dlmalloc) | 126,464 | 27.9% |
| endlogic-ui | 106,096 | 23.4% |
| name section (function names) | 97,088 | 21.4% |
| endlogic-core | 56,785 | 12.5% |
| data (string constants, tables) | 33,351 | 7.4% |
| wasm-bindgen, web-sys, js-sys (glue, imports, exports) | 16,495 | 3.6% |
| endlogic-web | 15,166 | 3.3% |
| wasm structure (types, tables, globals) | 1,281 | 0.3% |
| other custom sections | 303 | 0.1% |

std inside the .wasm:

| part | bytes | share of file |
|---|---:|---:|
| allocation and collections | 49,439 | 10.9% |
| formatting (`core::fmt`, float printing) | 33,278 | 7.3% |
| sorting | 17,339 | 3.8% |
| float math (`sinf`, `cosf`, `rem_pio2`, `hypotf`, rounding) | 7,523 | 1.7% |
| strings and chars | 7,200 | 1.6% |
| number parsing (`f32` from text) | 5,996 | 1.3% |
| other | 3,256 | 0.7% |
| panics | 2,433 | 0.5% |

20 largest functions:

| bytes | function |
|---:|---|
| 15,220 | `endlogic_ui::draw::draw` |
| 12,474 | `<endlogic_ui::layout::Layout>::compute` |
| 9,031 | `<endlogic_core::solver::Search>::with_fixed` |
| 9,015 | `<endlogic_core::generator::Generator>::step` |
| 8,897 | `endlogic_ui::draw::edit::draw` |
| 8,433 | `endlogic_core::code::encode` |
| 7,827 | `<endlogic_ui::app::App>::control_at` |
| 7,640 | `<endlogic_ui::app::App>::handle` |
| 6,275 | `endlogic_core::code::decode` |
| 6,080 | `core::slice::sort::shared::smallsort::small_sort_network::<(usize, usize), <(usize, usize) as core::cmp::Parti…` |
| 5,967 | `core::num::imp::flt2dec::strategy::dragon::format_shortest` |
| 4,921 | `core::num::imp::flt2dec::strategy::dragon::format_exact` |
| 4,737 | `<dlmalloc::dlmalloc::Dlmalloc<dlmalloc::sys::System>>::malloc` |
| 4,388 | `compiler_builtins::math::libm_math::rem_pio2_large::rem_pio2_large` |
| 3,783 | `start` |
| 3,505 | `endlogic_core::diagnose::diagnose` |
| 3,316 | `endlogic_ui::layout::edit::wide` |
| 3,244 | `<alloc::collections::btree::node::Handle<alloc::collections::btree::node::NodeRef<alloc::collections::btree::n…` |
| 3,130 | `endlogic_ui::layout::edit::tall` |
| 2,806 | `<endlogic_core::solver::Board>::filter` |

## Size experiments

Each row changes only what it names, through `CARGO_PROFILE_RELEASE_*`
variables and command-line flags in a separate target directory; nothing
in the repository was changed. Percentages are against the first row.
Every `.exe` was checked by drawing the Expert puzzle in screenshot mode:
all pictures are byte-identical to the baseline. Every page was opened in
a browser with an Expert puzzle link, drew it and showed a hint on `H`,
without console errors. The CLI printed the same `gen --difficulty hard
--seed 1` code everywhere.

| variant | endlogic-win.exe | endlogic.exe | endlogic_web_bg.wasm |
|---|---:|---:|---:|
| baseline | 696,320 | 423,424 | 453,029 |
| `opt-level = "s"` | 627,712 (-9.9%) | 389,632 (-8.0%) | 465,385 (+2.7%) |
| `opt-level = "z"` | 605,184 (-13.1%) | 382,976 (-9.6%) | 496,227 (+9.5%) |
| `lto = "fat"`, `codegen-units = 1` | 653,824 (-6.1%) | 399,360 (-5.7%) | 420,145 (-7.3%) |
| `panic = "abort"` | 594,432 (-14.6%) | 383,488 (-9.4%) | 453,394 (+0.1%) |
| `strip = "symbols"` | 696,320 (0.0%) | 423,424 (0.0%) | 359,141 (-20.7%) |
| std rebuilt (`-Zbuild-std`), `panic = "immediate-abort"` | 460,288 (-33.9%) | 279,552 (-34.0%) | 414,004 (-8.6%) |
| `wasm-opt -Oz` on the baseline | | | 330,163 (-27.1%) |
| all of the above except rebuilding std (`z`, LTO, one unit, `abort`, strip, `wasm-opt`) | 452,096 (-35.1%) | 306,688 (-27.6%) | 238,120 (-47.4%) |
| all of the above (`z`, LTO, one unit, std rebuilt with `immediate-abort`, strip, `wasm-opt`) | 321,024 (-53.9%) | 199,680 (-52.8%) | 199,914 (-55.9%) |
| the same with `opt-level = "s"` (page not opened) | 345,600 (-50.4%) | 210,944 (-50.2%) | 221,464 (-51.1%) |

Notes:

- On wasm, `opt-level` `s` and `z` alone make the file larger: less
  inlining means more functions, and the name section grows with them.
  Without names (`strip` or `wasm-opt`) they help.
- `panic = "abort"` changes nothing on wasm, where it is already the
  default.
- `wasm-opt -Oz` also drops the name section; on the combined builds it
  saves a further 16.7 KB (254,845 → 238,120) and 13.5 KB (213,421 →
  199,914). It needs the wasm features listed explicitly
  (`--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
  --enable-mutable-globals --enable-reference-types --enable-multivalue`).
- `strip = "symbols"` removes the `target_features` section too, and
  `wasm-bindgen` then warns that it cannot detect the wasm features; the
  pages still worked. Stripping after `wasm-bindgen` (with `wasm-opt`)
  avoids that.
- Compressed with gzip -9 the wasm goes from 163,691 bytes (baseline) to
  145,512 (`wasm-opt` only), 113,533 (all but rebuilding std) and 99,750
  (all).
- `endlogic_web.js` stays at 28.9 to 29.7 KB in every variant.

## What keeps endlogic-ui on std

The crate has no I/O, threads or platform calls; everything it takes from
std is available in `core` and `alloc` except some `f32` methods.

| kind | where | count | replacement |
|---|---|---:|---|
| `Vec`, `vec!`, `collect` into vectors | app, board, counters, draw, draw/edit, editor, hint, layout, layout/edit, shell | 47 `Vec`, 11 `vec!`, 26 `collect` | `alloc::vec::Vec`, `alloc::vec!`; the same type, so the shell contract (`Config`, `Effect`) does not change |
| `String`, `format!`, `to_owned`, `to_string`, `join` | text (14 `format!`), app, draw, draw/edit, layout, shell | 21 `String`, 23 `format!`, 12 `to_owned`, 3 `to_string`, 1 `join` | `alloc::string::{String, ToString}`, `alloc::format!`, `alloc::borrow::ToOwned`; `core::fmt` stays |
| string reading: `chars`, `split`, `split_whitespace`, `eq_ignore_ascii_case` | app, layout, draw/edit, text | 6 | in `core` already |
| `f32::floor` | app (2), layout (4), layout/edit (2), draw/edit (1) | 9 | `core::f32::math::floor` (unstable `core_float_math`) or a few lines of our own |
| `f32::round` | app (2), painter (2) | 4 | the same as `floor` |
| `f32::hypot` | app (2): nearest cell and drag threshold | 2 | compare squared distances; no square root needed |
| `f32::sin`, `f32::sin_cos` | draw (2): wobble and piece rotation | 2 | an own polynomial (angles are within ±π); may move drawn points by rounding, so `draw.txt` would need blessing |
| `f32::min`, `max`, `clamp`, `abs` | everywhere | 191 | in `core` already |
| `std::mem::take`, `std::mem::replace` | app, layout, draw/edit | 3 | `core::mem` |
| `core::f32::consts` | draw | 1 | already `core` |

Nothing needs a hash map, a `Box`, threads, time or I/O. The integration
tests and benchmarks use std and can keep it.

## Shells and text

In `endlogic-win.exe` the shell's own code is 32,190 bytes and the
`windows` crates 3,796 bytes (thin COM wrappers). The largest shell
functions are the window loop (5.7 KB), command-line parsing (4.6 KB),
the window procedure (3.2 KB) and screenshot mode (3.0 KB, plus 0.9 KB for
PNG saving). Text drawing with DirectWrite is small: `Fonts::layout` 1.9
KB, `Fonts::new` 1.0 KB, `text` 0.7 KB, `text_width` 0.2 KB and about 0.4
KB of DirectWrite wrappers, about 4.3 KB of code;
DirectWrite itself is a system DLL and adds nothing. The heaviest parts
that are not the project's code are the static C runtime (about 64 KB),
the panic and runtime machinery of std (about 44 KB) and the unwind tables
(`.pdata` 19 KB and part of `.rdata`).

In `endlogic_web_bg.wasm` the shell is 15,166 bytes and the
wasm-bindgen glue 16,495 bytes, plus `endlogic_web.js` (29,661 bytes,
5,692 gzipped). Canvas text is `text` 422 bytes, `text_width` 290 bytes
and a few imports (`fillText`, `measureText`, `font`), about 1 KB.

Drawing text with lines in endlogic-ui would remove only those 4.3 KB from
the .exe and about 1 KB from the .wasm, and add glyph outlines for Latin
and Cyrillic in two families and two weights (roughly 300 glyphs of 50 to
150 bytes each, 15 to 45 KB) plus code to lay them out, in both builds. It
would make text look the same everywhere, and the screenshots comparable
across machines, but it would not make either build smaller.

## After the release build settings

Measured on 2026-11-20 on the same machine and toolchain, with the release
profile `opt-level = "z"` (`endlogic-core`: 3), `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`. The artifacts
come from `just win-build`, `just cli-build` and `just web-build`: the
standard library is rebuilt with `-Zbuild-std` and
`panic = "immediate-abort"`, and the wasm goes through `wasm-opt -Oz`
(binaryen 116), which drops the name section.

| artifact | before | after | change |
|---|---:|---:|---:|
| `endlogic-win.exe` | 696,320 | 351,232 | -49.6% |
| `endlogic.exe` (CLI) | 423,424 | 229,376 | -45.8% |
| `endlogic_web_bg.wasm` | 453,029 | 231,475 | -48.9% |
| `endlogic_web_bg.wasm`, gzip -9 | 163,691 | 110,348 | -32.6% |
| `endlogic_web.js` | 29,661 | 29,577 | -0.3% |

Keeping the core at `opt-level = 3` costs about 30 KB in each build against
the all-`z` experiment (321,024 and 199,914 bytes) and keeps the solver at
its speed. The benchmarks (`just bench`, which builds the benches with this
profile; mean times):

| case | before | after |
|---|---:|---:|
| full search, wln006-2 | 141.8 µs | 101.5 µs |
| full search, wlh9000-1 | 67.2 µs | 57.0 µs |
| uniqueness, easy | 26.6 µs | 21.3 µs |
| uniqueness, normal | 53.4 µs | 51.0 µs |
| uniqueness, hard | 136.9 µs | 134.8 µs |
| uniqueness, expert (worst) | 2.47 ms (33.44 ms) | 2.52 ms (34.03 ms) |
| 12x12, first solution | 338.63 ms | 327.76 ms |
| 12x12, uniqueness | 424.63 ms | 433.56 ms |
| 16x16, first solution | 55.43 ms | 60.22 ms |
| 16x16, uniqueness | 86.59 ms | 82.60 ms |
| generator, easy | 45.5 µs | 41.3 µs |
| generator, normal | 150.6 µs | 156.2 µs |
| generator, hard | 206.4 µs | 181.7 µs |
| generator, expert (worst) | 2.67 ms (34.40 ms) | 2.60 ms (32.77 ms) |
| decode wln006-2 | 2.5 µs | 2.6 µs |
| idle frame, expert wide | 4.5 µs | 4.1 µs |
| idle frame, editor 16x16 wide | 30.2 µs | 23.4 µs |
| layout, expert wide | 2.4 µs | 3.1 µs |
| layout, expert tall | 376 ns | 619 ns |

The solver and the generator stay within the run-to-run noise (up to about
10% for the 16x16 case). The trainer's layout, now built for size, is
slower by 30 to 65 %, still well under a microsecond for most inputs.
