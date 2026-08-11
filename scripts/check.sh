#!/usr/bin/env bash
# Local quality gate — run before every push.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo fmt --check =="
cargo fmt --all -- --check

echo "== cargo clippy (-D warnings) =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== cargo test -p kcell_core --lib =="
cargo test -p kcell_core --lib

echo "check ok"
