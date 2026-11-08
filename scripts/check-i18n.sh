#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Documentation guardrails: primary docs are English, every public Markdown file
# has RU and zh-CN mirrors under i18n/, and every relative link resolves.
# See AGENTS.md.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> primary docs are English"
python3 scripts/check_primary_language.py

echo "==> markdown links"
python3 scripts/check_md_links.py

echo "==> i18n mirrors"
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
