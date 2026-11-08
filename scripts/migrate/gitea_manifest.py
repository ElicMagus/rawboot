#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Export a Gitea issue/PR tracker into the migration manifest."""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common


def _normalize_comment(raw):
    return {
        "author": (raw.get("user") or {}).get("login"),
        "created_at": raw.get("created_at"),
        "body": common.normalize_body(raw.get("body")),
    }


def _normalize_common(raw, comments, blocked_by):
    return {
        "number": int(raw["number"]),
        "state": raw.get("state"),
        "title": raw.get("title") or "",
        "body": common.normalize_body(raw.get("body")),
        "author": (raw.get("user") or {}).get("login"),
        "created_at": raw.get("created_at"),
        "closed_at": raw.get("closed_at"),
        "labels": sorted(label["name"] for label in raw.get("labels") or []),
        "milestone": (raw.get("milestone") or {}).get("title"),
        "comments": [_normalize_comment(item) for item in comments],
        "blocked_by": sorted(int(item["number"]) for item in blocked_by),
    }


def normalize_issue(raw, comments, blocked_by):
    item = _normalize_common(raw, comments, blocked_by)
    item["kind"] = "issue"
    item["pull"] = None
    return item


def _branch_name(side, number):
    label = (side.get("label") or "").strip()
    if ":" in label:
        label = label.split(":", 1)[1]
    if label and not label.startswith("refs/"):
        return label
    ref = (side.get("ref") or "").strip()
    if ref and not ref.startswith("refs/"):
        return ref
    return f"migration/pr-{number}-{side.get('ref', 'head')}"


def normalize_pull(raw, comments, blocked_by):
    item = _normalize_common(raw, comments, blocked_by)
    number = int(raw["number"])
    head = raw.get("head") or {}
    base = raw.get("base") or {}
    item["kind"] = "pull"
    item["pull"] = {
        "head": _branch_name(head, number),
        "base": _branch_name(base, number),
        "head_sha": head.get("sha"),
        "base_sha": None,
        "merged_at": raw.get("merged_at"),
        "merge_commit_sha": raw.get("merge_commit_sha"),
    }
    return item


def _normalize_label(raw):
    return {
        "name": raw.get("name"),
        "color": (raw.get("color") or "").lstrip("#"),
        "description": raw.get("description") or "",
    }


def _normalize_milestone(raw):
    return {
        "title": raw.get("title"),
        "state": raw.get("state"),
        "description": raw.get("description") or "",
    }


def export(base, token, with_dependencies=True):
    base = base.rstrip("/")
    labels = common.get_gitea_all(f"{base}/labels", token)
    milestones = common.get_gitea_all(f"{base}/milestones?state=all", token)
    issues = common.get_gitea_all(f"{base}/issues?state=all&type=issues", token)
    pulls = common.get_gitea_all(f"{base}/pulls?state=all", token)

    items = []
    for kind, batch, normalize in (
        ("issue", issues, normalize_issue),
        ("pull", pulls, normalize_pull),
    ):
        for raw in batch:
            number = int(raw["number"])
            comments = common.get_gitea_all(
                f"{base}/issues/{number}/comments", token
            )
            if with_dependencies:
                blocked_by = common.get_json(
                    f"{base}/issues/{number}/dependencies", token
                ) or []
            else:
                blocked_by = []
            items.append(normalize(raw, comments, blocked_by))
            print(f"  {kind} #{number}", file=sys.stderr)

    items.sort(key=lambda item: item["number"])
    return {
        "source": "gitea",
        "repo": base,
        "generated_at": common.now_iso(),
        "labels": sorted(
            (_normalize_label(label) for label in labels),
            key=lambda label: label["name"],
        ),
        "milestones": sorted(
            (_normalize_milestone(milestone) for milestone in milestones),
            key=lambda milestone: milestone["title"],
        ),
        "items": items,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base",
        required=True,
        help="Gitea API repository URL, e.g. https://host/api/v1/repos/owner/repo",
    )
    parser.add_argument("--output", required=True, help="manifest JSON path")
    parser.add_argument("--token-file", help="file with a Gitea token")
    parser.add_argument(
        "--no-dependencies",
        action="store_true",
        help="skip blocked-by links (useful for a quick snapshot)",
    )
    args = parser.parse_args()

    token = common.read_token("GITEA_TOKEN", args.token_file)
    if not token:
        raise SystemExit("error: set GITEA_TOKEN or pass --token-file")
    manifest = export(args.base, token, not args.no_dependencies)
    common.write_json(args.output, manifest)


if __name__ == "__main__":
    main()
