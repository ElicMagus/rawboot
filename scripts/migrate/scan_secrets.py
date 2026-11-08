#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Scan a git repository and tracker manifests for secrets and private paths.

Project-specific forbidden strings are supplied from outside the repository via
``--forbidden-file`` so the published copy of this tool stays generic.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_PATTERNS = [
    ("private-key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    (
        "github-token",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{20,})\b",
    ),
    ("aws-key", r"\bAKIA[0-9A-Z]{16}\b"),
    ("openai-key", r"\bsk-[A-Za-z0-9]{20,}\b"),
    (
        "auth-header",
        r"(?i)authorization\s*:\s*(?:token|bearer)\s+[A-Za-z0-9._\-]{16,}",
    ),
    ("url-credentials", r"\b[a-z][a-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@"),
    ("home-path", r"/home/[A-Za-z0-9._-]+/"),
    ("secret-file", r"(?:\." + "netrc|id_" + "rsa|id_" + "ed25519)"),
]


def load_patterns(forbidden_files):
    patterns = list(DEFAULT_PATTERNS)
    for path in forbidden_files or []:
        for line in Path(path).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            patterns.append((f"forbidden:{line}", line))
    return [(name, re.compile(expression)) for name, expression in patterns]


def load_allow(allow_files):
    expressions = []
    for path in allow_files or []:
        for line in Path(path).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            expressions.append(re.compile(line))
    return expressions


def scan_text(source, text, patterns, allow):
    findings = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        if any(expression.search(line) for expression in allow):
            continue
        for name, expression in patterns:
            if expression.search(line):
                findings.append((source, line_number, name, line.strip()[:200]))
    return findings


def _git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def _blob_oids(repo):
    paths = {}
    output = _git(repo, "rev-list", "--objects", "--all")
    for line in output.splitlines():
        parts = line.split(maxsplit=1)
        oid = parts[0]
        path = parts[1] if len(parts) > 1 else ""
        paths.setdefault(oid, set()).add(path)
    return paths


def scan_repository(repo, patterns, allow):
    findings = []
    paths = _blob_oids(repo)
    oids = sorted(paths)
    batch = subprocess.run(
        ["git", "-C", repo, "cat-file", "--batch"],
        input=("\n".join(oids) + "\n").encode(),
        capture_output=True,
        check=True,
    ).stdout

    position = 0
    for oid in oids:
        newline = batch.index(b"\n", position)
        header = batch[position:newline].decode("utf-8", "replace").strip()
        position = newline + 1
        parts = header.split()
        if len(parts) < 3:
            continue
        kind, size = parts[1], int(parts[2])
        content = batch[position : position + size]
        position += size + 1
        if kind != "blob" or b"\x00" in content[:8000]:
            continue
        text = content.decode("utf-8", "replace")
        label = ", ".join(sorted(paths.get(oid, {"<unknown>"})))
        findings.extend(scan_text(f"blob {label} ({oid[:12]})", text, patterns, allow))

    for sha in _git(repo, "log", "--all", "--format=%H").splitlines():
        message = _git(repo, "show", "-s", "--format=%B", sha)
        findings.extend(scan_text(f"commit {sha[:12]}", message, patterns, allow))
    return findings


def scan_manifest(path, patterns, allow):
    manifest = json.loads(Path(path).read_text(encoding="utf-8"))
    findings = []
    for item in manifest.get("items") or []:
        number = item.get("number")
        for field in ("title", "body"):
            findings.extend(
                scan_text(
                    f"manifest {path} #{number} {field}",
                    item.get(field) or "",
                    patterns,
                    allow,
                )
            )
        for index, comment in enumerate(item.get("comments") or [], start=1):
            findings.extend(
                scan_text(
                    f"manifest {path} #{number} comment {index}",
                    comment.get("body") or "",
                    patterns,
                    allow,
                )
            )
    for collection, field in (("labels", "description"), ("milestones", "description")):
        for entry in manifest.get(collection) or []:
            findings.extend(
                scan_text(
                    f"manifest {path} {collection} {entry.get('name') or entry.get('title')}",
                    entry.get(field) or "",
                    patterns,
                    allow,
                )
            )
    return findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="git repository to scan")
    parser.add_argument(
        "--manifest", action="append", default=[], help="manifest JSON to scan"
    )
    parser.add_argument(
        "--forbidden-file",
        action="append",
        default=[],
        help="file with one private regex per line (kept outside the repository)",
    )
    parser.add_argument(
        "--allow-file",
        action="append",
        default=[],
        help="file with one allow-listed regex per line",
    )
    parser.add_argument(
        "--max-findings", type=int, default=200, help="stop printing after N findings"
    )
    args = parser.parse_args()

    if not args.repo and not args.manifest:
        parser.error("pass --repo and/or --manifest")

    patterns = load_patterns(args.forbidden_file)
    allow = load_allow(args.allow_file)
    findings = []
    if args.repo:
        findings.extend(scan_repository(args.repo, patterns, allow))
    for path in args.manifest:
        findings.extend(scan_manifest(path, patterns, allow))

    for source, line_number, name, text in findings[: args.max_findings]:
        print(f"{source}:{line_number}: {name}: {text}")
    if len(findings) > args.max_findings:
        print(f"... {len(findings) - args.max_findings} more finding(s)")
    print(f"{len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
