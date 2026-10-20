# Hardware-in-the-loop (HIL) verification

The loader is verified on a real T113-S3. This page is the runbook: host setup, the load
sequence, the UART capture, and how to read the result. The script from this repo is
`scripts/hil-check.sh`; it automates the steps below.

## Board and wiring

- SoC: Allwinner T113-S3, DDR3, 128 MiB.
- Debug UART: **USART0**, pins **PE2/PE3**, **115200 8N1**. On the host it is the CH340
  (`/dev/ttyUSB0`, override with `UART_DEV`).
- LED: **PD22**. The loader toggles it in its main loop; after a handoff the payload owns it.
  The script cannot see the LED, so the run ends with a human check.
- FEL: USB OTG in FEL mode. The board enumerates as `1f3a:efe8` (some adapters report
  `1f3c:*`). Enter FEL by holding the FEL/BOOT button while applying power or reset.

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

Build the loader, then run the script. The loader is linked for SRAM at `0x00028000`; it
initialises DRAM itself, so `xfel ddr` is **not** part of the procedure.

```sh
cargo xtask image
scripts/hil-check.sh target/rawboot-fel.bin hil
```

For a ticket whose acceptance includes the card path, set `HIL_REQUIRE_SD=1`. The script
then also requires `SD/MMC card found` and a non-zero `SD: read N block(s)` line. Leave it
unset when no card is inserted. For a ticket that must prove the handoff, also set
`HIL_REQUIRE_HANDOFF=1`: the log must then contain a `BOOT: handoff to ...` line and the
loader's `S=0x` loop is no longer accepted.

Run HIL from the code branch under test, before switching to `master` for docs or a merge.
`cargo xtask image` writes `target/rawboot-fel.bin.build` with the source commit, branch and
dirty flag; `hil-check.sh` refuses an image whose commit is not `HEAD`, so rebuild after
switching branches (`HIL_SKIP_PROVENANCE=1` is the escape hatch).

Put the board into FEL with a power-cycle (or reset while holding the FEL/BOOT button), not
with `xfel reset`: `xfel reset` resets through the watchdog, which stays armed, and the
loader then boots in a loop. A loader left running from a previous run also keeps printing
on USART0, so clear it with a power-cycle before each run. On a terminal the script prints
this and waits for Enter; piped or `HIL_NO_PROMPT=1` runs skip the prompt.

`xfel version` succeeding does **not** prove the board is idle in FEL: the FEL USB gadget can
still answer while a loader from a previous run prints on USART0. The script's UART-quiet
preflight is the authority.

If a loader from a previous run is still printing, `HIL_SKIP_PREFLIGHT=1` drains USART0 and
replaces it with the image under test. The preflight warning is expected then: the old `S=`
lines are stale output, not a reset loop. Use this instead of repeated power-cycles when
`xfel` still answers; use a power-cycle when it does not.

The script:

1. checks the image build stamp against `HEAD`;
2. waits up to 3 minutes for a responsive FEL (`xfel version` succeeds);
3. requires the UART to be quiet for a short window, so a still-running loader is reported
   as a busy board instead of looking like a reset loop later (`HIL_SKIP_PREFLIGHT=1`
   downgrades this check to a warning);
4. drains the UART input buffer before loading;
5. writes the image at `0x00028000` and jumps to it;
6. captures USART0 for a few seconds, prints the unique lines and asserts `Hello`, `CLK:`,
   `DRAM simple test OK`, and either the loader loop (`S=0x`) or a payload handoff
   (`BOOT: handoff to ...`); with `HIL_REQUIRE_SD=1` it also asserts `SD/MMC card found` and a
   non-zero `SD: read N block(s)`, and with `HIL_REQUIRE_HANDOFF=1` it requires the handoff
   line. It warns if more than one `Hello` appears (stale buffer or a reset loop);
7. asks the human to confirm the PD22 LED blinks.

The raw capture is kept at `/tmp/rawboot-hil-<label>.log`.

### Manual load (no script)

```sh
xfel write 0x00028000 target/rawboot-fel.bin
xfel exec 0x00028000
```

Do not issue `xfel ddr t113-s3` first. It initialises DRAM and the loader's second
initialisation hangs on PLL_DDR (issue #23). The loader needs only SRAM until it has
initialised DRAM itself.

If the UART is captured by hand, drain it before `xfel exec` (for example with
`python3 -c 'import serial; s=serial.Serial("/dev/ttyUSB0",115200); s.reset_input_buffer()'`)
and capture at 115200 8N1.

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
BOOT: handoff to 0x42000000: 16 bytes at 0x42000000, BootInfo 0x47feffe0, SP 0x48000000
```

After the handoff the UART is silent unless the payload prints, so the capture ends after the
`BOOT: handoff to` line. The addresses depend on the image header and the detected DRAM size.

- `CLK: PLL_ddr` is read before `dram.init()`, so it shows the pre-DRAM value; the DRAM
  initialisation that follows sets 792 MHz. See `research/clk-port-vs-upstream.md`.
- `SD/MMC card found` and the `SD: read ...` line prove the SDHCI/IDMAC path. The first
  bytes are whatever block 0 contains; an empty test card returns zeros. With
  `HIL_REQUIRE_SD=1` these two lines become assertions.
- `BOOT: handoff to ...` is printed after the payload is placed, its checksum verified at the
  destination, and `BootInfo` written. `HIL_REQUIRE_HANDOFF=1` makes it mandatory.
- `SD/MMC card not found` is expected when no card is inserted. It does not invalidate the
  clock or DRAM checks. Full SD/MMC card parity (#20) is covered by
  `research/sdcard-port-vs-upstream.md`.
- `S=0x...` repeats while the loader's own LED loop runs. The script stops after a few seconds.
  A card with a valid `payload.bin` hands off instead and prints `BOOT: handoff to ...`.

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
- HIL fails with `missing SD/MMC card found` or `missing a successful SD read`: no card,
  an unreadable card, or `HIL_REQUIRE_SD=1` without a card. Insert a card and rerun, or
  unset the flag for a no-card test.
- HIL fails with `missing a payload handoff`: `HIL_REQUIRE_HANDOFF=1` was set but the loader
  stayed in its `S=0x` loop. Check that `payload.bin` is present and valid, and that the card
  path succeeded.
- The UART is silent: check USART0 PE2/PE3, 115200 8N1, and that the CH340 is `/dev/ttyUSB0`.
- The log starts with repeated `Hello`/`S=` lines: the tty buffer still holds a previous
  run, or the board is in a reset loop. The script drains the buffer and now fails the run
  on multiple `Hello` lines; power-cycle the board (do not `xfel reset`) and rerun.
- The loader prints `CLK: init timeout at ...`: a clock PLL did not lock; capture the full
  line and the register address for diagnosis.
