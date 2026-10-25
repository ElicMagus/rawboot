#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Load target/rawboot-fel.bin over FEL, capture USART0, and assert the boot log.
# Refuses an image whose build stamp is not HEAD and requires a quiet UART
# before loading. Runbook: docs/agents/hil.md.
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

# `HIL_FEL_PAYLOAD=1` uploads the payload image to SRAM and tells the loader to
# boot it through the FEL mailbox instead of the SD card. These addresses mirror
# `rawboot-image/src/fel.rs`; keep both together.
FEL_PAYLOAD=${HIL_FEL_PAYLOAD:-0}
FEL_MAILBOX_ADDR=0x00044000
FEL_PAYLOAD_ADDR=0x00039000
FEL_ARGS_ADDR=0x00038000
FEL_PAYLOAD_LIMIT=0xB000
FEL_ARGS_LIMIT=0x1000
FEL_MAILBOX_MAGIC=0x4d464252
PAYLOAD_IMG=${HIL_PAYLOAD_IMG:-target/rawboot-payload.img}

if [ "$FEL_PAYLOAD" = "1" ] && [ "${HIL_REQUIRE_SD:-0}" = "1" ]; then
    echo "error: HIL_FEL_PAYLOAD=1 boots from SRAM and never touches the SD card;" >&2
    echo "error: unset HIL_REQUIRE_SD or run the SD path without HIL_FEL_PAYLOAD" >&2
    exit 2
fi

# Refuse an image built from another tree: switching branches to commit docs
# used to leave the previously built image in place, so HIL tested stale code.
STAMP="${IMAGE}.build"
if [ -f "$STAMP" ]; then
    build_commit=$(grep '^commit=' "$STAMP" | cut -d= -f2- || true)
    build_branch=$(grep '^branch=' "$STAMP" | cut -d= -f2- || true)
    build_dirty=$(grep '^dirty=' "$STAMP" | cut -d= -f2- || true)
    echo "image built from ${build_branch:-unknown} @ ${build_commit:-unknown} (dirty=${build_dirty:-unknown})"
    head_commit=$(git rev-parse HEAD 2>/dev/null || true)
    if [ "${HIL_SKIP_PROVENANCE:-0}" != "1" ] && [ -n "$head_commit" ] \
        && [ "$build_commit" != "$head_commit" ]; then
        echo "error: image commit ${build_commit:-unknown} does not match HEAD $head_commit" >&2
        echo "error: rebuild with 'cargo xtask image' on the branch under test" >&2
        exit 2
    fi
    if [ "$build_dirty" = "true" ]; then
        echo "warning: image was built from a dirty tree" >&2
    fi
else
    echo "warning: no build stamp at $STAMP; cannot verify the image matches HEAD" >&2
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
  - remove power completely, hold FEL/BOOT, then apply power: the strap is
    sampled at cold power-on, so holding it through a reset does not work;
  - do not use `xfel reset`: a watchdog reset leaves the watchdog armed, so the
    loader then boots in a loop, and a loader left running from a previous run
    keeps printing on USART0.
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

# `xfel version` can succeed while a loader from a previous run still prints on
# USART0. Require silence before touching the board, so stale output is reported
# as a busy board rather than as a reset loop after loading.
echo "preflight: checking that UART is quiet (board idle in FEL)..."
PREFLIGHT=(python3 scripts/hil_serial.py --preflight --dev "$DEV")
ALLOW_BUSY=0
if [ "${HIL_SKIP_PREFLIGHT:-0}" = "1" ]; then
    echo "warning: HIL_SKIP_PREFLIGHT=1: draining UART and replacing any running loader" >&2
    PREFLIGHT+=(--allow-busy)
    ALLOW_BUSY=1
fi
if [ -r "$DEV" ]; then
    "${PREFLIGHT[@]}"
else
    sg dialout -c "$(printf '%q ' "${PREFLIGHT[@]}")"
fi

LOG=${TMPDIR:-/tmp}/rawboot-hil-${LABEL}.log
CAPTURE=(python3 scripts/hil_serial.py "$LOG" --seconds "$SECONDS_CAP" --dev "$DEV")
if [ "$ALLOW_BUSY" = "1" ]; then
    CAPTURE+=(--allow-busy)
fi
if [ "${HIL_REQUIRE_SD:-0}" = "1" ]; then
    echo "SD assertion: card detected and non-zero block read required"
    CAPTURE+=(--require-sd)
fi
if [ "${HIL_REQUIRE_HANDOFF:-0}" = "1" ]; then
    echo "handoff assertion: payload must be entered"
    CAPTURE+=(--require-handoff)
fi
if [ "$FEL_PAYLOAD" = "1" ]; then
    echo "FEL payload: uploading $PAYLOAD_IMG to $FEL_PAYLOAD_ADDR"
    CAPTURE+=(--require-fel)
fi
if [ "${HIL_REQUIRE_PAYLOAD:-0}" = "1" ] || [ "$FEL_PAYLOAD" = "1" ]; then
    if [ ! -f "$PAYLOAD_IMG" ]; then
        echo "error: payload image not found: $PAYLOAD_IMG (run cargo xtask payload)" >&2
        exit 2
    fi
    echo "payload assertion: handoff must match $PAYLOAD_IMG"
    CAPTURE+=(--payload "$PAYLOAD_IMG")
fi
if [ -n "${HIL_BOOT_CFG_LEN:-}" ]; then
    if ! [[ "$HIL_BOOT_CFG_LEN" =~ ^[0-9]+$ ]]; then
        echo "error: HIL_BOOT_CFG_LEN must be a byte count, got $HIL_BOOT_CFG_LEN" >&2
        exit 2
    fi
    echo "boot.cfg assertion: the SD path must hand off $HIL_BOOT_CFG_LEN bytes"
    CAPTURE+=(--boot-cfg "$HIL_BOOT_CFG_LEN")
fi

FEL_PAYLOAD_SIZE=0
if [ "$FEL_PAYLOAD" = "1" ]; then
    FEL_PAYLOAD_SIZE=$(stat -c %s "$PAYLOAD_IMG")
    if ((FEL_PAYLOAD_SIZE > FEL_PAYLOAD_LIMIT)); then
        echo "error: payload image is $FEL_PAYLOAD_SIZE bytes; the FEL SRAM window holds $FEL_PAYLOAD_LIMIT" >&2
        exit 2
    fi
    if [ -n "${HIL_FEL_ARGS:-}" ]; then
        fel_args_size=$(printf '%s\0' "$HIL_FEL_ARGS" | wc -c)
        if ((fel_args_size > FEL_ARGS_LIMIT)); then
            echo "error: HIL_FEL_ARGS needs $fel_args_size bytes; the FEL argument window holds $FEL_ARGS_LIMIT" >&2
            exit 2
        fi
        CAPTURE+=(--fel-args "$FEL_ARGS_ADDR")
    fi
fi

if [ -r "$DEV" ]; then
    "${CAPTURE[@]}" &
else
    sg dialout -c "$(printf '%q ' "${CAPTURE[@]}")" &
fi
capture_pid=$!
sleep 2

if [ "$FEL_PAYLOAD" = "1" ]; then
    echo "load: $XFEL write $FEL_PAYLOAD_ADDR $PAYLOAD_IMG, mailbox at $FEL_MAILBOX_ADDR, exec 0x00028000"
    "$XFEL" write 0x00028000 "$IMAGE" >/dev/null
    "$XFEL" write "$FEL_PAYLOAD_ADDR" "$PAYLOAD_IMG" >/dev/null

    fel_args=0
    if [ -n "${HIL_FEL_ARGS:-}" ]; then
        args_file=$(mktemp "${TMPDIR:-/tmp}/rawboot-fel-args.XXXXXX")
        printf '%s\0' "$HIL_FEL_ARGS" >"$args_file"
        "$XFEL" write "$FEL_ARGS_ADDR" "$args_file" >/dev/null
        rm -f "$args_file"
        fel_args=$((FEL_ARGS_ADDR))
        echo "load: args \"$HIL_FEL_ARGS\" at $FEL_ARGS_ADDR"
    fi

    "$XFEL" write32 "$((FEL_MAILBOX_ADDR + 4))" "$((FEL_PAYLOAD_ADDR))" >/dev/null
    "$XFEL" write32 "$((FEL_MAILBOX_ADDR + 8))" "$FEL_PAYLOAD_SIZE" >/dev/null
    "$XFEL" write32 "$((FEL_MAILBOX_ADDR + 12))" "$fel_args" >/dev/null
    "$XFEL" write32 "$((FEL_MAILBOX_ADDR))" "$((FEL_MAILBOX_MAGIC))" >/dev/null
    "$XFEL" exec 0x00028000 >/dev/null
else
    echo "load: $XFEL write 0x00028000 $IMAGE && $XFEL exec 0x00028000"
    "$XFEL" write 0x00028000 "$IMAGE" >/dev/null
    "$XFEL" exec 0x00028000 >/dev/null
fi

status=0
wait "$capture_pid" || status=$?
if [ "$status" -eq 0 ] && [ "${HIL_REQUIRE_LED:-0}" = "1" ]; then
    if [ "${HIL_LED_CONFIRMED:-0}" = "1" ]; then
        echo "LED confirmed via HIL_LED_CONFIRMED=1."
    elif [ -t 0 ] && [ "${HIL_NO_PROMPT:-0}" != "1" ]; then
        printf 'Does PD22 show the payload short-long blink (not the loader 50 ms toggle)? [y/N] '
        if ! read -r led_answer; then
            echo "error: LED confirmation aborted" >&2
            status=4
        else
            case "$led_answer" in
                [yY] | [yY][eE][sS]) echo "LED confirmed." ;;
                *)
                    echo "error: LED pattern not confirmed" >&2
                    status=4
                    ;;
            esac
        fi
    else
        echo "error: HIL_REQUIRE_LED=1 needs an interactive prompt or HIL_LED_CONFIRMED=1" >&2
        status=4
    fi
fi
echo "raw uart log: $LOG"
exit "$status"
