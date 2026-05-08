#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

need_cmd cargo

if ! cargo mutants --help >/dev/null 2>&1; then
    printf 'cargo-mutants is required for mutation testing. Install with: cargo install cargo-mutants\n' >&2
    exit 127
fi

TIMEOUT="${ANVIL_MUTATION_TIMEOUT:-300}"
MIN_TIMEOUT="${ANVIL_MUTATION_MINIMUM_TEST_TIMEOUT:-60}"
JOBS="${ANVIL_MUTATION_JOBS:-}"

step "mutation testing"
if [ -n "$JOBS" ]; then
    cargo mutants \
        --all-features \
        --timeout "$TIMEOUT" \
        --minimum-test-timeout "$MIN_TIMEOUT" \
        --jobs "$JOBS"
else
    cargo mutants \
        --all-features \
        --timeout "$TIMEOUT" \
        --minimum-test-timeout "$MIN_TIMEOUT"
fi
