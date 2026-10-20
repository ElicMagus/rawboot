#!/usr/bin/env python3
# SPDX-License-Identifier: Unlicense
"""Capture USART0, assert the rawboot boot log, and print a compact summary."""

import argparse
import pathlib
import re
import struct
import sys
import time
from dataclasses import dataclass

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
HANDOFF_LINE = re.compile(
    r"BOOT: handoff to (0x[0-9a-fA-F]+): (\d+) bytes at (0x[0-9a-fA-F]+), "
    r"checksum (0x[0-9a-fA-F]+),"
)
SD_FOUND = "SD/MMC card found"
SD_READ = re.compile(r"^SD: read [1-9][0-9]* block\(s\)", re.MULTILINE)
FEL_FOUND = "FEL: payload"
FEL_LINE = re.compile(
    r"FEL: payload image (0x[0-9a-fA-F]+), (\d+) bytes, args (0x[0-9a-fA-F]+)"
)
BOOT_CFG_LINE = re.compile(
    r"^ARGS: boot\.cfg (\d+) bytes at (0x[0-9a-fA-F]+)", re.MULTILINE
)
HANDOFF_ARGS = re.compile(
    r"BOOT: handoff to .*, args (0x[0-9a-fA-F]+)"
)
RESET_LOOP_HINT = (
    "power-cycle into FEL (do not xfel reset) and rerun"
)

# These mirror `rawboot-image/src/payload.rs`; update both together.
IMAGE_MAGIC = b"RBIF"
IMAGE_VERSION = 1
IMAGE_HEADER_SIZE = 28
MAX_IMAGE_SIZE = 128 * 1024 * 1024


@dataclass(frozen=True)
class PayloadExpectation:
    """The handoff fields a payload image declares in its header."""

    entry: int
    load_addr: int
    size: int
    checksum: int


def image_checksum(data: bytes) -> int:
    """The payload checksum: wrapping sum of little-endian 32-bit words."""
    padded = data + b"\0" * (-len(data) % 4)
    return sum(struct.unpack(f"<{len(padded) // 4}I", padded)) & 0xFFFF_FFFF


def payload_expectation(path: str) -> PayloadExpectation:
    """Read the payload image header and verify it against the image body."""
    image = pathlib.Path(path).read_bytes()
    if len(image) < IMAGE_HEADER_SIZE:
        raise ValueError(f"{path}: shorter than the {IMAGE_HEADER_SIZE}-byte payload header")
    if image[:4] != IMAGE_MAGIC:
        raise ValueError(f"{path}: bad payload magic")
    version, _flags, entry, load_addr, size, checksum = struct.unpack_from(
        "<6I", image, 4
    )
    if version != IMAGE_VERSION:
        raise ValueError(f"{path}: unsupported payload image version {version}")
    if size > MAX_IMAGE_SIZE:
        raise ValueError(f"{path}: payload size {size} exceeds the limit")
    if len(image) < IMAGE_HEADER_SIZE + size:
        raise ValueError(f"{path}: payload body truncated")
    actual = image_checksum(image[IMAGE_HEADER_SIZE : IMAGE_HEADER_SIZE + size])
    if actual != checksum:
        raise ValueError(
            f"{path}: payload checksum 0x{actual:08x} does not match header 0x{checksum:08x}"
        )
    return PayloadExpectation(entry, load_addr, size, checksum)


def evaluate(
    text: str,
    require_sd: bool = False,
    require_handoff: bool = False,
    require_fel: bool = False,
    expected_payload: PayloadExpectation | None = None,
    expected_fel_args: int | None = None,
    expected_boot_cfg: int | None = None,
    allow_busy: bool = False,
) -> tuple[bool, str]:
    """Judge one captured boot log.

    Returns (ok, message); message is empty when ok, otherwise the failure reason.
    A single clean boot passes; a reset loop (more than one ``Hello``) fails even
    when the required lines are present. The loader ends either in its LED loop
    (``S=0x``) or in a payload handoff (``BOOT: handoff to``); one of the two is
    required. With ``require_sd`` the log must also show a detected card and a
    non-zero block read; with ``require_fel`` it must show that the payload came
    from the FEL mailbox; with ``require_handoff`` the loader loop is not enough.
    ``expected_payload`` additionally requires the handoff to name the same
    entry, load address, size and checksum as that image. ``expected_fel_args``
    checks the argument pointer printed in the ``FEL: payload`` line.
    ``expected_boot_cfg`` checks the length and address in the SD ``ARGS:``
    line, and that the handoff reports the same ``args`` pointer.
    ``allow_busy`` drops the reset-loop check: the capture was taken while a
    loader from a previous run was being replaced (``HIL_SKIP_PREFLIGHT=1``), so
    its stale ``Hello`` lines and fragments must be ignored. Every other
    assertion still applies.
    """
    if not allow_busy and text.count("Hello") > 1:
        return False, f"multiple Hello lines; the board is reset-looping or the capture is stale - {RESET_LOOP_HINT}"

    if expected_payload is not None:
        require_handoff = True
    if expected_fel_args is not None:
        require_fel = True
    if expected_boot_cfg is not None:
        require_handoff = True

    required = list(REQUIRED)
    if require_sd:
        required.append(SD_FOUND)
    if require_fel:
        required.append(FEL_FOUND)
    missing = [item for item in required if item not in text]
    if missing:
        return False, "missing " + ", ".join(missing)

    if require_handoff and HANDOFF not in text:
        return False, "missing a payload handoff (BOOT: handoff to ...)"

    if not require_handoff and HANDOFF not in text and LOADER_RUNNING not in text:
        return False, "missing S=0x or BOOT: handoff (loader loop or payload handoff)"

    if require_sd and not SD_READ.search(text):
        return False, "missing a successful SD read (SD: read N block(s), N > 0)"

    if expected_fel_args is not None:
        match = FEL_LINE.search(text)
        if match is None:
            return False, "cannot parse the FEL payload line (image address, size, args)"
        actual = int(match.group(3), 16)
        if actual != expected_fel_args:
            return False, (
                f"FEL args mismatch: loader reported 0x{actual:08x}, "
                f"expected 0x{expected_fel_args:08x}"
            )

    if expected_payload is not None:
        match = HANDOFF_LINE.search(text)
        if match is None:
            return False, "cannot parse the payload handoff line (entry, size, load address, checksum)"
        actual = PayloadExpectation(
            entry=int(match.group(1), 16),
            size=int(match.group(2)),
            load_addr=int(match.group(3), 16),
            checksum=int(match.group(4), 16),
        )
        if actual != expected_payload:
            return False, (
                "payload mismatch: loader reported "
                f"entry 0x{actual.entry:08x}, {actual.size} bytes at "
                f"0x{actual.load_addr:08x}, checksum 0x{actual.checksum:08x}; "
                "host image has "
                f"entry 0x{expected_payload.entry:08x}, {expected_payload.size} bytes at "
                f"0x{expected_payload.load_addr:08x}, checksum 0x{expected_payload.checksum:08x}"
            )

    if expected_boot_cfg is not None:
        match = BOOT_CFG_LINE.search(text)
        if match is None:
            return False, "cannot parse the boot.cfg line (length, address)"
        length = int(match.group(1))
        address = int(match.group(2), 16)
        if length != expected_boot_cfg:
            return False, (
                f"boot.cfg length mismatch: loader reported {length} bytes, "
                f"expected {expected_boot_cfg}"
            )
        args = HANDOFF_ARGS.search(text)
        if args is None:
            return False, "cannot parse the handoff args pointer"
        actual = int(args.group(1), 16)
        if actual != address:
            return False, (
                f"handoff args mismatch: loader reported 0x{actual:08x}, "
                f"boot.cfg is at 0x{address:08x}"
            )

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
        help="warn instead of failing when preflight sees UART traffic; during "
        "capture, ignore stale Hello lines from the replaced loader instead of "
        "reporting a reset loop",
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
    parser.add_argument(
        "--require-fel",
        action="store_true",
        help="also require the payload to arrive through the FEL mailbox",
    )
    parser.add_argument(
        "--fel-args",
        metavar="ADDR",
        type=lambda value: int(value, 0),
        help="require the FEL argument pointer to equal ADDR (implies --require-fel)",
    )
    parser.add_argument(
        "--payload",
        metavar="PATH",
        help="payload image whose header the handoff line must match (implies --require-handoff)",
    )
    parser.add_argument(
        "--boot-cfg",
        metavar="BYTES",
        type=int,
        help="require boot.cfg to be BYTES long and the handoff to point at it "
        "(implies --require-handoff)",
    )
    parser.add_argument("--dev", default="/dev/ttyUSB0")
    parser.add_argument("--baud", type=int, default=115200)
    args = parser.parse_args()
    if args.log is None and not args.preflight:
        parser.error("log is required unless --preflight is given")

    expected_payload = None
    if args.payload:
        try:
            expected_payload = payload_expectation(args.payload)
        except (OSError, ValueError) as err:
            print(f"error: {err}", file=sys.stderr)
            return 2

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
        text,
        require_sd=args.require_sd,
        require_handoff=args.require_handoff,
        require_fel=args.require_fel,
        expected_payload=expected_payload,
        expected_fel_args=args.fel_args,
        expected_boot_cfg=args.boot_cfg,
        allow_busy=args.allow_busy,
    )
    if not ok:
        print(f"FAIL: {message}", file=sys.stderr)
        return 1
    print("PASS: UART assertions")
    if expected_payload is not None:
        print(
            "PASS: payload matches "
            f"{args.payload} (entry 0x{expected_payload.entry:08x}, "
            f"{expected_payload.size} bytes at 0x{expected_payload.load_addr:08x}, "
            f"checksum 0x{expected_payload.checksum:08x})"
        )
    if args.boot_cfg is not None:
        print(
            f"PASS: boot.cfg ({args.boot_cfg} bytes) is handed off as BootInfo.args"
        )
    if HANDOFF in text:
        print(
            "Human check: does PD22 show the payload's short-long blink "
            "(not the loader's steady 50 ms toggle)?"
        )
    else:
        print("Human check: does the LED on PD22 blink?")
    return 0


if __name__ == "__main__":
    sys.exit(main())
