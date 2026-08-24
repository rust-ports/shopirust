#!/usr/bin/env python3
"""Verify a Shopify CLI checkout against the reviewable theme parity lock."""

from __future__ import annotations

import hashlib
import csv
import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = ROOT / "parity/theme-upstream.lock.toml"


def fail(message: str) -> None:
    print(f"theme parity drift: {message}", file=sys.stderr)
    raise SystemExit(1)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative_files(root: Path, pattern: str) -> list[str]:
    return sorted(path.relative_to(root).as_posix() for path in root.glob(pattern))


def main() -> None:
    if len(sys.argv) not in (2, 3):
        fail("usage: scripts/theme-parity-check.py /path/to/shopify-cli [rust-shopify-binary]")
    upstream = Path(sys.argv[1]).resolve()
    if not (upstream / ".git").exists():
        fail(f"not a git checkout: {upstream}")

    lock = tomllib.loads(LOCK_PATH.read_text())
    baseline = lock["upstream"]
    revision = subprocess.run(
        ["git", "-C", str(upstream), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if revision != baseline["commit"]:
        fail(f"commit is {revision}, expected {baseline['commit']}")

    package = json.loads((upstream / "packages/theme/package.json").read_text())
    expected_versions = {
        "theme": package["version"],
        "theme_check": package["dependencies"]["@shopify/theme-check-node"],
        "language_server": package["dependencies"]["@shopify/theme-language-server-node"],
    }
    for key, actual in expected_versions.items():
        if actual != baseline[key]:
            fail(f"{key} is {actual}, expected {baseline[key]}")

    theme_root = upstream / "packages/theme"
    tests = relative_files(theme_root, "src/**/*.test.ts")
    if tests != lock["inventory"]["test_files"]:
        missing = sorted(set(lock["inventory"]["test_files"]) - set(tests))
        added = sorted(set(tests) - set(lock["inventory"]["test_files"]))
        fail(f"test inventory changed (missing={missing}, added={added})")

    with (ROOT / "parity/theme-test-matrix.csv").open(newline="") as matrix_file:
        rows = list(csv.DictReader(matrix_file))
    mapped = [row["upstream_test"] for row in rows]
    if len(mapped) != len(set(mapped)):
        fail("test matrix contains duplicate upstream test files")
    if sorted(mapped) != tests:
        missing = sorted(set(tests) - set(mapped))
        extra = sorted(set(mapped) - set(tests))
        fail(f"test matrix is incomplete (missing={missing}, extra={extra})")
    allowed_statuses = {"ported", "partial", "n/a"}
    if any(row["status"] not in allowed_statuses or not row["rust_test_location"] for row in rows):
        fail("test matrix rows require a Rust location and a valid status")

    command_root = theme_root / "src/cli/commands/theme"
    commands = relative_files(command_root, "**/*.ts")
    commands = [path.removesuffix(".ts") for path in commands if not path.endswith(".test.ts")]
    if commands != lock["inventory"]["commands"]:
        fail(f"command inventory changed: {commands}")

    if len(sys.argv) == 3:
        rust_cli = Path(sys.argv[2]).resolve()
        result = subprocess.run(
            [str(rust_cli), "commands", "--all", "--json"],
            check=True,
            capture_output=True,
            text=True,
        )
        manifest = json.loads(result.stdout)
        rust_commands = sorted(
            row["id"].removeprefix("theme:").replace(":", "/")
            for row in manifest
            if row["id"].startswith("theme:")
        )
        if rust_commands != sorted(commands):
            missing = sorted(set(commands) - set(rust_commands))
            extra = sorted(set(rust_commands) - set(commands))
            fail(f"Rust command manifest differs (missing={missing}, extra={extra})")

    for relative, expected in lock["graphql"].items():
        path = upstream / relative
        if not path.is_file():
            fail(f"GraphQL document is missing: {relative}")
        actual = sha256(path)
        if actual != expected:
            fail(f"GraphQL document changed: {relative} ({actual})")

    for relative, expected in lock["command_sources"].items():
        path = theme_root / relative
        if not path.is_file() or sha256(path) != expected:
            fail(f"command/flag contract changed: packages/theme/{relative}")

    print(
        f"theme parity baseline OK: @shopify/theme {baseline['theme']} "
        f"at {baseline['commit'][:12]} ({len(tests)} test files)"
    )


if __name__ == "__main__":
    main()
