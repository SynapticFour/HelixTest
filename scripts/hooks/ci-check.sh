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
# Live-stack crates (api/auth/e2e/workflow-tests) need running services; same excludes as CI.
cargo test --workspace \
  --exclude api-tests \
  --exclude auth-tests \
  --exclude e2e-tests \
  --exclude workflow-tests

echo "ci-check: OK"
