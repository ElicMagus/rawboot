# `rawboot` GPIO/IO/UART port vs current upstream `szemzoa/awboot`

Researched: 2026. Target: Allwinner T113-S3. Upstream is pinned to `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21). Files compared:
`arch/arm32/mach-t113s3/sunxi_gpio.c`, `arch/arm32/mach-t113s3/sunxi_gpio.h`,
`arch/arm32/mach-t113s3/sunxi_usart.c`, `arch/arm32/mach-t113s3/sunxi_usart.h`,
`include/io.h`, `arch/arm32/mach-t113s3/arch_timer.c` (`sdelay`), plus the board pin
tables in `board.c` and the SD pin setup in `sunxi_sdhci.c` [1]-[8]. The local side is
`sunxi-pl/src/gpio.rs`, `sunxi-pl/src/io.rs`, `sunxi-pl/src/cpu.rs`, `rawboot/src/uart/mod.rs`,
`rawboot/src/sdhci/mod.rs` and `rawboot/src/main.rs`; local code is cited by symbol, not by
line (see `docs/agents/domain.md`). Updated after issue #29, which moved the shared
hardware layer into the `sunxi-pl` crate (ADR-0010).

This note is the GPIO/IO/UART counterpart of `research/dram-port-vs-upstream.md` and
`research/clk-port-vs-upstream.md`; it is the static parity checklist for issue #17.

---

## 1. Function-by-function comparison

| Upstream | Local (after #17) | Status |
| --- | --- | --- |
| `_port_base_get` [1:24] | `port_base_get` | Same switch, bases `0x0200_0030`/`0060`/`0090`/`00c0`/`00f0`/`0120` (named constants). |
| `_pin_num` [1:52] | `pin_num` | Same `pin & ((1 << 5) - 1)`. |
| `sunxi_gpio_init` [1:57] | `sunxi_gpio_init` | Same `CFG0 + ((pin >> 3) << 2)`, masked 4-bit field, `if cfg` guard. |
| `sunxi_gpio_write` [1:72] | `sunxi_gpio_write` | Same read-modify-write of `DAT`, `!!value`. |
| `sunxi_gpio_read` [1:84] | `sunxi_gpio_read` | Same `!!(DAT & bit)`. |
| `sunxi_gpio_set_pull` [1:94] | `sunxi_gpio_set_pull` | Same `PUL0 + ((pin >> 4) << 2)` and upstream `v` map (up→1, down→2, none→0). |
| `sunxi_gpio_set_drive_lvl` [1:126] | `sunxi_gpio_set_drive_lvl` | Same `DRV0 + ((pin >> 4) << 2)`, `lvl &= 3`. |
| `read8/16/32/64`, `write8/16/32/64` [5] | same names | Added the missing widths; `read32`/`write32` unchanged. |
| `sdelay` [6:60] | `sdelay` | Same `subs`/`bne` decrement loop. |
| `get_arch_counter` [6:16] | `get_arch_counter` | Same `mrrc p15, 0, …, c14`, 64-bit counter. |
| `time_ms` [6:26] | `time_ms` | Same: counter / 24000. |
| `time_us` [6:34] | `time_us` | Same: counter / 24. |
| `udelay` [6:39] | `udelay` | Diverges: upstream loops on `time_us()`, ours compares the raw counter against a deadline (section 3). |
| `sunxi_usart_init` [3:24] | `sunxi_usart_init` | Same order: TX/RX mux, pull-ups, gate, reset, `div = 1500000/baud`, `LCR/DLL/LCR/FCR`. |
| `sunxi_usart_putc` [3:61] | `sunxi_usart_putc` | Same TFNF wait, THR write, BUSY wait. |
| `sunxi_usart_put` [3:72] | `sunxi_usart_put` | Same byte loop. |
| `sunxi_usart_get` [3:80] | `sunxi_usart_get` | Same RFNE wait, RBR read. |
| `sunxi_usart_t` [4:10] | `SunxiUsart` | Same fields: `id`, `gpio_tx`, `gpio_rx`. |
| `usart0_dbg` (board) [7:10] | `usart0_dbg` | Same pins/muxes: TX PE2 MUX6, RX PE3 MUX6. |
| `board_init_led` PD22 [7:76,99] | `main` PD22 | Same `GPIO_OUTPUT` then write 0. |
| SDHCI pins PF2/3/1/0/5/4 MUX2 [7:49-54] | `sdhci_t::default` | Same pins/muxes; same `init`+`set_pull(up)` order in `sdhci_t::init` [8:942-958]. |

## 2. Divergences fixed in #17

1. **UART RX pin was wrong.** The old `UartHandler::default` set `rx` to `gpio::Port::E, pin: 2`
   (`PE2`, the TX pin) while nominally selecting `pe3::UART0_RX` (see `85e3e1e`), whereas
   upstream `usart0_dbg.gpio_rx` is `GPIO_PIN(PORTE, 3)` [7:13]. Fixed in `usart0_dbg`:
   `gpio_rx.pin = gpio_pin(PORTE, 3)`.
2. **GPIO API and names.** The old port used a `Gpio` struct plus per-pin mux modules
   (`pd22`, `pe2`, `pe3`, `pf0..pf5`) and combined mux+pull in one `init`. Upstream uses a
   packed `gpio_t` (`port << 5 | pin`), `gpio_mux_t`, the `GPIO_INPUT`/`GPIO_OUTPUT`/
   `GPIO_PERIPH_MUXn`/`GPIO_DISABLED` constants, and separate `sunxi_gpio_init` /
   `sunxi_gpio_set_pull` [2]. Replaced by the upstream function/packing vocabulary, with
   Rust-idiomatic types (`GpioPin`, `GpioMux`, `GpioPull`) per ADR-0007.
3. **Pull encoding was wrong.** Old `Pull::UP = 0x01`, `Pull::DOWN = 0x10` (a bit position,
   not a value) and `init` cleared the whole 4-bit pull field. Upstream maps the pull
   constant to `v` and only masks/sets the `v` bits, so pull-down writes `0b10`
   [1:101-123]. Replaced by `GpioPull` (`Up`/`Down`/`None`) and the upstream mapping in
   `sunxi_gpio_set_pull`.
4. **UART init sequence.** Old `init` hard-coded UART0 and divisor 13, wrote IER/MCR and
   `FCR = 0xf7`, and rebuilt LCR from the old value. Upstream takes `baudrate`, computes
   `div = 1500000 / baudrate` (floor, min 1), and does `LCR = (1<<7)|3; DLL = div;
   LCR = 3; FCR = 1` [3:48-58]. Replaced by that sequence in `sunxi_usart_init`.
5. **`io` widths.** The old module only had `read32`/`write32`. Upstream `include/io.h`
   provides 8/16/32/64-bit accessors [5]; added the missing `read16`/`read64`/`write8`/
   `write16`/`write64`.
6. **`sdelay`.** Old `sdelay` was a `write_volatile` spin loop (see `85e3e1e`). Upstream
   `arch_timer.c` uses a `subs`/`bne` loop [6:60-67]; transcribed in `io::sdelay`.
7. **Board pin muxes.** LED PD22 output and USART0 PE2/PE3 at MUX6 now match upstream
   `board.c`; SDHCI keeps its PF MUX2 set and upstream's init/pull order [7][8].

## 3. Deliberate differences

- **Printing.** Upstream prints through `debug(...)`/xformat; the port keeps `core::fmt`
  and routes `print!` through `UartConsole` on `usart0_dbg`. Text and values are unchanged.
- **No per-SoC paths.** T113-S4/V851s pin tables are not transcribed; only T113-S3 is in
  scope (ADR-0007).
- **Types.** Function, constant and global names follow upstream (`sunxi_gpio_*`,
  `sunxi_usart_*`, `usart0_dbg`, `GPIO_PERIPH_MUX*`); Rust types and their constructors are
  idiomatic (`GpioPin`, `GpioMux`, `GpioPull`, `SunxiUsart`, `gpio_pin`) per ADR-0007,
  while register values and step order stay upstream's.
- **`udelay` counts the raw counter.** Upstream loops on `time_us()` [6:39-46]; ours reads
  the counter once, computes a deadline and waits until the counter passes it. The wait
  length is the same, but ours compares absolute values, so it does not survive the
  64-bit counter wrap, while upstream's subtraction does. Carried over unchanged from
  the pre-#29 port; fixing it is a separate parity question.

## 4. Hardware confirmation

HIL on T113-S3 (FEL, USART0 115200 8N1; `scripts/hil-check.sh target/rawboot-fel.bin iss17`)
after the #17 changes printed:

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

The script reported `PASS: UART assertions` (`Hello`, `CLK:`, `DRAM simple test OK`,
`S=0x`), one `Hello`, and no reset loop. The PD22 LED blink is checked by hand.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/sunxi_gpio.c` at `5380c00`:
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_gpio.c>
2. Upstream `arch/arm32/mach-t113s3/sunxi_gpio.h` at `5380c00`.
3. Upstream `arch/arm32/mach-t113s3/sunxi_usart.c` at `5380c00`.
4. Upstream `arch/arm32/mach-t113s3/sunxi_usart.h` at `5380c00`.
5. Upstream `include/io.h` at `5380c00`.
6. Upstream `arch/arm32/mach-t113s3/arch_timer.c` at `5380c00`.
7. Upstream `board.c` at `5380c00`.
8. Upstream `arch/arm32/mach-t113s3/sunxi_sdhci.c` at `5380c00`.
9. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
10. Issue #17, "Parity GPIO/IO/UART".
