# `rawboot` clock port vs current upstream `szemzoa/awboot` `sunxi_clk.c`

Researched: 2026. Target: Allwinner T113-S3. Upstream is pinned to `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21), file
`arch/arm32/mach-t113s3/sunxi_clk.c` [1]. The local side is `rawboot/src/clk/mod.rs` and
`rawboot/src/ccu/mod.rs` [2][3].

This note is the clock counterpart of `research/dram-port-vs-upstream.md`; it exists so a
parity review does not have to re-fetch the upstream source.

---

## 1. Function-by-function comparison

| Upstream `sunxi_clk.c` (5380c00) | `rawboot/src/clk/mod.rs` | Status |
| --- | --- | --- |
| `wait_for_bit_set(addr, mask)` | `wait_for_bit_set` | Same: 2,000,000 spins, `sdelay(1)`, first failing address recorded. |
| `sunxi_clk_get_fail_addr()` | `sunxi_clk_get_fail_addr` | Same, via `AtomicU32` instead of a static. |
| `set_pll_cpux_axi()` | `set_pll_cpux_axi` | Same order: PLL_PERI(1x) source, `sdelay(10)`, gating/LDO, `41 << 8`, lock/enable, wait bit 28, `sdelay(20)`, gating off, final `CPU_AXI_CFG` with `DIV2=1`. The final write omits an explicit `0 << 16`, equivalent to upstream. |
| `set_pll_periph0()` | `set_pll_periph0` | Same, including the early return when bit 31 is set. |
| `set_ahb()` | `set_ahb` | Same `PSI_CLK_REG = (2 << 0) | (0 << 8) | (0x03 << 24)`. |
| `set_apb()` | `set_apb` | Same `APB0_CLK_REG = (2 << 0) | (1 << 8) | (0x03 << 24)`. |
| `set_dma()` | `set_dma` | Same reset/gate sequence. |
| `set_mbus()` | `set_mbus` | Same reset and `0x00000d87` gating mask. |
| `set_module(addr)` | `set_module` | Same enable/lock/wait sequence. |
| `sunxi_clk_init()` | `sunxi_clk_init` | Same order: CPUX/AXI, PERI0, AHB, APB0, DMA, MBUS, then PERI0, VIDEO0, VIDEO1, VE, AUDIO0, AUDIO1. |
| `sunxi_clk_get_peri1x_rate()` | `sunxi_clk_get_peri1x_rate` | Same `(24 * plln / (pllm * p0)) >> 1 * 1000 * 1000` formula. |
| `sunxi_clk_dump()` | `sunxi_clk_dump` | Same CPU source switch (`CPU_AXI_CFG` bits 24-26), CPU PLL formula, PLL_PERI 2X/1X/800M and PLL_DDR branches. |

## 2. Deliberate differences

- **Dump guard and call site.** Upstream compiles `sunxi_clk_dump` under
  `CONFIG_ENABLE_CPU_FREQ_DUMP`. The port always compiles it, and `main` calls it before
  `dram.init()`. As a result `CLK: PLL_ddr=` reports the pre-DRAM value (432 MHz in the HIL
  log); the DRAM initialisation that follows sets 792 MHz. This is a diagnostic-order
  choice, not an initialisation difference. Moving the dump after `dram.init()` would make
  the line report the final PLL_DDR, if that matters later.
- **Printing.** Upstream uses `debug(...)`; the port uses `print!` on USART0. The text and
  values match.
- **CCU constant names.** Upstream uses `CCU_*` names from `reg-ccu.h`. The port's `ccu`
  module keeps the repo's existing `PLL_*_REG`/`*_REG` names; the values were checked
  against `reg-ccu.h` and map one-to-one. ADR-0007 asks for upstream `snake_case` for
  shared vocabulary; the function names above follow it. A strict `CCU_*` prefix rename is
  a mechanical follow-up, not a behavioural gap.

## 3. Hardware confirmation

HIL on T113-S3 (FEL, USART0 115200, no `xfel ddr`; see `docs/agents/hil.md`) after the
#16 merge printed:

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

No `CLK: init timeout` line; UART and DRAM come up and the LED loop runs.

---

## Sources

1. Upstream `sunxi_clk.c` at `5380c00`:
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_clk.c>
2. `rawboot/src/clk/mod.rs` (this repo).
3. `rawboot/src/ccu/mod.rs` (this repo).
4. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
5. Issue #16, "Parity тактов: ccu + clk" (clock parity: ccu + clk).
