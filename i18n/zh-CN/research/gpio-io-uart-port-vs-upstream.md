# `rawboot` 的 GPIO/IO/UART 移植 vs 当前 upstream `szemzoa/awboot`

调研时间：2026 年。目标：Allwinner T113-S3。upstream 固定在 `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`（2025-11-21）。比较的文件：
`arch/arm32/mach-t113s3/sunxi_gpio.c`、`arch/arm32/mach-t113s3/sunxi_gpio.h`、
`arch/arm32/mach-t113s3/sunxi_usart.c`、`arch/arm32/mach-t113s3/sunxi_usart.h`、
`include/io.h`、`arch/arm32/mach-t113s3/arch_timer.c`（`sdelay`），外加 `board.c`
中的板级引脚表和 `sunxi_sdhci.c` 中的 SD 引脚设置 [1]-[8]。本地一侧为
`sunxi-pl/src/gpio.rs`、`sunxi-pl/src/io.rs`、`sunxi-pl/src/cpu.rs`、
`rawboot/src/uart/mod.rs`、`rawboot/src/sdhci/mod.rs` 和 `rawboot/src/main.rs`；本地
代码按符号引用，而不是按行号（见 `docs/agents/domain.md`）。在 issue #29 将共享硬件层
移入 `sunxi-pl` crate（ADR-0010）之后更新。

本笔记是 `research/dram-port-vs-upstream.md` 和 `research/clk-port-vs-upstream.md` 的
GPIO/IO/UART 对口篇；它是 issue #17 的静态 parity 检查清单。

---

## 1. 逐函数比较

| upstream | 本地（#17 之后） | 状态 |
| --- | --- | --- |
| `_port_base_get` [1:24] | `port_base_get` | 相同 switch，基址 `0x0200_0030`/`0060`/`0090`/`00c0`/`00f0`/`0120`（具名常量）。 |
| `_pin_num` [1:52] | `pin_num` | 相同 `pin & ((1 << 5) - 1)`。 |
| `sunxi_gpio_init` [1:57] | `sunxi_gpio_init` | 相同 `CFG0 + ((pin >> 3) << 2)`、掩码 4 位字段、`if cfg` 守卫。 |
| `sunxi_gpio_write` [1:72] | `sunxi_gpio_write` | 相同的 `DAT` 读-改-写、`!!value`。 |
| `sunxi_gpio_read` [1:84] | `sunxi_gpio_read` | 相同 `!!(DAT & bit)`。 |
| `sunxi_gpio_set_pull` [1:94] | `sunxi_gpio_set_pull` | 相同 `PUL0 + ((pin >> 4) << 2)` 和 upstream 的 `v` 映射（up→1、down→2、none→0）。 |
| `sunxi_gpio_set_drive_lvl` [1:126] | `sunxi_gpio_set_drive_lvl` | 相同 `DRV0 + ((pin >> 4) << 2)`，`lvl &= 3`。 |
| `read8/16/32/64`、`write8/16/32/64` [5] | 相同名称 | 增加了缺失的宽度；`read32`/`write32` 未变。 |
| `sdelay` [6:60] | `sdelay` | 相同 `subs`/`bne` 递减循环。 |
| `get_arch_counter` [6:16] | `get_arch_counter` | 相同 `mrrc p15, 0, …, c14`，64 位计数器。 |
| `time_ms` [6:26] | `time_ms` | 相同：counter / 24000。 |
| `time_us` [6:34] | `time_us` | 相同：counter / 24。 |
| `udelay` [6:39] | `udelay` | 有差异：upstream 在 `time_us()` 上循环，我们的实现将原始 counter 与截止值比较（第 3 节）。 |
| `sunxi_usart_init` [3:24] | `sunxi_usart_init` | 顺序相同：TX/RX mux、上拉、门控、复位、`div = 1500000/baud`、`LCR/DLL/LCR/FCR`。 |
| `sunxi_usart_putc` [3:61] | `sunxi_usart_putc` | 相同 TFNF 等待、THR 写入、BUSY 等待。 |
| `sunxi_usart_put` [3:72] | `sunxi_usart_put` | 相同字节循环。 |
| `sunxi_usart_get` [3:80] | `sunxi_usart_get` | 相同 RFNE 等待、RBR 读取。 |
| `sunxi_usart_t` [4:10] | `SunxiUsart` | 相同字段：`id`、`gpio_tx`、`gpio_rx`。 |
| `usart0_dbg`（board）[7:10] | `usart0_dbg` | 相同引脚/mux：TX PE2 MUX6、RX PE3 MUX6。 |
| `board_init_led` PD22 [7:76,99] | `main` PD22 | 相同 `GPIO_OUTPUT` 然后写 0。 |
| SDHCI 引脚 PF2/3/1/0/5/4 MUX2 [7:49-54] | `sdhci_t::default` | 相同引脚/mux；`sdhci_t::init` 中相同的 `init`+`set_pull(up)` 顺序 [8:942-958]。 |

## 2. #17 中修复的差异

1. **UART RX 引脚错误。** 旧的 `UartHandler::default` 将 `rx` 设为
   `gpio::Port::E, pin: 2`（`PE2`，即 TX 引脚），而名义上选择的是 `pe3::UART0_RX`
   （见 `85e3e1e`），而 upstream `usart0_dbg.gpio_rx` 是 `GPIO_PIN(PORTE, 3)` [7:13]。
   在 `usart0_dbg` 中修复：`gpio_rx.pin = gpio_pin(PORTE, 3)`。
2. **GPIO API 与名称。** 旧移植使用 `Gpio` 结构体加每引脚 mux 模块（`pd22`、`pe2`、
   `pe3`、`pf0..pf5`），并在一个 `init` 中合并 mux+pull。upstream 使用打包的 `gpio_t`
   （`port << 5 | pin`）、`gpio_mux_t`、`GPIO_INPUT`/`GPIO_OUTPUT`/`GPIO_PERIPH_MUXn`/
   `GPIO_DISABLED` 常量，以及分开的 `sunxi_gpio_init` / `sunxi_gpio_set_pull` [2]。
   已替换为 upstream 的函数/打包词汇，并按 ADR-0007 使用 Rust 惯用类型（`GpioPin`、
   `GpioMux`、`GpioPull`）。
3. **Pull 编码错误。** 旧的 `Pull::UP = 0x01`、`Pull::DOWN = 0x10`（是位位置，不是
   值），且 `init` 清除了整个 4 位 pull 字段。upstream 将 pull 常量映射为 `v`，并仅
   掩码/设置 `v` 位，因此下拉写入 `0b10` [1:101-123]。已替换为 `GpioPull`
   （`Up`/`Down`/`None`）以及 `sunxi_gpio_set_pull` 中的 upstream 映射。
4. **UART 初始化序列。** 旧 `init` 硬编码 UART0 和除数 13，写入 IER/MCR 和
   `FCR = 0xf7`，并从旧值重建 LCR。upstream 接受 `baudrate`，计算
   `div = 1500000 / baudrate`（向下取整，最小 1），并执行 `LCR = (1<<7)|3; DLL = div;
   LCR = 3; FCR = 1` [3:48-58]。已在 `sunxi_usart_init` 中替换为该序列。
5. **`io` 宽度。** 旧模块只有 `read32`/`write32`。upstream 的 `include/io.h` 提供
   8/16/32/64 位访问器 [5]；增加了缺失的 `read16`/`read64`/`write8`/`write16`/
   `write64`。
6. **`sdelay`。** 旧 `sdelay` 是 `write_volatile` 自旋循环（见 `85e3e1e`）。upstream
   的 `arch_timer.c` 使用 `subs`/`bne` 循环 [6:60-67]；已转录到 `io::sdelay`。
7. **板级引脚 mux。** LED PD22 输出和 USART0 PE2/PE3 的 MUX6 现在与 upstream 的
   `board.c` 一致；SDHCI 保留其 PF MUX2 集以及 upstream 的 init/pull 顺序 [7][8]。

## 3. 有意差异

- **打印。** upstream 通过 `debug(...)`/xformat 打印；移植版保留 `core::fmt`，并将
  `print!` 通过 `usart0_dbg` 上的 `UartConsole` 路由。文本和值未变。
- **无按 SoC 的路径。** 未转录 T113-S4/V851s 引脚表；只有 T113-S3 在范围内
  （ADR-0007）。
- **类型。** 函数、常量和全局名称遵循 upstream（`sunxi_gpio_*`、`sunxi_usart_*`、
  `usart0_dbg`、`GPIO_PERIPH_MUX*`）；Rust 类型及其构造函数按 ADR-0007 保持惯用
  （`GpioPin`、`GpioMux`、`GpioPull`、`SunxiUsart`、`gpio_pin`），而寄存器值和步骤
  顺序保持 upstream 的。
- **`udelay` 使用原始 counter 计数。** upstream 在 `time_us()` 上循环 [6:39-46]；
  我们的实现读取一次 counter，计算截止值，并等待直到 counter 超过它。等待长度相同，
  但我们的实现比较绝对值，因此无法承受 64 位 counter 回绕，而 upstream 的减法可以。
  从 #29 之前的移植原样继承；修复它是单独的 parity 问题。

## 4. 硬件确认

在 #17 变更后，T113-S3 上的 HIL（FEL，USART0 115200 8N1；
`scripts/hil-check.sh target/rawboot-fel.bin iss17`）打印了：

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

脚本报告了 `PASS: UART assertions`（`Hello`、`CLK:`、`DRAM simple test OK`、
`S=0x`）、一个 `Hello`，且无重置循环。PD22 LED 闪烁由人工检查。

---

## 来源

1. upstream `arch/arm32/mach-t113s3/sunxi_gpio.c`，位于 `5380c00`：
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_gpio.c>
2. upstream `arch/arm32/mach-t113s3/sunxi_gpio.h`，位于 `5380c00`。
3. upstream `arch/arm32/mach-t113s3/sunxi_usart.c`，位于 `5380c00`。
4. upstream `arch/arm32/mach-t113s3/sunxi_usart.h`，位于 `5380c00`。
5. upstream `include/io.h`，位于 `5380c00`。
6. upstream `arch/arm32/mach-t113s3/arch_timer.c`，位于 `5380c00`。
7. upstream `board.c`，位于 `5380c00`。
8. upstream `arch/arm32/mach-t113s3/sunxi_sdhci.c`，位于 `5380c00`。
9. ADR-0007，`docs/adr/0007-align-behaviour-to-upstream.md`。
10. Issue #17，“Parity GPIO/IO/UART”。
