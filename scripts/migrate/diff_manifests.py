#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Diff two migration manifests and report every difference.

The expected manifest is produced by the renderer (Gitea text plus the English
translation); the actual one is exported from the target tracker. Timestamps are
ignored on purpose: the target server assigns them at import time.
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common


def _label_names(manifest):
    return {label["name"] for label in manifest.get("labels") or []}


def _milestones(manifest):
    return {
        milestone["title"]: milestone
        for milestone in manifest.get("milestones") or []
    }


def _short(text, limit=200):
    text = text.replace("\n", "\\n")
    return text if len(text) <= limit else text[:limit] + "..."


def _compare_sequence(kind, number, field, expected, actual, findings):
    expected_values = [entry.get("body") for entry in expected]
    actual_values = [entry.get("body") for entry in actual]
    if len(expected_values) != len(actual_values):
        findings.append(
            f"{kind} #{number}: {field} count {len(expected_values)} != "
            f"{len(actual_values)}"
        )
    for index, (left, right) in enumerate(zip(expected_values, actual_values)):
        if left != right:
            findings.append(
                f"{kind} #{number}: {field}[{index}] body differs: "
                f"{_short(left or '')!r} != {_short(right or '')!r}"
            )
    for index, (left, right) in enumerate(zip(expected, actual)):
        if left.get("author") != right.get("author"):
            findings.append(
                f"{kind} #{number}: {field}[{index}] author "
                f"{left.get('author')!r} != {right.get('author')!r}"
            )


def _compare_item(expected, actual, findings):
    number = expected["number"]
    kind = expected["kind"]
    if expected.get("kind") != actual.get("kind"):
        findings.append(
            f"#{number}: kind {expected.get('kind')!r} != {actual.get('kind')!r}"
        )
        return
    if expected.get("state") != actual.get("state"):
        findings.append(
            f"{kind} #{number}: state {expected.get('state')!r} != "
            f"{actual.get('state')!r}"
        )
    if expected.get("title") != actual.get("title"):
        findings.append(
            f"{kind} #{number}: title {expected.get('title')!r} != "
            f"{actual.get('title')!r}"
        )
    body_diff = common.compare_body(expected.get("body"), actual.get("body"))
    if body_diff:
        line, left, right = body_diff
        findings.append(
            f"{kind} #{number}: body line {line}: "
            f"{_short(left)!r} != {_short(right)!r}"
        )
    if sorted(expected.get("labels") or []) != sorted(actual.get("labels") or []):
        findings.append(
            f"{kind} #{number}: labels {expected.get('labels')} != "
            f"{actual.get('labels')}"
        )
    if expected.get("milestone") != actual.get("milestone"):
        findings.append(
            f"{kind} #{number}: milestone {expected.get('milestone')!r} != "
            f"{actual.get('milestone')!r}"
        )
    if expected.get("blocked_by") != actual.get("blocked_by"):
        findings.append(
            f"{kind} #{number}: blocked_by {expected.get('blocked_by')} != "
            f"{actual.get('blocked_by')}"
        )
    _compare_sequence(
        kind,
        number,
        "comments",
        expected.get("comments") or [],
        actual.get("comments") or [],
        findings,
    )
    if kind == "pull":
        expected_pull = expected.get("pull") or {}
        actual_pull = actual.get("pull") or {}
        for field in ("head", "base"):
            if expected_pull.get(field) != actual_pull.get(field):
                findings.append(
                    f"pull #{number}: {field} {expected_pull.get(field)!r} != "
                    f"{actual_pull.get(field)!r}"
                )


def compare(expected, actual):
    findings = []
    expected_labels = _label_names(expected)
    actual_labels = _label_names(actual)
    for name in sorted(expected_labels - actual_labels):
        findings.append(f"label missing: {name}")
    for name in sorted(actual_labels - expected_labels):
        findings.append(f"label extra: {name}")

    expected_milestones = _milestones(expected)
    actual_milestones = _milestones(actual)
    for title in sorted(set(expected_milestones) - set(actual_milestones)):
        findings.append(f"milestone missing: {title}")
    for title in sorted(set(actual_milestones) - set(expected_milestones)):
        findings.append(f"milestone extra: {title}")
    for title in sorted(set(expected_milestones) & set(actual_milestones)):
        left = expected_milestones[title]
        right = actual_milestones[title]
        for field in ("state", "description"):
            if left.get(field) != right.get(field):
                findings.append(
                    f"milestone {title}: {field} {left.get(field)!r} != "
                    f"{right.get(field)!r}"
                )

    expected_items = {item["number"]: item for item in expected.get("items") or []}
    actual_items = {item["number"]: item for item in actual.get("items") or []}
    for number in sorted(set(expected_items) - set(actual_items)):
        findings.append(f"item missing: #{number}")
    for number in sorted(set(actual_items) - set(expected_items)):
        findings.append(f"item extra: #{number}")
    for number in sorted(set(expected_items) & set(actual_items)):
        _compare_item(expected_items[number], actual_items[number], findings)
    return findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected", required=True, help="expected manifest JSON")
    parser.add_argument("--actual", required=True, help="actual manifest JSON")
    parser.add_argument(
        "--json", action="store_true", help="print findings as JSON"
    )
    args = parser.parse_args()

    expected = json.loads(Path(args.expected).read_text(encoding="utf-8"))
    actual = json.loads(Path(args.actual).read_text(encoding="utf-8"))
    findings = compare(expected, actual)

    if args.json:
        print(json.dumps(findings, ensure_ascii=False, indent=2))
    else:
        for finding in findings:
            print(finding)
        print(
            f"{len(findings)} difference(s) "
            f"({len(expected.get('items') or [])} expected items)"
        )
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
