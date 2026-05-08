#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

need_cmd cargo

if ! cargo tarpaulin --version >/dev/null 2>&1; then
    printf 'cargo-tarpaulin is required for coverage. Install with: cargo install cargo-tarpaulin\n' >&2
    exit 127
fi

REPORT_DIR="${ANVIL_COVERAGE_DIR:-target/quality/tarpaulin}"
FAIL_UNDER="${ANVIL_COVERAGE_FAIL_UNDER:-60}"
TIMEOUT="${ANVIL_TARPAULIN_TIMEOUT:-120}"

mkdir -p "$REPORT_DIR"

step "coverage via cargo tarpaulin"
cargo tarpaulin \
    --workspace \
    --all-features \
    --follow-exec \
    --exclude-files "*/tests/*" "crates/anvil-acceptance/*" \
    --out Json \
    --out Html \
    --output-dir "$REPORT_DIR" \
    --timeout "$TIMEOUT" \
    --fail-under "$FAIL_UNDER"

printf '\ncoverage report: %s/tarpaulin-report.html\n' "$REPORT_DIR"
