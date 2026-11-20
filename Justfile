set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Wasm features the compiler uses; wasm-opt must be told about them.
wasm_features := "--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue"

# Release builds rebuild the standard library for size and abort at once on a panic.
build_std := "'-Zbuild-std=std,panic_abort' -Zpanic-immediate-abort"

# List available recipes.
default:
    @just --list

# Format all crates in place.
fmt:
    cargo fmt --all

# Check formatting without changing files.
fmt-check:
    cargo fmt --all --check

# Run clippy on every target, treating warnings as errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all tests.
test:
    cargo test --workspace

# Build the core and UI crates for wasm32-unknown-unknown.
check-wasm:
    cargo build -p endlogic-core -p endlogic-ui --target wasm32-unknown-unknown

# Build the web shell for wasm32-unknown-unknown.
check-web:
    cargo build -p endlogic-web --target wasm32-unknown-unknown

# Check formatting, lint, test and build for wasm, in that order.
ci: fmt-check lint test check-wasm check-web

# Run the benchmarks of the core and the trainer (release build); prints tables.
bench:
    cargo bench -p endlogic-core -p endlogic-ui --bench core --bench ui

# Build the release artifacts and print their sizes in bytes.
[unix]
size: cli-build web-build
    wc -c "target/$(rustc -vV | sed -n 's/^host: //p')/release/endlogic" web/dist/endlogic_web_bg.wasm web/dist/endlogic_web.js

# Build the release artifacts and print their sizes in bytes.
[windows]
size: cli-build web-build win-build
    Get-Item target/x86_64-pc-windows-msvc/release/endlogic.exe, target/x86_64-pc-windows-msvc/release/endlogic-win.exe, web/dist/endlogic_web_bg.wasm, web/dist/endlogic_web.js | ForEach-Object { '{0,10}  {1}' -f $_.Length, $_.Name }

# Rewrite every golden file with the current results; review git diff after.
[unix]
[confirm("Rewrite all golden files with the current results? This accepts every difference as intended. [y/N]")]
bless:
    ENDLOGIC_BLESS=1 cargo test --workspace --test golden

# Rewrite every golden file with the current results; review git diff after.
[windows]
[confirm("Rewrite all golden files with the current results? This accepts every difference as intended. [y/N]")]
bless:
    $env:ENDLOGIC_BLESS = '1'; cargo test --workspace --test golden

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[unix]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve {{ quote(FILE) }} {{ ARGS }}

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[windows]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve '{{ replace(FILE, "'", "''") }}' {{ ARGS }}

# Build the release CLI with the standard library rebuilt for size.
[unix]
cli-build:
    CARGO_PROFILE_RELEASE_PANIC=immediate-abort cargo build -p endlogic-cli --release --target "$(rustc -vV | sed -n 's/^host: //p')" {{ build_std }}

# Build the release CLI with the standard library rebuilt for size.
[windows]
cli-build:
    $env:CARGO_PROFILE_RELEASE_PANIC = 'immediate-abort'; cargo build -p endlogic-cli --release --target x86_64-pc-windows-msvc {{ build_std }}

# Build the web version (release) into web/dist; needs wasm-bindgen-cli and wasm-opt of the binaryen version in Cargo.toml.
[unix]
web-build:
    v="$(sed -n 's/^binaryen = "\(.*\)"$/\1/p' Cargo.toml)"; command -v wasm-opt >/dev/null || { echo "wasm-opt not found: install binaryen version_$v" >&2; exit 1; }; wasm-opt --version | grep -q "version $v " || { echo "wasm-opt must come from binaryen version_$v, found: $(wasm-opt --version)" >&2; exit 1; }
    CARGO_PROFILE_RELEASE_PANIC=immediate-abort CARGO_PROFILE_RELEASE_STRIP=debuginfo cargo build -p endlogic-web --release --target wasm32-unknown-unknown {{ build_std }}
    wasm-bindgen --target web --no-typescript --out-dir web/dist target/wasm32-unknown-unknown/release/endlogic_web.wasm
    wasm-opt -Oz {{ wasm_features }} --strip-debug --strip-producers -o web/dist/endlogic_web_bg.wasm web/dist/endlogic_web_bg.wasm
    cp web/index.html web/dist/index.html

# Build the web version (release) into web/dist; needs wasm-bindgen-cli and wasm-opt of the binaryen version in Cargo.toml.
[windows]
web-build:
    $v = (Select-String -Path Cargo.toml -Pattern '^binaryen = .(.+).$').Matches[0].Groups[1].Value; if (-not (Get-Command wasm-opt -ErrorAction SilentlyContinue)) { Write-Host ('wasm-opt not found: install binaryen version_' + $v); exit 1 }; $have = wasm-opt --version; if ($have -notmatch ('version ' + $v + ' ')) { Write-Host ('wasm-opt must come from binaryen version_' + $v + ', found: ' + $have); exit 1 }
    $env:CARGO_PROFILE_RELEASE_PANIC = 'immediate-abort'; $env:CARGO_PROFILE_RELEASE_STRIP = 'debuginfo'; cargo build -p endlogic-web --release --target wasm32-unknown-unknown {{ build_std }}
    wasm-bindgen --target web --no-typescript --out-dir web/dist target/wasm32-unknown-unknown/release/endlogic_web.wasm
    wasm-opt -Oz {{ wasm_features }} --strip-debug --strip-producers -o web/dist/endlogic_web_bg.wasm web/dist/endlogic_web_bg.wasm
    Copy-Item web/index.html web/dist/index.html

# Serve web/dist at http://localhost:8080 (needs Python 3); stop with Ctrl+C.
[unix]
web-serve:
    python3 -m http.server 8080 --bind 127.0.0.1 --directory web/dist

# Serve web/dist at http://localhost:8080 (needs Python 3); stop with Ctrl+C.
[windows]
web-serve:
    python -m http.server 8080 --bind 127.0.0.1 --directory web/dist

# Build the Windows app (release): target/x86_64-pc-windows-msvc/release/endlogic-win.exe.
[windows]
win-build:
    $env:CARGO_PROFILE_RELEASE_PANIC = 'immediate-abort'; cargo build -p endlogic-win --bin endlogic-win --release --target x86_64-pc-windows-msvc {{ build_std }}

# Build the Windows app (release): target/x86_64-pc-windows-msvc/release/endlogic-win.exe.
[unix]
win-build:
    @echo "win-build is available on Windows only"

# Run the Windows app; an optional link or code opens that puzzle.
[windows]
win-run *ARGS:
    cargo run -q -p endlogic-win --release -- {{ ARGS }}

# Run the Windows app; an optional link or code opens that puzzle.
[unix]
win-run *ARGS:
    @echo "win-run is available on Windows only"

# Draw one frame into a PNG, e.g. just win-shot out.png 1366x768 --lang uk
[windows]
win-shot FILE SIZE *ARGS:
    cargo run -q -p endlogic-win --release -- --screenshot '{{ replace(FILE, "'", "''") }}' --size {{ SIZE }} {{ ARGS }}

# Draw one frame into a PNG, e.g. just win-shot out.png 1366x768 --lang uk
[unix]
win-shot FILE SIZE *ARGS:
    @echo "win-shot is available on Windows only"
