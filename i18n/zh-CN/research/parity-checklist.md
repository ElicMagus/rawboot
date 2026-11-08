# upstream parity 清单（loader 模块）

调研时间：2026 年。issue #14 的静态索引：每个范围内的 loader 模块一行，指向逐函数差异
清单。Parity 意味着与 upstream 的行为等价——相同的步骤顺序、常量、分支和表——而表达
保持我们惯用的 Rust（ADR-0007）。

upstream 锚点：`szemzoa/awboot` 的 `main`
`5380c00fc67c975433f25c57fb481aa2b91aebf8`（2025-11-21）。用
`scripts/fetch-upstream.sh` 获取它；按文件和行引用 upstream，按符号引用我们的代码
（`docs/agents/domain.md`）。本索引仅引用文件；行级引用在各链接的清单中。

## 范围内的模块

| 模块 | Upstream 源 | 本地代码 | 差异清单 | 工单 |
| --- | --- | --- | --- | --- |
| `ccu` | `arch/arm32/mach-t113s3/reg-ccu.h` | `rawboot/src/ccu/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `clk` | `arch/arm32/mach-t113s3/sunxi_clk.c` | `rawboot/src/clk/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `gpio` | `arch/arm32/mach-t113s3/sunxi_gpio.[ch]`、`board.c` 引脚表 | `sunxi-pl/src/gpio.rs`（调用点位于 `rawboot/src/sdhci/mod.rs`、`rawboot/src/main.rs`） | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `io` | `include/io.h`、`arch/arm32/mach-t113s3/arch_timer.c`（仅 `sdelay`） | `sunxi-pl/src/io.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `cpu`（定时器） | `arch/arm32/mach-t113s3/arch_timer.c` | `sunxi-pl/src/cpu.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17, #29 |
| `uart` | `arch/arm32/mach-t113s3/sunxi_usart.[ch]`、`board.c`（`usart0_dbg`） | `rawboot/src/uart/mod.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `dram` | `arch/arm32/mach-t113s3/dram.[ch]`、`reg-dram.h` | `rawboot/src/dram/mod.rs` | `research/dram-port-vs-upstream.md` | #15, #18 |
| `sdhci` | `arch/arm32/mach-t113s3/sunxi_sdhci.[ch]` | `rawboot/src/sdhci/mod.rs` | `research/sdhci-port-vs-upstream.md` | #19 |
| `sdcard` | `arch/arm32/mach-t113s3/sdmmc.[ch]` | `rawboot/src/sdcard/mod.rs` | `research/sdcard-port-vs-upstream.md` | #20 |

每个清单都包含 upstream 函数面以及保留的有意差异；#17/#19/#20 笔记还列举了其工单修复的
差异。DRAM 笔记还记录了旧移植背后的重构前历史（`mctl_hal.c` fork 点）。

## 有意差异

这是 parity 决策的一部分，不是要“修复”的差距；链接的章节拥有细节：

- **`dram_get_top` / `dram_top` 缺失** —— `research/dram-port-vs-upstream.md`（剩余
  差异）。只有在 payload 需要 top 地址时才添加（issue #14）。
- **T113-S4/V851s 路径缺失** —— ADR-0007。仅针对 T113-S3。
- **`abort()` 缺失** —— `research/dram-port-vs-upstream.md`（剩余差异）。
- **模块布局和 C 惯用法保持我们的** —— ADR-0007。不重现 upstream 的 `common.h`、
  `xformat` 和宏 `debug.h`。
- **`ccu` 保留 `PLL_*_REG` 常量名。** 值已对照 `reg-ccu.h` 检查；严格的 `CCU_*`
  前缀重命名是机械性的后续工作，不是行为差距（`research/clk-port-vs-upstream.md`，
  有意差异）。
- **延后的 upstream 驱动** —— #21 跟踪 DMA 引擎、SPI/SPI-NAND、看门狗以及
  cache/异常辅助。

## 验证

- 静态 oracle：上述清单加上固定的 upstream 源码。
- 执行接缝：T113-S3 上的 HIL 冒烟测试（`docs/agents/hil.md`）——UART 初始化日志加
  一个可工作的 blink payload。
