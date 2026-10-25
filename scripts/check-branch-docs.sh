#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Contract docs (ADR, GLOSSARY, spec) may ride a feat/fix branch. Runbooks,
# research notes and other docs go straight to master.
set -euo pipefail

cd "$(dirname "$0")/.."

branch=${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-}}
branch=${branch#refs/heads/}
if [ -z "$branch" ]; then
    branch=$(git symbolic-ref --short HEAD 2>/dev/null || true)
fi
case "$branch" in
    feat/* | fix/*) ;;
    *) exit 0 ;;
esac

base=origin/master
if ! git rev-parse --verify --quiet "$base" >/dev/null; then
    base=master
fi
if ! git rev-parse --verify --quiet "$base" >/dev/null; then
    echo "skip: no master ref to diff against" >&2
    exit 0
fi
if ! committed=$(git diff --name-only "$base"...HEAD); then
    echo "skip: cannot diff $base...HEAD" >&2
    exit 0
fi

changed=$(printf '%s\n' "$committed"; git diff --name-only; git diff --cached --name-only)
non_contract=$(
    printf '%s\n' "$changed" |
        sort -u |
        grep -E '^(docs/|research/|GLOSSARY\.md$)' |
        grep -vE '^(GLOSSARY\.md$|docs/spec-rawboot-v1\.md$|docs/adr/)' || true
)
if [ -n "$non_contract" ]; then
    echo "error: non-contract docs must be committed directly to master, not on $branch:" >&2
    printf '%s\n' "$non_contract" >&2
    exit 1
fi
