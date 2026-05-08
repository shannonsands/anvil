#!/usr/bin/env sh
set -eu

repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || pwd
}

step() {
    printf '\n==> %s\n' "$*"
}

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'missing required command: %s\n' "$1" >&2
        return 127
    fi
}
