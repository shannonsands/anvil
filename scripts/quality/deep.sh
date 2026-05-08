#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

scripts/quality/push.sh
scripts/quality/mutation.sh
