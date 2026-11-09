set shell := ["bash", "-cu"]

build:
    cargo build --locked

release:
    cargo build --release --locked

test:
    cargo test --locked

lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt

# Requires the dedicated local Meilisearch test instance from README.
live: release
    python3 tests/seed.py
    python3 tests/live.py

package: release
    python3 scripts/package.py
