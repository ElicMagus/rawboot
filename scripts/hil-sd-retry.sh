#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Retry the card-init step when the SD slot is flaky (#43). This runs
# hil-check.sh with SD, handoff and payload assertions; only a run that fails
# because the card was not found is retried, so a real assertion failure still
# stops the loop.
set -euo pipefail

cd "$(dirname "$0")/.."

IMAGE=${1:-target/rawboot-fel.bin}
LABEL=${2:-sd-retry}
ATTEMPTS=${HIL_SD_RETRIES:-3}
SECONDS_CAP=${HIL_CAPTURE_SECONDS:-6}

if ! [[ "$ATTEMPTS" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: HIL_SD_RETRIES must be a positive integer, got $ATTEMPTS" >&2
    exit 2
fi
if [ ! -f "$IMAGE" ]; then
    echo "error: image not found: $IMAGE (run cargo xtask image)" >&2
    exit 2
fi

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

for attempt in $(seq 1 "$ATTEMPTS"); do
    echo "==> SD init attempt $attempt/$ATTEMPTS"
    status=0
    HIL_NO_PROMPT="${HIL_NO_PROMPT:-1}" \
        HIL_SKIP_PREFLIGHT="${HIL_SKIP_PREFLIGHT:-1}" \
        HIL_REQUIRE_SD=1 \
        HIL_REQUIRE_HANDOFF=1 \
        HIL_REQUIRE_PAYLOAD=1 \
        scripts/hil-check.sh "$IMAGE" "${LABEL}-${attempt}" "$SECONDS_CAP" >"$tmp" 2>&1 \
        || status=$?
    cat "$tmp"

    if [ "$status" -eq 0 ]; then
        echo "PASS on attempt $attempt (raw log: ${TMPDIR:-/tmp}/rawboot-hil-${LABEL}-${attempt}.log)"
        exit 0
    fi
    if ! grep -q 'missing SD/MMC card found' "$tmp"; then
        exit "$status"
    fi
    echo "card not found on attempt $attempt; retrying"
done

echo "error: SD/MMC card not found after $ATTEMPTS attempts" >&2
exit 1
