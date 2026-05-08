#!/usr/bin/env sh
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
. "$ROOT/scripts/quality/common.sh"

cd "$ROOT"

git config core.hooksPath .githooks
printf 'configured git hooks path: .githooks\n'
