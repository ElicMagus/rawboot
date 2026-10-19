# `rawboot` SD/MMC card port vs current upstream `szemzoa/awboot` `sdmmc.c`

Researched: 2026. Target: Allwinner T113-S3. Upstream is pinned to `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21). Files compared:
`arch/arm32/mach-t113s3/sdmmc.c`, `arch/arm32/mach-t113s3/sdmmc.h`, `board.c`,
`board.h` and `Makefile` [1]-[5]. Local code is cited by symbol, not by line (see
`docs/agents/domain.md`).

This note is the card-protocol counterpart of `research/sdhci-port-vs-upstream.md`; it
is the static parity checklist for issue #20.

---

## 1. Executive summary

- Upstream `arch/arm32/mach-t113s3/sdmmc.c` is 764 lines at the pinned SHA [1];
  `sdmmc.h` is 198 lines [2]. The last upstream commit touching the driver is
  `385481ffab` (2025-11-15, "refactor sdmmc driver, create library for fatfs, fdt,
  loaders").
- Our loader boots an SD card, while the pinned T113-S3 `board.h` defaults to
  `CONFIG_BOOT_MMC=1` / `CONFIG_BOOT_SDCARD=0` [4]. Upstream's `VARIANT=sdmmc` build
  enables both and tries SD first, then resets and falls back to MMC [5]. The port
  transcribes that SD-first flow, so both branches are live: an SD card takes the SD
  path, and an MMC/eMMC card takes the MMC path.
- Issue #19 already replaced the PIO-only host path with the upstream 64-byte
  IDMAC/PIO split [6]. Issue #20 aligns the card protocol on top of that host API.
- `CONFIG_MMC_ENABLE_RSTN` defaults to `0` in `board.h` [4], so the optional eMMC
  hardware-reset CMD6 is not part of the pinned T113-S3 behaviour and is not
  transcribed.

### Verdict

The in-scope behaviour of `sdmmc.c` is ported: SD initialization, MMC initialization,
card identification, CID/CSD/EXT_CSD parsing, capacity calculation, SD 4-bit switch,
MMC high-speed/DDR/power-class/bus-width switching, and block reads. Names follow
upstream (`sdmmc_*`, `MMC_*`, `SD_CMD_*`, `OCR_*`, `SD_VERSION_*`,
`MMC_VERSION_*`, `EXT_CSD_*`); types and the HCI-ownership shape remain Rust-idiomatic
per ADR-0007.

---

## 2. Function-by-function comparison

| Upstream | Local | Status |
| --- | --- | --- |
| `UNSTUFF_BITS` macro [1:126-140] | `UNSTUFF_BITS` | Same mask, word index and cross-word shift. |
| `go_idle_state` [1:152-162] | same name | One CMD0, `MMC_RSP_R1` in SPI and `MMC_RSP_NONE` otherwise. |
| `sd_send_if_cond` [1:165-184] | same name | Same voltage-window argument, check pattern `0xaa`, R7 response and `SD_VERSION_2` on success. |
| `sd_send_op_cond` [1:186-241] | same name | CMD0 then CMD8 inside the function, CMD55/ACMD41 loop, 50 retries with upstream post-decrement semantics, SPI `MMC_SPI_READ_OCR`, `rca = 0`. |
| `mmc_send_op_cond` [1:270-309] | same name | CMD1 with exact `MMC_VDD_27_36` / `MMC_VDD_165_195` OCR arguments, 50 retries, 5 ms delay, `MMC_VERSION_UNKNOWN`, `rca = 1`. |
| `sdmmc_status` [1:311-331] | same name | 100 retries, ready bit `1 << 8`, 1 us delay, state `(response >> 9) & 0xf`, `-1` on timeout. |
| `sdmmc_read_blocks` [1:333-373] | same name | CMD17/CMD18, block vs byte addressing, R1, data descriptor, TRAN/DATA wait, CMD12 after multi-block. |
| `sdmmc_detect` [1:375-706] | same name | Reset, 400 kHz, 1-bit, CMD0, 2 ms, SD first, reset + CMD0 fallback to MMC; CID/CSD/RCA/select; version table; `tran_speed`, `dsr_imp`, block lengths; EXT_CSD; capacity; final width/clock. |
| `sdmmc_blk_read` [1:708-723] | same name | 127-block chunks, block number advance, buffer advance by `read_bl_len`. |
| `sdmmc_init` [1:725-763] | same name | `online` cleared once, 100 ms pause, `retries = 10` with upstream post-decrement (11 attempts), 0/-1 result. |
| `sdmmc_t` [2:147-163] | `Sdcard` | Same fields and widths. |
| `sdmmc_pdata_t` [2:165-171] | `Sdcard_data` | Same card/buffer/online fields; the owned `Sdhci` replaces upstream's `sdhci_t *`. |

### EXT_CSD and MMC switching

| Upstream behaviour | Local |
| --- | --- |
| Read 512-byte EXT_CSD for MMC >= 4, poll status to TRAN [1:552-580] | Same. |
| Set `tran_speed = 50 MHz` before EXT_CSD [1:553] | Same. |
| Map `EXT_CSD_REV` 1/2/3/5/6/7/8 to 4.1/4.2/4.3/4.41/4.5/5.0/5.1; default keeps the CSD version [1:582-620] | Same, including the default arm. |
| Capacity from CSD (SD) or `EXT_CSD_SEC_CNT` (MMC), then multiply by CSD block length [1:624-645] | Same. |
| Reject DDR when `EXT_CSD_CARD_TYPE` lacks DDR52; fall back to 50 MHz SDR [1:650-671] | Same warning and state change. |
| Fall back to 25 MHz when the card lacks 52 MHz/DDR52 timing [1:673-679] | Same. |
| CMD6 HS_TIMING, then power class when width >= 4, then BUS_WIDTH; 1 ms delays [1:681-720] | Same arguments, order and delays. |
| Final `sdhci_set_clock(clock_wanted)` and `sdhci_set_width(width)` outside the branch [1:722-725] | Same; SPI branch checks both calls and reports failure. |

---

## 3. Divergences fixed in #20

1. **SD initialization order.** The old port called CMD0 and CMD8 in `sdmmc_detect` and
   then entered `sd_send_op_cond`; upstream calls both inside `sd_send_op_cond`. The
   retry count was 100 instead of upstream's 50 and the loop shape differed. Replaced
   with the upstream CMD0/CMD8/CMD55/ACMD41 sequence and post-decrement retry budget.
2. **MMC initialization.** The old port issued an extra CMD1 with argument 0, used
   voltage masks instead of upstream's exact-equality OCR arguments and set `rca = 0`.
   Replaced with upstream's 50-retry CMD1 loop, `0x40ff8000` / `0x40ff0080` arguments,
   `MMC_VERSION_UNKNOWN` and `rca = 1`; the SD-failure fallback now resets the
   controller and reissues CMD0 exactly as upstream.
3. **CSD/EXT_CSD semantics.** The old port defaulted an unknown `EXT_CSD_REV` to
   version 0 and never applied upstream's `tran_speed = 50 MHz` before reading
   EXT_CSD. Both now match upstream.
4. **MMC speed and width switching.** The old MMC branch sent CMD55 + CMD6 with a raw
   width value. The port now performs upstream's full sequence: DDR capability check,
   high-speed timing, HS_TIMING write, 4-bit power class, BUS_WIDTH write, 1 ms
   delays, and the common final clock/width update.
5. **Read-status wait.** The old condition was inverted while being rewritten: it
   exited when the card was neither TRAN nor DATA. It now loops until TRAN or DATA,
   matching upstream and allowing `sdmmc_blk_read` to return.
6. **Names.** Command, response, status, OCR, data-flag, version and EXT_CSD names
   now match upstream; `sdmmc_init`/`sdmmc_blk_read` replace the old
   `Sdcard_data::init`/`blk_read` methods.

---

## 4. Deliberate differences

- **Types and ownership.** `Sdcard`, `Sdcard_data`, `Sdhci` and `SdhciCmd` replace the
  C structs; the HCI is owned by the data struct instead of referenced, and the
  read/init entry points return `u64`/`i32` with `&mut` arguments. Behaviour and field
  order stay upstream's (ADR-0007).
- **Logging.** Upstream `trace`/`debug`/`warning`/`error` become `print!`. The loader
  keeps its concise `SD/MMC card found` / `SD/MMC card not found` lines instead of
  upstream's `SHMC: ... card detected`; error text is transcribed. The EXT_CSD version
  and floating-point capacity debug lines are not printed.
- **eMMC hardware reset.** `CONFIG_MMC_ENABLE_RSTN` is `0` at the pinned board, so the
  optional RST_N CMD6 and its debug line are deliberately absent.
- **SPI branch.** `isspi` remains a field and the SPI branches are transcribed, but the
  T113-S3 board instance is not SPI; no SPI host is instantiated.
- **eMMC board instance.** Only `sdhci0` is instantiated. An eMMC on `sdhci2` is outside
  the T113-S3 SD feature and remains deferred.
- **No host seam.** As decided by the parent issue, there is no host test for the card
  driver; parity is checked by this static table plus HIL on T113-S3.

---

## 5. Hardware confirmation

Verified on the T113-S3 with `scripts/hil-check.sh target/rawboot-fel.bin iss20` from
branch `feat/20-parity-sdcard` at commit `0ae1ecc` (image stamp `dirty=false`). The UART
still carried the previous loader, so the run used the documented
`HIL_SKIP_PREFLIGHT=1` path; the script drained it, reported one `Hello`, and asserted
the required lines:

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

- `SD/MMC card found` means CMD0/CMD8/CMD55/ACMD41, CID/CSD/RCA/select and the 50 MHz
  4-bit switch all succeeded on the SD path.
- The 512-byte read is above the 64-byte threshold and therefore goes through IDMAC; the
  block is zero-filled because the test card is blank.
- The owner confirmed the PD22 LED blinks in the payload loop.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/sdmmc.c` at `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.c>
2. Upstream `arch/arm32/mach-t113s3/sdmmc.h` at the same SHA —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.h>
3. Upstream `board.c` at the same SHA (sdhci0/sdhci2 instances, PF pins).
4. Upstream `board.h` at the same SHA (`CONFIG_BOOT_*`, `CONFIG_MMC_ENABLE_RSTN`).
5. Upstream `Makefile` at the same SHA (`VARIANT=sdmmc` enables SD and MMC).
6. `research/sdhci-port-vs-upstream.md` and issue #19.
7. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
8. Issue #20 "Parity SD/MMC-карты: sdcard" and parent issue #14.
