# T113-S3 上 BROM 之后的 SD 冷启动（#43）

调研时间：2026 年。目标：Allwinner T113-S3，SMHC0。upstream 固定在
`szemzoa/awboot` `5380c00fc67c975433f25c57fb481aa2b91aebf8`。本地代码按
符号引用，而不是按行号。

本笔记记录了 issue #43 背后的调查，以及当时考虑过的与
upstream 的差异。

**HIL 复测之后的状态（2026-10-25）：loader 不是原因，第 3-4 节中的
更改未被合并。** 基线 `master @ 60e6867` 不包含这些更改，在 SD 卡真正
断电重启之后 3/3 次真实冷启动全部通过。台架上的 PASS/FAIL 交替
来自 USB-UART 适配器向板子反向供电，使卡在 FEL 断电期间保持有状态。
见第 5 节和 `docs/agents/hil.md`。

---

## 1. 症状

移除电源、保持 FEL/BOOT 跳线、槽中已插入可用的 SD 卡，
然后上电，loader 经常打印：

```
SMHC: cmd1 timeout (rint=0x00000100, flag=0x00000004)
SD/MMC card not found
```

有些运行还会在 CMD1 行之前打印 `SMHC: cmd8 timeout`。同一镜像在
板子进入 FEL 之后再插入卡（热插拔），或在不断电的情况下通过 FEL
重新执行 loader 时都能正常工作。基线 `master @ ab49d66` 和
#38 之后的树表现出相同行为，所以这不是 #38 的回归。

失败的 CMD1 是 **MMC 回退**。SD 路径（`sd_send_op_cond`）首先
静默地拒绝该卡：CMD8 可能超时，或 ACMD41 轮询时 busy 位始终
不被置位。只有到那时 `sdmmc_detect` 才复位 host 并尝试 CMD1，而 SD 卡
不会应答它。因此可见的 CMD1 超时是结果，而不是根因
失败。

## 2. HIL 日志排除的内容

- **重试预算。** `fix/43-sd-init-retries` 提高了尝试次数。一次冷启动
  在两次 CMD1 超时后通过；另一次在超过五秒的时间里记录了约四十次
  CMD1 超时，然后 loader 放弃。更多的尝试和更长的延迟并不能让
  卡应答。
- **仅由断电造成的卡状态。** 在不断电的情况下重新执行 loader 能
  恢复卡，所以卡并未被锁存在只有断电才能清除的状态。
- **#38/#39 回归。** 相同的超时在 #38 之前基线和 #39 分支上都能复现；
  块/FAT 层不涉及。

剩下的就是 BROM 留下的状态。BROM 在移交给 FEL 之前已经初始化了 SMHC0
和卡以读取引导扇区。因此 loader 的首次尝试从活动的控制器和已识别的
卡开始，这与热插拔情形不同——在热插拔中，卡一直被 BROM 仍在运行的
SMHC 时钟驱动，直到 loader 替换它。

## 3. 两个缺失的 host 步骤

固定的 upstream `sunxi_sdhci.c` 两者都不做 [1:525-645,
647-651]；主线 Linux 的 `sunxi-mmc.c` 两者都做（`sunxi_mmc_reset_host`、
`sunxi_mmc_request`）[2]。移植转录了 upstream，所以这里两者都缺失。

1. **等待控制器复位完成。** `sdhci_reset` 写入
   `SMHC_GCTRL_HARDWARE_RESET` 后立即返回。复位位是
   自清除的。Linux 的 `sunxi_mmc_reset_host` 轮询 `REG_GCTRL` 直到它们清除，
   超时为 250 ms，然后才编程时钟。在控制器仍在复位时编程 `CLKCR`、CCU
   门控/复位和时序寄存器是一种
   竞态；在冷启动时控制器来自 BROM 的活动状态，所以该竞态在
   那里可见，而在 loader 到 loader 的重新执行之后不可见。
2. **随 CMD0 发送 80 时钟初始化序列。** Linux 为 `MMC_GO_IDLE_STATE`
   在 `REG_CMDR` 中设置 `SDXC_SEND_INIT_SEQUENCE`。SD 物理
   层要求在 CMD0 之前至少有 74 个时钟且 CMD/DAT 为高。当该位置位时，
   SMHC 硬件会生成这些时钟。不设置它时，CMD0 会在卡时钟使能后
   立即发送；在 host 复位停止并重启时钟之后，卡可能尚未看到足够的时钟
   来接受该命令。随后 CMD8/ACMD41 会失败，尽管卡是健康的。

较早的 Rust 快照（`embed/app/src/sdhci/mod.rs`）确实设置了
`SEND_INIT_SEQUENCE`；#19 中的 upstream parity 重写将其删除了，因为 upstream
C 没有使用它。

## 4. 选定的更改

- `sdhci_reset` 首先使能控制器门控并撤销其 CCU 复位，然后
  轮询 `GCTRL` 直到复位位清除（250 ms 上限）并报告
  超时，而不是立即返回。先使能门控很重要，因为被门控的
  SMHC 块无法被可靠地轮询。
- 当命令是 CMD0 时，`sdhci_transfer` 设置 `SMHC_CMD_SEND_INIT_SEQUENCE`。
- `sd_send_op_cond` 只在传输成功且 busy 位
  被置位时才接受 ACMD41；传输失败会在同一有界预算内重试，而
  不是被误认为 SD 初始化完成。
- 如果复位超时，`sdmmc_detect` 停止该次尝试；停止/busy 错误路径
  复用轮询式复位。
- `sunxi_sdhci_init` 记录复位前的 `GCTRL`、`CLKCR`、`WIDTH`、`RINT` 和
  `STATUS` 一次，这样冷启动日志会记录 BROM 留下的内容。如果 SMHC 时钟
  门控关闭，它记录这一点，而不是去操作被门控的块。
- 重试预算、100 ms 的尝试间隔暂停和 CMD0 后的 2 ms 延迟
  保持不变。没有延长任何超时来掩盖失败。

两项更改都是对固定 upstream 的有意差异，取自
主线 Linux，因为它们是 host 初始化要求，而不是卡协议
策略。它们经过调查但未被合并：下面的 HIL 复测表明
观察到的失败是台架伪影。`research/sdhci-port-vs-upstream.md` 没有
为它们更新。

## 5. HIL 结果（更正）

统计验收已运行，但首次尝试仍产生严格的
PASS/FAIL 交替。失败的运行能应答 CMD8 和 CMD55/ACMD41
（`app_fail=0`、`ocr=0x00ff8000`），但从不置位 busy 位，即使在
轮询 5 s 之后；在不断电板子的情况下重新执行 loader 也会以同样方式失败。

当断电时拔下 USB-UART 适配器后，交替现象消失了：
适配器通过串口线向板子反向供电，所以卡从未失去电源，
并把其状态带入下一次"冷"启动。在真正断电时，驱动不需要上述更改：

- `master @ 60e6867`（无代码更改）：3/3 次冷启动都到达 `SD/MMC card found`
  并成功块读取。
- 带更改的分支在卡真正断电重启后也通过，但
  基线结果表明这些更改并不是修复。

确定性流程在 `docs/agents/hil.md` 中：在板子关闭时拔下 USB-UART
适配器，等待卡电源轨放电，然后在保持 FEL/BOOT 的情况下上电
并重新连接适配器。热插拔能工作，因为它
对卡断电重启。Issue #43 作为台架流程问题关闭，而不是
loader 缺陷。

## 来源

1. `szemzoa/awboot`，位于 `5380c00fc67c975433f25c57fb481aa2b91aebf8`：
   `arch/arm32/mach-t113s3/sunxi_sdhci.c` [525-645, 647-651]；
   `arch/arm32/mach-t113s3/sdmmc.c` [186-241, 375-706, 725-763]。
2. Linux `drivers/mmc/host/sunxi-mmc.c`：`sunxi_mmc_reset_host`（GCTRL 轮询）和
   `sunxi_mmc_request`（针对 `MMC_GO_IDLE_STATE` 的 `SDXC_SEND_INIT_SEQUENCE`）。
