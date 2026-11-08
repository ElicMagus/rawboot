#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Every public Markdown document must have RU and zh-CN mirrors under i18n/.
# The English file is the primary source; see AGENTS.md.
set -euo pipefail

cd "$(dirname "$0")/.."

missing=0
while IFS= read -r file; do
    case "$file" in
        i18n/*) continue ;;
    esac
    for lang in ru zh-CN; do
        if [ ! -s "i18n/$lang/$file" ]; then
            echo "missing: i18n/$lang/$file" >&2
            missing=1
        fi
    done
done < <(git ls-files '*.md')

if [ "$missing" -ne 0 ]; then
    echo "error: public documents are not mirrored for every language" >&2
    exit 1
fi

echo "i18n: every public document has ru and zh-CN mirrors"
