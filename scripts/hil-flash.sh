#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Run an in-board FEL flash with a UART capture and assert the flasher status.
# The board must already be in FEL with the card inserted; this script does not
# power-cycle. One attempt per cold boot: the DDR-init stub is not idempotent.
set -euo pipefail

cd "$(dirname "$0")/.."

UART_DEV=${UART_DEV:-/dev/ttyUSB0}
CAPTURE_SECONDS=${CAPTURE_SECONDS:-20}
LABEL=${LABEL:-flasher}
SKIP_BUILD=0

usage() {
    cat <<'HELP'
usage: scripts/hil-flash.sh [--uart <dev>] [--seconds <n>] [--label <name>] [--skip-build]

Cold-start the board into FEL with the card inserted, then run this script.
It runs `cargo xtask fel-flash --yes`, captures USART0 for the flasher's driver
log, and fails unless the status is `done`/`ok`.

Environment: UART_DEV, CAPTURE_SECONDS, LABEL, HIL_NO_PROMPT.
HELP
}

die() {
    echo "error: $*" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --uart)
            [ "$#" -ge 2 ] || die "--uart needs a value"
            UART_DEV=$2
            shift 2
            ;;
        --seconds)
            [ "$#" -ge 2 ] || die "--seconds needs a value"
            CAPTURE_SECONDS=$2
            shift 2
            ;;
        --label)
            [ "$#" -ge 2 ] || die "--label needs a value"
            LABEL=$2
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            die "unexpected argument: $1"
            ;;
    esac
done

[ -e "$UART_DEV" ] || die "UART $UART_DEV not found; pass --uart or set UART_DEV"
command -v sg >/dev/null 2>&1 || die "sg (util-linux) is required for dialout access"
command -v stty >/dev/null 2>&1 || die "stty is required"

echo "Cold-start the board into FEL with the card inserted."
echo "Do one fel-flash attempt per cold boot; the DDR-init stub is not idempotent."
if [ "${HIL_NO_PROMPT:-0}" != "1" ]; then
    printf 'Press Enter when the board is in FEL with the card inserted: '
    read -r _
fi

uart_log=/tmp/rawboot-flasher-${LABEL}.log
host_log=/tmp/rawboot-fel-flash-${LABEL}.host.log
rm -f "$uart_log" "$host_log"

sg dialout -c "stty -F '$UART_DEV' 115200 raw -echo; timeout '$CAPTURE_SECONDS' cat '$UART_DEV'" \
    >"$uart_log" 2>&1 &
capture_pid=$!
sleep 1

args=(--yes)
if [ "$SKIP_BUILD" = "1" ]; then
    args+=(--skip-build)
fi

set +e
cargo xtask fel-flash "${args[@]}" 2>&1 | tee "$host_log"
status=${PIPESTATUS[0]}
set -e

wait "$capture_pid" || true

printf '\n--- UART tail ---\n'
tail -n 80 "$uart_log" || true
if [ ! -s "$uart_log" ]; then
    echo "warning: no UART log from $UART_DEV" >&2
fi

if [ "$status" -ne 0 ]; then
    echo "FAIL: fel-flash exited $status; logs: $host_log, $uart_log" >&2
    exit "$status"
fi
if ! grep -q 'step [0-9]* (done), result 0 (ok)' "$host_log"; then
    echo "FAIL: flasher status is not done/ok; logs: $host_log, $uart_log" >&2
    exit 1
fi

echo "PASS: fel-flash status done/ok; power-cycle from the card and confirm handoff/LED."
