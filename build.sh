set -eu pipefail
anchor build
cargo clippy -- --deny clippy::pedantic
cargo +nightly fmt --check
cargo test
