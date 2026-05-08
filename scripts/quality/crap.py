#!/usr/bin/env python3
"""Coverage-backed CRAP-style maintainability gate for Anvil Rust code."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path


FN_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\b")
IMPL_RE = re.compile(r"^\s*impl(?:\s*<[^>{}]*>)?\s+([A-Za-z_][A-Za-z0-9_:<>]*)")
CONTROL_PATTERNS = (
    re.compile(r"\bif\b"),
    re.compile(r"\bmatch\b"),
    re.compile(r"\bfor\b"),
    re.compile(r"\bwhile\b"),
    re.compile(r"\bloop\b"),
    re.compile(r"&&"),
    re.compile(r"\|\|"),
    re.compile(r"\?"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--coverage",
        default="target/quality/tarpaulin/tarpaulin-report.json",
        help="tarpaulin JSON report path",
    )
    parser.add_argument(
        "--baseline",
        default=".quality/crap-baseline.json",
        help="approved CRAP baseline path",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=30.0,
        help="CRAP score that requires approval",
    )
    parser.add_argument(
        "--allowed-increase",
        type=float,
        default=0.5,
        help="allowed score drift for existing baseline entries",
    )
    parser.add_argument(
        "--update-baseline",
        action="store_true",
        help="write current over-threshold functions as the approved baseline",
    )
    parser.add_argument(
        "--max-report",
        type=int,
        default=25,
        help="maximum functions to print",
    )
    return parser.parse_args()


def report_path(parts: object) -> Path:
    if isinstance(parts, str):
        return Path(parts)
    if not isinstance(parts, list):
        raise TypeError(f"unexpected tarpaulin path shape: {parts!r}")
    text_parts = [str(part) for part in parts]
    if text_parts and text_parts[0] == "/":
        return Path("/" + "/".join(text_parts[1:]))
    return Path(*text_parts)


def load_coverage(path: Path) -> dict[Path, tuple[set[int], set[int]]]:
    if not path.exists():
        raise FileNotFoundError(
            f"{path} does not exist; run scripts/quality/coverage.sh first"
        )

    data = json.loads(path.read_text())
    coverage: dict[Path, tuple[set[int], set[int]]] = {}

    for file_entry in data.get("files", []):
        source_path = report_path(file_entry["path"]).resolve()
        coverable: set[int] = set()
        covered: set[int] = set()
        for trace in file_entry.get("traces", []):
            line = int(trace["line"])
            coverable.add(line)
            hits = trace.get("stats", {}).get("Line", 0)
            if hits and int(hits) > 0:
                covered.add(line)
        coverage[source_path] = (coverable, covered)

    return coverage


def brace_delta(line: str) -> int:
    stripped = re.sub(r'"(?:\\.|[^"\\])*"', '""', line)
    stripped = re.sub(r"//.*", "", stripped)
    return stripped.count("{") - stripped.count("}")


def find_block_end(lines: list[str], start: int) -> int | None:
    depth = 0
    opened = False
    for index in range(start, len(lines)):
        line = lines[index]
        if "{" in line:
            opened = True
        if opened:
            depth += brace_delta(line)
            if depth <= 0:
                return index
    return None


def cfg_test_cutoff(lines: list[str]) -> int:
    for index, line in enumerate(lines):
        if "#[cfg(test)]" in line:
            return index
    return len(lines)


def impl_blocks(lines: list[str], cutoff: int) -> list[tuple[int, int, str]]:
    blocks: list[tuple[int, int, str]] = []
    for index, line in enumerate(lines[:cutoff]):
        match = IMPL_RE.search(line)
        if not match:
            continue
        end = find_block_end(lines, index)
        if end is not None:
            blocks.append((index, end, match.group(1)))
    return blocks


def impl_context(blocks: list[tuple[int, int, str]], fn_start: int) -> str | None:
    matches = [name for start, end, name in blocks if start <= fn_start <= end]
    return matches[-1] if matches else None


def function_blocks(path: Path) -> list[dict[str, object]]:
    lines = path.read_text().splitlines()
    cutoff = cfg_test_cutoff(lines)
    impls = impl_blocks(lines, cutoff)
    functions: list[dict[str, object]] = []

    for index, line in enumerate(lines[:cutoff]):
        match = FN_RE.search(line)
        if not match:
            continue
        end = find_block_end(lines, index)
        if end is None:
            continue
        name = match.group(1)
        context = impl_context(impls, index)
        display = f"{context}::{name}" if context else name
        functions.append(
            {
                "name": display,
                "start": index + 1,
                "end": end + 1,
                "lines": lines[index : end + 1],
            }
        )

    return functions


def complexity(lines: list[str]) -> int:
    score = 1
    for line in lines:
        source = re.sub(r"//.*", "", line)
        for pattern in CONTROL_PATTERNS:
            score += len(pattern.findall(source))
    return score


def function_result(
    repo: Path,
    path: Path,
    fn: dict[str, object],
    coverable: set[int],
    covered: set[int],
) -> dict[str, object] | None:
    start = int(fn["start"])
    end = int(fn["end"])
    fn_coverable = {line for line in coverable if start <= line <= end}
    if not fn_coverable:
        return None
    fn_covered = {line for line in covered if line in fn_coverable}
    cov = len(fn_covered) / len(fn_coverable)
    comp = complexity(list(fn["lines"]))
    crap = comp**2 * (1 - cov) ** 3 + comp
    rel = path.relative_to(repo) if path.is_relative_to(repo) else path
    key = f"{rel}::{fn['name']}"
    return {
        "key": key,
        "path": str(rel),
        "line": start,
        "name": fn["name"],
        "complexity": comp,
        "coverage": round(cov, 4),
        "covered_lines": len(fn_covered),
        "coverable_lines": len(fn_coverable),
        "score": round(crap, 2),
    }


def analyze(repo: Path, coverage: dict[Path, tuple[set[int], set[int]]]) -> list[dict[str, object]]:
    results: list[dict[str, object]] = []
    for path, line_sets in sorted(coverage.items()):
        if not path.exists() or path.suffix != ".rs":
            continue
        if "/tests/" in path.as_posix():
            continue
        coverable, covered = line_sets
        for fn in function_blocks(path):
            result = function_result(repo, path, fn, coverable, covered)
            if result is not None:
                results.append(result)
    return sorted(results, key=lambda item: (-float(item["score"]), str(item["key"])))


def load_baseline(path: Path) -> dict[str, dict[str, object]]:
    if not path.exists():
        return {}
    data = json.loads(path.read_text())
    return data.get("functions", {})


def write_baseline(path: Path, threshold: float, results: list[dict[str, object]]) -> None:
    offenders = {
        str(item["key"]): {
            "score": item["score"],
            "complexity": item["complexity"],
            "coverage": item["coverage"],
            "path": item["path"],
            "line": item["line"],
        }
        for item in results
        if float(item["score"]) > threshold
    }
    payload = {
        "version": 1,
        "threshold": threshold,
        "note": "Approved baseline for existing CRAP offenders. Increases require human approval.",
        "functions": dict(sorted(offenders.items())),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def print_result(item: dict[str, object]) -> None:
    print(
        f"{item['path']}:{item['line']} {item['name']} "
        f"score={item['score']} complexity={item['complexity']} "
        f"coverage={float(item['coverage']) * 100:.1f}% "
        f"lines={item['covered_lines']}/{item['coverable_lines']}"
    )


def main() -> int:
    args = parse_args()
    repo = Path.cwd().resolve()
    coverage = load_coverage(Path(args.coverage))
    results = analyze(repo, coverage)
    offenders = [item for item in results if float(item["score"]) > args.threshold]

    if args.update_baseline:
        write_baseline(Path(args.baseline), args.threshold, results)
        print(
            f"updated {args.baseline} with {len(offenders)} "
            f"function(s) over CRAP threshold {args.threshold:g}"
        )
        return 0

    baseline = load_baseline(Path(args.baseline))
    failures: list[tuple[str, dict[str, object], float | None]] = []

    for item in offenders:
        key = str(item["key"])
        prior = baseline.get(key)
        if prior is None:
            failures.append(("new", item, None))
            continue
        previous_score = float(prior.get("score", math.inf))
        if float(item["score"]) > previous_score + args.allowed_increase:
            failures.append(("worsened", item, previous_score))

    print(
        f"CRAP analyzed {len(results)} function(s); "
        f"{len(offenders)} over threshold {args.threshold:g}."
    )

    if offenders:
        print("\nTop CRAP scores:")
        for item in offenders[: args.max_report]:
            print_result(item)

    if not failures:
        print("\nCRAP gate passed.")
        return 0

    print("\nCRAP gate failed:")
    for kind, item, previous in failures[: args.max_report]:
        if previous is None:
            suffix = "new over-threshold function"
        else:
            suffix = f"worsened from approved score {previous:.2f}"
        print(f"- {kind}: {suffix}")
        print_result(item)

    print(
        "\nAdd focused tests or refactor before changing the baseline. "
        "Use --update-baseline only after human approval."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
