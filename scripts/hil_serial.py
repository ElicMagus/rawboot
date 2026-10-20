#!/usr/bin/env python3
# SPDX-License-Identifier: Unlicense
"""Capture USART0, assert the rawboot boot log, and print a compact summary."""

import argparse
import re
import sys
import time

try:
    import serial
except ImportError:
    print(
        "error: pyserial is required (install python3-serial or pyserial)",
        file=sys.stderr,
    )
    sys.exit(2)

REQUIRED = ["Hello", "CLK:", "DRAM simple test OK"]
LOADER_RUNNING = "S=0x"
HANDOFF = "BOOT: handoff to "
SD_FOUND = "SD/MMC card found"
SD_READ = re.compile(r"^SD: read [1-9][0-9]* block\(s\)", re.MULTILINE)
RESET_LOOP_HINT = (
    "power-cycle into FEL (do not xfel reset) and rerun"
)


def evaluate(
    text: str, require_sd: bool = False, require_handoff: bool = False
) -> tuple[bool, str]:
    """Judge one captured boot log.

    Returns (ok, message); message is empty when ok, otherwise the failure reason.
    A single clean boot passes; a reset loop (more than one ``Hello``) fails even
    when the required lines are present. The loader ends either in its LED loop
    (``S=0x``) or in a payload handoff (``BOOT: handoff to``); one of the two is
    required. With ``require_sd`` the log must also show a detected card and a
    non-zero block read; with ``require_handoff`` the loader loop is not enough.
    """
    if text.count("Hello") > 1:
        return False, f"multiple Hello lines; the board is reset-looping or the capture is stale - {RESET_LOOP_HINT}"

    required = list(REQUIRED)
    if require_sd:
        required.append(SD_FOUND)
    missing = [item for item in required if item not in text]
    if missing:
        return False, "missing " + ", ".join(missing)

    if require_handoff and HANDOFF not in text:
        return False, "missing a payload handoff (BOOT: handoff to ...)"

    if not require_handoff and HANDOFF not in text and LOADER_RUNNING not in text:
        return False, "missing S=0x or BOOT: handoff (loader loop or payload handoff)"

    if require_sd and not SD_READ.search(text):
        return False, "missing a successful SD read (SD: read N block(s), N > 0)"

    return True, ""


def busy_message(data: bytes) -> str | None:
    """Explain non-empty preflight traffic; a quiet board yields ``None``.

    The FEL USB gadget can still answer `xfel` while a loader from a previous
    run is printing on USART0, so UART silence is the authority on an idle board.
    """
    if not data:
        return None
    text = data.decode("utf-8", "replace")
    if "Hello" in text or "S=0x" in text:
        detail = "a loader is already running"
    else:
        detail = "the UART line is noisy (floating line or bad ground)"
    return (
        f"UART is not quiet ({len(data)} bytes); {detail}. "
        "Power-cycle into FEL (do not xfel reset) and rerun, or set "
        "HIL_SKIP_PREFLIGHT=1 to drain and replace the running loader."
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", nargs="?")
    parser.add_argument("--seconds", type=float, default=4.0)
    parser.add_argument("--drain", type=float, default=1.0)
    parser.add_argument("--quiet", type=float, default=0.5)
    parser.add_argument("--preflight", action="store_true", help="require a quiet UART and exit")
    parser.add_argument(
        "--allow-busy",
        action="store_true",
        help="warn instead of failing when preflight sees UART traffic",
    )
    parser.add_argument(
        "--require-sd",
        action="store_true",
        help="also require a detected card and a non-zero block read",
    )
    parser.add_argument(
        "--require-handoff",
        action="store_true",
        help="require the payload handoff line instead of the loader loop",
    )
    parser.add_argument("--dev", default="/dev/ttyUSB0")
    parser.add_argument("--baud", type=int, default=115200)
    args = parser.parse_args()
    if args.log is None and not args.preflight:
        parser.error("log is required unless --preflight is given")

    try:
        port = serial.Serial(args.dev, args.baud, timeout=0.05)
    except (serial.SerialException, OSError) as err:
        print(f"error: cannot open {args.dev}: {err}", file=sys.stderr)
        print("see docs/agents/hil.md (dialout / udev rule)", file=sys.stderr)
        return 2

    if args.preflight:
        port.reset_input_buffer()
        drain_until = time.time() + args.drain
        while time.time() < drain_until:
            port.read(65536)
        port.reset_input_buffer()

        start = time.time()
        data = b""
        while time.time() - start < args.quiet:
            chunk = port.read(4096)
            if chunk:
                data += chunk
                break
        message = busy_message(data)
        if message:
            if args.allow_busy:
                print(f"warning: {message}", file=sys.stderr)
                print("continuing: the image will replace the running loader.")
                return 0
            print(f"error: {message}", file=sys.stderr)
            return 3
        print("UART quiet: board is idle in FEL.")
        return 0

    port.reset_input_buffer()
    drain_until = time.time() + args.drain
    while time.time() < drain_until:
        port.read(65536)
    port.reset_input_buffer()

    start = time.time()
    chunks = []
    with open(args.log, "w", encoding="utf-8") as log:
        while time.time() - start < args.seconds:
            data = port.read(4096)
            if data:
                log.write(f"{time.time() - start:7.3f} {data!r}\n")
                log.flush()
                chunks.append(data)

    text = b"".join(chunks).decode("utf-8", "replace")
    hello = text.count("Hello")
    print(f"--- uart ({len(text)} bytes, hello={hello}) ---")
    seen = set()
    for line in text.splitlines():
        if line and line not in seen:
            seen.add(line)
            print(line)

    ok, message = evaluate(
        text, require_sd=args.require_sd, require_handoff=args.require_handoff
    )
    if not ok:
        print(f"FAIL: {message}", file=sys.stderr)
        return 1
    print("PASS: UART assertions")
    print("Human check: does the LED on PD22 blink?")
    return 0


if __name__ == "__main__":
    sys.exit(main())
