#!/bin/sh

set -e

clean() {
    # some shells will call EXIT after the INT signal
    # causing EXIT trap to be executed, so we trap EXIT after INT
    trap '' EXIT
    
    cargo clean
}

trap clean INT QUIT TERM EXIT

# this script runs the tests and checks that also run as part of the`test.yml` github action workflow
cargo clean
cargo fmt --all -- --check
cargo clippy -p spotipi-core --no-default-features
cargo clippy -p spotipi-core

cargo hack clippy --each-feature -p spotipi-discovery
cargo hack clippy --each-feature -p spotipi-playback
cargo hack clippy --each-feature

cargo build --workspace --examples
cargo test --workspace
cargo check -p spotipi-core --no-default-features
cargo check -p spotipi-core
cargo hack check --no-dev-deps --each-feature -p spotipi-discovery
cargo hack check --no-dev-deps --each-feature -p spotipi-playback
cargo hack check --no-dev-deps --each-feature