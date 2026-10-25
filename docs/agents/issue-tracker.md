# Issue tracker: Gitea

Issues for this repo live in self-hosted Gitea: https://github.com/ElicMagus/rawboot

## Access

- Base URL: https://github.com/ElicMagus/rawboot
- API base: https://github.com/ElicMagus/rawboot/api/v1
- Owner/repo: `ElicMagus/rawboot-internal`
- Auth: Personal Access Token stored OUTSIDE the repo at
  `~/.config/external-secret-file/gitea-token` (chmod 600). Pass it as
  `Authorization: token $(cat ~/.config/external-secret-file/gitea-token)`.
  **Never print, echo, or commit the token.**
- TLS: valid Let's Encrypt wildcard (`*.nikmaos.ru`); `curl` and `git` work
  without `-k` / `GIT_SSL_NO_VERIFY`.

### Network git operations (push / fetch / ls-remote)

Any network `git` command against Gitea over HTTPS prompts for a username and the
token as the password. Without a helper, terminal-less commands fail with
`could not read Username for 'https://github.com/ElicMagus/rawboot'`. Use a `GIT_ASKPASS`
helper for every network operation instead of embedding the token in the remote
URL, so the token never lands in `.git/config` or the shell history:

```sh
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git ls-remote origin
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git push -u origin <branch>
```

## Workflow

Issue -> branch -> pull request -> merge into `master`.

- Branch name: `feat/<N>-<slug>` for features, `fix/<N>-<slug>` for fixes
  (`<N>` = issue number from Gitea).
- Open a Gitea pull request referencing the issue (e.g. `Closes #<N>`), then
  merge into `master`.

## Operations

All calls use header `Authorization: token <token>`. `{B}` below is
`https://github.com/ElicMagus/rawboot/api/v1/repos/ElicMagus/rawboot-internal`.

- **Publish an issue** (create):
  `POST {B}/issues` with JSON `{"title": ..., "body": ..., "labels": [<label ids>]}`
  -> the response's `number` is the issue number.
- **Fetch an issue**: `GET {B}/issues/<N>`
- **Fetch the brief**: `GET {B}/issues/<N>/comments`. The `## Agent Brief` comment written by
  triage is authoritative: it carries the scope, acceptance criteria and out-of-scope items the
  body may omit. Read it before implementing.
- **List issues**: `GET {B}/issues?state=all&limit=50`
- **Comment**: `POST {B}/issues/<N>/comments` with `{"body": ...}`
- **Apply labels**: `POST {B}/issues/<N>/labels` with `{"labels": [<label ids>]}`
- **Close / reopen**: `PATCH {B}/issues/<N>` with `{"state":"closed"}` / `{"state":"open"}`

### Dependencies (native blocked-by links)

Prefer Gitea's native dependency links over a textual `Blocked by:` convention.
The `dependencies` of issue `<N>` are the issues that **block** it; its `blocks`
are the issues it blocks. Both are read with `GET` and edited with `POST` /
`DELETE`, the body being an `IssueMeta`: `{"owner":"ElicMagus","repo":"rawboot-internal","index":<other issue>}`.
**Gitea requires `owner` and `repo` explicitly.**

```sh
# make #42 depend on #40 (#40 blocks #42)
curl -s -X POST -H "Authorization: token $(cat ~/.config/external-secret-file/gitea-token)" \
  -H "Content-Type: application/json" \
  -d '{"owner":"ElicMagus","repo":"rawboot-internal","index":40}' \
  "{B}/issues/42/dependencies"

# read #42's blockers
curl -s -H "Authorization: token $(cat ~/.config/external-secret-file/gitea-token)" \
  "{B}/issues/42/dependencies"
```

A ticket `<N>` is **unblocked** when every item from `/dependencies` reports
`state == "closed"`.

### Label ids

`POST {B}/issues/<N>/labels` and `PUT {B}/issues/<N>/labels` (replace the whole
set) take numeric ids, not names. Keep this table in sync with
`GET {B}/labels`:

| Label           | id  |
| --------------- | --- |
| bugfix          | 61  |
| feature         | 62  |
| needs-info      | 57  |
| needs-triage    | 56  |
| ready-for-agent | 58  |
| ready-for-human | 59  |
| wontfix         | 60  |

## Notes

- Issue bodies and comments must not contain non-BMP characters (emoji): the
  Gitea/MySQL schema stores text as `utf8mb3` and rejects such input with HTTP
  500. Keep tracker text to plain BMP (ASCII/Cyrillic).
- This file is editable at any time; re-run `/setup-matt-pocock-skills` only to
  change the tracker kind or start over.
