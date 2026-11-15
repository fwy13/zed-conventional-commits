#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all
cargo fmt --all -- --check
cargo test --all-targets
cargo build --release
cargo run --quiet -- compose --type feat --scope api --gitmoji :sparkles: --subject 'add new feature' --skip-ci
