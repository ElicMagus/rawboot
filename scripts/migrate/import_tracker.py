#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Create the public archive tracker on GitHub from a migration manifest.

Items are created strictly in ascending number order so GitHub assigns the same
numbers as the source tracker. Pull requests are archived as closed records; the
temporary branches used to open them are deleted afterwards.
"""

import argparse
import base64
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common

API = "https://api.github.com"


class ApiError(Exception):
    def __init__(self, method, url, code, detail):
        super().__init__(f"{method} {url} -> HTTP {code}: {detail}")
        self.code = code


class Client:
    def __init__(self, repo, token, delay):
        self.repo = repo
        self.token = token
        self.delay = delay
        self.base = f"{API}/repos/{repo}"

    def call(self, method, path, body=None, allow=()):
        url = path if path.startswith("http") else self.base + path
        data = json.dumps(body).encode("utf-8") if body is not None else None
        headers = {
            "User-Agent": common.USER_AGENT,
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": common.GITHUB_API_VERSION,
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        request = urllib.request.Request(url, data=data, method=method, headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                raw = response.read()
                return json.loads(raw.decode("utf-8")) if raw else None
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", "replace")[:400]
            if error.code in allow:
                return None
            raise ApiError(method, url, error.code, detail) from error
        finally:
            if method != "GET":
                time.sleep(self.delay)

    def get(self, path):
        return self.call("GET", path)

    def all(self, path):
        return common.get_github_all(self.base + path, self.token)


def _ref_path(ref):
    return urllib.parse.quote(ref, safe="/")


def _bootstrap_main(client):
    try:
        reference = client.get("/git/ref/heads/main")
    except ApiError as error:
        if error.code not in (404, 409):
            raise
        client.call(
            "PUT",
            "/contents/README.md",
            {
                "message": "migration: initial commit",
                "branch": "main",
                "content": base64.b64encode(
                    b"# Migration sandbox\n\nRehearsal target for the archive import.\n"
                ).decode("ascii"),
            },
        )
        reference = client.get("/git/ref/heads/main")
    return reference["object"]["sha"]


def _ensure_ref(client, ref, sha):
    path = f"/git/refs/heads/{_ref_path(ref)}"
    created = client.call(
        "POST", "/git/refs", {"ref": f"refs/heads/{ref}", "sha": sha}, allow=(422,)
    )
    if created is None:
        client.call("PATCH", path, {"sha": sha, "force": True})


def _delete_ref(client, ref):
    client.call(
        "DELETE", f"/git/refs/heads/{_ref_path(ref)}", allow=(404, 422)
    )


def _synthetic_head(client, main_sha, number):
    main_commit = client.get(f"/git/commits/{main_sha}")
    tree = client.call(
        "POST",
        "/git/trees",
        {
            "base_tree": main_commit["tree"]["sha"],
            "tree": [
                {
                    "path": f".migration/pr-{number}.txt",
                    "mode": "100644",
                    "type": "blob",
                    "content": f"archived pull request #{number}\n",
                }
            ],
        },
    )
    commit = client.call(
        "POST",
        "/git/commits",
        {
            "message": f"archive: pull request #{number}",
            "tree": tree["sha"],
            "parents": [main_sha],
        },
    )
    return commit["sha"]


def _ensure_label(client, label):
    body = {
        "name": label["name"],
        "color": label["color"] or "ededed",
        "description": label["description"],
    }
    created = client.call("POST", "/labels", body, allow=(422,))
    if created is None:
        client.call("PATCH", f"/labels/{urllib.parse.quote(label['name'])}", body)


def _ensure_milestones(client, milestones):
    existing = {
        milestone["title"]: milestone["number"]
        for milestone in client.all("/milestones?state=all&per_page=100")
    }
    numbers = {}
    for milestone in milestones:
        title = milestone["title"]
        if title in existing:
            numbers[title] = existing[title]
            continue
        body = {
            "title": title,
            "state": milestone["state"],
            "description": milestone["description"],
        }
        created = client.call("POST", "/milestones", body)
        numbers[title] = created["number"]
    return numbers


def _create_issue(client, item, milestone_number):
    body = {
        "title": item["title"],
        "body": item["body"],
        "labels": item["labels"],
    }
    if milestone_number is not None:
        body["milestone"] = milestone_number
    issue = client.call("POST", "/issues", body)
    if issue["number"] != item["number"]:
        raise SystemExit(
            f"error: wanted #{item['number']}, GitHub created #{issue['number']}; "
            "numbering is out of sync"
        )
    if item["state"] == "closed":
        client.call("PATCH", f"/issues/{issue['number']}", {"state": "closed"})
    return issue["id"], issue["number"]


def _create_pull(client, item, mode, main_sha, temp_refs):
    pull = item["pull"] or {}
    head_ref = pull.get("head")
    base_ref = pull.get("base")
    if not head_ref or not base_ref:
        raise SystemExit(f"error: pull #{item['number']} has no head/base ref")

    if mode == "synthetic":
        _ensure_ref(client, base_ref, main_sha)
        head_sha = _synthetic_head(client, main_sha, item["number"])
    else:
        if not pull.get("head_sha") or not pull.get("base_sha"):
            raise SystemExit(
                f"error: pull #{item['number']} has no rewritten head/base SHA"
            )
        _ensure_ref(client, base_ref, pull["base_sha"])
        head_sha = pull["head_sha"]
    _ensure_ref(client, head_ref, head_sha)
    temp_refs.update({base_ref, head_ref})

    body = {"title": item["title"], "body": item["body"], "head": head_ref, "base": base_ref}
    pull_request = client.call("POST", "/pulls", body)
    if pull_request["number"] != item["number"]:
        raise SystemExit(
            f"error: wanted #{item['number']}, GitHub created "
            f"#{pull_request['number']}; numbering is out of sync"
        )
    metadata = {"labels": item["labels"]}
    milestone_number = _milestone_number(item)
    if milestone_number is not None:
        metadata["milestone"] = milestone_number
    client.call("PATCH", f"/issues/{pull_request['number']}", metadata)
    client.call("PATCH", f"/pulls/{pull_request['number']}", {"state": "closed"})
    return pull_request["id"], pull_request["number"]


def _milestone_number(item):
    return item.get("_milestone_number")


def run(args, manifest, client):
    labels = manifest.get("labels") or []
    milestones = manifest.get("milestones") or []
    items = sorted(manifest.get("items") or [], key=lambda item: item["number"])

    if args.dry_run:
        for label in labels:
            print(f"label: {label['name']}")
        for milestone in milestones:
            print(f"milestone: {milestone['title']} ({milestone['state']})")
        for item in items:
            print(f"{item['kind']} #{item['number']}: {item['title']}")
        print(f"dry-run: {len(items)} item(s), nothing written")
        return 0

    for label in labels:
        _ensure_label(client, label)
    if args.prune_labels:
        wanted = {label["name"] for label in labels}
        for existing_label in client.all("/labels?per_page=100"):
            if existing_label["name"] not in wanted:
                client.call(
                    "DELETE",
                    f"/labels/{urllib.parse.quote(existing_label['name'])}",
                    allow=(404,),
                )
    milestone_numbers = _ensure_milestones(client, milestones)
    for item in items:
        item["_milestone_number"] = milestone_numbers.get(item.get("milestone"))

    main_sha = _bootstrap_main(client)
    existing = client.all("/issues?state=all&per_page=100")
    id_map = {entry["number"]: entry["id"] for entry in existing}
    max_number = max(id_map, default=0)
    temp_refs = set()
    expected = max_number + 1

    for item in items:
        number = item["number"]
        if number <= max_number:
            print(f"skip #{number}: already present")
            continue
        if expected != number:
            raise SystemExit(
                f"error: next GitHub number is #{expected}, manifest expects "
                f"#{number}; aborting before numbering drifts"
            )
        if item["kind"] == "issue":
            item_id, created = _create_issue(
                client, item, item["_milestone_number"]
            )
        else:
            item_id, created = _create_pull(
                client, item, args.mode, main_sha, temp_refs
            )
        expected = created + 1
        id_map[created] = item_id
        for index, comment in enumerate(item.get("comments") or [], start=1):
            client.call(
                "POST",
                f"/issues/{created}/comments",
                {"body": comment["body"]},
            )
            print(f"  comment {index}/{len(item['comments'])} on #{created}")

    for item in items:
        for blocker in item.get("blocked_by") or []:
            blocker_id = id_map.get(blocker)
            if blocker_id is None:
                raise SystemExit(
                    f"error: blocked-by #{blocker} of #{item['number']} is unknown"
                )
            client.call(
                "POST",
                f"/issues/{item['number']}/dependencies/blocked_by",
                {"issue_id": blocker_id},
                allow=(422,),
            )

    for ref in sorted(temp_refs):
        if ref == "main":
            continue
        _delete_ref(client, ref)

    print(f"created or verified {len(items)} item(s)")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="GitHub repository owner/name")
    parser.add_argument("--manifest", required=True, help="migration manifest JSON")
    parser.add_argument("--token-file", help="file with a GitHub token")
    parser.add_argument(
        "--mode",
        choices=("synthetic", "refs"),
        default="synthetic",
        help="synthetic creates throwaway PR branches; refs uses rewritten SHAs",
    )
    parser.add_argument("--delay", type=float, default=0.2, help="seconds between writes")
    parser.add_argument(
        "--prune-labels",
        action="store_true",
        help="delete target labels that are absent from the manifest",
    )
    parser.add_argument("--dry-run", action="store_true", help="print the plan only")
    args = parser.parse_args()

    token = common.read_token("GITHUB_TOKEN", args.token_file)
    if not token:
        raise SystemExit("error: set GITHUB_TOKEN or pass --token-file")
    manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
    client = Client(args.repo, token, args.delay)
    try:
        return run(args, manifest, client)
    except ApiError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
