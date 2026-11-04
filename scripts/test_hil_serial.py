#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Host tests for the HIL log parser in hil_serial.py."""

import os
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import hil_serial  # noqa: E402

PAYLOAD_DATA = b"blink payload"
PAYLOAD_CHECKSUM = hil_serial.image_checksum(PAYLOAD_DATA)
PAYLOAD_ENTRY = 0x42000000
PAYLOAD_LOAD_ADDR = 0x42000000
HANDOFF_LINE_TEXT = (
    f"BOOT: handoff to 0x{PAYLOAD_ENTRY:08x}: {len(PAYLOAD_DATA)} bytes at "
    f"0x{PAYLOAD_LOAD_ADDR:08x}, checksum 0x{PAYLOAD_CHECKSUM:08x}, "
    "BootInfo 0x47feffe0, SP 0x48000000"
)

CLEAN_BOOT = """Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
DRAM simple test OK.
SD/MMC card not found
S=0x10000
"""

# Output left by a loader replaced through `HIL_SKIP_PREFLIGHT=1`: extra `Hello`
# lines and interleaved fragments ahead of one clean boot pass.
STALE_BOOT = "Hello\nHello\nCLK: CPU PLL=PLL_CPU FREQ=Hello\n" + CLEAN_BOOT

CARD_BOOT = CLEAN_BOOT.replace(
    "SD/MMC card not found",
    "SD/MMC card found\nSD: read 1 block(s), first bytes [00, 00, 00, 00]",
)

HANDOFF_BOOT = CLEAN_BOOT.replace(
    "SD/MMC card not found",
    "SD/MMC card found\nSD: read 1 block(s), first bytes [eb, 58, 90, 4d]",
).replace(
    "S=0x10000\n",
    "FAT: mounted Fat32 at LBA 0 (987758592 bytes)\n" + HANDOFF_LINE_TEXT + "\n",
)

FEL_BOOT = CLEAN_BOOT.replace(
    "SD/MMC card not found",
    "FEL: payload image 0x00039000, 236 bytes, args 0x00000000",
).replace(
    "S=0x10000\n",
    HANDOFF_LINE_TEXT + "\n",
)

FEL_ARGS_BOOT = FEL_BOOT.replace("args 0x00000000", "args 0x00038000")

BOOT_CFG_LEN = 11
BOOT_CFG_ADDR = 0x47FFEFD0
BOOT_CFG_BOOT = (
    HANDOFF_BOOT.replace(
        "FAT: mounted Fat32 at LBA 0 (987758592 bytes)\n",
        "FAT: mounted Fat32 at LBA 0 (987758592 bytes)\n"
        f"ARGS: boot.cfg {BOOT_CFG_LEN} bytes at 0x{BOOT_CFG_ADDR:08x}\n",
    )
    .replace(
        "BootInfo 0x47feffe0, SP 0x48000000",
        f"BootInfo 0x47feffe0, SP 0x48000000, args 0x{BOOT_CFG_ADDR:08x}",
    )
)


def payload_image(data: bytes = PAYLOAD_DATA) -> bytes:
    header = struct.pack(
        "<4s6I",
        b"RBIF",
        1,
        0,
        PAYLOAD_ENTRY,
        PAYLOAD_LOAD_ADDR,
        len(data),
        hil_serial.image_checksum(data),
    )
    return header + data


def expected_payload() -> hil_serial.PayloadExpectation:
    return hil_serial.PayloadExpectation(
        entry=PAYLOAD_ENTRY,
        load_addr=PAYLOAD_LOAD_ADDR,
        size=len(PAYLOAD_DATA),
        checksum=PAYLOAD_CHECKSUM,
    )


class TestEvaluate(unittest.TestCase):
    def test_clean_boot_passes(self):
        ok, message = hil_serial.evaluate(CLEAN_BOOT)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_reset_loop_fails(self):
        ok, message = hil_serial.evaluate(CLEAN_BOOT * 2)
        self.assertFalse(ok)
        self.assertIn("Hello", message)

    def test_missing_required_line_fails(self):
        text = CLEAN_BOOT.replace("DRAM simple test OK.", "")
        ok, message = hil_serial.evaluate(text)
        self.assertFalse(ok)
        self.assertIn("DRAM simple test OK", message)

    def test_empty_capture_fails(self):
        ok, message = hil_serial.evaluate("")
        self.assertFalse(ok)
        self.assertIn("missing", message)

    def test_single_hello_with_noise_passes(self):
        ok, _ = hil_serial.evaluate("noise\n" + CLEAN_BOOT)
        self.assertTrue(ok)

    def test_stale_hello_output_fails_as_a_reset_loop(self):
        ok, message = hil_serial.evaluate(STALE_BOOT)
        self.assertFalse(ok)
        self.assertIn("Hello", message)

    def test_allow_busy_ignores_stale_hello_lines(self):
        ok, message = hil_serial.evaluate(STALE_BOOT, allow_busy=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_allow_busy_still_requires_the_boot_lines(self):
        text = STALE_BOOT.replace("DRAM simple test OK.", "")
        ok, message = hil_serial.evaluate(text, allow_busy=True)
        self.assertFalse(ok)
        self.assertIn("DRAM simple test OK", message)

    def test_allow_busy_still_requires_the_loader_loop_or_handoff(self):
        text = STALE_BOOT.replace("S=0x10000\n", "")
        ok, message = hil_serial.evaluate(text, allow_busy=True)
        self.assertFalse(ok)
        self.assertIn("S=0x or BOOT: handoff", message)

    def test_allow_busy_still_requires_a_handoff_when_asked(self):
        ok, message = hil_serial.evaluate(STALE_BOOT, require_handoff=True, allow_busy=True)
        self.assertFalse(ok)
        self.assertIn("BOOT: handoff", message)

    def test_allow_busy_still_checks_sd_assertions(self):
        text = STALE_BOOT.replace(
            "SD/MMC card not found",
            "SD/MMC card found\nSD: read 1 block(s), first bytes [00, 00, 00, 00]",
        )
        ok, message = hil_serial.evaluate(text, require_sd=True, allow_busy=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_require_sd_rejects_no_card(self):
        ok, message = hil_serial.evaluate(CLEAN_BOOT, require_sd=True)
        self.assertFalse(ok)
        self.assertIn("SD/MMC card found", message)

    def test_require_sd_accepts_detected_card_and_read(self):
        ok, message = hil_serial.evaluate(CARD_BOOT, require_sd=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_require_sd_rejects_zero_block_read(self):
        text = CARD_BOOT.replace("read 1 block", "read 0 block")
        ok, message = hil_serial.evaluate(text, require_sd=True)
        self.assertFalse(ok)
        self.assertIn("SD: read N block(s)", message)

    def test_handoff_passes_without_the_loader_loop(self):
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, require_sd=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_require_fel_rejects_the_sd_path(self):
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, require_fel=True)
        self.assertFalse(ok)
        self.assertIn("FEL: payload", message)

    def test_require_fel_accepts_the_fel_path(self):
        ok, message = hil_serial.evaluate(FEL_BOOT, require_fel=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_fel_payload_is_checked_against_the_host_image(self):
        ok, message = hil_serial.evaluate(
            FEL_BOOT, require_fel=True, expected_payload=expected_payload()
        )
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_fel_args_pointer_can_be_checked(self):
        ok, message = hil_serial.evaluate(FEL_ARGS_BOOT, expected_fel_args=0x00038000)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_fel_args_mismatch_fails(self):
        ok, message = hil_serial.evaluate(FEL_ARGS_BOOT, expected_fel_args=0x00038001)
        self.assertFalse(ok)
        self.assertIn("FEL args mismatch", message)

    def test_expected_fel_args_requires_the_fel_line(self):
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, expected_fel_args=0x00038000)
        self.assertFalse(ok)
        self.assertIn("FEL: payload", message)

    def test_boot_cfg_length_and_handoff_pointer_can_be_checked(self):
        ok, message = hil_serial.evaluate(
            BOOT_CFG_BOOT, require_sd=True, expected_boot_cfg=BOOT_CFG_LEN
        )
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_boot_cfg_length_mismatch_fails(self):
        ok, message = hil_serial.evaluate(
            BOOT_CFG_BOOT, expected_boot_cfg=BOOT_CFG_LEN + 1
        )
        self.assertFalse(ok)
        self.assertIn("boot.cfg length mismatch", message)

    def test_boot_cfg_pointer_mismatch_fails(self):
        text = BOOT_CFG_BOOT.replace(
            f"args 0x{BOOT_CFG_ADDR:08x}", "args 0x47ffefd1"
        )
        ok, message = hil_serial.evaluate(text, expected_boot_cfg=BOOT_CFG_LEN)
        self.assertFalse(ok)
        self.assertIn("handoff args mismatch", message)

    def test_expected_boot_cfg_requires_the_args_line(self):
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, expected_boot_cfg=BOOT_CFG_LEN)
        self.assertFalse(ok)
        self.assertIn("cannot parse the boot.cfg line", message)

    def test_expected_boot_cfg_requires_a_handoff(self):
        text = BOOT_CFG_BOOT.replace(HANDOFF_LINE_TEXT, "")
        ok, message = hil_serial.evaluate(text, expected_boot_cfg=BOOT_CFG_LEN)
        self.assertFalse(ok)
        self.assertIn("BOOT: handoff", message)

    def test_loader_loop_or_handoff_is_required(self):
        text = CLEAN_BOOT.replace("S=0x10000\n", "")
        ok, message = hil_serial.evaluate(text)
        self.assertFalse(ok)
        self.assertIn("S=0x or BOOT: handoff", message)

    def test_require_handoff_rejects_the_loader_loop(self):
        ok, message = hil_serial.evaluate(CARD_BOOT, require_sd=True, require_handoff=True)
        self.assertFalse(ok)
        self.assertIn("BOOT: handoff", message)

    def test_require_handoff_accepts_a_handoff(self):
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, require_sd=True, require_handoff=True)
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_expected_payload_accepts_a_matching_handoff(self):
        ok, message = hil_serial.evaluate(
            HANDOFF_BOOT, require_sd=True, expected_payload=expected_payload()
        )
        self.assertTrue(ok)
        self.assertEqual(message, "")

    def test_expected_payload_rejects_a_different_checksum(self):
        expected = hil_serial.PayloadExpectation(
            entry=PAYLOAD_ENTRY,
            load_addr=PAYLOAD_LOAD_ADDR,
            size=len(PAYLOAD_DATA),
            checksum=PAYLOAD_CHECKSUM ^ 1,
        )
        ok, message = hil_serial.evaluate(HANDOFF_BOOT, expected_payload=expected)
        self.assertFalse(ok)
        self.assertIn("payload mismatch", message)
        self.assertIn("checksum", message)

    def test_expected_payload_requires_a_handoff(self):
        ok, message = hil_serial.evaluate(CARD_BOOT, expected_payload=expected_payload())
        self.assertFalse(ok)
        self.assertIn("BOOT: handoff", message)

    def test_expected_payload_requires_a_parseable_handoff_line(self):
        text = HANDOFF_BOOT.replace(", checksum 0x", " checksum 0x")
        ok, message = hil_serial.evaluate(text, expected_payload=expected_payload())
        self.assertFalse(ok)
        self.assertIn("cannot parse", message)

    def test_boot_error_is_not_a_handoff(self):
        text = HANDOFF_BOOT.replace(
            HANDOFF_LINE_TEXT,
            "BOOT: invalid payload image: bad image magic",
        )
        ok, message = hil_serial.evaluate(text, require_handoff=True)
        self.assertFalse(ok)
        self.assertIn("BOOT: handoff", message)


class TestPayloadExpectation(unittest.TestCase):
    def write_image(self, image: bytes) -> str:
        with tempfile.NamedTemporaryFile(delete=False) as handle:
            handle.write(image)
            path = handle.name
        self.addCleanup(os.unlink, path)
        return path

    def test_reads_the_header_and_verifies_the_body(self):
        expected = hil_serial.payload_expectation(self.write_image(payload_image()))
        self.assertEqual(expected, expected_payload())

    def test_rejects_bad_magic(self):
        image = bytearray(payload_image())
        image[0] ^= 0xFF
        with self.assertRaisesRegex(ValueError, "magic"):
            hil_serial.payload_expectation(self.write_image(bytes(image)))

    def test_rejects_a_short_header(self):
        with self.assertRaisesRegex(ValueError, "shorter"):
            hil_serial.payload_expectation(self.write_image(b"RBIF"))

    def test_rejects_bad_version(self):
        image = bytearray(payload_image())
        struct.pack_into("<I", image, 4, 99)
        with self.assertRaisesRegex(ValueError, "version"):
            hil_serial.payload_expectation(self.write_image(bytes(image)))

    def test_rejects_a_truncated_body(self):
        image = payload_image()[:-1]
        with self.assertRaisesRegex(ValueError, "truncated"):
            hil_serial.payload_expectation(self.write_image(image))

    def test_rejects_an_oversized_payload(self):
        image = bytearray(payload_image())
        struct.pack_into("<I", image, 20, hil_serial.MAX_IMAGE_SIZE + 1)
        with self.assertRaisesRegex(ValueError, "exceeds"):
            hil_serial.payload_expectation(self.write_image(bytes(image)))

    def test_rejects_a_corrupted_body(self):
        image = bytearray(payload_image())
        image[-1] ^= 0xFF
        with self.assertRaisesRegex(ValueError, "checksum"):
            hil_serial.payload_expectation(self.write_image(bytes(image)))


class TestBusyMessage(unittest.TestCase):
    def test_quiet_uart_has_no_message(self):
        self.assertIsNone(hil_serial.busy_message(b""))

    def test_loader_traffic_means_the_board_is_busy(self):
        message = hil_serial.busy_message(b"S=0x10000\n")
        self.assertIn("not quiet", message)
        self.assertIn("loader is already running", message)
        self.assertIn("Power-cycle", message)

    def test_noise_is_reported_separately(self):
        message = hil_serial.busy_message(b"\xff\xf0\x00\x01")
        self.assertIn("noisy", message)
        self.assertIn("HIL_SKIP_PREFLIGHT=1", message)


if __name__ == "__main__":
    unittest.main()
