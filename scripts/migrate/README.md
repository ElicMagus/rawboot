# Public-migration tooling

Generic helpers for publishing the internal history and tracker to the public
repository (ADR-0016). Repository URLs, tokens and private strings are passed in
at run time, so this directory carries no internal host, token path or slug.

## Prerequisites

- `git-filter-repo` (`pipx install git-filter-repo`)
- a Gitea token in `GITEA_TOKEN` and a GitHub token in `GITHUB_TOKEN`

## Export a manifest

```sh
scripts/migrate/gitea_manifest.py \
    --base https://<gitea-host>/api/v1/repos/<owner>/<repo> \
    --output target/migrate/gitea.json

scripts/migrate/github_manifest.py \
    --repo <owner>/<repo> \
    --output target/migrate/github.json
```

A manifest captures labels, milestones, issues, pull requests, comments, authors
and blocked-by links. Timestamps are recorded but never compared.

## Diff manifests

```sh
scripts/migrate/diff_manifests.py \
    --expected target/migrate/expected.json \
    --actual target/migrate/github.json
```

`expected.json` is the rendered public manifest: Gitea text with the English
translation on top and the Russian original in `<details>`, not the raw Gitea
export. Any difference exits non-zero.

## Scan for secrets

```sh
scripts/migrate/scan_secrets.py \
    --repo /tmp/public-clone \
    --manifest target/migrate/gitea.json \
    --forbidden-file ~/private/forbidden.txt
```

Built-in patterns cover private keys, GitHub and cloud tokens, `Authorization`
headers, credentials in URLs, `/home/<user>/` and secret-file names. Project
specific strings (internal host, slug, token path) live in a private file
outside the repository; one regex per line. `--allow-file` suppresses known-safe
matches. The scan reads every historical blob, every commit message and every
tracker text.

## Rewrite the history

```sh
scripts/migrate/rewrite_history.sh \
    --source /path/to/internal/repo \
    --output /tmp/public-clone \
    --replacements ~/private/replacements.txt
```

It clones the source, strips `.gitea/`, `scripts/export-public.sh`,
`scripts/issue-view.sh`, `target/` and `embed/`, applies the replacement file,
renames `master` to `main` and prints the result. Author and committer dates are
preserved. It never pushes.

`replacements.example.txt` shows the `git-filter-repo --replace-text` format;
copy it outside the repository and fill in the real values.

## Import the tracker

```sh
scripts/migrate/import_tracker.py \
    --repo <owner>/<repo> \
    --manifest target/migrate/expected.json \
    --token-file ~/private/github-token
```

Items are created strictly in ascending number order. The script aborts if
GitHub's next number does not match the manifest, so a stray object cannot
silently shift the archive. `--mode synthetic` boots an empty repository and
opens throwaway pull requests (rehearsal); `--mode refs` uses the rewritten
`head_sha`/`base_sha` from the manifest and needs the history already pushed.
Archived pull requests are closed after creation and their temporary branches
are deleted. `--prune-labels` removes target labels that are not in the manifest
(for example GitHub's default set). Use `--dry-run` to print the plan.

Rehearsal flow for an empty sandbox repository:

```sh
scripts/migrate/import_tracker.py --repo <owner>/sandbox --manifest gitea.json \
    --token-file ~/private/github-token --mode synthetic
scripts/migrate/github_manifest.py --repo <owner>/sandbox --output github.json --token-file ~/private/github-token
scripts/migrate/diff_manifests.py --expected gitea.json --actual github.json
```

## Gate order

1. `rewrite_history.sh`
2. `scan_secrets.py` over the clone and the manifests
3. `scripts/check.sh` inside the clone
4. tracker import
5. `diff_manifests.py`
