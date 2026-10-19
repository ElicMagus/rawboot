# `rawboot` GPIO/IO/UART port vs current upstream `szemzoa/awboot`

Researched: 2026. Target: Allwinner T113-S3. Upstream is pinned to `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21). Files compared:
`arch/arm32/mach-t113s3/sunxi_gpio.c`, `arch/arm32/mach-t113s3/sunxi_gpio.h`,
`arch/arm32/mach-t113s3/sunxi_usart.c`, `arch/arm32/mach-t113s3/sunxi_usart.h`,
`include/io.h`, `arch/arm32/mach-t113s3/arch_timer.c` (`sdelay`), plus the board pin
tables in `board.c` and the SD pin setup in `sunxi_sdhci.c` [1]-[8]. The local side is
`rawboot/src/gpio/mod.rs`, `rawboot/src/io/mod.rs`, `rawboot/src/uart/mod.rs`,
`rawboot/src/sdhci/mod.rs` and `rawboot/src/main.rs` [9]-[13].

This note is the GPIO/IO/UART counterpart of `research/dram-port-vs-upstream.md` and
`research/clk-port-vs-upstream.md`; it is the static parity checklist for issue #17.

---

## 1. Function-by-function comparison

| Upstream | Local (after #17) | Status |
| --- | --- | --- |
| `_port_base_get` [1:24] | `port_base_get` [9:55] | Same switch, bases `0x0200_0030`/`0060`/`0090`/`00c0`/`00f0`/`0120`. |
| `_pin_num` [1:52] | `pin_num` [9:65] | Same `pin & ((1 << 5) - 1)`. |
| `sunxi_gpio_init` [1:57] | `sunxi_gpio_init` [9:72] | Same `CFG0 + ((pin >> 3) << 2)`, masked 4-bit field, `if cfg` guard. |
| `sunxi_gpio_write` [1:72] | `sunxi_gpio_write` [9:85] | Same read-modify-write of `DAT`, `!!value`. |
| `sunxi_gpio_read` [1:84] | `sunxi_gpio_read` [9:95] | Same `!!(DAT & bit)`. |
| `sunxi_gpio_set_pull` [1:94] | `sunxi_gpio_set_pull` [9:103] | Same `PUL0 + ((pin >> 4) << 2)` and upstream `v` map (UP→1, DOWN→2, NONE→0). |
| `sunxi_gpio_set_drive_lvl` [1:126] | `sunxi_gpio_set_drive_lvl` [9:120] | Same `DRV0 + ((pin >> 4) << 2)`, `lvl &= 3`. |
| `read8/16/32/64`, `write8/16/32/64` [5] | same names [10] | Added the missing widths; `read32`/`write32` unchanged. |
| `sdelay` [6:60] | `sdelay` [10:58] | Same `subs`/`bne` decrement loop. |
| `sunxi_usart_init` [3:24] | `sunxi_usart_init` [11:46] | Same order: TX/RX mux, pull-ups, gate, reset, `div = 1500000/baud`, `LCR/DLL/LCR/FCR`. |
| `sunxi_usart_putc` [3:61] | `sunxi_usart_putc` [11:78] | Same TFNF wait, THR write, BUSY wait. |
| `sunxi_usart_put` [3:72] | `sunxi_usart_put` [11:86] | Same byte loop. |
| `sunxi_usart_get` [3:80] | `sunxi_usart_get` [11:92] | Same RFNE wait, RBR read. |
| `sunxi_usart_t` [4:10] | `sunxi_usart_t` [11:28] | Same fields: `id`, `gpio_tx`, `gpio_rx`. |
| `usart0_dbg` (board) [7:10] | `usart0_dbg` [11:34] | Same pins/muxes: TX PE2 MUX6, RX PE3 MUX6. |
| `board_init_led` PD22 [7:76,99] | `main` PD22 [12:28] | Same `GPIO_OUTPUT` then write 0. |
| SDHCI pins PF2/3/1/0/5/4 MUX2 [7:49-54] | `sdhci_t::default` [13:201] | Same pins/muxes; same `init`+`set_pull(UP)` order in `init` [8:942-958][13:536]. |

## 2. Divergences fixed in #17

1. **UART RX pin was wrong.** `UartHandler::default` set `rx` to `gpio::Port::E, pin: 2`
   (`PE2`, the TX pin) while nominally selecting `pe3::UART0_RX` [local @ `85e3e1e`:
   `rawboot/src/uart/mod.rs:95-100`], whereas upstream `usart0_dbg.gpio_rx` is
   `GPIO_PIN(PORTE, 3)` [7:13]. Fixed: `gpio_rx.pin = gpio_pin(PORTE, 3)` [11:40].
2. **GPIO API and names.** The old port used a `Gpio` struct plus per-pin mux modules
   (`pd22`, `pe2`, `pe3`, `pf0..pf5`) and combined mux+pull in one `init`. Upstream uses a
   packed `gpio_t` (`port << 5 | pin`), `gpio_mux_t`, the `GPIO_INPUT`/`GPIO_OUTPUT`/
   `GPIO_PERIPH_MUXn`/`GPIO_DISABLED` constants, and separate `sunxi_gpio_init` /
   `sunxi_gpio_set_pull` [2]. Replaced by the upstream names and packing [9].
3. **Pull encoding was wrong.** Old `Pull::UP = 0x01`, `Pull::DOWN = 0x10` (a bit position,
   not a value) and `init` cleared the whole 4-bit pull field. Upstream maps the pull
   constant to `v` and only masks/sets the `v` bits, so `GPIO_PULL_DOWN` writes `0b10`
   [1:101-123]. Replaced by the upstream `gpio_pull_t` constants and mapping [9:103].
4. **UART init sequence.** Old `init` hard-coded UART0 and divisor 13, wrote IER/MCR and
   `FCR = 0xf7`, and rebuilt LCR from the old value. Upstream takes `baudrate`, computes
   `div = 1500000 / baudrate` (floor, min 1), and does `LCR = (1<<7)|3; DLL = div;
   LCR = 3; FCR = 1` [3:48-58]. Replaced by the upstream sequence [11:63-75].
5. **`io` widths.** The old module only had `read32`/`write32`. Upstream `include/io.h`
   provides 8/16/32/64-bit accessors [5]; added the missing ones [10:6-51].
6. **`sdelay`.** Old `sdelay` was a `write_volatile` spin loop [local @ `85e3e1e`:
   `rawboot/src/io/mod.rs:19-35`]. Upstream `arch_timer.c` uses a `subs`/`bne` loop
   [6:60-67]; transcribed [10:58].
7. **Board pin muxes.** LED PD22 output and USART0 PE2/PE3 at MUX6 now match upstream
   `board.c`; SDHCI keeps its PF MUX2 set and upstream's init/pull order [7][8].

## 3. Deliberate differences

- **Printing.** Upstream prints through `debug(...)`/xformat; the port keeps `core::fmt`
  and routes `print!` through `UartConsole` on `usart0_dbg` [11:101-108]. Text and values
  are unchanged.
- **No per-SoC paths.** T113-S4/V851s pin tables are not transcribed; only T113-S3 is in
  scope (ADR-0007).
- **Types.** `gpio_mux_t`/`sunxi_usart_t` keep the upstream names; the Rust code keeps its
  own `Result`-free bare-metal shape and register constants.

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
9. `rawboot/src/gpio/mod.rs` (this repo, after #17).
10. `rawboot/src/io/mod.rs` (this repo, after #17).
11. `rawboot/src/uart/mod.rs` (this repo, after #17).
12. `rawboot/src/main.rs` (this repo, after #17).
13. `rawboot/src/sdhci/mod.rs` (this repo, after #17).
14. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
15. Issue #17, "Parity GPIO/IO/UART".
