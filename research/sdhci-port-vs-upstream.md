# `rawboot` SDHCI port vs current upstream `szemzoa/awboot` `sunxi_sdhci.c`

Researched: 2026, status updated after issue #19. Target: Allwinner T113-S3. Upstream is
pinned to `szemzoa/awboot` `main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`
(2025-11-21). Local code is cited by symbol, per `docs/agents/domain.md`; line numbers would
rot.

Reference numbers in square brackets map to the **Sources** section at the end.

---

## 1. Executive summary

- At the pinned SHA, `arch/arm32/mach-t113s3/sunxi_sdhci.c` is 972 lines / 28,936 B and
  `sunxi_sdhci.h` is 148 lines / 5,574 B [1][2]. The last upstream commit touching the driver
  is `385481ffab` (2025-11-15, "refactor sdmmc driver, create library for fatfs, fdt,
  loaders") [9].
- The old local module was a near line-for-line port of the older upstream Rust snapshot
  (`embed/app/src/sdhci/mod.rs`) and inherited its PIO-only data path: every transfer went
  through the FIFO. Upstream moved SD transfers larger than 64 bytes to the controller's
  internal IDMAC with a descriptor chain, keeping PIO only for small transfers [1:312-418,
  582-597].
- Issue #19 ports the current upstream driver behaviour: reset, voltage field handling, bus
  width, clocking (CCU M/N/post-divider, NTSR new-timing mode, sample-delay calibration),
  command path, and the IDMAC/PIO split. Names follow upstream snake_case; types stay
  Rust-idiomatic per ADR-0007.

### Verdict

The in-scope behaviour of `sunxi_sdhci.c` is now ported: IDMAC above 64 bytes, PIO at or
below 64 bytes, upstream command flags, clock/delay setup, and start-bit detection. The
deliberate differences are the Rust type/API expression, the absent eMMC board instance, the
unimplemented upstream `sdhci_set_voltage` declaration, logging through `print!`, and the
#43 cold-start reset/CMD0-init additions taken from mainline Linux.

The full SD/MMC *card* protocol parity remains issue #20; #19 only adapts `sdcard` to the new
host API and fixes the state-passing needed to exercise a read.

---

## 2. Register and constant surface

The register block is transcribed with upstream field names (`gctrl`, `clkcr`, `timeout`,
`width`, `blksz`, `bytecnt`, `cmd`, `arg`, `resp0..3`, `status`, `ftrglevel`, `ntsr`, `dmac`,
`dlba`, `idst`, `idie`, `dsbd`, `drv_dl`, `samp_dl`, `skew_ctrl`, `fifo`, …) as offset
constants in the private `reg` module [1:18-77][2:18-77]. Bit names keep the upstream prefixes
`SMHC_GCTRL_*`, `SMHC_CLKCR_*`, `SMHC_CMD_*`, `SMHC_RINT_*`, `SMHC_STATUS_*`,
`SMHC_IDMAC_*`, `SMHC_IDMAC_DES0_*`, and `SDXC_*` [1:42-227].

Clock selectors and bus widths use the upstream numeric values:

| Name | Value | Meaning |
| --- | --- | --- |
| `MMC_CLK_400K` … `MMC_CLK_200M` | `0..6` | timing index used by `odly`/`sdly` |
| `MMC_VDD_27_36` | `1` | 2.7–3.6 V voltage field |
| `MMC_VDD_165_195` | `2` | 1.65–1.95 V voltage field |
| `MMC_BUS_WIDTH_1` | `1` | 1-bit bus |
| `MMC_BUS_WIDTH_4` | `2` | 4-bit bus |

---

## 3. Function-by-function comparison

| Upstream | Local after #19 | Status |
| --- | --- | --- |
| `sdhci_configure_start_bit_detection` [1:229-241] | same name | Same `dsbd` clear/set, half-cycle bit for non-HS400. |
| `smhc_get_ccu_params` [1:243-276] | same name | Same `id` → SMHC0/1/2 clock, gate and reset mapping; returns `Option<SmhcCcuParams>` instead of out-params. |
| `set_read_timeout` [1:278-310] | same name | Same `DTO_MAX` calculation and `gctrl` 256x-timeout bit. |
| `prepare_dma` [1:312-389] | same name | Same 4 KiB fragments, `size-1` encoding, `des_chain`/`own`/`dic`, first/last flags, `dlba >> 2`, IDMAC soft reset, fix-burst, IRQ enable, and trigger level. |
| `wait_done` [1:391-418] | same name | Same timeout/error checks and the DMA completion condition (`rint` flag plus `idst` receive bit); returns `Result<u32, u32>`. |
| `read_bytes` [1:420-478] | same name | Same FIFO-empty spin, FIFO-level drain, data-over/auto-stop completion, leftover check. |
| `write_bytes` [1:480-523] | same name | Same FIFO-full spin, word push, completion and leftover check. |
| `sdhci_transfer` [1:525-645] | same name | Same stop shortcut, response flags, hold register at `>= MMC_CLK_50M`, data setup, `rint` clear, **64-byte IDMAC/PIO split**, completion waits, busy wait, response order and IDMA cleanup; plus `SEND_INIT_SEQUENCE` on CMD0 (deliberate, #43). |
| `sdhci_reset` [1:647-651] | same name | Writes `HARDWARE_RESET` and polls `GCTRL` until the self-clearing bits are gone (250 ms bound); deliberate divergence from upstream, #43. |
| `sdhci_set_width` [1:653-694] | same name | Same width constants, DDR gating, start-bit detection, DDR re-calibration. |
| `init_default_timing` [1:696-715] | same name | Same `TM5_OUT_PH*` / `TM5_IN_PH*` tables. |
| `config_delay` [1:717-787] | same name | Same `drv_dl`, `ntsr` sample delay, hardware skew mode, and `samp_dl` calibration for `> MMC_CLK_400K`. |
| `update_card_clock` [1:789-801] | same name | Same `UPCLK_ONLY` handshake and `rint` clear. |
| `sdhci_set_clock` [1:803-938] | same name | Same CCU post-divider compensation, DDR 2x module clock, OSC24M/PLL_PERIPH1X source, M/N division, new-timing bit, gate/reset sequence, `clkcr` divider, and update handshake. |
| `sunxi_sdhci_init` [1:940-971] | same name | Same GPIO mux/pull order, default timing, reset, `rint` clear, DMA trigger level, and start-bit detection. |
| `sdhci_set_voltage` [2:142] | same name | Upstream only declares it; local keeps a `false` stub because the header has no implementation and no caller in the T113-S3 path. |

---

## 4. Divergences fixed in #19

1. **Data path.** The old module always used `read_bytes`/`write_bytes` through the FIFO.
   Transfers with `blkcnt * blksz > 64` now build an IDMAC descriptor chain and let the SMHC
   controller move the data; transfers up to 64 bytes keep the upstream PIO path. The same
   `prepare_dma` cleanup clears `idst`, disables `idie`, `dmac` and the DMA gate.
2. **Clocking.** The old `init` hard-coded CCU gate/reset writes and a `pclk = peri1x / 12`
   ratio; `set_clock` used a simple `pclk` divider. The port now does what upstream does:
   select OSC24M or PLL_PERIPH1X, use the CCU M/N divider with the T113 post-divider
   compensation, force `NTSR` new-timing mode, use a fixed `clkcr` divider (2 in DDR mode),
   and run `config_delay` on every clock change.
3. **Reset and init.** `sunxi_sdhci_init` now only muxes/pulls the pins, loads the timing
   tables, resets the controller, clears interrupts, sets the DMA trigger level and enables
   start-bit detection. Gate/reset/clock enable happens in `sdhci_set_clock`, as upstream.
4. **Bus width.** Width values now use upstream's `MMC_BUS_WIDTH_1 = 1`,
   `MMC_BUS_WIDTH_4 = 2`, and `sdhci_set_width` applies the DDR/edge-latch `gctrl` bits.
5. **Command path.** `sdhci_transfer` now clears `rint` before the argument write, sets
   `USE_HOLD_REGISTER` at 50 MHz and above, clears the matching `idst` direction bit, sets
   `DATA_EXPIRE`/`WAIT_PRE_OVER` and `SEND_AUTO_STOP`, and uses the upstream 100 ms command
   and 6000 ms data timeouts.
6. **Start-bit detection and delays.** `dsbd` programming, `drv_dl`/`samp_dl`/`skew_ctrl`
   calibration and the delay tables are transcribed rather than guessed.
7. **Names.** Driver functions and register/bit constants now match upstream; the register
   block is addressed by named offsets instead of magic numbers.
8. **PLL_peri rate (blocking bug).** `sunxi_clk_get_peri1x_rate` multiplied `24 * plln` in
   `u8`, so a 1200 MHz PLL_peri0 read back as 100 MHz and `sdhci_set_clock(50M)` programmed a
   divider of 1 instead of 6. The arithmetic is now `u32`, matching the C promotion rules;
   HIL confirms `CCU_SMHC0_CLK_REG` reads back the expected M/N fields and the card reaches
   CMD16/read at 50 MHz.

---

## 5. Deliberate differences

- **Types and control flow.** `Sdhci`, `SdhciCmd`, `SdhciData` and `SdhciIdmaDesc` replace the
  C structs; `Option<&SdhciData>` replaces the `NULL`-able data pointer; `wait_done` returns
  `Result<u32, u32>`; `smhc_get_ccu_params` returns a small struct. Behaviour and field order
  stay upstream's (ADR-0007).
- **Board instances.** Only the SD instance (`sdhci0`, PF pins, 4-bit, 50 MHz) is
  transcribed. The eMMC instance (`sdhci2`) is outside the T113-S3 SD feature and is not
  instantiated; the CCU helper still knows controllers 0..2.
- **Voltage.** `sdhci_set_voltage` is a `false` stub: upstream declares it in
  `sunxi_sdhci.h` but does not define it in `sunxi_sdhci.c`, and the SD path only reads
  `hci->voltage` in `sdmmc.c` [1][2].
- **Logging.** Upstream `trace`/`debug`/`warning`/`error` become `print!` on USART0, matching
  the rest of the loader.
- **PIO buffer access.** `read_bytes`/`write_bytes` use `read_unaligned`/`write_unaligned`
  instead of C casts, so a caller's byte buffer need not be word-aligned. The FIFO still
  moves 32-bit words, as upstream.
- **Descriptor guard.** `prepare_dma` rejects a transfer needing more than the 32 descriptors
  in `Sdhci` instead of writing past the array.
- **FIFO level refresh.** `read_bytes` re-reads `status` after the FIFO-empty spin before
  computing the level, rather than using the value sampled before the spin.
- **Reset completion.** `sdhci_reset` polls `GCTRL` until the `HARDWARE_RESET` bits clear
  (250 ms bound), as mainline Linux `sunxi_mmc_reset_host` does. Upstream returns right
  after the write, so the clock/timing programming can race an in-progress reset after
  BROM hands over. Adopted for #43; see `research/sd-cold-start-brom.md`.
- **CMD0 init sequence.** `sdhci_transfer` sets `SEND_INIT_SEQUENCE` for
  `MMC_GO_IDLE_STATE`, as mainline Linux does. The SMHC hardware then supplies the
  74+ clocks the SD physical layer requires before CMD0; upstream relies on the caller's
  post-CMD0 delay instead. Adopted for #43; see `research/sd-cold-start-brom.md`.

### Not required

- **Cache maintenance.** The loader runs with MMU and caches off (ADR-0004/ADR-0006); the
  IDMAC descriptors and buffers are plain SRAM/DRAM accesses, so no clean/invalidate step is
  needed.
- **DMA engine, SPI-NAND, watchdog and exception helpers.** Deferred by the parent issue
  (#14) and tracked separately (#21).
- **T113-S4/V851s.** Out of scope; only the T113-S3 pin/clock tables are kept.

---

## 6. `sdcard` impact

Issue #19 changed only what the new host API forced; issue #20 then aligned the full card
protocol with `sdmmc.c` (see `research/sdcard-port-vs-upstream.md`). The #19 changes were:

- `Sdhci` is now non-`Copy` and is owned by `Sdcard_data`; the card and HCI are passed as
  `&mut`/`&` instead of by value, so responses, RCA, CSD and clock/width state survive
  detection.
- `sdhci_cmd_t::cmdidx/cmdarg` become `SdhciCmd::idx/arg`; `hci.transfer(...)` becomes
  `sdhci_transfer(..., Option<&SdhciData>)`.
- Width checks use the upstream `MMC_BUS_WIDTH_*` values, and the final clock uses
  `hci.clock_wanted`.
- The missing 2 ms post-CMD0 delay from current `sdmmc_detect` was added; without it the
  first CMD8 on a cold boot timed out.
- The CMD55 retry loop is bounded, so a missing card fails detection instead of spinning
  forever printing `cmd55 timeout`.
- Detection retries with a 100 ms pause, matching `sdmmc_init`; this removes the
  cold-start flakiness where the first CMD8 arrived before the card was ready.

CID/CSD/EXT_CSD parsing and command sequencing remained the old port until #20; see
`research/sdcard-port-vs-upstream.md` for the completed card-protocol parity.

---

## 7. Hardware confirmation

Verified on the T113-S3 with `scripts/hil-check.sh target/rawboot-fel.bin iss19` after a clean
power-cycle into FEL. The script confirmed a quiet UART, an image built from the code branch
(`f31d65a`, dirty=false), one `Hello`, and all required assertions:

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
PASS: UART assertions
```

- `SD/MMC card found` means CMD0/CMD8/ACMD41/CID/CSD/RCA/select and the 50 MHz 4-bit switch
  all succeeded on a cold boot.
- The 512-byte read is above the 64-byte threshold and therefore goes through IDMAC; the
  returned block is zero-filled because the test card is blank.
- `S=0x4` is the LED-loop status line after the read.
- No cache maintenance is performed; the loader runs with caches off.
- The LED on PD22 is the script's human check and blinks in the same loop.

---

## 8. Open questions

1. **IDMAC and SRAM.** The descriptor array lives in the loader's SRAM-resident `Sdhci`
   value; the buffers live in DRAM. Upstream keeps `sdhci0` in its global data. HIL
   confirmed a 512-byte IDMAC read works with this layout.
2. **`set_read_timeout` field type.** Upstream divides `clock_active` (a timing *index*, not
   Hz) by 1000, so the computed value is normally zero and the code falls back to the
   maximum timeout. The port transcribes that behaviour rather than silently "fixing" it;
   worth confirming against a future upstream change.
3. **eMMC path.** `sdhci_set_width` only knows 1- and 4-bit modes, matching the T113-S3
   upstream header; an 8-bit eMMC instance would need the deferred `sdhci2` work.
4. **Card read timing.** The IDMAC receive path completed a 512-byte read on the T113-S3 with
   caches off; no cache maintenance was needed.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/sunxi_sdhci.c` at `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_sdhci.c>
2. Upstream `arch/arm32/mach-t113s3/sunxi_sdhci.h` at the same SHA —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_sdhci.h>
3. Upstream `arch/arm32/mach-t113s3/sdmmc.h` at the same SHA (command/response widths) —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.h>
4. Upstream `arch/arm32/mach-t113s3/sdmmc.c` at the same SHA (card protocol; issue #20) —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.c>
5. Upstream `board.c` at the same SHA (sdhci0 instance and PF pins) —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/board.c>
6. Upstream `arch/arm32/mach-t113s3/reg-ccu.h` at the same SHA (SMHC clock/gate registers) —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/reg-ccu.h>
7. Local `rawboot/src/sdhci/mod.rs`, `rawboot/src/sdcard/mod.rs`,
   `rawboot/src/ccu/mod.rs`, `sunxi-pl/src/cpu.rs` (the timer, re-exported by `rawboot-rt`).
8. Vendored pre-port reference `embed/app/src/sdhci/mod.rs`.
9. GitHub API, commit history for `arch/arm32/mach-t113s3/sunxi_sdhci.c` —
   <https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/sunxi_sdhci.c&per_page=20>
10. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
11. Issue #19 "Parity SD-хоста: sdhci (IDMAC)" (SD host parity: sdhci (IDMAC)) and parent issue #14.
12. Prior research notes with the same format: `research/gpio-io-uart-port-vs-upstream.md`,
    `research/dram-port-vs-upstream.md`.
