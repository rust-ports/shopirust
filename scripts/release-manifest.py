#!/usr/bin/env python3
"""Write a checksum manifest for a staged release directory."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: release-manifest.py <release-root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1]).resolve()
    if not root.is_dir():
        print(f"release root does not exist: {root}", file=sys.stderr)
        return 1
    files = [
        {"path": path.relative_to(root).as_posix(), "sha256": digest(path), "size": path.stat().st_size}
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.name != "manifest.json"
    ]
    (root / "manifest.json").write_text(
        json.dumps({"format": 1, "files": files}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
