#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Fast guardrail for documentation-only changes. CI still runs the full
# scripts/check.sh.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> branch docs rule"
bash scripts/check-branch-docs.sh

echo "==> documentation"
bash scripts/check-i18n.sh

echo "all docs checks passed"
