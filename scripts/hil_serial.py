#!/usr/bin/env python3
# SPDX-License-Identifier: Unlicense
"""Capture USART0, assert the rawboot boot log, and print a compact summary."""

import argparse
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("log")
    parser.add_argument("--seconds", type=float, default=4.0)
    parser.add_argument("--drain", type=float, default=1.0)
    parser.add_argument("--dev", default="/dev/ttyUSB0")
    parser.add_argument("--baud", type=int, default=115200)
    args = parser.parse_args()

    try:
        port = serial.Serial(args.dev, args.baud, timeout=0.05)
    except (serial.SerialException, OSError) as err:
        print(f"error: cannot open {args.dev}: {err}", file=sys.stderr)
        print("see docs/agents/hil.md (dialout / udev rule)", file=sys.stderr)
        return 2

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
    if hello > 1:
        print(
            "FAIL: multiple Hello lines; the board is reset-looping or the capture is "
            "stale - power-cycle into FEL (do not xfel reset) and rerun",
            file=sys.stderr,
        )
        return 1

    required = ["Hello", "CLK:", "DRAM simple test OK", "S=0x"]
    missing = [item for item in required if item not in text]
    if missing:
        print("FAIL: missing " + ", ".join(missing), file=sys.stderr)
        return 1
    print("PASS: UART assertions")
    print("Human check: does the LED on PD22 blink?")
    return 0


if __name__ == "__main__":
    sys.exit(main())
