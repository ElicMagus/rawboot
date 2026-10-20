# Upstream parity checklist (loader modules)

Researched: 2026. Static index for issue #14: one row per in-scope loader module, pointing at
the function-by-function divergence checklist. Parity means behavioural equivalence with
upstream — same step order, constants, branches and tables — while the expression stays our
idiomatic Rust (ADR-0007).

Upstream anchor: `szemzoa/awboot` `main` `5380c00fc67c975433f25c57fb481aa2b91aebf8`
(2025-11-21). Fetch it with `scripts/fetch-upstream.sh`; cite upstream by file and line, our
code by symbol (`docs/agents/domain.md`). This index cites files only; the line-level
references are in each linked checklist.

## In-scope modules

| Module | Upstream source | Local code | Divergence checklist | Ticket |
| --- | --- | --- | --- | --- |
| `ccu` | `arch/arm32/mach-t113s3/reg-ccu.h` | `rawboot/src/ccu/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `clk` | `arch/arm32/mach-t113s3/sunxi_clk.c` | `rawboot/src/clk/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `gpio` | `arch/arm32/mach-t113s3/sunxi_gpio.[ch]`, `board.c` pin tables | `sunxi-pl/src/gpio.rs` (call sites in `rawboot/src/sdhci/mod.rs`, `rawboot/src/main.rs`) | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `io` | `include/io.h`, `arch/arm32/mach-t113s3/arch_timer.c` (`sdelay` only) | `sunxi-pl/src/io.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `cpu` (timer) | `arch/arm32/mach-t113s3/arch_timer.c` | `sunxi-pl/src/cpu.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17, #29 |
| `uart` | `arch/arm32/mach-t113s3/sunxi_usart.[ch]`, `board.c` (`usart0_dbg`) | `rawboot/src/uart/mod.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `dram` | `arch/arm32/mach-t113s3/dram.[ch]`, `reg-dram.h` | `rawboot/src/dram/mod.rs` | `research/dram-port-vs-upstream.md` | #15, #18 |
| `sdhci` | `arch/arm32/mach-t113s3/sunxi_sdhci.[ch]` | `rawboot/src/sdhci/mod.rs` | `research/sdhci-port-vs-upstream.md` | #19 |
| `sdcard` | `arch/arm32/mach-t113s3/sdmmc.[ch]` | `rawboot/src/sdcard/mod.rs` | `research/sdcard-port-vs-upstream.md` | #20 |

Each checklist carries the upstream function surface and the deliberate differences that
remain; the #17/#19/#20 notes also enumerate the divergences their tickets fixed. The DRAM
note additionally records the pre-rework history (`mctl_hal.c` fork point) behind the old port.

## Deliberate differences

Part of the parity decision, not gaps to "fix"; the linked section owns the detail:

- **`dram_get_top` / `dram_top` absent** — `research/dram-port-vs-upstream.md` (remaining
  differences). Add it only when a payload needs a top address (issue #14).
- **T113-S4/V851s paths absent** — ADR-0007. Only T113-S3 is targeted.
- **`abort()` absent** — `research/dram-port-vs-upstream.md` (remaining differences).
- **Module layout and C idioms stay ours** — ADR-0007. Upstream `common.h`, `xformat` and
  the macro `debug.h` are not reproduced.
- **`ccu` keeps `PLL_*_REG` constant names.** Values were checked against `reg-ccu.h`; a
  strict `CCU_*` prefix rename is a mechanical follow-up, not a behavioural gap
  (`research/clk-port-vs-upstream.md`, deliberate differences).
- **Deferred upstream drivers** — #21 tracks the DMA engine, SPI/SPI-NAND, watchdog and
  cache/exception helpers.

## Verification

- Static oracle: the checklists above plus the pinned upstream sources.
- Execution seam: HIL smoke on T113-S3 (`docs/agents/hil.md`) — UART initialisation log plus
  a working blink payload.
