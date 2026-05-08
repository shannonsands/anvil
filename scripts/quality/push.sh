#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

scripts/quality/fast.sh
scripts/quality/coverage.sh
scripts/quality/crap.sh
