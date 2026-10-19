#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Load target/rawboot-fel.bin over FEL, capture USART0, and assert the boot log.
# Runbook: docs/agents/hil.md.
set -euo pipefail

cd "$(dirname "$0")/.."

IMAGE=${1:-target/rawboot-fel.bin}
LABEL=${2:-hil}
SECONDS_CAP=${3:-4}
DEV=${UART_DEV:-/dev/ttyUSB0}
XFEL=${XFEL:-$(command -v xfel || true)}
if [ -z "$XFEL" ] && [ -x "$HOME/.local/bin/xfel" ]; then
    XFEL="$HOME/.local/bin/xfel"
fi
if [ -z "$XFEL" ]; then
    echo "error: xfel not found; see docs/agents/hil.md" >&2
    exit 2
fi
if [ ! -f "$IMAGE" ]; then
    echo "error: image not found: $IMAGE (run cargo xtask image)" >&2
    exit 2
fi

can_read() {
    [ -r "$DEV" ] || sg dialout -c "test -r '$DEV'" 2>/dev/null
}
if ! can_read; then
    echo "error: $DEV is not readable; see docs/agents/hil.md (dialout / udev)" >&2
    exit 2
fi

# FEL hand-off is still manual: ask the human to power-cycle the board into FEL.
# Piped stdin or HIL_NO_PROMPT=1 skips the prompt for automated runs.
if [ -t 0 ] && [ "${HIL_NO_PROMPT:-0}" != "1" ]; then
    cat <<'EOF'
Put the board into FEL:
  - hold the FEL/BOOT button while applying power, or hold it and press reset;
  - power-cycle rather than `xfel reset`: a watchdog reset leaves the watchdog
    armed, so the loader then boots in a loop, and a loader left running from a
    previous run keeps printing on USART0.
EOF
    printf 'Press Enter once the board is in FEL... '
    read -r _
    echo
fi

echo "waiting for responsive FEL (up to 180 s; power-cycle, do not xfel reset)..."
for _ in $(seq 1 180); do
    if "$XFEL" version >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
if ! "$XFEL" version >/dev/null 2>&1; then
    echo "error: no responsive FEL after 180 s" >&2
    exit 2
fi

LOG=${TMPDIR:-/tmp}/rawboot-hil-${LABEL}.log
CAPTURE=(python3 scripts/hil_serial.py "$LOG" --seconds "$SECONDS_CAP" --dev "$DEV")
if [ -r "$DEV" ]; then
    "${CAPTURE[@]}" &
else
    sg dialout -c "$(printf '%q ' "${CAPTURE[@]}")" &
fi
capture_pid=$!
sleep 2

echo "load: $XFEL write 0x00028000 $IMAGE && $XFEL exec 0x00028000"
"$XFEL" write 0x00028000 "$IMAGE" >/dev/null
"$XFEL" exec 0x00028000 >/dev/null

status=0
wait "$capture_pid" || status=$?
echo "raw uart log: $LOG"
exit "$status"
