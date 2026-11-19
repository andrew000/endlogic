set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

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

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[unix]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve {{ quote(FILE) }} {{ ARGS }}

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[windows]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve '{{ replace(FILE, "'", "''") }}' {{ ARGS }}

# Build the web version (release) into web/dist; needs wasm-bindgen-cli.
[unix]
web-build:
    cargo build -p endlogic-web --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir web/dist target/wasm32-unknown-unknown/release/endlogic_web.wasm
    cp web/index.html web/dist/index.html

# Build the web version (release) into web/dist; needs wasm-bindgen-cli.
[windows]
web-build:
    cargo build -p endlogic-web --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir web/dist target/wasm32-unknown-unknown/release/endlogic_web.wasm
    Copy-Item web/index.html web/dist/index.html

# Serve web/dist at http://localhost:8080 (needs Python 3); stop with Ctrl+C.
[unix]
web-serve:
    python3 -m http.server 8080 --bind 127.0.0.1 --directory web/dist

# Serve web/dist at http://localhost:8080 (needs Python 3); stop with Ctrl+C.
[windows]
web-serve:
    python -m http.server 8080 --bind 127.0.0.1 --directory web/dist
