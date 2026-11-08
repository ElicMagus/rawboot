# `rawboot` 的 DRAM 移植 vs 当前 upstream `szemzoa/awboot` 的 `dram.c`

调研时间：2026 年，状态在 issue #18 之后更新。目标：Allwinner T113-S3（DDR3，默认
792 MHz）。upstream 的论断均引用确切的一手来源（固定 SHA 加行号）。本地代码按符号引用，
依据 `docs/agents/domain.md`；本地文件会变化，所以行号会失效。

方括号中的参考编号对应文末的 **来源** 章节。

---

## 1. 摘要

- upstream 固定在 `szemzoa/awboot` 的 `main` HEAD
  `5380c00fc67c975433f25c57fb481aa2b91aebf8`（2025-11-21，“Merge pull request #41 ...
  psci-rebase”）[12]。最近一个触及 `arch/arm32/mach-t113s3/dram.c` 的提交是
  `385481ffab45089d90410cb67b93981a870edd7c`（2025-11-15）[4][9]。在该 SHA 下
  `dram.c` 为 1453 行 / 40,623 B [1]。
- 我们的 Rust 移植并非派生自 `dram.c` 的任何修订版本。它几乎是逐行移植自重构前的
  `arch/arm32/mach-t113s3/mctl_hal.c`，upstream 在提交 `381606b0d68f`（“import Andre
  Przywara's DRAM init rework code”，2023-01-17）中添加 `dram_sun20i_d1.c` 时**删除**了
  该文件 [5][6][7]。那些标志性的名字 `auto_cal_timing`、`auto_set_timing_para`、
  `dram_vol_set`、`sid_read_ldoB_cal`、`DRAMC_get_dram_size`、`dramc_simple_wr_test`
  只存在于 `mctl_hal.c` 中，而不在 `dram.c` 里 [6][1]。
- 因此，值得注意的差异是 upstream 在 fork 点之后所做的更改：2023-01-17 的 Przywara
  重构、2023-03-13 的 `auto_scan_dram_size` 重写，以及 2025-11-15 的重构。

### parity 工单之后的状态

- **`auto_scan_dram_size`** 现在是来自 `9c8b8e40` 的 per-rank 扫描器：独立的
  `rank1_addr`、per-rank 测试模式、row/bank/column 探测，以及 `dram_para2` rank 数量
  测试。已在 issue #18 中修复。
- 已从 `dram_voltage_set` 中移除**强制 `vol = 25`**；改用按类型的值（DDR2 → 47，
  DDR3 → 25，默认 → 0）。已在 issue #18 中修复。
- 已移除对高 16 位的 **MR 写入门控**；计算出的 `mr0..mr3` 与 upstream 一样被直接写入。
  已在 issue #18 中修复。
- **VTFCR** 在 OR 上 `0x300` 之前从 VTFCR 读取；旧的 ZQCR 读取已移除。已在 issue #15
  中修复。
- **函数与常量名称**遵循 upstream 的 snake_case（`dram_voltage_set`、
  `dram_enable_all_master`、`dramc_get_dram_size`、`dramc_simple_wr_test`、`ns_to_t`、
  `mctl_set_timing_params`、`ccu_set_pll_ddr_clk`、`MCTL_PHY_BASE`、`MCTL_COM_BASE`）。
  `DramPara` 类型保持 Rust 惯用风格，接收者为 `&mut self`，使扫描结果和 `dram_clk`
  得以传播。已在 issue #18 中修复。
- **`dram_get_top()` / `dram_top`** 仍有意缺失：payload 契约不需要 top 地址，父 issue
  将 API 推迟到确实需要时再加 [14]。
- `mctl_phy_ac_remapping` 中的 **T113-S4 chip-ID 例外**有意未移植：目标仅为 T113-S3 [14]。

### 结论

DRAM 模块现在在所有范围内行为差异上都与固定的 upstream 一致：auto-scan、电压选择、
模式寄存器写入、VTFCR 和术语。其余差异是有意为之：没有 `dram_get_top`，没有 T113-S4
路径，以及较旧的（对 DDR3 在数值上等价的）时序例程结构及其死代码 `dram_tpr0/1/2`
重建。

---

## 2. upstream 修订与历史

### 2.1 固定修订

| 项目 | 值 |
|---|---|
| upstream 仓库 | `szemzoa/awboot` |
| 分支 | `main` |
| 固定 SHA | `5380c00fc67c975433f25c57fb481aa2b91aebf8` |
| HEAD 日期 / 主题 | 2025-11-21 / “Merge pull request #41 from cornera-org/psci-rebase” [12] |
| `dram.c` | 1453 行，40,623 B [1] |
| `dram.h` | 78 行，2,276 B [2] |
| `reg-dram.h` | 122 行，4,053 B [3] |
| 最近的 `dram.c` 提交 | `385481ffab45089d90410cb67b93981a870edd7c`，2025-11-15 [4][9] |

当前 upstream 的文件头明确说明它是“a reworked version, to match the U-Boot coding
style”，基线“lifted from awboot ... de-compilation of some original Allwinner code
bits”[1:1-15]。

### 2.2 DRAM 源码的历史

GitHub 的按路径历史给出 `arch/arm32/mach-t113s3/dram.c` 的六个提交 [4]，其直接前身
`arch/arm32/mach-t113s3/dram_sun20i_d1.c` 有四个 [5]。包括 `mctl_hal.c` 的删除，相关
序列为：

| 提交 | 日期 | 主题 | 变更内容 |
|---|---|---|---|
| `23e5c393` | 2023-01-17（父提交） | — | 最后一个包含 `mctl_hal.c`（39265 B）的 upstream 树 [6][7] |
| `381606b0` | 2023-01-17 | import Andre Przywara's DRAM init rework code | **删除 `mctl_hal.c`**，新增 `dram_sun20i_d1.c`（1392 行）[7] |
| `63a45cbf` | 2023-02-16 | add DRAM controller register header and fix printf issue | 变更 463/451 行；新增 `reg-dram.h` [5] |
| `9deb6cee` | 2023-02-17 | add new ac remap and SUNXI_R_CPUCFG_BASE define | 将 AC 表重新格式化为十六进制（值相同），新增寄存器定义 [5] |
| `7ade271d` | 2023-03-01 | add v851s files ... | 将 `dram_sun20i_d1.c` 重命名为 `dram.c` [4] |
| `9c8b8e40` | 2023-03-13 | add t113-s4 support 256M SiP DRAM, no ac remap. **update t113-s3 auto_scan_dram_size func** | 重写 `auto_scan_dram_size`；为 AC remap 增加 chip-ID/fuse 分支 [8] |
| `e6c37db4`, `ed1be708` | 2023-03-13 | “replace with tab”，“fix tab” | 仅空白变更 [4] |
| `78a943aa` | 2023-12-09 | printf format checking, u32/uint32_t, warnings | 主要是 `%d` → `%" PRIu32 "` 和类型拼写 [4] |
| `385481ff` | 2025-11-15 | refactor sdmmc driver, create library for fatfs, fdt, loaders | 新增 `dram_top`/`dram_get_top`；大规模风格重构（213/204 行）[9] |

### 2.3 我们的移植匹配哪个修订？

历史上，不匹配任何 `dram.c` 修订版本。它匹配最后一个包含 `mctl_hal.c` 的树
（`23e5c393`，`381606b0` 的父提交）中的 `mctl_hal.c` [6][7]。证据：

- 函数清单：`auto_cal_timing`、`auto_set_timing_para`、`dram_vol_set`、
  `sid_read_ldoB_cal`、`DRAMC_get_dram_size`、`dramc_simple_wr_test`，以及
  `mctl_hal.c` 的 `auto_scan_dram_size` 函数体——全部存在于 `mctl_hal.c` 中，而且在
  #18 之前的每个 `dram.c` 修订版本中都缺失 [6][1]。
- 旧的 `vol = 25` 覆盖和 MR 写入门控只存在于 `mctl_hal.c` 中，而不在 `dram.c` 中
  [6][1:84-105,480-484]。
- VTFCR 写入读取寄存器 `0x140`（ZQCR），与 `mctl_hal.c` 完全一致；`dram.c` 读取
  VTFCR [6:1597-1600][1:1397-1398]。已在 #15 中修复。
- Rust 时序例程保留了旧的 SP 槽位注释（`s4`、`s8`、`80(sp)`、`s5`）和死代码
  `dram_tpr0/1/2` 重建，二者都存在于 `mctl_hal.c` 中，并在重构时被移除
  [6:183-355][1:190-505]。在 #18 中有意保留；见 §4.5。

移植所使用的确切 `mctl_hal.c` 修订版本无法从本地树证明，因为 Rust 代码没有携带来源
注释；`23e5c393` 是**最接近的存活 upstream 祖先**。精确匹配为**未验证（UNVERIFIED）**。

---

## 3. 函数映射表

upstream 名称来自 `5380c00` 处的 `dram.c` [1]；本地名称来自 `rawboot/src/dram/mod.rs`。

| upstream C（`dram.c` @5380c00） | 本地 Rust | 状态 |
|---|---|---|
| `sunxi_dram_init` [1:1420] | `DramPara::default` + `DramPara::init_dram` | 已合并；无 `dram_top` 副作用 |
| `init_DRAM` [1:1306] | `DramPara::init_dram` | 存在 |
| `dram_get_top` [1:117] | — | **有意缺失**，直到 payload 需要它 [14] |
| `dram_disable_all_master` [1:122] | `dram_disable_all_master` | 存在 |
| `dram_enable_all_master` [1:107] | `dram_enable_all_master` | 存在，在 #18 中重命名 |
| `dram_voltage_set` [1:84] | `dram_voltage_set` | 存在，无强制 25（#18） |
| `sid_read_ldoB_cal` [1:60] | `sid_read_ldo_b_cal` | 存在 |
| `mctl_sys_init` [1:555] | `mctl_sys_init` | 存在 |
| `ccu_set_pll_ddr_clk` [1:512] | `ccu_set_pll_ddr_clk` | 存在，在 #18 中重命名 |
| `mctl_com_init` [1:595] | `mctl_com_init` | 存在 |
| `mctl_core_init` [1:1070] | `mctl_core_init` | 存在，调用顺序相同 |
| `mctl_channel_init` [1:773] | `mctl_channel_init` | 存在 |
| `mctl_vrefzq_init` [1:1053] | `mctl_vrefzq_init` | 存在 |
| `mctl_phy_ac_remapping` [1:697] | `mctl_phy_ac_remapping` | 存在；T113-S4 分支有意省略 |
| `mctl_set_timing_params` [1:190] | `mctl_set_timing_params` | 存在，结构较旧；MR 写入在 #18 中对齐 |
| `ns_to_t` [1:47] | `ns_to_t` | 在 #18 中对齐 |
| `calculate_rank_size` [1:932] | 内联于 `dramc_get_dram_size` | 存在，已内联 |
| `DRAMC_get_dram_size` [1:949] | `dramc_get_dram_size` | 存在，在 #18 中重命名 |
| `auto_scan_dram_config` [1:1288] | `auto_scan_dram_config` | 存在 |
| `auto_scan_dram_rank_width` [1:1257] | `auto_scan_dram_rank_width` | 存在 |
| `auto_scan_dram_size` [1:1096] | `auto_scan_dram_size` | 存在，**自 #18 起为 per-rank 算法** |
| `dqs_gate_detect` [1:975] | `dqs_gate_detect` | 存在 |
| `eye_delay_compensation` [1:130] | `eye_delay_compensation` | 存在 |
| `dramc_simple_wr_test` [1:1016] | `dramc_simple_wr_test` | 存在，在 #18 中重命名 |
| `abort` [1:54] | — | **缺失**（Rust 代码从不调用它） |

`ns_to_t` 计算 `DIV_ROUND_UP((dram_clk/2) * ns, 1000)` [1:47-52]；Rust 辅助函数从
`dram_clk / 2` 计算相同的向上取整除法。

---

## 4. 算法差异

### 4.1 `auto_scan_dram_size`：对齐到 per-rank 扫描器（#18）

**旧移植** 是 `mctl_hal.c` 算法 [6:1287-1452]：

- rank 数量来自 `(dram_para2 & 0xf000) != 0`。
- 测试模式在 `SDRAM_BASE` 处写入一次。
- 对 `i = 11..17` 做行扫描，固定基址 `SDRAM_BASE + (1 << (i + 11))`。
- 一个存储 0/1 标志的 A22 bank 探测。
- 从 `SDRAM_BASE` 对 `i = 9..14` 做列扫描。
- rank 1 在 `0x3202000`/`0x3202004` 处编程。

**当前本地**（`auto_scan_dram_size`）转录 upstream HEAD [1:1096-1249]，其本身是
`9c8b8e40` 的重写 [8]：

- 仅当 `((dram_para2 >> 12) & 0xf) == 0x1` 时 `rank_count = 2`。
- 一个 per-rank 循环，`mc_work_mode = MCTL_COM_BASE + MCTL_COM_WORK_MODE0 + 4 * current_rank`。
- 对于 rank 1，将 `MCTL_COM_WORK_MODE0` 和 `MCTL_COM_WORK_MODE1` 重新编程为
  16-row/4-bank/512 B 页，并设置 `rank1_addr = SDRAM_BASE + (1 << 27)`（128 MiB）。
- 按 rank 写入测试模式；在 `rank1_addr` 处比较 rank 1。
- 行扫描 `i = 11..17`，`ret = SDRAM_BASE + (1 << (i + 2 + 9))`，并在 `ret + j*4` 处读取。
- bank 探测是单次 A11 迭代；循环索引（0/1）存入 `dram_para1`。
- 列扫描 `i = 9..14` 和页大小编码与 upstream 相同。
- 对两个 `dram_para1` 半部分进行双 rank 比较，不同时设置 `dram_para2` 第 8 位。

Rust 转录省略了 `addr_line` 累加器：upstream 计算它但从不读取。rank-1 page-mode 块被
逐字转录，尽管两个 upstream 调用都指向 `mc_work_mode`；那是 upstream 当前的行为，悄悄
改变它会成为新的差异。

**接收者变更。** `DramPara` 不再派生 `Copy`，可变方法接收 `&mut self`。否则，旧的
`mut self` 方法在副本上操作，方法返回时每个扫描结果（以及 `mctl_sys_init` 设置的
`dram_clk`）都会被丢弃。移除 `Copy` 是使新扫描器生效的一部分，而不是无关的重构。

**结论：已在 #18 中修复。** 硬件确认在 §4.7 中跟踪。

### 4.2 AC remapping：T113-S4 chip-ID 例外有意省略

本地 `mctl_phy_ac_remapping` 匹配 `mctl_hal.c`：

- 读取 fuse `(SID+0x28 & 0xf00) >> 8` 和一个未使用的 `mark_id`。
- 如果 `dram_tpr13` 第 18-19 位置位 → `cfg7`；否则 fuse 8→`cfg2`、9→`cfg3`、10→`cfg5`、
  11→`cfg4`、12→`cfg1`、13/14→零。
- DDR2：fuse 15 返回；否则 `cfg6`。
- 从 22 字节表构建四个 32 位 `REMAP0..3` 字并设置 REMAP0 的第 0 位。

upstream HEAD 还读取 `chipid`，对于 DDR3 fuse 10，当 `chipid == 0x6800`（T113-S4）时
选择“不重映射”[1:697-755]。在 T113-S3 上 chip ID 不是 `0x6800`，因此该分支不生效；
父 issue 将非 T113-S3 SoC 排除在范围外 [14]。**有意差异。**

### 4.3 电压 / LDO 处理（#18）

旧的 `vol_set` 计算按类型的值，然后无条件用 `vol = 25` 覆盖。`dram_voltage_set` 现在
直接使用按类型的值：

| `dram_type` | 值 |
|---|---|
| DDR2 (2) | 47 |
| DDR3 (3) | 25 |
| 其他 | 0 |

LDO 掩码（清除 `0x20ff00`，`vol << 8`）与 upstream 一致，写入之后在
`sid_read_ldo_b_cal` 之前有 `udelay(1)`，与 upstream 相同 [1:84-105]。
`sid_read_ldo_b_cal` 读取 SID `0x300621c` 的第 8-15 位并应用相同的 DDR3 调整
（`reg > 0x20` → `reg -= 0x16`）；其嵌套的 `if` 在行为上等价于 upstream 的提前返回。

**结论：已在 #18 中修复。** 旧的覆盖只能改变 DDR2（47 → 25）；T113-S3 是 DDR3。

### 4.4 VTFCR 写入读取了错误的寄存器（在 #15 中修复）

旧的 `init` 做了：

```rust
let addr  = MCTL_PHY_BASE + ZQCR;  // 0x140
let addr1 = MCTL_PHY_BASE + VTFCR; // 0xb8
write32(addr, read32(addr) | (1 << 31));
if (self.dram_tpr13 & (1 << 8)) != 0 {
    write32(addr1, read32(addr) | 0x300);
}
```

这与 `mctl_hal.c:1597-1600` 完全一致。upstream 读取 VTFCR 自身：
`writel(readl(VTFCR) | 0x300, VTFCR)` [1:1397-1398]。该分支在随附默认值
`dram_tpr13 = 0x34000100`（第 8 位置位）下是激活的，因此旧代码在每次启动时都把
`read32(ZQCR) | 0x300` 写入 VTFCR。Issue #15 将第二次读取改为 VTFCR；`init_dram`
现在执行 `write32(addr1, read32(addr1) | 0x300)`。

**状态：已在 #15 中修复。** 硬件确认是 #15 HIL 运行的一部分。

### 4.5 时序参数与模式寄存器写入（#18）

`mctl_set_timing_params` 仍是旧的 `mctl_hal.c` 例程 [6:183-617]，而不是 upstream 的
`mctl_set_timing_params` [1:190-505]。形式上的差异：

- 它保留派生于汇编的 SP 槽位注释，并从计算值重建 `dram_tpr0/1/2`；upstream 不再写入
  这些字段，之后也不会读取它们。重建是死代码但无害；会保留到单独的清理时。
- 它使用 `ns_to_t(nanoseconds)` 以 `dram_clk / 2` 计算时序，等价于 upstream 的向上
  取整除法 [1:47-52]。
- 对于 DDR3（T113-S3 默认值），在 792 MHz 下结果数值等价：
  `tfaw = ns(50)`、`trcd = ns(15)`、`trp = ns(15)`、`trc = ns(53)`、`tras = ns(38)`、
  `twtr = ns(8)+2`、`trrd = max(ns(10),2)`、`txp = max(ns(10),2)`、`tcl = 6`、
  `tcwl = 4`、`t_rdata_en = 4`、`mr0 = 0x1c70`、`mr2 = 0x18`、`trd2wr = 5`
  [1:252-345]。
- **模式寄存器写入已对齐（#18）：** 旧代码仅在对应字段的高 16 位为零时才覆盖
  `dram_mr0/1/2/3`，然后写入结构体字段。upstream 直接写入本地计算的 `mr0..mr3`
  [1:480-484]。`mctl_set_timing_params` 现在直接写入 `mr0..mr3`；`mr1`/`mr3` 仍像
  upstream 一样从 `dram_mr1`/`dram_mr3` 开始。

`mctl_vrefzq_init`、`mctl_com_init`、`mctl_sys_init`、`ccu_set_pll_ddr_clk`、
`mctl_channel_init`、`eye_delay_compensation` 和 `dqs_gate_detect` 已对照 upstream
重构做抽查，寄存器等价（相同掩码、相同地址、相同顺序）。未对 `mctl_channel_init`
进行穷举的指令级等价证明。

### 4.6 大小报告与 `dram_top`

`dramc_get_dram_size` 是 upstream 的 `DRAMC_get_dram_size` 将 `calculate_rank_size`
内联；位运算相同 [1:932-969]。缺失的是 upstream 的 `dram_top` 副作用：
`sunxi_dram_init` 计算 `size = init_DRAM(...) * 1 MiB` 并存储
`dram_top = SDRAM_BASE + size`，通过 `dram_get_top()` 暴露 [1:115-120,1449-1452]。

**有意缺失（#18）。** 父 issue 说只有在 payload 契约实际需要 top 地址时才添加
`dram_get_top` [14]；目前不需要。

### 4.7 硬件确认

`scripts/hil-check.sh` 在 T113-S3 上通过 FEL 加载 `target/rawboot-fel.bin` 并断言
启动日志。在断电重启进入 FEL 后，#18 镜像产生了 `hello=1`、`PASS: UART assertions`，
以及以下序列：

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

`DRAM simple test OK` 以及随后的 SD 行意味着新扫描器已完成且 loader 继续执行；PD22
LED 闪烁（人工检查）也得到确认。干净的单独一次 `hil-check.sh` 运行需要先断电重启进入
FEL：先前的 `xfel reset` 会使看门狗武装，重置循环会触发脚本的多重 `Hello` 防护
（`docs/agents/hil.md`）。

---

## 5. 剩余差异

按对 T113-S3 的可能影响排序：

1. **`dram_get_top()` / `dram_top`** —— 按设计缺失，直到 payload 需要它 [14]。
2. **T113-S4 AC-remap 例外** —— 未移植；在 T113-S3 上不生效，非 T113-S3 在范围外。
3. **时序例程结构** —— 旧 `mctl_hal.c` 形态，带有死代码 `dram_tpr0/1/2` 重建；对
   DDR3 默认值数值等价。MR 写入门控在 #18 中修复。
4. **`abort()`** —— 未移植；Rust 代码从不调用它。
5. **寄存器访问** —— 在 upstream 使用 `MCTL_PHY_*` 宏的地方仍保留许多原始偏移量；
   基础常量和 #18 涉及的名称已对齐。

已修复且不再是差异：per-rank `auto_scan_dram_size`（#18）、强制 `vol = 25`（#18）、
MR 写入门控（#18）、VTFCR 源寄存器（#15），以及范围内的函数/变量名（#18）。

未缺失：ZQ 启动、PLL/MBUS/DRAM 时钟时序、PHY AC remap 表（`cfg1..cfg7` 匹配 upstream
索引 1..7）、DQS 门检测、Vref/ZQ 初始化、通道初始化、eye-delay 补偿、简单读/写测试，
以及默认 `dram_para_t` 值（792 MHz、类型 3、ZQ `0x7b7bfb`、`tpr11 = 0x00340000`、
`tpr12 = 0x46`、`tpr13 = 0x34000100`）[1:1420-1447]。

---

## 6. 待解问题

1. **双 rank T113-S3 行为。** per-rank 扫描器现已就位，但没有可用于 A/B 硬件测试的
   双 rank 模块。upstream 的 `9c8b8e40` 消息称其为更新而非缺陷修复，代码有可疑之处
   （单次迭代 bank 循环；rank-1 模式仍写入 `SDRAM_BASE`，而比较使用 `rank1_addr`）
   [1:1096-1249][8]。**未在双 rank 硬件上验证**（HIL 板为 128 MiB 单 rank）。
2. **`dram_get_top` 集成。** `BootInfo` 契约是否需要 DRAM top 地址，还是返回的 MiB
   大小就足够？由父 issue 推迟 [14]。
3. **`mctl_channel_init` 的穷举等价性。** 抽查显示相同的寄存器/掩码，但未做完整的
   逐行证明，因为 Rust 混用字面量和计算地址。对每次写入**未验证**。
4. **许可。** upstream `dram.c` 是 GPL-2.0+ [1:1]；所获取文件中的 `mctl_hal.c` 没有
   明确的 SPDX 头 [6]。项目在 CC-BY-4.0（ADR-0001）下转录事实和行为；如果逐字复制了
   任何 `mctl_hal.c` 文本，应在再分发前重新检查许可立场。

---

## 来源

1. upstream `arch/arm32/mach-t113s3/dram.c`，位于 `5380c00fc67c975433f25c57fb481aa2b91aebf8` ——
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.c
2. upstream `arch/arm32/mach-t113s3/dram.h`，同一 SHA ——
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.h
3. upstream `arch/arm32/mach-t113s3/reg-dram.h`，同一 SHA ——
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/reg-dram.h
4. GitHub API，`arch/arm32/mach-t113s3/dram.c` 的提交历史 ——
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram.c&per_page=100
5. GitHub API，`arch/arm32/mach-t113s3/dram_sun20i_d1.c` 的提交历史 ——
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram_sun20i_d1.c&per_page=100
6. upstream `arch/arm32/mach-t113s3/mctl_hal.c`，位于 `23e5c39396588c831e45c77415aed89f12a66a54`
   （重构前的最后一个树）——
   https://raw.githubusercontent.com/szemzoa/awboot/23e5c39396588c831e45c77415aed89f12a66a54/arch/arm32/mach-t113s3/mctl_hal.c
7. GitHub API，提交 `381606b0d68f`（“import Andre Przywara's DRAM init rework code”，
   删除 `mctl_hal.c`，新增 `dram_sun20i_d1.c`）——
   https://api.github.com/repos/szemzoa/awboot/commits/381606b0d68f
8. upstream `arch/arm32/mach-t113s3/dram.c`，位于 `9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22`
   （“add t113-s4 support ... update t113-s3 auto_scan_dram_size func”）——
   https://raw.githubusercontent.com/szemzoa/awboot/9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22/arch/arm32/mach-t113s3/dram.c
9. GitHub API，提交 `385481ffab45089d90410cb67b93981a870edd7c`（2025-11-15 重构；新增
   `dram_top`/`dram_get_top`）——
   https://api.github.com/repos/szemzoa/awboot/commits/385481ffab45089d90410cb67b93981a870edd7c
10. 本地 `rawboot/src/dram/mod.rs` —— <workspace>/rust/awboot/rawboot/src/dram/mod.rs
11. 本地 `embed/app/src/dram/mod.rs`（vendored 移植前参考）——
    <workspace>/rust/awboot/embed/app/src/dram/mod.rs
12. GitHub API，调研时的 `main` HEAD（解析为 `5380c00fc67c975433f25c57fb481aa2b91aebf8`）——
    https://api.github.com/repos/szemzoa/awboot/commits/main
13. 先前的研究笔记（风格参考）—— <workspace>/rust/awboot/research/awboot-rust-migration.md
14. Issue #18 “Parity DRAM：dram” 和父 issue #14 “rawboot loader 与 upstream awboot 的行为一致性”。
