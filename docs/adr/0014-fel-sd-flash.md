# FEL flasher: one command updates the card in the board

After #41 the bus had a returning DDR-init stub and a DRAM flasher with a job/status contract, but
it did not touch SD yet. The card was updated from the host: `cargo xtask image`, `dd` of the
prefix, copying `payload.bin` into FAT by hand. To update the card in the device it had to be
removed. A user-facing path is needed that writes `payload.bin` (and, if given, `boot.cfg`) into
the existing FAT root and the SD loader into sector 16 over FEL, preserving the MBR and the
partition, and then re-reads what was written and names the failing step.

Decision: `cargo xtask fel-flash` builds or reuses the images, checks them and calls
`FelDevice::flash_sd_card`. The host lays out a plan and three areas in DRAM, the flasher executes
the `FLASH_COMMAND_FLASH_SD` command, writes the files through the shared FAT adapter and the
loader directly into sector 16, and then compares what was read back with the original. The
job/status contract from ADR-0013 does not change the layout: the new command uses the same
`FlashJob`/`FlashStatus`, and the plan is moved into a separate block.

## Plan and areas in DRAM

| What | Address | Size |
| --- | --- | --- |
| `FlashPlan` (`RBFP`) | `0x40000200` | 40 bytes |
| `payload.bin` | `0x40100000` | up to 8 MiB |
| `boot.cfg` | `0x40900000` | up to 4095 bytes |
| sealed SD loader | `0x40901000` | up to 64 KiB |
| end of window | `0x40911000` | — |

`FlashPlan` stores the length and checksum of each of the three areas. The host builds the plan
before contacting the device, so an oversized payload, `boot.cfg` or loader is rejected locally.
The flasher checks the plan's magic/version/size/checksum, the lengths and checksums of the areas,
and `verify_sealed` for the loader. This protects against a corrupted FEL transfer between `write`
and `exec`.

New status steps (`plan`, `SD init`, `layout`, `read MBR`, `write payload`, `write boot.cfg`,
`verify FAT files`, `write loader`, `verify loader`, `raw write`, `raw verify`) and results
(`bad plan`, `SD card offline`,
`no MBR`, `no FAT partition`, `unsafe card layout`, `card too small`, `read failed`,
`write failed`, `create file failed`, `truncate failed`, `file write failed`, `file flush
failed`, `read-back mismatch`, `MBR changed`) let the host print the step and the cause of the
failure. A successful status is still `done`/`ok`.

## Write diagnostics

`FLASH_COMMAND_RAW_WRITE_TEST` and `cargo xtask raw-write-test` test the block write path itself
without requiring an MBR, FAT and staged images: the flasher writes a known block to LBA 16, reads
it back and compares. Sector 0 is not touched; sector 16 is overwritten, so on a prepared card the
loader must be written again after the test. The `raw write` and `raw verify` steps separate a
CMD24 failure from a read error, and `write failed`/`read failed`/`read-back mismatch` name the
cause.

Unlike `fel-flash`, the diagnostics do not read the MBR and do not perform a layout check: they
are intended for an empty or already verified card where sector 16 is not occupied by the
partition. Therefore the write requires an explicit `--yes`, prints a warning about overwriting
sector 16 and is not a safe way to update a prepared card.

## Order and safety

1. `cargo xtask fel-flash` (without `--skip-build`) invokes `image`, `payload` and `flasher`, then
   reads `target/rawboot-boot-sd.bin`, `target/rawboot-payload.img`, `target/rawboot-ddr.bin` and
   `target/rawboot-flasher.bin`. `--dry-run` performs all this checking and prints the plan without
   opening FEL; the actual write requires `--yes`.
2. The loader must be a sealed eGON. The FEL variant is rejected by file name, by `variant=fel` in
   the `.build` stamp and by matching bytes with `target/rawboot-fel.bin`. The payload must parse
   with `rawboot_image::parse`, and `boot.cfg` must be no longer than 4095 bytes.
3. `flash_sd_card` checks the plan, runs the DDR-init stub, checks DRAM with a pattern and only
   then writes the plan and the areas.
4. The flasher initializes SD, requires the MBR signature `55 AA` and a FAT partition in the table.
   The partition is found with the same `partition::candidates` as the loader; a bare FAT without an
   MBR is rejected.
5. `check_sd_layout` requires the partition start to be beyond sector 16, the end of the loader to
   be no later than the partition start and the end of the partition to be within the card.
6. First `payload.bin` and (if given) `boot.cfg` are written into the FAT root and read back. Only
   then is the sealed loader written into sector 16 and read back. Sector 0 is never written; at
   the end the flasher re-reads the MBR and reports `MBR changed` if it has changed.
7. Any failure before the loader is written leaves the old boot image; a failure after it reports
   the step, but the MBR and partition are not reformatted.

## Build and feature boundaries

The flasher needs `sunxi-drivers/write`, and the loader does not (ADR-0005). If both packages are
built in one `cargo build`, Cargo will unify the feature and pull writing into the loader, exceeding
the 64 KiB SRAM. Therefore `rawboot-flasher` was removed from `default-members`: `cargo build
--release` and `cargo clippy` check the loader by default, and `scripts/check.sh` lints the flasher
with a separate invocation. `cargo xtask flasher` builds both images in its own cargo invocation.

## Considered Options

- **A single FEL command with a plan in DRAM** (accepted): the card stays in the board, the data is
  not limited by SRAM, the plan is versioned and checked on both sides, and the plan, MBR/FAT
  parsing, layout and order of FEL transfers are tested on the host; the actual SD write remains
  with HIL.
- **Update the card from a card reader** (rejected): this is the current manual path, it requires
  removing the card and does not verify that what was written is exactly what was built.
- **Pass the area addresses in `arg0..arg2`** (rejected): three arguments do not cover the payload,
  `boot.cfg`, the loader and their checksums; fixed addresses with a plan are easier to check.
- **Write the loader first** (rejected): on a payload failure the card would be left with a new
  bootloader and an old payload. Payload first and loader last preserves the working old
  bootloader until the final step.
- **Leave `rawboot-flasher` in default-members** (rejected): feature unification enables
  `sunxi-drivers/write` in the loader and breaks linking against the SRAM budget.
- **Detect the FEL variant by name only** (rejected): a copy of `rawboot-fel.bin` under a different
  name would pass; the bytes and the build stamp are checked.

## Consequences

- `rawboot-image` publishes `FlashPlan`, the `FLASH_COMMAND_FLASH_SD` command, the area addresses,
  the steps/results and `check_sd_layout`; everything is checked by host tests.
- `rawboot-fel` gains `flash_sd_card` and the `LoaderNotSealed` error; host tests check the order
  of FEL transfers, rejection of an oversized `boot.cfg` and insufficient DRAM for the window.
- `rawboot-flasher` enables `sunxi-drivers/write`, writes FAT and sector 16, re-reads the files,
  the loader and the MBR. `rawboot-flasher` is no longer part of `default-members`;
  `scripts/check.sh` gained a separate clippy pass.
- `xtask` gains `fel-flash` with `--payload`, `--boot-cfg`, `--loader`, `--flasher`, `--ddr-stub`,
  `--sequence`, `--skip-build`, `--dry-run` and `--yes`; without `--yes` the command does not open
  FEL, and `--dry-run` checks the images and prints the plan. `scripts/check.sh` runs the dry-run
  after building the images.
- `rawboot-flasher` brings up USART0 and installs the driver `Console`, so SDHCI/FAT errors are
  visible in the HIL UART log; `scripts/hil-flash.sh` runs `fel-flash --yes`, captures UART and
  requires the status `done`/`ok`.
- `sunxi-drivers` gains `fat::file_matches` for a host-testable read-back; a separate
  `scripts/check-features.sh` checks before the build that `sunxi-drivers/write` did not get into
  `rawboot`.
- HIL: after a successful `fel-flash` a cold boot from the card must reach handoff, and PD22 must
  blink the payload. This is not claimed before the run.
- HIL #42 first hit the write blocker: on the bench SDSC card (`hc=0`, 25 MHz, CSD write-protect
  off) the card did not respond to CMD24, and `fel-flash` stopped at `write payload`
  (`truncate`/`file write`/`file flush failed`) with the MBR untouched. The defect was moved to #47
  and fixed (PR #49); the blocker concerned only the SD write path and does not explain the
  DDR-init behavior. The HIL #42 results are recorded in the tracker, not in this ADR.
- A repeated DDR-init stub within a single power-on is not idempotent: the stub runs
  `DramPara::init_dram()` again without checking the DRAM state and waits for the PLL_DDR lock, so a
  failure of the same class as #23 is possible. HIL #52 once ran three `bringup`s in a row, a repeat
  of `fel-flash` after a successful one and a repeat after an SD error — all returned to BROM. Then
  a repeated stub hung: after a successful `raw-write-test` the next `fel-flash` got a USB timeout,
  the board stayed on the bus, but `cargo fel version` did not respond; a cold start was needed.
  Therefore the rule is one stub per cold start, not a recommendation.
- Four cold starts with USB-UART connected and an invalid sector 16 (after `raw-write-test`) gave
  DDR-init 4/4 PASS; the card alternated strictly: odd starts `SD card offline`, even ones
  `done`/`ok`. This is the bench behavior of #43 (card power via UART backfeed), not a DDR failure.
  With a bootable card some starts send BROM into SD boot instead of FEL; the condition matrix is in
  the runbook.
