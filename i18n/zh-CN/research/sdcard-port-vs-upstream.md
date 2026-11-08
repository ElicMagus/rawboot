# `rawboot` 的 SD/MMC 卡移植 vs 当前 upstream `szemzoa/awboot` 的 `sdmmc.c`

调研时间：2026 年。目标：Allwinner T113-S3。upstream 固定在 `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`（2025-11-21）。比较的文件：
`arch/arm32/mach-t113s3/sdmmc.c`、`arch/arm32/mach-t113s3/sdmmc.h`、`board.c`、
`board.h` 和 `Makefile` [1]-[5]。本地代码按符号引用，而不是按行号（见
`docs/agents/domain.md`）。

本笔记是 `research/sdhci-port-vs-upstream.md` 的卡协议对口篇；它是 issue #20 的
静态 parity 检查清单。

---

## 1. 摘要

- upstream 的 `arch/arm32/mach-t113s3/sdmmc.c` 在固定 SHA 处有 764 行 [1]；
  `sdmmc.h` 有 198 行 [2]。最后一个触及该驱动的 upstream 提交是
  `385481ffab`（2025-11-15，"refactor sdmmc driver, create library for fatfs, fdt,
  loaders"）。
- 我们的 loader 从 SD 卡启动，而固定的 T113-S3 `board.h` 默认使用
  `CONFIG_BOOT_MMC=1` / `CONFIG_BOOT_SDCARD=0` [4]。upstream 的 `VARIANT=sdmmc` 构建
  会同时启用两者，并先尝试 SD，然后复位并回退到 MMC [5]。移植转录了这条 SD 优先的流程，
  因此两个分支都处于生效状态：SD 卡走 SD 路径，MMC/eMMC 卡走 MMC 路径。
- issue #19 已经把仅 PIO 的 host 路径替换为 upstream 的 64 字节
  IDMAC/PIO 拆分 [6]。issue #20 在此 host API 之上对齐卡协议。
- `CONFIG_MMC_ENABLE_RSTN` 在 `board.h` 中默认为 `0` [4]，所以可选的 eMMC
  硬件复位 CMD6 不属于固定 T113-S3 的行为，也不被转录。

### 结论

`sdmmc.c` 的范围内行为已经移植：SD 初始化、MMC 初始化、
卡识别、CID/CSD/EXT_CSD 解析、容量计算、SD 4-bit 切换、
MMC high-speed/DDR/power-class/bus-width 切换以及块读取。命名遵循
upstream（`sdmmc_*`、`MMC_*`、`SD_CMD_*`、`OCR_*`、`SD_VERSION_*`、
`MMC_VERSION_*`、`EXT_CSD_*`）；类型和 HCI 所有权形态按 ADR-0007 保持
Rust 惯用风格。

---

## 2. 逐函数比较

| Upstream | 本地 | 状态 |
| --- | --- | --- |
| `UNSTUFF_BITS` 宏 [1:126-140] | `UNSTUFF_BITS` | 相同的掩码、字索引和跨字移位。 |
| `go_idle_state` [1:152-162] | 同名 | 一条 CMD0，SPI 下为 `MMC_RSP_R1`，其余为 `MMC_RSP_NONE`。 |
| `sd_send_if_cond` [1:165-184] | 同名 | 相同的电压窗口参数、检查模式 `0xaa`、R7 响应，成功时置 `SD_VERSION_2`。 |
| `sd_send_op_cond` [1:186-241] | 同名 | 函数内先 CMD0 再 CMD8，CMD55/ACMD41 循环，50 次尝试预算，SPI 下 `MMC_SPI_READ_OCR`，`rca = 0`；ACMD41 传输失败会消耗一次重试，而不是被当作就绪（#43）。 |
| `mmc_send_op_cond` [1:270-309] | 同名 | 使用精确的 `MMC_VDD_27_36` / `MMC_VDD_165_195` OCR 参数发 CMD1，50 次重试，5 ms 延迟，`MMC_VERSION_UNKNOWN`，`rca = 1`。 |
| `sdmmc_status` [1:311-331] | 同名 | 100 次重试，就绪位 `1 << 8`，1 us 延迟，状态 `(response >> 9) & 0xf`，超时返回 `-1`。 |
| `sdmmc_read_blocks` [1:333-373] | 同名 | CMD17/CMD18，块寻址 vs 字节寻址，R1，数据描述符，等待 TRAN/DATA，多块之后发 CMD12。 |
| `sdmmc_detect` [1:375-706] | 同名 | 复位、400 kHz、1-bit、CMD0、2 ms、先试 SD，复位 + CMD0 回退到 MMC；CID/CSD/RCA/select；版本表；`tran_speed`、`dsr_imp`、块长度；EXT_CSD；容量；最终宽度/时钟。 |
| `sdmmc_blk_read` [1:708-723] | 同名 | 每块 127 个的块组分块，块号递增，缓冲区按 `read_bl_len` 递增。 |
| `sdmmc_init` [1:725-763] | 同名 | `online` 只清除一次，100 ms 暂停，`retries = 10` 配合 upstream 的后置自减（11 次尝试），返回 0/-1。 |
| `sdmmc_t` [2:147-163] | `Sdcard` | 相同的字段和宽度。 |
| `sdmmc_pdata_t` [2:165-171] | `Sdcard_data` | 相同的 card/buffer/online 字段；拥有的 `Sdhci` 取代了 upstream 的 `sdhci_t *`。 |

### EXT_CSD 与 MMC 切换

| Upstream 行为 | 本地 |
| --- | --- |
| 对 MMC >= 4 读取 512 字节 EXT_CSD，轮询状态直到 TRAN [1:552-580] | 相同。 |
| 在 EXT_CSD 之前设置 `tran_speed = 50 MHz` [1:553] | 相同。 |
| 将 `EXT_CSD_REV` 1/2/3/5/6/7/8 映射为 4.1/4.2/4.3/4.41/4.5/5.0/5.1；默认保留 CSD 版本 [1:582-620] | 相同，包括默认分支。 |
| 容量来自 CSD（SD）或 `EXT_CSD_SEC_CNT`（MMC），然后乘以 CSD 块长度 [1:624-645] | 相同。 |
| 当 `EXT_CSD_CARD_TYPE` 不含 DDR52 时拒绝 DDR；回退到 50 MHz SDR [1:650-671] | 相同的警告和状态变更。 |
| 当卡不支持 52 MHz/DDR52 时序时回退到 25 MHz [1:673-679] | 相同。 |
| CMD6 HS_TIMING，然后当宽度 >= 4 时设置 power class，再设 BUS_WIDTH；1 ms 延迟 [1:681-720] | 相同的参数、顺序和延迟。 |
| 分支之外最终的 `sdhci_set_clock(clock_wanted)` 和 `sdhci_set_width(width)` [1:722-725] | 相同；SPI 分支检查这两个调用并报告失败。 |

---

## 3. #20 中修复的差异

1. **SD 初始化顺序。** 旧移植在 `sdmmc_detect` 中调用 CMD0 和 CMD8，
   然后进入 `sd_send_op_cond`；upstream 把这两个调用都放在 `sd_send_op_cond` 内。
   重试次数是 100 而不是 upstream 的 50，循环形态也不同。已替换为
   upstream 的 CMD0/CMD8/CMD55/ACMD41 序列和后置自减重试预算。
2. **MMC 初始化。** 旧移植多发了一次参数为 0 的 CMD1，使用了
   电压掩码而不是 upstream 的精确相等 OCR 参数，并把 `rca` 设为 `0`。
   已替换为 upstream 的 50 次重试 CMD1 循环、`0x40ff8000` / `0x40ff0080` 参数、
   `MMC_VERSION_UNKNOWN` 和 `rca = 1`；SD 失败回退现在会复位
   控制器并完全按 upstream 重发 CMD0。
3. **CSD/EXT_CSD 语义。** 旧移植把未知的 `EXT_CSD_REV` 默认为
   版本 0，并且从未在读取 EXT_CSD 之前应用 upstream 的 `tran_speed = 50 MHz`。
   两者现在都与 upstream 一致。
4. **MMC 速度与宽度切换。** 旧 MMC 分支以原始宽度值发送 CMD55 + CMD6。
   移植现在执行 upstream 的完整序列：DDR 能力检查、
   high-speed 时序、HS_TIMING 写入、4-bit power class、BUS_WIDTH 写入、1 ms
   延迟，以及共同的最终时钟/宽度更新。
5. **读状态等待。** 旧条件在重写过程中被弄反了：当卡既不处于 TRAN 也不处于 DATA 时
   它就退出。现在它会循环直到 TRAN 或 DATA，
   与 upstream 一致，并让 `sdmmc_blk_read` 得以返回。
6. **命名。** 命令、响应、状态、OCR、数据标志、版本和 EXT_CSD 名称
   现在与 upstream 一致；`sdmmc_init`/`sdmmc_blk_read` 取代了旧的
   `Sdcard_data::init`/`blk_read` 方法。

---

## 4. 有意差异

- **类型与所有权。** `Sdcard`、`Sdcard_data`、`Sdhci` 和 `SdhciCmd` 取代
  C 结构体；HCI 由数据结构体持有而不是被引用，读/初始化入口返回
  `u64`/`i32` 并接收 `&mut` 参数。行为和字段顺序保持 upstream 的（ADR-0007）。
- **日志。** upstream 的 `trace`/`debug`/`warning`/`error` 变为 `print!`。loader
  保留其简洁的 `SD/MMC card found` / `SD/MMC card not found` 行，而不是
  upstream 的 `SHMC: ... card detected`；错误文本被转录。EXT_CSD 版本
  和浮点容量调试行不打印。
- **eMMC 硬件复位。** 在固定 board 上 `CONFIG_MMC_ENABLE_RSTN` 为 `0`，所以
  可选的 RST_N CMD6 及其调试行被有意省略。
- **SPI 分支。** `isspi` 保留为字段，SPI 分支被转录，但
  T113-S3 board 实例不是 SPI；没有实例化 SPI host。
- **eMMC board 实例。** 只实例化了 `sdhci0`。`sdhci2` 上的 eMMC 超出
  T113-S3 SD 特性范围，继续推迟。
- **复位超时（#43）。** 当 `sdhci_reset` 报告超时时，`sdmmc_detect` 停止该次尝试；
  upstream 忽略复位结果，因为其复位会立即返回。
  轮询式复位和 CMD0 初始化序列位于 `sdhci` 中；见
  `research/sd-cold-start-brom.md`。
- **ACMD41 传输失败（#43）。** `sd_send_op_cond` 只在 ACMD41 成功并
  置位 busy 位时才离开轮询循环。upstream 在传输失败时也会跳出，然后
  把过期的响应当作 OCR；这里失败会在同一 50 次尝试预算内消耗一次重试，
  因此 SD 路径会在 MMC 回退之前被拒绝。
- **无 host 接缝。** 按父 issue 的决定，卡驱动没有 host 测试；
  parity 由本静态表和 T113-S3 上的 HIL 检查。

---

## 5. 硬件确认

在 T113-S3 上，用分支 `feat/20-parity-sdcard` 的提交 `0ae1ecc`
（镜像 stamp `dirty=false`）运行 `scripts/hil-check.sh target/rawboot-fel.bin iss20` 完成验证。
UART 上仍载有先前的 loader，因此该次运行走了文档记载的
`HIL_SKIP_PREFLIGHT=1` 路径；脚本将其排空，报告一个 `Hello`，并断言
了必需的行：

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

- `SD/MMC card found` 意味着 CMD0/CMD8/CMD55/ACMD41、CID/CSD/RCA/select 以及 50 MHz
  4-bit 切换都在 SD 路径上成功。
- 512 字节读取高于 64 字节阈值，因此走 IDMAC；该
  块被零填充，因为测试卡是空白的。
- 所有者确认 PD22 LED 在 payload 循环中闪烁。

---

## 来源

1. upstream `arch/arm32/mach-t113s3/sdmmc.c`，位于 `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.c>
2. upstream `arch/arm32/mach-t113s3/sdmmc.h`，同一 SHA —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.h>
3. upstream `board.c`，同一 SHA（sdhci0/sdhci2 实例，PF 引脚）。
4. upstream `board.h`，同一 SHA（`CONFIG_BOOT_*`、`CONFIG_MMC_ENABLE_RSTN`）。
5. upstream `Makefile`，同一 SHA（`VARIANT=sdmmc` 启用 SD 和 MMC）。
6. `research/sdhci-port-vs-upstream.md` 与 issue #19。
7. ADR-0007，`docs/adr/0007-align-behaviour-to-upstream.md`。
8. Issue #20 "Parity SD/MMC-карты: sdcard" 与父 issue #14。
