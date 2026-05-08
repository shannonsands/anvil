#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

COVERAGE_REPORT="${ANVIL_COVERAGE_REPORT:-target/quality/tarpaulin/tarpaulin-report.json}"
BASELINE="${ANVIL_CRAP_BASELINE:-.quality/crap-baseline.json}"
THRESHOLD="${ANVIL_CRAP_THRESHOLD:-30}"
ALLOWED_INCREASE="${ANVIL_CRAP_ALLOWED_INCREASE:-0.5}"

step "CRAP analysis"
python3 scripts/quality/crap.py \
    --coverage "$COVERAGE_REPORT" \
    --baseline "$BASELINE" \
    --threshold "$THRESHOLD" \
    --allowed-increase "$ALLOWED_INCREASE"
