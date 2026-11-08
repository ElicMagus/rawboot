#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Fail if a primary Markdown document is not in English.

Translations live under `i18n/`. Research notes may quote historical Russian
titles, so they are exempt; the README language switch line is exempt too.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CYRILLIC = re.compile(r"[А-Яа-яЁё]")
EXEMPT_PREFIXES = ("i18n/", "research/", "vendor/")


def tracked_files():
    result = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "*.md"],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.split()


def main():
    findings = []
    for name in tracked_files():
        if name.startswith(EXEMPT_PREFIXES):
            continue
        path = ROOT / name
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if name == "README.md" and number == 1:
                continue
            if CYRILLIC.search(line):
                findings.append((name, number, line.strip()[:160]))
    for name, number, line in findings:
        print(f"not English: {name}:{number}: {line}", file=sys.stderr)
    print(f"primary docs language: {len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
