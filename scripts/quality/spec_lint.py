#!/usr/bin/env python3
"""Small deterministic lint pass for executable Gherkin specs."""

from __future__ import annotations

import sys
from pathlib import Path


STEP_PREFIXES = ("Given ", "When ", "Then ", "And ", "But ")
STRUCTURAL_PREFIXES = (
    "Feature:",
    "Rule:",
    "Background:",
    "Scenario:",
    "Scenario Outline:",
    "Examples:",
)


def lint_file(path: Path) -> list[str]:
    errors: list[str] = []
    lines = path.read_text().splitlines()
    feature_count = 0
    scenario_count = 0
    in_docstring = False
    seen_scenario = False

    for index, line in enumerate(lines, start=1):
        stripped = line.strip()
        if "\t" in line:
            errors.append(f"{path}:{index}: tabs are not allowed")
        if line.rstrip() != line:
            errors.append(f"{path}:{index}: trailing whitespace")
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == '"""':
            in_docstring = not in_docstring
            continue
        if in_docstring:
            continue
        if stripped.startswith("Feature:"):
            feature_count += 1
            continue
        if stripped.startswith(("Scenario:", "Scenario Outline:")):
            scenario_count += 1
            seen_scenario = True
            continue
        if stripped.startswith(("@", "|")):
            continue
        if stripped.startswith(("Rule:", "Background:", "Examples:")):
            continue
        if stripped.startswith(STEP_PREFIXES):
            if not seen_scenario:
                errors.append(f"{path}:{index}: step appears before first scenario")
            continue
        if not seen_scenario:
            continue
        if not stripped.startswith(STRUCTURAL_PREFIXES):
            errors.append(f"{path}:{index}: unrecognized Gherkin line: {stripped}")

    if in_docstring:
        errors.append(f"{path}: unterminated docstring")
    if feature_count != 1:
        errors.append(f"{path}: expected exactly one Feature, found {feature_count}")
    if scenario_count == 0:
        errors.append(f"{path}: expected at least one Scenario")
    return errors


def main() -> int:
    spec_dir = Path("specs")
    files = sorted(spec_dir.glob("*.feature"))
    if not files:
        print("no specs/*.feature files found")
        return 1

    errors: list[str] = []
    for path in files:
        errors.extend(lint_file(path))

    if errors:
        print("Gherkin lint failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(f"Gherkin lint passed for {len(files)} feature file(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
