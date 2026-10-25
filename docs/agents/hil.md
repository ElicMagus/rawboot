# Hardware-in-the-loop (HIL) verification

The loader is verified on a real T113-S3. This page is the runbook: host setup, the load
sequence, the UART capture, and how to read the result. The script from this repo is
`scripts/hil-check.sh`; it automates the steps below.

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
parses.

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
