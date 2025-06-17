# Default runs build
default: build

build:
    cargo build --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt -- --check

clippy:
    cargo clippy --workspace -- -D warnings

doc:
    cargo doc --no-deps --workspace

ci: fmt clippy test
