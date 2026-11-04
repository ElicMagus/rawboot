#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-4.0
"""Host-side tests for scripts/flash-sd.sh; never touches a block device."""

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCRIPT = REPO / "scripts" / "flash-sd.sh"

# Variables the script reads; a developer's shell must not leak into the tests.
OVERRIDES = {
    "SD_DEV",
    "SD_PART",
    "START_LBA",
    "PARTITION_SECTORS",
    "LOADER_IMG",
    "PAYLOAD_IMG",
    "BOOT_CFG",
    "PAYLOAD_NAME",
    "FORMAT_PARTITION",
    "COPY_PAYLOAD",
    "SKIP_BUILD",
    "ASSUME_YES",
    "DRY_RUN",
    "ALLOW_FILE",
}
BASE_ENV = {key: value for key, value in os.environ.items() if key not in OVERRIDES}


def run(*args, env=None):
    return subprocess.run(
        ["bash", str(SCRIPT), *args],
        cwd=REPO,
        env={**BASE_ENV, **(env or {})},
        capture_output=True,
        text=True,
        check=False,
    )


def write_tool(bin_dir, name, body):
    path = Path(bin_dir) / name
    path.write_text(textwrap.dedent(body), encoding="utf-8")
    path.chmod(0o755)


def with_fake_tools(bin_dir, lsblk_body):
    write_tool(
        bin_dir,
        "findmnt",
        """\
        #!/bin/sh
        echo /dev/sda1
        """,
    )
    write_tool(bin_dir, "lsblk", lsblk_body)


class FlashSdTest(unittest.TestCase):
    def setUp(self):
        fd, path = tempfile.mkstemp(prefix="rawboot-sd-test-", suffix=".img")
        os.ftruncate(fd, 2 * 1024 * 1024)
        os.close(fd)
        self.image = path

    def tearDown(self):
        os.unlink(self.image)

    def output(self, result):
        return result.stdout + result.stderr

    def test_help_lists_the_flags(self):
        result = run("--help")

        self.assertEqual(result.returncode, 0)
        self.assertIn("--partition-sectors", result.stdout)
        self.assertIn("--dry-run", result.stdout)

    def test_auto_detection_reports_when_no_candidate_is_found(self):
        with tempfile.TemporaryDirectory() as bin_dir:
            with_fake_tools(
                bin_dir,
                """\
                #!/bin/sh
                case "$*" in
                    *-P*) exit 0 ;;
                    *) echo sda ;;
                esac
                """,
            )
            result = run(env={"PATH": f"{bin_dir}:{os.environ['PATH']}"})

        self.assertEqual(result.returncode, 2)
        self.assertIn("no removable/USB/MMC disk", result.stderr)
        self.assertIn("pass --device", result.stderr)

    def test_multiple_candidates_need_an_explicit_device_when_not_a_tty(self):
        with tempfile.TemporaryDirectory() as bin_dir:
            with_fake_tools(
                bin_dir,
                """\
                #!/bin/sh
                case "$*" in
                    *-P*)
                        printf '%s\\n' \\
                            'NAME="sdb" RM="1" TRAN="usb" TYPE="disk" SIZE="8G"' \\
                            'NAME="sdc" RM="1" TRAN="usb" TYPE="disk" SIZE="16G"'
                        ;;
                    *MOUNTPOINT*) exit 0 ;;
                    *) echo sda ;;
                esac
                """,
            )
            result = run(env={"PATH": f"{bin_dir}:{os.environ['PATH']}"})

        self.assertEqual(result.returncode, 2)
        self.assertIn("candidate disks", result.stderr)
        self.assertIn("multiple candidates; pass --device", result.stderr)

    def test_refuses_a_regular_file_without_allow_file(self):
        result = run("--device", self.image, "--dry-run")

        self.assertEqual(result.returncode, 2)
        self.assertIn("not a block device", result.stderr)

    def test_dry_run_plans_the_dd_and_writes_nothing(self):
        before = Path(self.image).read_bytes()
        result = run(
            "--device",
            self.image,
            "--allow-file",
            "--start-lba",
            "2048",
            "--partition-sectors",
            "2048",
            "--dry-run",
        )

        self.assertEqual(result.returncode, 0, self.output(result))
        self.assertIn("dd if=target/rawboot-sd-mbr.img", self.output(result))
        self.assertIn("nothing was built or written", result.stdout)
        self.assertEqual(Path(self.image).read_bytes(), before)

    def test_rejects_the_fel_loader(self):
        result = run(
            "--device",
            self.image,
            "--allow-file",
            "--dry-run",
            "--loader",
            "target/rawboot-fel.bin",
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("rawboot-boot-sd.bin", result.stderr)

    def test_rejects_a_non_boolean_environment_value(self):
        result = run(
            "--device",
            self.image,
            "--allow-file",
            "--dry-run",
            env={"COPY_PAYLOAD": "no"},
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("COPY_PAYLOAD must be 0 or 1", result.stderr)

    def test_rejects_a_path_as_payload_name(self):
        result = run(
            "--device",
            self.image,
            "--allow-file",
            "--dry-run",
            "--payload-name",
            "../payload.bin",
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("plain file name", result.stderr)

    def test_reads_the_partition_size_from_a_file(self):
        result = run("--device", self.image, "--allow-file", "--dry-run")

        self.assertEqual(result.returncode, 0, self.output(result))
        self.assertIn("4096 sectors", result.stdout)

    def test_refuses_format_on_a_file(self):
        result = run("--device", self.image, "--allow-file", "--format")

        self.assertEqual(result.returncode, 2)
        self.assertIn("need a real block device", result.stderr)

    def test_rejects_a_zero_start_lba(self):
        result = run("--device", self.image, "--allow-file", "--dry-run", "--start-lba", "0")

        self.assertEqual(result.returncode, 2)
        self.assertIn("start LBA", result.stderr)

    def test_rejects_zero_partition_sectors(self):
        result = run(
            "--device",
            self.image,
            "--allow-file",
            "--dry-run",
            "--partition-sectors",
            "0",
        )

        self.assertEqual(result.returncode, 2)
        self.assertIn("partition size", result.stderr)

    def test_rejects_an_unknown_argument(self):
        result = run("--device", self.image, "--allow-file", "--dry-run", "--wat")

        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown argument", result.stderr)

    def test_environment_variables_are_honoured(self):
        result = run(
            env={
                "SD_DEV": self.image,
                "ALLOW_FILE": "1",
                "DRY_RUN": "1",
                "START_LBA": "2048",
                "PARTITION_SECTORS": "2048",
            }
        )

        self.assertEqual(result.returncode, 0, self.output(result))
        self.assertIn("dd if=target/rawboot-sd-mbr.img", self.output(result))


if __name__ == "__main__":
    unittest.main()
