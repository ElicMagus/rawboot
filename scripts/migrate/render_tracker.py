#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Render the archive manifest: English on top, the Russian original in details.

Combines the Gitea manifest with the translation files, adds provenance notes
(original dates, archived pull requests and their rewritten merge commits) and
applies the private replacement rules so no internal string reaches the archive.
"""

import argparse
import json
import subprocess
import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common


def load_translations(paths):
    merged = {"milestones": {}, "items": {}}
    for path in paths:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        merged["milestones"].update(data.get("milestones") or {})
        merged["items"].update(data.get("items") or {})
    return merged


def load_replacements(path):
    rules = []
    if not path:
        return rules
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("literal:"):
            body = line[len("literal:"):]
            if "==>" not in body:
                continue
            old, new = body.split("==>", 1)
            rules.append((old, new))
        else:
            raise SystemExit(f"error: unsupported replacement rule: {line}")
    return rules


def apply_rules(text, rules):
    for old, new in rules:
        text = text.replace(old, new)
    return text


def load_commit_map(path):
    mapping = {}
    if not path:
        return mapping
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if len(parts) == 2 and len(parts[0]) >= 40 and len(parts[1]) >= 40:
            mapping[parts[0]] = parts[1]
    return mapping


def git_sha(repo, revision):
    result = subprocess.run(
        ["git", "-C", repo, "rev-parse", revision],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _day(value):
    return value[:10] if value else "unknown"


def _note(item, import_day, merge_url):
    opened = _day(item.get("created_at"))
    if item["kind"] == "pull":
        pull = item.get("pull") or {}
        merged = _day(pull.get("merged_at"))
        note = (
            "> Archived pull request imported from the internal Gitea tracker on "
            f"{import_day}.\n> Opened {opened}"
        )
        if merge_url:
            short = merge_url.rsplit("/", 1)[-1][:7]
            note += f"; merged as [{short}]({merge_url})"
        elif merged != "unknown":
            note += f"; merged {merged}"
        return note + "."
    if item.get("state") == "closed":
        state = f"closed {_day(item.get('closed_at'))}"
    else:
        state = "still open"
    return (
        "> Historical issue imported from the internal Gitea tracker on "
        f"{import_day}.\n> Opened {opened}; {state}."
    )


def _details(ru_title, ru_body, ru_note=None):
    parts = []
    if ru_title is not None:
        parts.append(f"**Оригинальный заголовок:** {ru_title}")
    if ru_note:
        parts.append(ru_note)
    if (ru_body or "").strip():
        parts.append(ru_body.strip())
    if not parts:
        return ""
    body = "\n\n".join(parts)
    return f"<details>\n<summary>Оригинал (RU)</summary>\n\n{body}\n\n</details>"


def compose(en_body, ru_body, ru_title=None, ru_note=None):
    sections = []
    if (en_body or "").strip():
        sections.append(en_body.strip())
    details = _details(ru_title, ru_body, ru_note)
    if details:
        sections.append(details)
    return "\n\n".join(sections)


def _rewrite_pull(pull, commit_map, repo, public_repo):
    if not pull:
        return None
    updated = dict(pull)
    old_merge = pull.get("merge_commit_sha")
    new_merge = commit_map.get(old_merge)
    if new_merge:
        updated["merge_commit_sha"] = new_merge
    old_head = pull.get("head_sha")
    new_head = commit_map.get(old_head)
    if new_head:
        updated["head_sha"] = new_head
    if repo and new_merge:
        updated["base_sha"] = git_sha(repo, f"{new_merge}^1")
        if not new_head:
            updated["head_sha"] = git_sha(repo, f"{new_merge}^2")
    merge_url = None
    if public_repo and new_merge:
        merge_url = f"https://github.com/{public_repo}/commit/{new_merge}"
    return updated, merge_url


def render(manifest, translations, args):
    rules = load_replacements(args.replacements)
    commit_map = load_commit_map(args.commit_map)
    import_day = args.import_date or date.today().isoformat()

    milestone_titles = {}
    milestones = []
    for source in manifest.get("milestones") or []:
        translation = translations["milestones"].get(source["title"])
        if translation is None:
            if not args.allow_missing:
                raise SystemExit(
                    f"error: no translation for milestone {source['title']!r}"
                )
            translation = {"title_en": source["title"], "description_en": ""}
        title_en = apply_rules(translation.get("title_en") or source["title"], rules)
        description_en = apply_rules(
            translation.get("description_en") or "", rules
        )
        description = description_en
        original = source.get("description") or ""
        if original.strip():
            description = (
                f"{description_en}\n\nОригинальное название: {source['title']}\n"
                f"Оригинальное описание: {apply_rules(original, rules)}"
            ).strip()
        milestone_titles[source["title"]] = title_en
        milestones.append(
            {
                "title": title_en,
                "state": source["state"],
                "description": description,
            }
        )

    items = []
    for source in manifest.get("items") or []:
        translation = translations["items"].get(str(source["number"]))
        if translation is None:
            if not args.allow_missing:
                raise SystemExit(
                    f"error: no translation for item #{source['number']}"
                )
            translation = {
                "title_en": source["title"],
                "body_en": "",
                "comments_en": [],
            }
        comments_en = translation.get("comments_en") or []
        if len(comments_en) != len(source.get("comments") or []):
            raise SystemExit(
                f"error: item #{source['number']} has {len(comments_en)} translated "
                f"comments, expected {len(source.get('comments') or [])}"
            )

        item = dict(source)
        item["title"] = apply_rules(translation.get("title_en") or source["title"], rules)
        item["body"] = compose(
            apply_rules(translation.get("body_en") or "", rules),
            apply_rules(source.get("body") or "", rules),
            source["title"],
        )
        if source.get("milestone"):
            item["milestone"] = milestone_titles.get(source["milestone"])
        item["comments"] = [
            {
                "author": comment.get("author"),
                "created_at": comment.get("created_at"),
                "body": compose(
                    apply_rules(comments_en[index] or "", rules),
                    apply_rules(comment.get("body") or "", rules),
                ),
            }
            for index, comment in enumerate(source.get("comments") or [])
        ]
        note = _note(source, import_day, None)
        if source["kind"] == "pull":
            rewritten = _rewrite_pull(
                source.get("pull"), commit_map, args.repo, args.public_repo
            )
            if rewritten:
                item["pull"], merge_url = rewritten
                note = _note(source, import_day, merge_url)
        item["body"] = f"{note}\n\n{item['body']}".strip()
        items.append(item)

    return {
        "source": manifest.get("source"),
        "repo": manifest.get("repo"),
        "generated_at": common.now_iso(),
        "labels": manifest.get("labels") or [],
        "milestones": milestones,
        "items": items,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, help="Gitea manifest JSON")
    parser.add_argument(
        "--translations",
        action="append",
        required=True,
        help="translation JSON (repeatable)",
    )
    parser.add_argument("--output", required=True, help="rendered manifest JSON")
    parser.add_argument(
        "--commit-map",
        help="git-filter-repo commit-map (old and new SHA pairs)",
    )
    parser.add_argument(
        "--repo", help="rewritten clone, used to derive pre-merge base SHAs"
    )
    parser.add_argument(
        "--public-repo", help="public repository as owner/name, for merge links"
    )
    parser.add_argument(
        "--replacements",
        help="literal replacement rules (kept outside the repository)",
    )
    parser.add_argument("--import-date", help="migration date shown in the notes")
    parser.add_argument(
        "--allow-missing",
        action="store_true",
        help="fall back to the Russian text instead of failing",
    )
    args = parser.parse_args()

    manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
    translations = load_translations(args.translations)
    rendered = render(manifest, translations, args)
    common.write_json(args.output, rendered)


if __name__ == "__main__":
    main()
