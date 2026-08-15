#!/usr/bin/env bash
# Mirror .github/workflows/conformance.yml cargo gates for HelixTest.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "ci-check: cargo fmt --check"
cargo fmt --all -- --check

echo "ci-check: cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "ci-check: tests"
# Live-stack crates stay excluded; same as `make test` / CI `make prove`.
make test

echo "ci-check: OK"
