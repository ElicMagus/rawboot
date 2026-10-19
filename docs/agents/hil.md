# Hardware-in-the-loop (HIL) verification

The loader is verified on a real T113-S3. This page is the runbook: host setup, the load
sequence, the UART capture, and how to read the result. The script from this repo is
`scripts/hil-check.sh`; it automates the steps below.

## Board and wiring

- SoC: Allwinner T113-S3, DDR3, 128 MiB.
- Debug UART: **USART0**, pins **PE2/PE3**, **115200 8N1**. On the host it is the CH340
  (`/dev/ttyUSB0`, override with `UART_DEV`).
- LED: **PD22**. The loader toggles it in its main loop; the script cannot see it, so the run
  ends with a human check.
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
6. captures USART0 for a few seconds, prints the unique lines and asserts
   `Hello`, `CLK:`, `DRAM simple test OK`, `S=0x`; it warns if more than one `Hello`
   appears (stale buffer or a reset loop);
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

The current `master` prints (with a card inserted):

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

- `CLK: PLL_ddr` is read before `dram.init()`, so it shows the pre-DRAM value; the DRAM
  initialisation that follows sets 792 MHz. See `research/clk-port-vs-upstream.md`.
- `SD/MMC card found` and the `SD: read ...` line prove the SDHCI/IDMAC path. The first
  bytes are whatever block 0 contains; an empty test card returns zeros.
- `SD/MMC card not found` is expected when no card is inserted. It does not invalidate the
  clock or DRAM checks. Full SD/MMC card parity (#20) is covered by
  `research/sdcard-port-vs-upstream.md`.
- `S=0x...` repeats while the LED toggles. The script stops after a few seconds.

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
- The UART is silent: check USART0 PE2/PE3, 115200 8N1, and that the CH340 is `/dev/ttyUSB0`.
- The log starts with repeated `Hello`/`S=` lines: the tty buffer still holds a previous
  run, or the board is in a reset loop. The script drains the buffer and now fails the run
  on multiple `Hello` lines; power-cycle the board (do not `xfel reset`) and rerun.
- The loader prints `CLK: init timeout at ...`: a clock PLL did not lock; capture the full
  line and the register address for diagnosis.
