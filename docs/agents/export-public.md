# Publishing the public repository

The internal Gitea repository is canonical. The public GitHub repository
(`https://github.com/ElicMagus/rawboot`) is a mirror with rewritten history
(ADR-0016). The old single-squashed-commit export is retired.

## What is published

- `master` through the `v0.1.0` tag (207 commits) and the annotated tag itself.
- Every tracked file except `.gitea/`, `scripts/export-public.sh`,
  `scripts/issue-view.sh`, `target/` and `embed/`.
- Code, environment and decisions, including `AGENTS.md`,
  `CODING_STANDARDS.md`, `docs/agents/` and `research/`.
- Commits made after the tag (this runbook, the migration tooling, the `i18n/`
  tree and later fixes) land on top as normal public commits.
- Unmerged internal branches are not published; merge first, then run a manual
  publish.

## Rewriting

The history is rewritten with `git-filter-repo`:

- strip the internal paths listed above from every revision;
- preserve author and committer dates and the real name and e-mail addresses;
- replace secrets and passwords, the external token-file path, home paths of the
  form `/home/<user>` and links to the internal Gitea with neutral text;
- keep commit subjects and bodies in Russian.

The tag message, tagger and date survive; only the tag target changes.

## Credentials

Publishing needs a GitHub token with **Contents**, **Issues** and **Pull
requests** read/write on the target repository. A read of a public repository
proves nothing: probe write access with a throwaway label (`POST` then `DELETE
/labels`) and a temporary branch push before starting. Keep the token in the
owner's secrets directory, never in the repository.

## Private inputs

Two files are kept outside the repository, in the owner's secrets directory:

- `migrate-replacements.txt` — the `git-filter-repo --replace-text` rules with
  the internal host, slug, token-file path and home paths;
- `migrate-allow.txt` — allow-listed regexes for the fake fixtures in the
  migration tests and for historical blobs that predate the fixture cleanup.

The migration and fix-publish commands take their paths explicitly. Never commit
them.

## Verification gates

All four are blocking; run them on a clean clone before any push.

1. **Manifest diff.** `scripts/migrate/` exports a manifest from Gitea and from
   GitHub, then diffs numbers, kinds, states, bodies, labels, milestone, comment
   count and bodies, and blocked-by edges. Zero differences required.
2. **Secret scan.** Grep every historical blob
   (`git rev-list --objects --all`) and every tracker text for token and password
   patterns, the token-file path, `/home/<user>` and internal Gitea links.
3. **Build.** `scripts/check.sh` passes in the rewritten clone.
4. **Spot check.** Ten issues compared by hand against Gitea.

## One-time migration

1. Freeze Gitea numbering: create no issue or pull request until step 5 ends, or
   GitHub numbers will shift permanently.
2. Take bundles: an internal `git bundle` of `master` plus tags and a bundle of
   the current public `main`. Keep both until the final verification passes.
3. Dry-run in a temporary directory: rewrite, run all gates, count commits,
   confirm the tag target.
4. Rehearse the tracker in a throwaway repository: create all 56 objects in
   ascending order and diff the manifest.
5. Push the rewritten `main` with `--force` and push the `v0.1.0` tag.
6. Create the labels and the milestone, then create objects #1–56 in ascending
   order. Pull requests are archived as closed records with a temporary base
   branch and a body that links the real merge commit. Import the 72 comments,
   the blocked-by edges and the bilingual EN/RU text (English on top, the Russian
   original in `<details>`).
7. Delete the temporary base branches and rerun the gates.

## Publishing a later fix

1. Merge the fix into internal `master` and tag it if it is a release.
2. Rewrite a fresh clone with the same filters and run the gates.
3. Push the rewritten `main` and any new tag. New commits appear after `v0.1.0`.

## Tracker

GitHub issues and pull requests are a read-only archive; do not create new ones
there. Gitea remains the only live tracker.

## Caveats

- The force-push is irreversible: the bundles from step 2 are the only way back.
- GitHub sets issue, comment and pull-request timestamps to the migration date;
  each body records the original date.
- The internal mail domain stays visible in commit metadata by owner decision.
