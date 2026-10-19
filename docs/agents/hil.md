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

The script:

1. waits up to 3 minutes for a responsive FEL (`xfel version` succeeds);
2. drains the UART input buffer before loading (stale bytes from the previous run otherwise
   look like a reset loop);
3. writes the image at `0x00028000` and jumps to it;
4. captures USART0 for a few seconds, prints the unique lines and asserts
   `Hello`, `CLK:`, `DRAM simple test OK`, `S=0x`;
5. asks the human to confirm the PD22 LED blinks.

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

The current `master` prints:

```
Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
trefi:7.8ms
DRAMC read ODT  off.
DRAM ODT value: 0x42.
DRAM simple test OK.
get state
SD/MMC card not found
S=0x10000
```

- `CLK: PLL_ddr` is read before `dram.init()`, so it shows the pre-DRAM value; the DRAM
  initialisation that follows sets 792 MHz. See `research/clk-port-vs-upstream.md`.
- `SD/MMC card not found` is expected when no card is inserted. It does not invalidate the
  clock or DRAM checks; SD bring-up is tracked in #19/#20.
- `S=0x10000` repeats while the LED toggles. The script stops after a few seconds.

## Troubleshooting

- `xfel` says `No FEL device found`: the board is not in FEL, or the USB cable is data-only
  and not connected to the OTG/FEL port. Re-enter FEL and power-cycle.
- `xfel` says `Can't connect to device`: the device node is not writable by the user. Check
  the udev rule and `ls -l /dev/bus/usb/<bus>/<dev>`.
- The UART is silent: check USART0 PE2/PE3, 115200 8N1, and that the CH340 is `/dev/ttyUSB0`.
- The log starts with repeated `Hello`/`S=` lines: that is the tty buffer from a previous
  run. The script drains it; a manual capture must drain too.
- The loader prints `CLK: init timeout at ...`: a clock PLL did not lock; capture the full
  line and the register address for diagnosis.
