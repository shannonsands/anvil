#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

step "cargo fmt"
cargo fmt --all --check

step "clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Gherkin lint"
python3 scripts/quality/spec_lint.py

step "acceptance specs"
cargo test -p anvil-acceptance --test acceptance

step "workspace tests"
cargo test --workspace --all-features

step "git whitespace check"
git diff --check
