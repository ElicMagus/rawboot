#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Host-side tests for the public-migration tooling."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "migrate"))

import common
import diff_manifests
import gitea_manifest
import github_manifest
import scan_secrets

LABEL = {"name": "bugfix", "color": "d73a4a", "description": ""}
MILESTONE = {"title": "Release", "state": "open", "description": ""}


def make_item(number=1, **overrides):
    item = {
        "number": number,
        "kind": "issue",
        "state": "open",
        "title": "title",
        "body": "body",
        "author": "owner",
        "created_at": "2026-01-01T00:00:00Z",
        "closed_at": None,
        "labels": ["bugfix"],
        "milestone": None,
        "comments": [],
        "blocked_by": [],
        "pull": None,
    }
    item.update(overrides)
    return item


def make_manifest(items, labels=None, milestones=None):
    return {
        "source": "test",
        "repo": "test",
        "generated_at": "",
        "labels": list(labels if labels is not None else [LABEL]),
        "milestones": list(milestones if milestones is not None else [MILESTONE]),
        "items": list(items),
    }


class DiffManifestTests(unittest.TestCase):
    def test_identical_manifests_have_no_findings(self):
        manifest = make_manifest([make_item()])
        self.assertEqual(diff_manifests.compare(manifest, manifest), [])

    def test_detects_state_and_body(self):
        expected = make_manifest([make_item(body="line 1\nline 2")])
        actual = make_manifest([make_item(state="closed", body="line 1\nline X")])
        findings = diff_manifests.compare(expected, actual)
        self.assertTrue(any("state" in finding for finding in findings))
        self.assertTrue(any("body line 2" in finding for finding in findings))

    def test_detects_comment_and_blocked_by(self):
        expected = make_manifest(
            [
                make_item(
                    comments=[{"author": "owner", "created_at": "", "body": "c"}],
                    blocked_by=[2, 3],
                )
            ]
        )
        actual = make_manifest(
            [
                make_item(
                    comments=[{"author": "owner", "created_at": "", "body": "x"}],
                    blocked_by=[2],
                )
            ]
        )
        findings = diff_manifests.compare(expected, actual)
        self.assertTrue(any("comments[0]" in finding for finding in findings))
        self.assertTrue(any("blocked_by" in finding for finding in findings))

    def test_detects_missing_and_extra_items_and_labels(self):
        expected = make_manifest([make_item(1), make_item(2)])
        actual = make_manifest([make_item(1), make_item(3)], labels=[])
        findings = diff_manifests.compare(expected, actual)
        self.assertIn("item missing: #2", findings)
        self.assertIn("item extra: #3", findings)
        self.assertIn("label missing: bugfix", findings)

    def test_compares_pull_merge_presence_not_sha(self):
        expected = make_manifest(
            [
                make_item(
                    kind="pull",
                    pull={
                        "head": "feat/1",
                        "base": "master",
                        "merged_at": "2026-01-01T00:00:00Z",
                        "merge_commit_sha": "old",
                    },
                )
            ]
        )
        actual = make_manifest(
            [
                make_item(
                    kind="pull",
                    pull={
                        "head": "feat/1",
                        "base": "master",
                        "merged_at": "2026-01-01T00:00:00Z",
                        "merge_commit_sha": "new",
                    },
                )
            ]
        )
        self.assertEqual(diff_manifests.compare(expected, actual), [])


class NormalizeTests(unittest.TestCase):
    def test_gitea_issue(self):
        raw = {
            "number": 7,
            "state": "closed",
            "title": "T",
            "body": "a\r\nb\r\n",
            "user": {"login": "owner"},
            "labels": [{"name": "feature"}, {"name": "bugfix"}],
            "milestone": {"title": "Release"},
        }
        comments = [{"user": {"login": "owner"}, "body": "hello\r\n"}]
        dependencies = [{"number": 2}, {"number": 1}]
        item = gitea_manifest.normalize_issue(raw, comments, dependencies)
        self.assertEqual(item["body"], "a\nb")
        self.assertEqual(item["labels"], ["bugfix", "feature"])
        self.assertEqual(item["blocked_by"], [1, 2])
        self.assertEqual(item["comments"][0]["body"], "hello")
        self.assertIsNone(item["pull"])

    def test_gitea_pull(self):
        raw = {
            "number": 8,
            "state": "closed",
            "title": "P",
            "body": "",
            "user": {"login": "owner"},
            "head": {"ref": "feat/8"},
            "base": {"ref": "master"},
            "merged_at": "2026-01-02T00:00:00Z",
            "merge_commit_sha": "abc",
        }
        item = gitea_manifest.normalize_pull(raw, [], [])
        self.assertEqual(item["kind"], "pull")
        self.assertEqual(item["pull"]["head"], "feat/8")
        self.assertEqual(item["pull"]["merge_commit_sha"], "abc")

    def test_github_pull_uses_detail(self):
        raw = {
            "number": 8,
            "state": "closed",
            "title": "P",
            "body": "",
            "user": {"login": "owner"},
            "labels": [],
        }
        detail = {
            "head": {"ref": "feat/8"},
            "base": {"ref": "main"},
            "merged_at": "2026-01-02T00:00:00Z",
            "merge_commit_sha": "def",
        }
        item = github_manifest.normalize_pull(raw, detail, [], [])
        self.assertEqual(item["pull"]["base"], "main")
        self.assertEqual(item["pull"]["merge_commit_sha"], "def")


class ScanTests(unittest.TestCase):
    def setUp(self):
        self.patterns = scan_secrets.load_patterns([])
        self.allow = []

    def test_clean_text_has_no_findings(self):
        text = "the loader hands off to the payload"
        self.assertEqual(
            scan_secrets.scan_text("src", text, self.patterns, self.allow), []
        )

    def test_finds_github_token_and_home_path(self):
        text = f"token ghp_{'A' * 36}\npath /home/alice/project"
        findings = scan_secrets.scan_text("src", text, self.patterns, self.allow)
        names = {finding[2] for finding in findings}
        self.assertIn("github-token", names)
        self.assertIn("home-path", names)

    def test_finds_authorization_header(self):
        text = "Authorization: token abcdef0123456789abcdef"
        findings = scan_secrets.scan_text("src", text, self.patterns, self.allow)
        self.assertIn("auth-header", {finding[2] for finding in findings})

    def test_allow_list_suppresses_finding(self):
        text = "path /home/alice/project"
        allow = [scan_secrets.re.compile(r"/home/alice/")]
        self.assertEqual(scan_secrets.scan_text("src", text, self.patterns, allow), [])

    def test_scans_manifest_texts(self):
        manifest = make_manifest(
            [make_item(comments=[{"author": "owner", "created_at": "", "body": "x"}])]
        )
        manifest["items"][0]["body"] = "ghp_" + "B" * 36
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            findings = scan_secrets.scan_manifest(path, self.patterns, self.allow)
        self.assertTrue(any(finding[2] == "github-token" for finding in findings))


class CompareBodyTests(unittest.TestCase):
    def test_equal_after_normalization(self):
        self.assertIsNone(common.compare_body("a\r\nb\r\n", "a\nb"))

    def test_reports_first_differing_line(self):
        result = common.compare_body("a\nb", "a\nc")
        self.assertEqual(result, (2, "b", "c"))


if __name__ == "__main__":
    unittest.main()
