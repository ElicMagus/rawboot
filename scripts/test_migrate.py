#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Host-side tests for the public-migration tooling."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "migrate"))

import common
import diff_manifests
import gitea_manifest
import github_manifest
import render_tracker
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

    def test_ignores_merged_at_and_merge_sha(self):
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
                        "merged_at": None,
                        "merge_commit_sha": None,
                    },
                )
            ]
        )
        self.assertEqual(diff_manifests.compare(expected, actual), [])

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
            "head": {
                "ref": "refs/pull/8/head",
                "label": "owner:feat/8",
                "sha": "head-sha",
            },
            "base": {"ref": "master", "label": "master"},
            "merged_at": "2026-01-02T00:00:00Z",
            "merge_commit_sha": "abc",
        }
        item = gitea_manifest.normalize_pull(raw, [], [])
        self.assertEqual(item["kind"], "pull")
        self.assertEqual(item["pull"]["head"], "feat/8")
        self.assertEqual(item["pull"]["base"], "master")
        self.assertEqual(item["pull"]["head_sha"], "head-sha")
        self.assertIsNone(item["pull"]["base_sha"])
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


class RenderTrackerTests(unittest.TestCase):
    class Args:
        replacements = None
        commit_map = None
        import_date = "2026-11-08"
        allow_missing = False
        repo = None
        public_repo = None

    def _manifest(self):
        return make_manifest(
            [
                make_item(
                    1,
                    title="Заголовок",
                    body="Тело `rawboot-internal`",
                    comments=[
                        {"author": "owner", "created_at": "", "body": "Комментарий"}
                    ],
                ),
                make_item(
                    2,
                    kind="pull",
                    title="PR",
                    body="PR body",
                    pull={
                        "head": "feat/2",
                        "base": "master",
                        "head_sha": "h" * 40,
                        "base_sha": None,
                        "merged_at": "2026-01-02T00:00:00Z",
                        "merge_commit_sha": "m" * 40,
                    },
                ),
            ],
            milestones=[],
        )

    def _translations(self):
        return {
            "milestones": {},
            "items": {
                "1": {
                    "title_en": "Title",
                    "body_en": "Body `rawboot-internal`",
                    "comments_en": ["Comment"],
                },
                "2": {
                    "title_en": "PR",
                    "body_en": "PR body",
                    "comments_en": [],
                },
            },
        }

    def test_bilingual_layout(self):
        rendered = render_tracker.render(
            self._manifest(), self._translations(), self.Args()
        )
        issue = rendered["items"][0]
        self.assertEqual(issue["title"], "Title")
        self.assertTrue(issue["body"].startswith("> Historical issue imported"))
        self.assertIn("Body `rawboot-internal`", issue["body"])
        self.assertIn("<details>", issue["body"])
        self.assertIn("**Оригинальный заголовок:** Заголовок", issue["body"])
        self.assertIn("Тело `rawboot-internal`", issue["body"])
        self.assertIn("Comment", issue["comments"][0]["body"])
        self.assertIn("Комментарий", issue["comments"][0]["body"])

    def test_applies_replacements(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "replacements.txt"
            path.write_text("literal:rawboot-internal==>rawboot-internal\n", encoding="utf-8")
            args = self.Args()
            args.replacements = str(path)
            rendered = render_tracker.render(
                self._manifest(), self._translations(), args
            )
        body = rendered["items"][0]["body"]
        self.assertNotIn("rawboot-internal", body)
        self.assertEqual(body.count("rawboot-internal"), 2)

    def test_maps_merge_commit_and_links_it(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "commit-map"
            path.write_text(f"{'m' * 40} {'n' * 40}\n", encoding="utf-8")
            args = self.Args()
            args.commit_map = str(path)
            args.public_repo = "owner/repo"
            rendered = render_tracker.render(
                self._manifest(), self._translations(), args
            )
        pull = rendered["items"][1]
        self.assertEqual(pull["pull"]["merge_commit_sha"], "n" * 40)
        self.assertIn("https://github.com/owner/repo/commit/" + "n" * 40, pull["body"])

    def test_missing_translation_fails(self):
        translations = self._translations()
        del translations["items"]["2"]
        with self.assertRaises(SystemExit):
            render_tracker.render(self._manifest(), translations, self.Args())


class PullRewriteTests(unittest.TestCase):
    def test_squash_merge_uses_merge_commit_as_head(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            def run(*args):
                subprocess.run(
                    ["git", "-C", str(repo), *args],
                    check=True,
                    capture_output=True,
                    text=True,
                )
            run("init", "-q")
            run("config", "user.name", "Test")
            run("config", "user.email", "test@example.com")
            (repo / "a.txt").write_text("a", encoding="utf-8")
            run("add", ".")
            run("commit", "-q", "-m", "base")
            base = run("rev-parse", "HEAD").stdout.strip()
            (repo / "b.txt").write_text("b", encoding="utf-8")
            run("add", ".")
            run("commit", "-q", "-m", "squash")
            squash = run("rev-parse", "HEAD").stdout.strip()

            pull = {"merge_commit_sha": "old", "head_sha": "old-head"}
            updated, url = render_tracker._rewrite_pull(
                pull, {"old": squash}, str(repo), "owner/repo"
            )
        self.assertEqual(updated["base_sha"], base)
        self.assertEqual(updated["head_sha"], squash)
        self.assertIn(squash, url)


class CompareBodyTests(unittest.TestCase):
    def test_equal_after_normalization(self):
        self.assertIsNone(common.compare_body("a\r\nb\r\n", "a\nb"))

    def test_reports_first_differing_line(self):
        result = common.compare_body("a\nb", "a\nc")
        self.assertEqual(result, (2, "b", "c"))


if __name__ == "__main__":
    unittest.main()
