#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Export a GitHub issue/PR tracker into the migration manifest."""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common

API = "https://api.github.com"


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


def normalize_pull(raw, pull_detail, comments, blocked_by):
    item = _normalize_common(raw, comments, blocked_by)
    item["kind"] = "pull"
    item["pull"] = {
        "head": (pull_detail.get("head") or {}).get("ref"),
        "base": (pull_detail.get("base") or {}).get("ref"),
        "head_sha": (pull_detail.get("head") or {}).get("sha"),
        "base_sha": (pull_detail.get("base") or {}).get("sha"),
        "merged_at": pull_detail.get("merged_at"),
        "merge_commit_sha": pull_detail.get("merge_commit_sha"),
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


def export(repo, token, with_dependencies=True):
    base = f"{API}/repos/{repo}"
    labels = common.get_github_all(f"{base}/labels", token)
    milestones = common.get_github_all(f"{base}/milestones?state=all", token)
    raw_items = common.get_github_all(f"{base}/issues?state=all", token)

    items = []
    for raw in raw_items:
        number = int(raw["number"])
        comments = common.get_github_all(f"{base}/issues/{number}/comments", token)
        if with_dependencies:
            blocked_by = common.get_json_optional(
                f"{base}/issues/{number}/dependencies/blocked_by",
                token,
                api_version=common.GITHUB_API_VERSION,
            ) or []
        else:
            blocked_by = []
        if "pull_request" in raw:
            detail = common.get_json(
                f"{base}/pulls/{number}",
                token,
                api_version=common.GITHUB_API_VERSION,
            )
            item = normalize_pull(raw, detail, comments, blocked_by)
        else:
            item = normalize_issue(raw, comments, blocked_by)
        items.append(item)
        print(f"  {item['kind']} #{number}", file=sys.stderr)

    items.sort(key=lambda item: item["number"])
    return {
        "source": "github",
        "repo": repo,
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
        "--repo", required=True, help="GitHub repository as owner/name"
    )
    parser.add_argument("--output", required=True, help="manifest JSON path")
    parser.add_argument("--token-file", help="file with a GitHub token")
    parser.add_argument(
        "--no-dependencies",
        action="store_true",
        help="skip blocked-by links (useful for a quick snapshot)",
    )
    args = parser.parse_args()

    token = common.read_token("GITHUB_TOKEN", args.token_file)
    if not token:
        print(
            "warning: no GITHUB_TOKEN; unauthenticated requests are rate-limited",
            file=sys.stderr,
        )
    manifest = export(args.repo, token, not args.no_dependencies)
    common.write_json(args.output, manifest)


if __name__ == "__main__":
    main()
