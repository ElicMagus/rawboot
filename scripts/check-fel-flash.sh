#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Validate the fel-flash artifacts and print its plan without opening FEL or
# touching a card. The caller builds the images first.
set -euo pipefail

cd "$(dirname "$0")/.."

output=$(cargo xtask fel-flash --dry-run --skip-build)
printf '%s\n' "$output"

if ! grep -q '^dry-run:' <<<"$output"; then
    echo "error: fel-flash dry run did not report its plan" >&2
    exit 1
fi
