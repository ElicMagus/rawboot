# FEL flasher：一条命令更新板内的卡

在 #41 之后，总线上出现了返回式 DDR-init stub 和带 job/status 契约的 DRAM flasher，但它还没有触及 SD。此前从主机更新卡：`cargo xtask image`、`dd` prefix、手动将 `payload.bin` 复制到 FAT。要更新设备中的卡，不得不将其取出。需要一个用户路径，通过 FEL 将 `payload.bin`（以及，如果指定了，`boot.cfg`）写入现有 FAT 根目录，并将 SD loader 写入 sector 16，同时保留 MBR 和分区，然后回读已写入的内容并指出失败的步骤。

决策：`cargo xtask fel-flash` 构建或复用镜像，检查它们并调用 `FelDevice::flash_sd_card`。主机将计划和三个区域布置到 DRAM 中，flasher 执行命令 `FLASH_COMMAND_FLASH_SD`，通过共享 FAT 适配器写入文件，并将 loader 直接写入 sector 16，之后将读取的内容与原始内容进行比较。ADR-0013 中的 job/status 契约不改变布局：新命令使用相同的 `FlashJob`/`FlashStatus`，而计划被移到单独的块中。

## 计划与 DRAM 中的区域

| 项目 | 地址 | 大小 |
| --- | --- | --- |
| `FlashPlan`（`RBFP`） | `0x40000200` | 40 字节 |
| `payload.bin` | `0x40100000` | 至 8 MiB |
| `boot.cfg` | `0x40900000` | 至 4095 字节 |
| sealed SD loader | `0x40901000` | 至 64 KiB |
| 窗口末端 | `0x40911000` | — |

`FlashPlan` 保存三个区域各自长度和 checksum。主机在访问设备之前构建计划，因此过大的 payload、`boot.cfg` 或 loader 会在本地被拒绝。flasher 检查计划的 magic/version/size/checksum、各区域的长度和 checksums，以及 loader 的 `verify_sealed`。这可以防止 `write` 和 `exec` 之间的损坏 FEL 传输。

新的状态步骤（`plan`、`SD init`、`layout`、`read MBR`、`write payload`、`write boot.cfg`、`verify FAT files`、`write loader`、`verify loader`、`raw write`、`raw verify`）和结果（`bad plan`、`SD card offline`、
`no MBR`、`no FAT partition`、`unsafe card layout`、`card too small`、`read failed`、
`write failed`、`create file failed`、`truncate failed`、`file write failed`、`file flush
failed`、`read-back mismatch`、`MBR changed`）使主机能够打印步骤和失败原因。
成功状态仍然是 `done`/`ok`。

## 写入诊断

`FLASH_COMMAND_RAW_WRITE_TEST` 和 `cargo xtask raw-write-test` 检查块写入路径本身，不需要 MBR、FAT 和 staged 镜像：flasher 将一个已知块写入 LBA 16，回读并比较。sector 0 不被触碰；sector 16 会被重写，因此在准备好的卡上，测试后需要再次写入 loader。步骤 `raw write` 和 `raw verify` 将 CMD24 失败与读取错误区分开，而 `write failed`/`read failed`/`read-back mismatch` 指出原因。

与 `fel-flash` 不同，诊断不读取 MBR，也不做 layout 检查：它适用于空白或已经检查过的卡，其中 sector 16 未被分区占用。因此写入需要显式的 `--yes`，会打印关于重写 sector 16 的警告，并且不是更新已准备卡的安全路径。

## 顺序与安全

1. `cargo xtask fel-flash`（不带 `--skip-build`）调用 `image`、`payload` 和 `flasher`，然后读取 `target/rawboot-boot-sd.bin`、`target/rawboot-payload.img`、`target/rawboot-ddr.bin` 和 `target/rawboot-flasher.bin`。`--dry-run` 执行所有这些检查并打印计划，而不打开 FEL；实际写入需要 `--yes`。
2. loader 必须是 sealed eGON。FEL 变体按文件名、按 `.build` 戳中的 `variant=fel` 以及与 `target/rawboot-fel.bin` 的字节匹配而被拒绝。payload 必须能被 `rawboot_image::parse` 解析，`boot.cfg` 不得超过 4095 字节。
3. `flash_sd_card` 检查计划，执行 DDR-init stub，用模式校验 DRAM，然后才写入计划和区域。
4. flasher 初始化 SD，要求 MBR 签名 `55 AA` 和表中的 FAT 分区。分区使用与 loader 相同的 `partition::candidates` 查找；没有 MBR 的裸 FAT 被拒绝。
5. `check_sd_layout` 要求分区起点 > sector 16、loader 结束不晚于分区起点，并且分区结束在卡范围内。
6. 首先将 `payload.bin` 和（如果指定）`boot.cfg` 写入 FAT 根目录并回读。只有之后才将 sealed loader 写入 sector 16 并回读。sector 0 从不写入；最后 flasher 重新读取 MBR，如果它发生了变化，则报告 `MBR changed`。
7. 在写入 loader 之前的任何失败都会保留旧的引导镜像；之后的失败会报告步骤，但 MBR 和分区不会被重新格式化。

## 构建与 feature 边界

flasher 需要 `sunxi-drivers/write`，而 loader 不需要（ADR-0005）。如果在同一个 `cargo build` 中构建两个包，Cargo 会合并 feature 并将写入拖入 loader，超出 64 KiB SRAM。因此 `rawboot-flasher` 从 `default-members` 中移除：`cargo build --release` 和 `cargo clippy` 默认检查 loader，而 `scripts/check.sh` 通过单独调用对 flasher 进行 lint。`cargo xtask flasher` 在它自己的 cargo 调用中构建两个镜像。

## 考虑过的选项

- **一条带 DRAM 计划的 FEL 命令**（已采用）：卡留在板内，数据不受 SRAM 限制，计划在两端都进行版本化校验，计划、MBR/FAT 解析、layout 和 FEL 传输顺序在主机上测试；实际 SD 写入留给 HIL。
- **用读卡器更新卡**（已拒绝）：这是当前的手动路径，需要取出卡，并且不检查写入的正是所构建的内容。
- **将区域地址传入 `arg0..arg2`**（已拒绝）：三个参数无法覆盖 payload、`boot.cfg`、loader 及其 checksums；带计划的固定地址更容易检查。
- **先写 loader**（已拒绝）：如果在 payload 上失败，卡会留下新引导加载程序和旧 payload。payload 在前、loader 在最后，可保持旧的可用引导加载程序直到最后一步。
- **将 `rawboot-flasher` 保留在 default-members 中**（已拒绝）：feature unification 会将 `sunxi-drivers/write` 包含进 loader，并破坏 SRAM 预算下的链接。
- **仅按名称识别 FEL 变体**（已拒绝）：以其他名称复制的 `rawboot-fel.bin` 会通过；字节和 build 戳都会检查。

## 后果

- `rawboot-image` 发布 `FlashPlan`、命令 `FLASH_COMMAND_FLASH_SD`、区域地址、步骤/结果和 `check_sd_layout`；全部由 host 测试检查。
- `rawboot-fel` 获得 `flash_sd_card` 和错误 `LoaderNotSealed`；host 测试检查 FEL 传输顺序、拒绝过大的 `boot.cfg` 以及 DRAM 不足以容纳窗口。
- `rawboot-flasher` 启用 `sunxi-drivers/write`，写入 FAT 和 sector 16，回读文件、loader 和 MBR。`rawboot-flasher` 不再属于 `default-members`；`scripts/check.sh` 增加了单独的 clippy 检查。
- `xtask` 获得 `fel-flash`，带 `--payload`、`--boot-cfg`、`--loader`、`--flasher`、`--ddr-stub`、`--sequence`、`--skip-build`、`--dry-run` 和 `--yes`；没有 `--yes` 时命令不打开 FEL，而 `--dry-run` 检查镜像并打印计划。`scripts/check.sh` 在构建镜像后运行 dry-run。
- `rawboot-flasher` 启动 USART0 并设置驱动 `Console`，因此 SDHCI/FAT 错误在 HIL 的 UART 日志中可见；`scripts/hil-flash.sh` 运行 `fel-flash --yes`，捕获 UART 并要求状态 `done`/`ok`。
- `sunxi-drivers` 获得 `fat::file_matches`，用于可进行 host 测试的 read-back；单独的 `scripts/check-features.sh` 在构建前检查 `sunxi-drivers/write` 未进入 `rawboot`。
- HIL：成功 `fel-flash` 后，从卡冷启动必须到达 handoff，而 PD22 必须为 payload 闪烁。在运行之前不作此声明。
- HIL #42 最初遇到了写入阻塞：在台架的 SDSC 卡上（`hc=0`、25 MHz、CSD write-protect 关闭），卡不应答 CMD24，`fel-flash` 在 MBR 未受影响的情况下停在 `write
  payload`（`truncate`/`file write`/`file flush failed`）。该缺陷被拆分到 #47 并修复（PR #49）；该阻塞仅涉及 SD 写入路径，并不解释 DDR-init 的行为。HIL #42 的结果记录在跟踪器中，而不是本 ADR 中。
- 在同一次上电中重复执行 DDR-init stub 不是幂等的：stub 会重新执行 `DramPara::init_dram()` 而不检查 DRAM 状态，并等待 PLL_DDR 锁定，因此可能与 #23 同类的失败。HIL #52 曾连续运行三次 `bringup`、成功后的 `fel-flash` 重试以及 SD 错误后的重试——都返回了 BROM。随后重复的 stub 挂起：在成功的 `raw-write-test` 之后，下一个 `fel-flash` 遇到 USB 超时，板子仍留在总线上，但 `cargo fel version` 无应答；需要冷启动。因此规则是每次冷启动一个 stub，而不是建议。
- 在连接 USB-UART 且 sector 16 无效（在 `raw-write-test` 之后）的情况下，四次冷启动给出 DDR-init 4/4 PASS；同时卡严格交替：奇数启动为 `SD card offline`，偶数为 `done`/`ok`。这是 #43 的台架行为（通过 UART backfeed 给卡供电），而不是 DDR 失败。使用启动卡时，部分启动中 BROM 会转入从 SD 启动而不是 FEL；条件矩阵在 runbook 中。
