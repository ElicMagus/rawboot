# Hardware-in-the-loop (HIL) verification

The loader is verified on a real T113-S3. This page is the runbook: host setup, the load
sequence, the UART capture, and how to read the result. The script from this repo is
`scripts/hil-check.sh`; it automates the steps below.

## Agent-driven runs

When an agent runs these steps, it asks one `question` per physical action (power cycle,
card insertion, FEL/BOOT, LED confirmation) and waits for the answer before starting the
next action. It does not batch several cold starts into one prompt. In one `question` call
it presents two tabs: the physical step and a confirmation "Sure?" with "Yes" and "No".
It acts only when the step is confirmed and "Yes" is selected; otherwise it returns to the
step question. This guards against an accidental selection. After each run it reports
PASS/FAIL, the key log lines and the next step.

## Board and wiring

- SoC: Allwinner T113-S3, DDR3, 128 MiB.
- Debug UART: **USART0**, pins **PE2/PE3**, **115200 8N1**. On the host it is the CH340
  (`/dev/ttyUSB0`, override with `UART_DEV`).
- LED: **PD22**. The loader toggles it every 50 ms in its main loop; after a handoff the payload
  blinks a short-long cadence instead. The script cannot see the LED, so the run ends with a human
  check.
- FEL: USB OTG in FEL mode. The board enumerates as `1f3a:efe8` (some adapters report
  `1f3c:*`). The BROM samples the FEL strap at **cold power-on** only: remove power completely,
  hold FEL/BOOT, then apply power. Holding FEL through a reset (or FEL+RESET) does not re-sample
  it, so an automated rig has to switch the power rail, not just `NRST`.

## Host setup

1. Serial access: add the user to `dialout` (`sudo usermod -aG dialout "$USER"`, then log in
   again). A shell that has not picked up the group can still run the capture through
   `sg dialout`.
2. FEL access: install a udev rule so `xfel` can open the device without root:
   ```sh
   echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="1f3a", ATTR{idProduct}=="efe8", MODE="0666"' \
     | sudo tee /etc/udev/rules.d/99-sunxi-fel.rules
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```
   Add a second rule for `idVendor=="1f3c"` if your board reports that ID.
3. Tools: `xfel` (v1.4.0 was used; it lives in `~/.local/bin/xfel` here) and Python
   `pyserial` (`sudo apt install python3-serial` or `pip install pyserial`).
4. Check: `xfel version` must print a `T113` line while the board is in FEL.

## Load and capture

`cargo xtask image` writes two loader images from the same code: `target/rawboot-fel.bin`,
linked for SRAM at `0x00028000`, and `target/rawboot-boot-sd.bin`, linked at `0x00020000` for
BROM SD/SPI boot from sector 16. Use the FEL image with `hil-check.sh`; write the boot image to
the card. The loader initialises DRAM itself, so `xfel ddr` is **not** part of either procedure.

Build the loader, then run the script:

```sh
cargo xtask image
scripts/hil-check.sh target/rawboot-fel.bin hil
```

For a ticket whose acceptance includes the card path, set `HIL_REQUIRE_SD=1`. The script
then also requires `SD/MMC card found` and a non-zero `SD: read N block(s)` line. Leave it
unset when no card is inserted. For a ticket that must prove the handoff, also set
`HIL_REQUIRE_HANDOFF=1`: the log must then contain a `BOOT: handoff to ...` line and the
loader's `S=0x` loop is no longer accepted. `HIL_REQUIRE_PAYLOAD=1` additionally compares the
handoff's entry, load address, size and checksum with `target/rawboot-payload.img`
(`HIL_PAYLOAD_IMG` overrides the path), so a stale `payload.bin` on the card fails instead of
passing. `HIL_REQUIRE_LED=1` makes the human LED confirmation part of the result: the script
prompts on a TTY, or accepts `HIL_LED_CONFIRMED=1` in non-interactive runs.

### Flaky SD init

Cold starts with a card inserted can fail with `SD/MMC card not found` (#43).
`scripts/hil-sd-retry.sh [image] [label]` repeats `hil-check.sh` up to
`HIL_SD_RETRIES` times (default 3) and retries only that failure; handoff,
payload and other assertion failures stop at once. It forces
`HIL_SKIP_PREFLIGHT=1` and `HIL_NO_PROMPT=1` because each retry replaces the
loader left running by the previous attempt. Set
`HIL_REQUIRE_LED=1 HIL_LED_CONFIRMED=1` to include the LED assertion, or run
`hil-check.sh` directly when a clean interactive run is required. If FEL stops
answering between attempts, power-cycle the board into FEL and rerun.

Retrying does not power-cycle the card. Before treating a `cmd1`/ACMD41 failure
as a loader defect, run a true cold start as described below.

#### Cold start must power-cycle the card (#43)

A "cold" start only exercises the driver if the card itself loses power. On this
bench the USB-UART adapter can backfeed the board's 3.3 V rail through the serial
lines. With the adapter left connected, the card keeps its state across an FEL
power-off; cold starts then alternate PASS/FAIL even on unmodified `master`. The
failure looks like a successful CMD8 followed by ACMD41 never setting the busy
bit, then the usual `cmd1 timeout` MMC fallback, and a loader re-exec without a
board power cycle does not recover it.

A true cold start:

1. Unplug the USB-UART adapter (and any other cable that can backfeed the
   board), then remove power from the board.
2. Wait a few seconds so the card rail discharges, insert the card, hold
   FEL/BOOT, apply power, and only then reconnect the USB-UART adapter.
3. Run `HIL_REQUIRE_SD=1 scripts/hil-check.sh target/rawboot-fel.bin cold<N>`.

Then repeat the two recovery paths: insert the card after the board is in FEL
and load the loader; and re-exec the loader over FEL without removing power
(`HIL_SKIP_PREFLIGHT=1`). Finally, remove the card and confirm
`SD/MMC card not found` without a hang or an unbounded retry loop.

Baseline `master @ 60e6867` passed 3/3 true cold starts this way, so a failure
with the adapter disconnected points at the loader or the card, not at the
bench. Hot-plug after FEL and a loader re-exec do not power-cycle the card; they
test recovery from a card that is already powered. Issue #43 was closed as a
bench-procedure issue; see `research/sd-cold-start-brom.md` for the evidence.

### Payload image on the card

The loader reads `payload.bin` from the FAT partition. Build the blink payload, then copy the
wrapped image to the mounted card. You can write the card on any machine with a card reader;
for HIL it only has to be in the board, not in this host:

```sh
cargo xtask payload
cp target/rawboot-payload.img /media/$USER/<partition>/payload.bin
```

Copy `target/rawboot-payload.img`, not the raw
`target/armv7a-none-eabi/release/payload.bin`: the former has the image header the loader
parses. If the card is already in the board and the board is in FEL, `cargo xtask fel-flash`
updates `payload.bin` and the loader without removing the card; see
[FEL flashing the card in place](#fel-flashing-the-card-in-place-42).

An optional `boot.cfg` next to `payload.bin` is copied into DRAM and exposed to the payload as
a NUL-terminated string through `BootInfo.args` (up to 4095 bytes; the 4 KiB window includes
the terminator). Without the file, `BootInfo.args` is zero. To assert the handoff end to end,
put a known file on the card and pass its byte length:

```sh
printf 'console=ttyS0' > /media/$USER/<partition>/boot.cfg   # 13 bytes
HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_BOOT_CFG_LEN=13 \
    scripts/hil-check.sh target/rawboot-fel.bin bootcfg
```

`HIL_BOOT_CFG_LEN` makes the script require an `ARGS: boot.cfg <len> bytes at 0x...` line and
the same pointer in the handoff's `args 0x...` field. A file that is unreadable or does not fit
aborts the boot with `BOOT: boot.cfg: ...` instead of booting with silently truncated arguments.

For the full end-to-end acceptance (SD -> FAT -> DRAM -> handoff -> blink), run with all
assertions:

```sh
HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_REQUIRE_PAYLOAD=1 HIL_REQUIRE_LED=1 \
    scripts/hil-check.sh target/rawboot-fel.bin blink
```

### FEL payload (no SD card)

`HIL_FEL_PAYLOAD=1` uploads `target/rawboot-payload.img` to SRAM and writes the FEL mailbox, so
the loader boots it without an SD card. The mailbox follows `rawboot-image/src/fel.rs`:

| word | address     | value                                      |
| ---- | ----------- | ------------------------------------------ |
| 0    | `0x00044000` | magic `0x4d464252` (`RBFM` little-endian) |
| 1    | `0x00044004` | payload image address (`0x00039000`)      |
| 2    | `0x00044008` | payload image size in bytes                |
| 3    | `0x0004400c` | argument string address, or zero           |

The image window is `0x00039000..0x00044000` (44 KiB); the optional arguments go at
`0x00038000`. This path deliberately avoids `xfel ddr` (issue #23) and skips SD initialisation.
Run it with the handoff and LED checks:

```sh
cargo xtask image
cargo xtask payload
HIL_FEL_PAYLOAD=1 HIL_REQUIRE_LED=1 scripts/hil-check.sh target/rawboot-fel.bin fel
```

`HIL_FEL_ARGS="..."` passes a NUL-terminated argument string through the mailbox at
`0x00038000` (at most 4 KiB, including the terminator); the run asserts the pointer the loader
reports. The payload assertion is automatic on this path, and `HIL_REQUIRE_SD` is rejected
because no SD access happens.

### SD/SPI boot (no FEL)

BROM loads a boot image from sector 16 straight to `0x00020000`, while the FEL image is linked
at `0x00028000` and is written by `xfel`. Put **`target/rawboot-boot-sd.bin`** on the card:
the FEL image in sector 16 runs 32 KiB off and dies before UART, leaving the board silent with
a steady LED.

The card needs a FAT partition (this runbook assumes LBA 2048, as in the #37 bring-up) and
`payload.bin` (or `boot.cfg`) in its root. `cargo xtask sd-image` builds the boot prefix: an MBR
pointing at that partition plus `target/rawboot-boot-sd.bin` at sector 16, exactly 2048 sectors
(1 MiB) by default. It does not format anything or copy the payload; both stay on the partition.

Build and write the prefix over a card that already has a FAT partition at LBA 2048:

```sh
cargo xtask image
cargo xtask payload
cargo xtask sd-image --partition-sectors 1843200
sudo dd if=target/rawboot-sd-mbr.img of=/dev/sdX bs=512 conv=notrunc,fsync
cp target/rawboot-payload.img /media/$USER/<partition>/payload.bin
```

`--partition-sectors` must be the partition's actual size in sectors (read it from
`fdisk -l /dev/sdX`); `--start-lba` defaults to 2048 and changes both the prefix length and the MBR
entry; `--loader` overrides the default `target/rawboot-boot-sd.bin`. The prefix carries no FAT
volume and no payload: formatting the whole device (`mkfs.vfat /dev/sdX`) or writing a full disk
image over it destroys the loader in sector 16. Format and copy only the partition
(`mkfs.vfat /dev/sdX1`, mount it, copy `payload.bin`), or re-run the `dd` of the prefix afterwards.

The wrapper `scripts/flash-sd.sh` runs the steps above, detects the partition start and size, and
refuses a mounted device, a GPT/multi-partition card, or one that holds `/`. Without `--device` it
picks the only removable/USB/MMC disk and asks when several are present; `--list` shows them. Run
it as your user, not via `sudo`: it escalates only `dd`, `mkfs` and `mount`, so cargo keeps
`target/` user-owned.

```sh
# see which cards/disks the script can pick
scripts/flash-sd.sh --list
# update the loader; keep the partition and its payload
scripts/flash-sd.sh --copy-payload
# recreate the partition as FAT32 (destroys its contents), then copy the payload
scripts/flash-sd.sh --device /dev/sdX --format --yes
```

`--dry-run` only prints the plan; `--skip-build` reuses the existing images; `--partition`,
`--start-lba`, `--partition-sectors`, `--loader`, `--payload`, `--boot-cfg` and `--payload-name`
override detection and the defaults. The same values can come from `SD_DEV`, `SD_PART`,
`START_LBA`, `PARTITION_SECTORS`, `LOADER_IMG`, `PAYLOAD_IMG`, `BOOT_CFG`, `FORMAT_PARTITION` and
`COPY_PAYLOAD`. The script asks before every destructive action unless `--yes` is passed (the
`--format --yes` example skips both prompts), refuses `rawboot-fel.bin` in sector 16, requires the
partition to already be FAT unless `--format` is given, and only formats the partition, never the
whole disk.

Capture a self-boot run by starting the capture **before** the power-cycle, then apply power
(the BROM strap is sampled at cold power-on; see [Capture a self-booting board](#capture-a-self-booting-board-no-fel)):

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

The run passes when the log reaches `BOOT: handoff to ...` and PD22 then shows the payload's
short-long cadence (120 ms on, 180 ms off, 600 ms on, 900 ms off). A steady 50 ms toggle means
the loader is still in its `S=0x` loop; silence points at the wrong image in sector 16.

Run HIL from the code branch under test. Verification instructions and runbook updates may be
committed before the run; record a PASS only after it succeeds. `cargo xtask image` writes
`target/rawboot-fel.bin.build` and `target/rawboot-boot-sd.bin.build` with the source commit,
branch and dirty flag; `hil-check.sh` refuses a FEL image whose commit is not `HEAD`, so rebuild
after switching branches (`HIL_SKIP_PROVENANCE=1` is the escape hatch).

Put the board into FEL with a cold power-cycle (power off, hold FEL/BOOT, power on), not with
`xfel reset` or a plain reset: the strap is sampled only at power-on, and `xfel reset` resets
through the watchdog, which stays armed, so the loader then boots in a loop. A loader left
running from a previous run also keeps printing on USART0, so clear it with a power-cycle
before each run. On a terminal the script prints this and waits for Enter; piped or
`HIL_NO_PROMPT=1` runs skip the prompt.

`xfel version` succeeding does **not** prove the board is idle in FEL: the FEL USB gadget can
still answer while a loader from a previous run prints on USART0. The script's UART-quiet
preflight is the authority.

If a loader from a previous run is still printing, `HIL_SKIP_PREFLIGHT=1` drains USART0 and
replaces it with the image under test. The preflight warning is expected then: the old `S=`
lines are stale output, not a reset loop. The capture drops the reset-loop check for the same
reason, so the stale `Hello` lines and fragments it catches while the old loader is replaced no
longer fail the run; every other assertion still applies, so a genuinely absent boot line is
still reported. Use this instead of repeated power-cycles when `xfel` still answers; use a
power-cycle when it does not.

The script:

1. checks the image build stamp against `HEAD`;
2. waits up to 3 minutes for a responsive FEL (`xfel version` succeeds);
3. requires the UART to be quiet for a short window, so a still-running loader is reported
   as a busy board instead of looking like a reset loop later (`HIL_SKIP_PREFLIGHT=1`
   downgrades this check to a warning);
4. drains the UART input buffer before loading;
5. writes the image at `0x00028000` and jumps to it; with `HIL_FEL_PAYLOAD=1` it also writes the
   payload image and mailbox to SRAM before the jump;
6. captures USART0 for a few seconds, prints the unique lines and asserts `Hello`, `CLK:`,
   `DRAM simple test OK`, and either the loader loop (`S=0x`) or a payload handoff
   (`BOOT: handoff to ...`); with `HIL_REQUIRE_SD=1` it also asserts `SD/MMC card found` and a
   non-zero `SD: read N block(s)`, with `HIL_FEL_PAYLOAD=1` it requires the `FEL: payload` line,
   with `HIL_REQUIRE_HANDOFF=1` it requires the handoff line, and with `HIL_REQUIRE_PAYLOAD=1`
   (or `HIL_FEL_PAYLOAD=1`) it requires the handoff to match the host payload image.
   It fails when more than one `Hello` appears (a reset loop or a stale capture); with
   `HIL_SKIP_PREFLIGHT=1` that check is dropped so output left by the replaced loader is
   ignored;
7. with `HIL_REQUIRE_LED=1`, asks the human to confirm PD22 shows the payload's short-long
   cadence (120 ms on, 180 ms off, 600 ms on, 900 ms off, repeating), not the loader's steady
   50 ms toggle; on an interactive TTY (unless `HIL_NO_PROMPT=1`) it reads the answer,
   otherwise it requires `HIL_LED_CONFIRMED=1`. Without the flag it only prints the
   human-check prompt.

The raw capture is kept at `/tmp/rawboot-hil-<label>.log`.

### Manual load (no script)

```sh
xfel write 0x00028000 target/rawboot-fel.bin
xfel exec 0x00028000
```

For a manual FEL payload boot, write the image and mailbox before the jump:

```sh
size=$(stat -c %s target/rawboot-payload.img)
xfel write 0x00028000 target/rawboot-fel.bin
xfel write 0x00039000 target/rawboot-payload.img
xfel write32 0x00044004 0x00039000
xfel write32 0x00044008 "$size"
xfel write32 0x0004400c 0
xfel write32 0x00044000 0x4d464252
xfel exec 0x00028000
```

Do not issue `xfel ddr t113-s3` first. It initialises DRAM and the loader's second
initialisation hangs on PLL_DDR (issue #23). The loader needs only SRAM until it has
initialised DRAM itself.

If the UART is captured by hand, drain it before `xfel exec` (for example with
`python3 -c 'import serial; s=serial.Serial("/dev/ttyUSB0",115200); s.reset_input_buffer()'`)
and capture at 115200 8N1.

### In-tree FEL transport (no `xfel`)

`cargo fel` speaks the BROM's FEL protocol directly; it needs the udev rule above but no
external `xfel`. This checks issue #40 end to end:

```sh
cargo xtask image
cargo fel version        # must print `SoC:        T113-S3 (chip id 0x1859)`
```

Write a pattern to SRAM and read it back (the read prints a hex dump; `--output` saves the
bytes for `cmp`):

```sh
size=$(stat -c %s target/rawboot-payload.img)
cargo fel write 0x00039000 target/rawboot-payload.img
cargo fel read 0x00039000 "$size" --output /tmp/rawboot-readback.img
cmp target/rawboot-payload.img /tmp/rawboot-readback.img
```

For exec, upload a stub that returns to the BROM (`bx lr`), run it, then read the memory
again:

```sh
printf '\x1e\xff\x2f\xe1' > /tmp/rawboot-return.bin   # bx lr
cargo fel write 0x00030000 /tmp/rawboot-return.bin
cargo fel exec 0x00030000
cargo fel read 0x00030000 4
```

`cargo fel exec` returns only when the stub gives control back to the BROM; a stub that does
not return ends in a transfer timeout. `scripts/hil-check.sh` still uses `xfel`; this CLI is
for the manual checks above.

### DDR-init stub and flasher (no SD)

Issue #41 splits the FEL path into a returning DDR-init stub and a DRAM-linked flasher; issue
#42 adds the SD/FAT job (see the next section). Build both images and run the whole spike with
the in-tree transport:

```sh
cargo xtask flasher
cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin
```

`bringup` identifies the SoC, writes the stub at `0x00028000`, executes it and reads its SRAM
result (`0x0002d000`: magic `RBDD`, then the DRAM size in MiB), writes and reads back a pattern
at `0x40002000`, then places a `NOOP` job at `0x40000000`, the flasher at `0x40010000`, executes
it and reads the status at `0x40000100`. The expected tail is:

```
DRAM:       128 MiB
pattern:    OK at 0x40002000
status:     sequence 1, step 2 (done), result 0 (ok)
```

The addresses and the `FlashJob`/`FlashStatus` layout live in `rawboot-image/src/flash.rs`
(ADR-0013); the CLI prints the same values. This path deliberately does **not** use `xfel ddr`:
the stub is what initialises DRAM.

- If the stub does not return, `bringup` blocks on the next FEL read until the transport timeout
  and the board no longer answers FEL. Record the failing step and return #41 to triage; do not
  retry `xfel ddr` as a workaround.
- `DDR-init stub did not report success` means `exec` returned but the SRAM result is missing or
  the driver returned size `0`; the board is still usable, re-run after checking the log.
- `DRAM read-back ... returned ...` names the first mismatching word. Do not proceed to the
  flasher; the DRAM window is not trustworthy.
- A status with `result` not `0 (ok)` is still a successful round trip, but the job failed; the
  CLI exits non-zero after printing the step and result.

The stub and flasher carry no eGON header and are not interchangeable with the loader images.

### FEL flashing the card in place (#42)

The card stays in the board. Put the board into FEL with a cold power-cycle, leave the card
inserted, and run:

```sh
cargo xtask fel-flash --yes
```

`--dry-run` performs the same build and validation, prints the plan and exits without opening
FEL or touching the card; use it to check artifacts and paths. Actual writing requires `--yes`.

Without `--skip-build` the command runs `cargo xtask image`, `payload` and `flasher` first, then
reads `target/rawboot-boot-sd.bin`, `target/rawboot-payload.img`, `target/rawboot-ddr.bin` and
`target/rawboot-flasher.bin`. It checks the loader is a sealed SD/SPI image and refuses the
FEL variant (`rawboot-fel.bin`, a `variant=fel` build stamp or matching bytes), parses the
payload image and limits `boot.cfg` to 4095 bytes. Then it identifies the T113-S3, runs the
DDR-init stub, checks DRAM with a pattern, stages a flash plan and the three regions in DRAM and
executes the flasher. The flasher initialises USART0 and prints SD/FAT driver diagnostics there,
so a failing CMD24 or FAT step is visible in the UART log as well as in the status.

Options:

- `--payload <path>` — payload image (default `target/rawboot-payload.img`); it must be the
  wrapped image, not the raw `payload.bin`.
- `--boot-cfg <path>` — also write this file as `boot.cfg`; without it the existing `boot.cfg`
  is left untouched.
- `--loader <path>`, `--flasher <path>`, `--ddr-stub <path>` — override the built images.
- `--sequence <n>` — job/status sequence (default 1).
- `--skip-build` — reuse the images already in `target/`; rebuild after switching branches.
- `--dry-run` — build/validate and print the plan; never open FEL.
- `--yes` — required for actual writing; without it the command validates and stops.

`scripts/hil-flash.sh` wraps the capture: it starts a UART logger, runs
`cargo xtask fel-flash --yes`, prints the host output and UART tail, and requires
`step 2 (done), result 0 (ok)`. It does not power-cycle; cold-start the board first.

What the flasher checks and writes:

1. the MBR has the `55 AA` signature and a FAT partition-table entry; a bare FAT volume without
   an MBR, an MBR without a FAT entry, and a partition starting at or before sector 16 are
   rejected;
2. the loader at sector 16 ends before the partition, and the partition lies inside the card;
3. `payload.bin` (and `boot.cfg` when supplied) is replaced in the FAT root and read back;
4. only then is the sealed loader written to sector 16 and read back;
5. sector 0 is never written; the flasher re-reads the MBR and reports `MBR changed` if it
   differs.

The expected tail on success:

```
SoC:        T113-S3
DRAM:       128 MiB
staged:     payload N bytes, boot.cfg 0 bytes, loader N bytes
status:     sequence 1, step 2 (done), result 0 (ok)
card:       payload files and loader written; MBR unchanged
```

**DDR-init and repeated attempts.** The stub is not idempotent: every run calls
`DramPara::init_dram()` again and waits for PLL_DDR to lock, so a repeat can hang the same way as
`xfel ddr` before a loader. That is a different condition from #23, which is specifically about
external `xfel ddr` run before the loader. HIL #52 measured the repeat conditions: three
`bringup` runs on one power cycle, a second `fel-flash` after a successful one, and a rerun after
an SD error all returned to the BROM and finished with `step 2 (done), result 0 (ok)`. A later
repeat hung, however: after a successful `raw-write-test` on one power cycle, the next
`fel-flash` timed out waiting for the DDR stub to return. The device still enumerated on USB,
but `cargo fel version` did not answer and the board needed a cold start. A repeat stub is
therefore not a verified path: run one stub per cold boot and power-cycle before retrying.

| Condition | Procedure | Expected | Observed |
| --- | --- | --- | --- |
| Cold start, one `bringup` | cold power-cycle into FEL, run `cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin` | `DRAM: 128 MiB`, `pattern: OK at 0x40002000`, `status: sequence 1, step 2 (done), result 0 (ok)` | PASS, HIL #52 |
| Three `bringup` on one power cycle | run `bringup` three times without removing power | three identical successes | PASS, HIL #46; re-confirmed HIL #52 |
| Repeat `fel-flash` after a successful `fel-flash` | run `cargo xtask fel-flash --yes` twice on one power cycle | the second run reports `step 2 (done), result 0 (ok)` | PASS once on HIL #52 (after an in-FEL card re-insert); a later repeat hung, so do not rely on it |
| Repeat `fel-flash` after an SD error | make the flasher fail before the status (remove the card, or use a partition starting at or inside sector 16), then rerun `fel-flash --yes` on the same power cycle | the second run reports a clean status | PASS once on HIL #52 (first run `SD card offline`, second run ok); a later repeat hung |
| Four cold starts, adapter attached, bootable card | power-cycle four times without unplugging the adapter or removing the card | DDR-init succeeds whenever FEL is reached; the BROM may boot the card instead of FEL | HIL #52: FEL reached 4/4; DDR 4/4 PASS; card `found` 1/4, `SD card offline` 3/4 |
| Four cold starts, adapter attached, sector 16 invalidated | run `cargo xtask raw-write-test --yes` once, then power-cycle four times without unplugging the adapter or removing the card | FEL is deterministic; DDR-init succeeds every time; SD may alternate (#43) | HIL #52: DDR 4/4 PASS; SD odd starts FAIL (2/2), even starts PASS (2/2) |

Record a result in the table only after the owner confirms it on the bench; do not mark an
unmeasured row PASS. Repeat the cold-start row after any board or card change.

**Cold starts with the adapter attached.** Leaving the USB-UART adapter connected backfeeds the
board, so the card is not truly power-cycled, and a card carrying a bootable loader in sector 16
can also make the BROM boot from SD instead of entering FEL. On HIL #52, with a bootable card,
only one of four such starts enumerated the card; a start that does not answer `cargo fel
version` has booted the card, not failed DDR-init. Check `cargo fel version` before starting a
matrix row.

To measure the card alternation without the BROM stealing the boot, invalidate sector 16 first:
`cargo xtask raw-write-test --yes` writes an `RBRW` test block there, leaves sector 0 and the FAT
partition untouched, and makes FEL deterministic. With that card HIL #52 saw four cold starts
with the adapter attached: DDR-init passed 4/4, while SD alternation was strict (odd starts `SD
card offline`, even starts `done`/`ok`). Restore the loader with `fel-flash` afterwards. This
alternation is #43 (card power state), not a DDR-init result. Unplug the adapter before the
power-off for a true cold start, and remove the card when FEL entry must be deterministic.

The flasher reports a failing step and reason in the status, and `fel-flash` prints it before
exiting non-zero. Common cases:

- `SD init: SD card offline` — no card, a card that did not enumerate, or the #43 bench issue;
  insert the card and cold-start the board into FEL.
- `read MBR: read failed` — the first card block could not be read; treat the card as offline
  and retry after a cold start.
- `layout: no MBR` / `layout: no FAT partition` — the card is not prepared; `fel-flash` does
  not format or create a partition. Prepare it first (out of scope), then rerun.
- `layout: unsafe card layout` — the FAT partition starts at or inside sector 16, or the loader
  would reach into it.
- `layout: card too small` — the MBR partition or the loader does not fit the card.
- `write payload` / `write boot.cfg` `create file failed`, `truncate failed`, `file write
  failed` or `file flush failed` — the card did not accept a FAT write; the MBR and partition
  are left in place, but do not trust the updated files until a rerun passes. On the SDSC bench
  card this was #47, fixed in PR #49; if it reappears, treat it as a new write-path defect, not
  as a DDR-init problem.
- `write loader` `write failed` — writing sector 16 failed; the old loader may still be there,
  but do not boot from the card until a rerun passes.
- `verify FAT files` / `verify loader` `read-back mismatch` — the card did not return what was
  written; rerun, and replace the card if it persists.
- `verify loader` `MBR changed` — stop and do not boot from this card; inspect it.
- `plan: bad plan or staged image` — the staged plan/loader/payload failed validation; rebuild
  with `cargo xtask image`/`payload`/`flasher` and rerun (without `--skip-build`).
- `USB device disconnected` or a FEL timeout before the status — the stub or flasher did not
  return to the BROM. Cold-power-cycle the board into FEL and rerun; do not fall back to
  `xfel ddr`. Prefer a fresh cold start over rerunning the stub on the same power cycle, which
  the table above leaves unmeasured.

After a successful run, remove the FEL strap and cold-boot from the card. Capture it with
`hil_serial.py` (start the capture before applying power, as in
[Capture a self-booting board](#capture-a-self-booting-board-no-fel)) and assert the handoff:

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

The card path passes when the log shows `SD/MMC card found`, `FAT: mounted` and
`BOOT: handoff to ...`, and PD22 then shows the payload's short-long cadence. A steady 50 ms
toggle means the loader is still in its `S=0x` loop; silence points at the wrong image in
sector 16. For the full HIL run with all assertions, use the cold-start procedure above with
`HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_REQUIRE_PAYLOAD=1 HIL_REQUIRE_LED=1`.

### Raw SD write diagnostic (no MBR/FAT)

When a card write fails but reads work, separate the block-write path from FAT and partition
state with a cold FEL start and:

```sh
cargo xtask raw-write-test --yes
```

It runs the DDR-init stub, writes a known 512-byte block to LBA 16, reads it back and prints
the step/result. It requires `--yes`, and it overwrites sector 16 (the SD loader on a prepared
card); sector 0 is never written. A successful run ends with:

```
status:     sequence 1, step 2 (done), result 0 (ok)
raw test:   sector 16 written and read back identical
```

A write that reports `step 12 (raw write), result 9 (write failed)` means the driver could not
complete CMD24; `step 13 (raw verify)` separates a read-back failure from a write failure.

### Capture a self-booting board (no FEL)

When the board boots on its own — from SD or SPI, with no `xfel` step — start the capture
**before** the power-cycle and let `hil_serial.py` judge it. It drains the port for one second
(`--drain`) and discards what it reads, so apply power only after that window. It then records
the log with timestamps and prints the unique lines plus the assertions:

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

`sg dialout` is only needed when the shell has not picked up the `dialout` group. The raw log
grows fast on a busy board (it can reach megabytes); read the `hil_serial.py` summary instead
of dumping the file.

## Expected log

The current `master` prints (with a card inserted, no valid `payload.bin`):

```
Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
trefi:7.8ms
DRAMC read ODT  off.
DRAM ODT value: 0x42.
DRAM simple test OK.
SD/MMC card found
SD: read 1 block(s), first bytes [00, 00, 00, 00]
S=0x4
```

With a valid `payload.bin` the loader prints the mount and handoff lines instead of entering
its loop:

```
FAT: mounted Fat32 at LBA 0 (987758592 bytes)
ARGS: no boot.cfg
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x00000000
```

With a `boot.cfg` on the card the argument line names the file and its DRAM address, and the
handoff carries the same pointer:

```
FAT: mounted Fat32 at LBA 0 (987758592 bytes)
ARGS: boot.cfg 13 bytes at 0x47ffefd0
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x47ffefd0
```

With `HIL_FEL_PAYLOAD=1` it skips SD initialisation and prints the mailbox request instead:

```
FEL: payload image 0x00039000, 192 bytes, args 0x00000000
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x00000000
```

After the handoff the UART is silent unless the payload prints, so the capture ends after the
`BOOT: handoff to` line. The payload then blinks PD22: 120 ms on, 180 ms off, 600 ms on,
900 ms off, repeating. The loader's own loop toggles every 50 ms, so the cadence tells a
completed handoff apart from a loader that never left its loop. The addresses, payload size
and checksum depend on the image header and the detected DRAM size.

- `CLK: PLL_ddr` is read before `dram.init()`, so it shows the pre-DRAM value; the DRAM
  initialisation that follows sets 792 MHz. See `research/clk-port-vs-upstream.md`.
- `SD/MMC card found` and the `SD: read ...` line prove the SDHCI/IDMAC path. The first
  bytes are whatever block 0 contains; an empty test card returns zeros. With
  `HIL_REQUIRE_SD=1` these two lines become assertions.
- `BOOT: handoff to ...` is printed after the payload is placed, its checksum verified at the
  destination, and `BootInfo` written. `HIL_REQUIRE_HANDOFF=1` makes it mandatory;
  `HIL_REQUIRE_PAYLOAD=1` also matches its entry, load address, size and checksum against
  `target/rawboot-payload.img`. Its `args` field is the `boot.cfg` pointer, or zero.
- `ARGS: boot.cfg ...` (SD path) proves the loader read the optional file and reserved a
  NUL-terminated copy; `ARGS: no boot.cfg` means the payload gets `args = 0`. The loader does not
  parse the file: the payload owns its meaning.
- `FEL: payload ...` proves the loader read the FEL mailbox and chose the uploaded image.
  `HIL_FEL_PAYLOAD=1` makes it mandatory and prints the argument pointer from the mailbox
  (`args 0x...`); the SD path reports the same field on the handoff line and through `ARGS:`.
- `SD/MMC card not found` is expected when no card is inserted. It does not invalidate the
  clock or DRAM checks. Full SD/MMC card parity (#20) is covered by
  `research/sdcard-port-vs-upstream.md`.
- `S=0x...` repeats while the loader's own LED loop runs, with a 50 ms toggle. The script stops
  after a few seconds. A card with a valid `payload.bin` hands off instead, prints
  `BOOT: handoff to ...` and blinks the short-long payload cadence.

## Troubleshooting

- `xfel` says `No FEL device found`: the board is not in FEL, or the USB cable is data-only
  and not connected to the OTG/FEL port. Re-enter FEL and power-cycle.
- `xfel` says `Can't connect to device`: the device node is not writable by the user. Check
  the udev rule and `ls -l /dev/bus/usb/<bus>/<dev>`.
- Preflight says `UART is not quiet`: a loader from a previous run is still printing (or the
  line is floating), even if `xfel version` answers. Rerun with `HIL_SKIP_PREFLIGHT=1` to
  drain it and replace the loader, or power-cycle into FEL (do not `xfel reset`) and rerun.
- `image commit ... does not match HEAD`: the image predates a branch switch or commit.
  Rebuild with `cargo xtask image` on the branch under test.
- `... is linked at 0x..., expected 0x...`: the loader was linked with the wrong script.
  `cargo xtask image` builds both variants and fails here rather than producing an unbootable
  image; check `rawboot/build.rs` and `rawboot-rt/link*.x`.
- A cold power-on from SD is silent and PD22 stays steady: sector 16 holds the FEL image
  (`0x00028000`) instead of `target/rawboot-boot-sd.bin` (`0x00020000`). The two images are not
  interchangeable; rebuild and rewrite the prefix with `cargo xtask image` and
  `cargo xtask sd-image --partition-sectors <n>` followed by `dd ... conv=notrunc`.
- HIL fails with `missing SD/MMC card found` or `missing a successful SD read`: no card,
  an unreadable card, or `HIL_REQUIRE_SD=1` without a card. Insert a card and rerun, or
  unset the flag for a no-card test.
- `SMHC: cmd1 timeout (rint=0x00000100, flag=0x00000004)` followed by `SD/MMC card not
  found` on a cold power-on with the card already inserted: CMD1 is the MMC fallback, so
  the SD path already failed (CMD8 or ACMD41). First check that the card actually lost
  power: a USB-UART adapter left connected can backfeed the board and keep the card
  stateful, which makes cold starts alternate PASS/FAIL on unmodified `master` (issue #43).
  Unplug the adapter while the board is off, then rerun the true cold-start procedure above.
  A passing run can still log an earlier `cmd1 timeout` before `SD/MMC card found`, because
  the driver retries. A card with a valid boot prefix in sector 16 also makes the BROM
  occasionally boot from SD instead of sampling FEL on a cold power-on; remove the card when
  you need a deterministic FEL entry.
- HIL fails with `missing a payload handoff`: `HIL_REQUIRE_HANDOFF=1` was set but the loader
  stayed in its `S=0x` loop. Check that `payload.bin` is present and valid, and that the card
  path succeeded.
- `BOOT: boot.cfg: is N bytes, but the buffer holds 4095`: the optional argument file exceeds
  the 4 KiB window (including the NUL terminator). Shorten it; the loader refuses to hand off
  with truncated arguments.
- HIL fails with `payload mismatch`: the card's `payload.bin` is not the image HIL compared with.
  Rebuild with `cargo xtask payload` and copy `target/rawboot-payload.img` over `payload.bin` on
  the FAT partition, or pass the image you actually wrote via `HIL_PAYLOAD_IMG`.
- The log shows `BOOT: handoff to ...` but PD22 keeps the loader's steady 50 ms toggle: the card
  has an old payload. Rebuild with `cargo xtask payload` and copy
  `target/rawboot-payload.img` over `payload.bin` on the FAT partition.
- HIL fails with `LED pattern not confirmed` or `HIL_REQUIRE_LED=1 needs an interactive prompt`:
  confirm the short-long cadence and rerun, or set `HIL_LED_CONFIRMED=1` after checking the
  board.
- HIL fails with `payload image not found`: run `cargo xtask payload`, or point
  `HIL_PAYLOAD_IMG` at the image you wrote to the card.
- HIL fails with `missing FEL: payload` under `HIL_FEL_PAYLOAD=1`: the mailbox magic/addresses
  did not reach the loader, or the loader is older than this path. Rebuild with
  `cargo xtask image` and check the `xfel write32` values against the mailbox table.
- The loader prints `BOOT: FEL payload image ... is outside the SRAM staging window`: the
  mailbox image address/size is wrong, or the image exceeds the 44 KiB window. Keep the image
  at `0x00039000` and rebuild it with `cargo xtask payload` if it grew.
- HIL refuses `HIL_FEL_PAYLOAD=1` with `HIL_REQUIRE_SD`: the FEL path skips the SD card;
  unset `HIL_REQUIRE_SD` for a FEL run.
- The UART is silent: check USART0 PE2/PE3, 115200 8N1, and that the CH340 is `/dev/ttyUSB0`.
- The log starts with repeated `Hello`/`S=` lines: the tty buffer still holds a previous
  run, or the board is in a reset loop. The script drains the buffer and now fails the run
  on multiple `Hello` lines; power-cycle the board (do not `xfel reset`) and rerun, or set
  `HIL_SKIP_PREFLIGHT=1` to drain and replace the running loader and ignore those stale lines.
- The loader prints `CLK: init timeout at ...`: a clock PLL did not lock; capture the full
  line and the register address for diagnosis.
