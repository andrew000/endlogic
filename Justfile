set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# List available recipes.
default:
    @just --list

# Format all crates in place.
fmt:
    cargo fmt --all

# Run clippy on every target, treating warnings as errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all tests.
test:
    cargo test --workspace

# Build the core crate for wasm32-unknown-unknown.
check-wasm:
    cargo build -p endlogic-core --target wasm32-unknown-unknown

# Format, lint, test and build for wasm, in that order.
ci: fmt lint test check-wasm

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[unix]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve {{ quote(FILE) }} {{ ARGS }}

# Solve a puzzle file; extra arguments go to the CLI (e.g. --count 50).
[windows]
solve FILE *ARGS:
    cargo run --release -q -p endlogic-cli -- solve '{{ replace(FILE, "'", "''") }}' {{ ARGS }}
