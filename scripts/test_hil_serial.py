#!/usr/bin/env python3
# SPDX-License-Identifier: Unlicense
"""Host tests for the HIL log parser in hil_serial.py."""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import hil_serial  # noqa: E402

CLEAN_BOOT = """Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
DRAM simple test OK.
SD/MMC card not found
S=0x10000
"""


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


if __name__ == "__main__":
    unittest.main()
