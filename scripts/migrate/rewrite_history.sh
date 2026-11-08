#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Rewrite a clone of the internal repository into the publishable history.
#
# Never pushes. The replacement file and the source repository are passed in,
# so this script carries no internal host, path or token.
set -euo pipefail

usage() {
    cat <<'EOF'
Rewrite a clone of the internal repository into the publishable history.

Usage: scripts/migrate/rewrite_history.sh --source <repo> --output <dir>
       [--replacements <file>] [--from-branch <name>] [--to-branch <name>]

The default branch names are master -> main. Internal paths are stripped:
.gitea/, scripts/export-public.sh, scripts/issue-view.sh, target/, embed/.

The --replacements file is a git-filter-repo --replace-text file, kept outside
this repository. Author and committer dates are preserved. The script never
pushes; inspect and verify the output clone first.
EOF
}

source_repo=
output=
replacements=
from_branch=master
to_branch=main

while [ $# -gt 0 ]; do
    case "$1" in
        --source)
            source_repo=${2:?--source needs a repository path}
            shift 2
            ;;
        --output)
            output=${2:?--output needs a directory}
            shift 2
            ;;
        --replacements)
            replacements=${2:?--replacements needs a file}
            shift 2
            ;;
        --from-branch)
            from_branch=${2:?--from-branch needs a name}
            shift 2
            ;;
        --to-branch)
            to_branch=${2:?--to-branch needs a name}
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ -z "$source_repo" ] || [ -z "$output" ]; then
    usage >&2
    exit 2
fi
if [ ! -d "$source_repo/.git" ] && [ ! -d "$source_repo" ]; then
    echo "error: source repository not found: $source_repo" >&2
    exit 1
fi
if [ -e "$output" ] && [ -n "$(ls -A "$output" 2>/dev/null)" ]; then
    echo "error: output directory is not empty: $output" >&2
    exit 1
fi
if [ -n "$replacements" ] && [ ! -f "$replacements" ]; then
    echo "error: replacements file not found: $replacements" >&2
    exit 1
fi

if git filter-repo --version >/dev/null 2>&1; then
    runner=(git filter-repo)
elif python3 -c 'import git_filter_repo' >/dev/null 2>&1; then
    runner=(python3 -m git_filter_repo)
else
    echo "error: git-filter-repo is not installed" >&2
    echo "hint: pipx install git-filter-repo" >&2
    exit 1
fi

stripped_paths=(
    .gitea
    scripts/export-public.sh
    scripts/issue-view.sh
    target
    embed
)

mkdir -p "$output"
git clone --no-local --quiet "$source_repo" "$output"
rm -rf "$output/.git/replace"

filter_args=(--force --invert-paths --replace-refs delete-no-add --prune-empty never)
for path in "${stripped_paths[@]}"; do
    filter_args+=(--path "$path")
done
if [ -n "$replacements" ]; then
    filter_args+=(--replace-text "$replacements")
fi

(
    cd "$output"
    "${runner[@]}" "${filter_args[@]}"
    if git rev-parse --verify --quiet "refs/heads/$from_branch" >/dev/null; then
        git branch -m "$from_branch" "$to_branch"
    fi
    while IFS= read -r branch; do
        [ "$branch" = "$to_branch" ] && continue
        git branch -D "$branch"
    done < <(git for-each-ref --format='%(refname:short)' refs/heads)
)

echo "==> rewritten history"
git -C "$output" rev-list --count HEAD | sed 's/^/commits: /'
git -C "$output" log -1 --format='head:    %h %ad %an <%ae>%nsubject: %s' --date=iso
echo "branches:"
git -C "$output" for-each-ref refs/heads \
    --format='  %(refname:short) %(objectname:short)'
echo "tags:"
git -C "$output" for-each-ref refs/tags \
    --format='  %(refname:short) -> %(objectname:short) %(subject)'

cat <<EOF

Rewritten clone: $output
This clone is not verified yet. Run scripts/migrate/scan_secrets.py and
scripts/migrate/diff_manifests.py, then scripts/check.sh inside it.
Nothing was pushed.
EOF
