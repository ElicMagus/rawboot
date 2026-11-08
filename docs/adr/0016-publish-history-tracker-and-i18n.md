# Public repository: real history, a 1:1 tracker and trilingual documentation

Before this decision the public GitHub repository `ElicMagus/rawboot` was a single squash commit
without history, and the course of decisions lived only in the internal Gitea: 207 commits up to the
tag `v0.1.0`, 52 issues, 4 PRs, 72 comments and 33 blocked-by links. The user needs the course of
decisions, and the project is complete and will only be maintained with fixes going forward.
Decision: publish the real history with rewritten paths and contents, recreate the tracker on
GitHub one-to-one and keep the documentation in three languages.

## History and publication boundaries

The history of `master` up to and including the tag `v0.1.0` (207 commits) is published together
with the annotated tag itself; the tag message, tagger and date are preserved, and the target after
rewriting is new. The commits of the migration itself and of the translations are laid on top as new
public commits.

The history is rewritten with `git-filter-repo`: author and committer dates are preserved, and the
owner's real name and emails remain. Tokens and passwords, the path to the token file, home paths
of the form `/home/<user>` and references to the internal Gitea are scrubbed. Everything related to
code, environment and decisions is published (including `AGENTS.md`, `CODING_STANDARDS.md`,
`docs/agents/` and `research/`); `.gitea/`, `scripts/export-public.sh`, `scripts/issue-view.sh`,
`target/` and `embed/` remain internal. A force-push replaces the previous squash commit. Unmerged
branches (at the time of the decision — `fix/54`) remain internal until merged and are published in
a separate manual pass.

## Tracker

Gitea remains the canonical tracker and source of truth; GitHub receives a one-to-one archive.
Numbers 1–56 are preserved: objects are created strictly in ascending order, including the 4 PRs at
their numbers. PRs are formatted as closed archival records with a temporary base branch and a link
to the real merge commit; a "replay" with a Merged badge is rejected because it yields new SHAs and
dates. 72 comments, labels, a milestone and 33 blocked-by links are transferred — the latter via
GitHub's native dependencies. GitHub sets the creation and closing dates on the server, so they
equal the migration date; the original dates are recorded in the object body. Texts are bilingual:
English on top, the Russian original in `<details>`. Commit messages remain Russian.

## Documentation

English is the primary source and lives at the current paths. The Russian and Chinese translations
mirror the tree in `i18n/ru/` and `i18n/zh-CN/` with the same relative paths. The entire public set
is translated, including `research/`, `AGENTS.md` and `CODING_STANDARDS.md`. API names, register
names and glossary terms are not translated. Translation drafts are made by the agent without a
separate review; the owner edits them as needed. In case of divergence, the English version takes
precedence.

## Acceptance

Before pushing, the following are mandatory: a diff of the Gitea↔GitHub manifest (numbers, types,
states, bodies, labels, milestone, comments, dependencies), a secret scan of all historical blobs
and of all tracker texts, and `scripts/check.sh` on the public HEAD. Any red check blocks the push.

## Considered Options

- **A squash commit plus a separate document with the decision history** (rejected): it does not
  provide "decision → commit → PR" traceability.
- **A synthetic curated history** (rejected): it looks real but is not.
- **A PR replay with a real merge** (rejected): a Merged badge at the cost of new SHAs and current
  dates.
- **Archival closed PRs with a link to the merge commit** (accepted): history and dates are exact.
- **A tracker in English only or Russian only** (rejected): either the primary source or
  accessibility for foreign users is lost.
- **Translations next to each folder** (rejected): they scatter copies and complicate checking
  completeness; a single `i18n/` mirror is accepted.

## Consequences

- The public squash commit disappears; all previous links to it stop working.
- `scripts/export-public.sh` and the "do not export history" rule from
  `docs/agents/export-public.md` are revoked; publication becomes a separate procedure.
- GitHub object numbering is frozen during the migration: any extra issue will shift it
  irreversibly.
- The internal email domain remains visible in commit metadata (the owner's decision).
- Translation drift is accepted: the project is complete, and the English version remains the
  truth.
- `fix/54` and other unmerged branches are published only after merging.
