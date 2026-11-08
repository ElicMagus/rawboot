# `rawboot` 的时钟移植 vs 当前 upstream `szemzoa/awboot` 的 `sunxi_clk.c`

调研时间：2026 年。目标：Allwinner T113-S3。upstream 固定在 `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`（2025-11-21），文件
`arch/arm32/mach-t113s3/sunxi_clk.c` [1]。本地一侧为 `rawboot/src/clk/mod.rs` 和
`rawboot/src/ccu/mod.rs` [2][3]。

本笔记是 `research/dram-port-vs-upstream.md` 的时钟对口篇；它的存在是为了让 parity
评审不必重新获取 upstream 源码。

---

## 1. 逐函数比较

| upstream `sunxi_clk.c`（5380c00） | `rawboot/src/clk/mod.rs` | 状态 |
| --- | --- | --- |
| `wait_for_bit_set(addr, mask)` | `wait_for_bit_set` | 相同：2,000,000 次自旋，`sdelay(1)`，记录首个失败地址。 |
| `sunxi_clk_get_fail_addr()` | `sunxi_clk_get_fail_addr` | 相同，通过 `AtomicU32` 而非静态变量。 |
| `set_pll_cpux_axi()` | `set_pll_cpux_axi` | 顺序相同：PLL_PERI(1x) 源、`sdelay(10)`、门控/LDO、`41 << 8`、锁定/使能、等待第 28 位、`sdelay(20)`、关闭门控、最终 `CPU_AXI_CFG` 且 `DIV2=1`。最终写入省略了显式的 `0 << 16`，等价于 upstream。 |
| `set_pll_periph0()` | `set_pll_periph0` | 相同，包括第 31 位置位时的提前返回。 |
| `set_ahb()` | `set_ahb` | 相同 `PSI_CLK_REG = (2 << 0) | (0 << 8) | (0x03 << 24)`。 |
| `set_apb()` | `set_apb` | 相同 `APB0_CLK_REG = (2 << 0) | (1 << 8) | (0x03 << 24)`。 |
| `set_dma()` | `set_dma` | 相同的复位/门控序列。 |
| `set_mbus()` | `set_mbus` | 相同的复位和 `0x00000d87` 门控掩码。 |
| `set_module(addr)` | `set_module` | 相同的使能/锁定/等待序列。 |
| `sunxi_clk_init()` | `sunxi_clk_init` | 顺序相同：CPUX/AXI、PERI0、AHB、APB0、DMA、MBUS，然后 PERI0、VIDEO0、VIDEO1、VE、AUDIO0、AUDIO1。 |
| `sunxi_clk_get_peri1x_rate()` | `sunxi_clk_get_peri1x_rate` | 相同 `(24 * plln / (pllm * p0)) >> 1 * 1000 * 1000` 公式。 |
| `sunxi_clk_dump()` | `sunxi_clk_dump` | 相同的 CPU 源 switch（`CPU_AXI_CFG` 第 24-26 位）、CPU PLL 公式、PLL_PERI 2X/1X/800M 和 PLL_DDR 分支。 |

## 2. 有意差异

- **Dump 守卫与调用点。** upstream 在 `CONFIG_ENABLE_CPU_FREQ_DUMP` 下编译
  `sunxi_clk_dump`。移植版总是编译它，并且 `main` 在 `dram.init()` 之前调用它。因此
  `CLK: PLL_ddr=` 报告的是 DRAM 之前的值（HIL 日志中为 432 MHz）；随后的 DRAM 初始化
  设为 792 MHz。这是诊断顺序的选择，不是初始化差异。如果以后有影响，将 dump 移到
  `dram.init()` 之后会让该行报告最终的 PLL_DDR。
- **打印。** upstream 使用 `debug(...)`；移植版在 USART0 上使用 `print!`。文本和值一致。
- **CCU 常量名称。** upstream 使用 `reg-ccu.h` 中的 `CCU_*` 名称。移植版的 `ccu`
  模块保留仓库现有的 `PLL_*_REG`/`*_REG` 名称；这些值已对照 `reg-ccu.h` 检查，一一
  对应。ADR-0007 要求共享词汇使用 upstream `snake_case`；上述函数名遵循这一点。严格的
  `CCU_*` 前缀重命名是机械性的后续工作，不是行为差距。

## 3. 硬件确认

在 #16 合并后，T113-S3 上的 HIL（FEL，USART0 115200，无 `xfel ddr`；见
`docs/agents/hil.md`）打印了：

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

没有 `CLK: init timeout` 行；UART 和 DRAM 正常启动，LED 循环运行。

---

## 来源

1. upstream `sunxi_clk.c`，位于 `5380c00`：
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_clk.c>
2. `rawboot/src/clk/mod.rs`（本仓库）。
3. `rawboot/src/ccu/mod.rs`（本仓库）。
4. ADR-0007，`docs/adr/0007-align-behaviour-to-upstream.md`。
5. Issue #16，“Parity 时钟：ccu + clk”。
