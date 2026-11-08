#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Verify that relative links in the i18n mirrors resolve."""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r"\]\(([^)\s]+)\)")


def main():
    broken = []
    for path in sorted((ROOT / "i18n").rglob("*.md")):
        text = path.read_text(encoding="utf-8")
        for match in LINK.finditer(text):
            target = match.group(1).strip()
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = target.split("#", 1)[0]
            if not target:
                continue
            if not (path.parent / target).resolve().exists():
                broken.append((path.relative_to(ROOT), target))
    for path, target in broken:
        print(f"broken link: {path}: {target}", file=sys.stderr)
    print(f"i18n links: {len(broken)} broken")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
