#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# AGENTS.md sends ADRs, GLOSSARY.md, research/ and docs/ straight to master.
# Fail a feat/fix branch that carries them.
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
docs=$(
    printf '%s\n' "$changed" |
        sort -u |
        grep -E '^(docs/|research/|GLOSSARY\.md$)' || true
)
if [ -n "$docs" ]; then
    echo "error: docs must be committed directly to master, not on $branch:" >&2
    printf '%s\n' "$docs" >&2
    exit 1
fi
