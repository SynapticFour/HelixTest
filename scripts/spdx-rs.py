#!/usr/bin/env python3
"""Add or verify SPDX-License-Identifier on first-party Rust sources."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SKIP_DIR_NAMES = {
    "third_party",
    "target",
    "vendor",
    ".git",
    "node_modules",
}


def iter_rs(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*.rs"):
        if any(part in SKIP_DIR_NAMES for part in path.parts):
            continue
        files.append(path)
    return sorted(files)


def first_lines(text: str, n: int = 15) -> str:
    return "\n".join(text.splitlines()[:n])


def has_spdx(text: str) -> bool:
    return "SPDX-License-Identifier:" in first_lines(text, 20)


def apply_header(path: Path, license_id: str) -> bool:
    text = path.read_text(encoding="utf-8")
    if has_spdx(text):
        return False
    header = f"// SPDX-License-Identifier: {license_id}\n"
    if text.startswith("#!"):
        nl = text.find("\n")
        if nl == -1:
            path.write_text(text + "\n" + header, encoding="utf-8")
        else:
            path.write_text(text[: nl + 1] + header + text[nl + 1 :], encoding="utf-8")
    else:
        path.write_text(header + text, encoding="utf-8")
    return True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--license", required=True)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    missing: list[Path] = []
    written = 0
    for path in iter_rs(root):
        text = path.read_text(encoding="utf-8")
        if has_spdx(text):
            continue
        missing.append(path)
        if args.write:
            apply_header(path, args.license)
            written += 1
    if args.check:
        if missing:
            print(f"missing SPDX-License-Identifier in {len(missing)} first-party .rs file(s):")
            for path in missing:
                print(f"  {path.relative_to(root)}")
            return 1
        print(f"SPDX OK ({args.license}) under {root}")
        return 0
    if args.write:
        print(f"wrote SPDX headers to {written} file(s) under {root}")
        return 0
    print("pass --check or --write")
    return 2


if __name__ == "__main__":
    sys.exit(main())
