# 硬件在环（HIL）验证

loader 在真实的 T113-S3 上验证。本页是 runbook：主机设置、加载序列、UART 捕获以及如何解读
结果。本仓库中的脚本是 `scripts/hil-check.sh`；它自动化了下面的步骤。

## 由 agent 驱动的运行

当 agent 执行这些步骤时，它针对每个物理动作（断电重启、插入卡、FEL/BOOT、LED 确认）询问一次
`question`，并在开始下一个动作之前等待回答。它不会将多次冷启动合并到一个提示中。在一次
`question` 调用中，它呈现两个标签页：物理步骤，以及带有“是”和“否”的确认项“确定？”。
仅当步骤已确认且选择了“是”时它才执行；否则返回步骤问题。这可以防止误触。每次运行后它报告
PASS/FAIL、关键日志行和下一步。

## 开发板与接线

- SoC：Allwinner T113-S3，DDR3，128 MiB。
- 调试 UART：**USART0**，引脚 **PE2/PE3**，**115200 8N1**。在主机上它是 CH340
  （`/dev/ttyUSB0`，可通过 `UART_DEV` 覆盖）。
- LED：**PD22**。loader 在其主循环中每 50 ms 翻转一次；handoff 之后改由 payload 以短-长节奏
  闪烁。脚本看不到 LED，因此运行以人工检查结束。
- FEL：FEL 模式下的 USB OTG。开发板枚举为 `1f3a:efe8`（有些适配器报告
  `1f3c:*`）。BROM 仅在**冷上电**时采样 FEL strap：彻底断电，按住 FEL/BOOT，然后上电。在
  reset 期间保持 FEL（或 FEL+RESET）不会重新采样它，因此自动化测试架必须切换电源轨，而不只是
  `NRST`。

## 主机设置

1. 串口访问：将用户加入 `dialout`（`sudo usermod -aG dialout "$USER"`，然后重新登录）。
   尚未获取该组的 shell 仍可通过 `sg dialout` 运行捕获。
2. FEL 访问：安装一条 udev 规则，使 `xfel` 无需 root 即可打开设备：
   ```sh
   echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="1f3a", ATTR{idProduct}=="efe8", MODE="0666"' \
     | sudo tee /etc/udev/rules.d/99-sunxi-fel.rules
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```
   如果你的开发板报告 `idVendor=="1f3c"`，为它再添加一条规则。
3. 工具：`xfel`（使用的是 v1.4.0；此处位于 `~/.local/bin/xfel`）和 Python
   的 `pyserial`（`sudo apt install python3-serial` 或 `pip install pyserial`）。
4. 检查：当开发板处于 FEL 时，`xfel version` 必须打印一行 `T113`。

## 加载与捕获

`cargo xtask image` 从同一份代码写出两个 loader 镜像：链接到 `0x00028000` 处以供 SRAM 使用的
`target/rawboot-fel.bin`，以及链接到 `0x00020000` 处用于 BROM 从扇区 16 进行 SD/SPI 启动的
`target/rawboot-boot-sd.bin`。将 FEL 镜像用于 `hil-check.sh`；将启动镜像写入卡。loader 会自行
初始化 DRAM，因此 `xfel ddr` **不是**这两个流程的一部分。

构建 loader，然后运行脚本：

```sh
cargo xtask image
scripts/hil-check.sh target/rawboot-fel.bin hil
```

如果某个 issue 的验收包含卡路径，请设置 `HIL_REQUIRE_SD=1`。脚本随后还会要求出现
`SD/MMC card found` 以及非零的 `SD: read N block(s)` 行。未插入卡时保持不设置。对于必须证明
handoff 的 issue，还要设置 `HIL_REQUIRE_HANDOFF=1`：此时日志必须包含 `BOOT: handoff to ...`
行，并且不再接受 loader 的 `S=0x` 循环。`HIL_REQUIRE_PAYLOAD=1` 还会将 handoff 的入口、加载
地址、大小和校验和与 `target/rawboot-payload.img` 比较（`HIL_PAYLOAD_IMG` 覆盖路径），因此卡上
过期的 `payload.bin` 会失败而不是通过。`HIL_REQUIRE_LED=1` 将人工 LED 确认纳入结果：脚本会在
TTY 上提示，或在非交互运行中接受 `HIL_LED_CONFIRMED=1`。

### SD 初始化不稳定

插入卡时的冷启动可能以 `SD/MMC card not found` 失败（#43）。
`scripts/hil-sd-retry.sh [image] [label]` 最多重复 `hil-check.sh` `HIL_SD_RETRIES` 次
（默认 3 次），并且只重试该失败；handoff、payload 及其他断言失败会立即停止。它强制设置
`HIL_SKIP_PREFLIGHT=1` 和 `HIL_NO_PROMPT=1`，因为每次重试都会替换上一次尝试遗留并仍在运行的
loader。设置 `HIL_REQUIRE_LED=1 HIL_LED_CONFIRMED=1` 可包含 LED 断言，或在需要干净的交互式
运行时直接运行 `hil-check.sh`。如果 FEL 在尝试之间停止响应，将开发板断电重启进入 FEL 并重新
运行。

重试不会给卡断电重启。在将 `cmd1`/ACMD41 失败视为 loader 缺陷之前，请按下文所述执行一次真正
的冷启动。

#### 冷启动必须给卡断电重启（#43）

只有当卡本身断电时，“冷”启动才会真正检验驱动。在此测试台上，USB-UART 适配器可能通过串口线
向开发板的 3.3 V 电源轨反灌电流。保持适配器连接时，卡在 FEL 断电期间会保留其状态；随后即使
在未修改的 `master` 上，冷启动也会交替 PASS/FAIL。该失败看起来像 CMD8 成功之后 ACMD41 始终
不置位 busy 位，然后是通常的 `cmd1 timeout` MMC 回退，而在不重启开发板电源的情况下重新执行
loader 无法恢复。

真正的冷启动：

1. 拔掉 USB-UART 适配器（以及任何其他可能向开发板反灌电流的线缆），然后断开开发板电源。
2. 等待几秒钟让卡电源轨放电，插入卡，按住 FEL/BOOT，上电，然后才重新连接 USB-UART 适配器。
3. 运行 `HIL_REQUIRE_SD=1 scripts/hil-check.sh target/rawboot-fel.bin cold<N>`。

然后重复两条恢复路径：在开发板进入 FEL 后插入卡并加载 loader；以及在不移除电源的情况下通过
FEL 重新执行 loader（`HIL_SKIP_PREFLIGHT=1`）。最后，移除卡并确认出现
`SD/MMC card not found`，且没有挂起或无界重试循环。

基线 `master @ 60e6867` 以这种方式通过了 3/3 次真正的冷启动，因此在断开适配器时出现失败指向
loader 或卡，而不是测试台。FEL 之后的热插拔和 loader 重新执行不会给卡断电重启；它们测试的是
从已经供电的卡恢复。Issue #43 作为测试台流程问题关闭；证据见
`research/sd-cold-start-brom.md`。

### 卡上的 payload 镜像

loader 从 FAT 分区读取 `payload.bin`。构建 blink payload，然后将封装后的镜像复制到已挂载的卡。
你可以在任何带读卡器的机器上写卡；对于 HIL，它只需在开发板中，而不必在此主机中：

```sh
cargo xtask payload
cp target/rawboot-payload.img /media/$USER/<partition>/payload.bin
```

复制 `target/rawboot-payload.img`，而不是原始的
`target/armv7a-none-eabi/release/payload.bin`：前者带有 loader 解析的镜像头。如果卡已在开发板中
且开发板处于 FEL，`cargo xtask fel-flash` 可在不移除卡的情况下更新 `payload.bin` 和 loader；
参见[就地通过 FEL 刷写卡](#fel-flashing-the-card-in-place-42)。

`payload.bin` 旁边可选的 `boot.cfg` 会被复制到 DRAM，并通过 `BootInfo.args` 以 NUL 结尾字符串
的形式暴露给 payload（最多 4095 字节；4 KiB 窗口包含终止符）。没有该文件时，
`BootInfo.args` 为零。要端到端断言 handoff，请在卡上放置一个已知文件并传入其字节长度：

```sh
printf 'console=ttyS0' > /media/$USER/<partition>/boot.cfg   # 13 bytes
HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_BOOT_CFG_LEN=13 \
    scripts/hil-check.sh target/rawboot-fel.bin bootcfg
```

`HIL_BOOT_CFG_LEN` 使脚本要求出现 `ARGS: boot.cfg <len> bytes at 0x...` 行，并且 handoff 的
`args 0x...` 字段中有相同指针。不可读或放不下的文件会以 `BOOT: boot.cfg: ...` 中止启动，而不是
用被静默截断的参数启动。

对于完整的端到端验收（SD -> FAT -> DRAM -> handoff -> blink），请带上所有断言运行：

```sh
HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_REQUIRE_PAYLOAD=1 HIL_REQUIRE_LED=1 \
    scripts/hil-check.sh target/rawboot-fel.bin blink
```

### FEL payload（无 SD 卡）

`HIL_FEL_PAYLOAD=1` 将 `target/rawboot-payload.img` 上传到 SRAM 并写入 FEL mailbox，因此
loader 无需 SD 卡即可启动它。mailbox 遵循 `rawboot-image/src/fel.rs`：

| 字 | 地址        | 值                                         |
| -- | ----------- | ------------------------------------------ |
| 0  | `0x00044000` | magic `0x4d464252`（`RBFM` 小端）          |
| 1  | `0x00044004` | payload 镜像地址（`0x00039000`）           |
| 2  | `0x00044008` | payload 镜像大小（字节）                   |
| 3  | `0x0004400c` | 参数字符串地址，或零                       |

镜像窗口是 `0x00039000..0x00044000`（44 KiB）；可选参数位于
`0x00038000`。此路径刻意避开 `xfel ddr`（issue #23）并跳过 SD 初始化。带 handoff 和 LED 检查
运行：

```sh
cargo xtask image
cargo xtask payload
HIL_FEL_PAYLOAD=1 HIL_REQUIRE_LED=1 scripts/hil-check.sh target/rawboot-fel.bin fel
```

`HIL_FEL_ARGS="..."` 通过 `0x00038000` 处的 mailbox 传入以 NUL 结尾的参数字符串（最多
4 KiB，包含终止符）；运行会断言 loader 报告的指针。此路径上 payload 断言是自动的，并且会拒绝
`HIL_REQUIRE_SD`，因为不会发生 SD 访问。

### SD/SPI 启动（无 FEL）

BROM 从扇区 16 直接将启动镜像加载到 `0x00020000`，而 FEL 镜像链接在 `0x00028000` 并由 `xfel`
写入。将 **`target/rawboot-boot-sd.bin`** 放到卡上：扇区 16 中的 FEL 镜像会偏移 32 KiB 运行，
并在到达 UART 之前崩溃，使开发板静默且 LED 常亮。

卡需要 FAT 分区（本 runbook 假设 LBA 2048，如 #37 bring-up 中那样）以及根目录中的
`payload.bin`（或 `boot.cfg`）。`cargo xtask sd-image` 构建 boot prefix：一个指向该分区的 MBR，
加上位于扇区 16 的 `target/rawboot-boot-sd.bin`，默认恰好 2048 个扇区（1 MiB）。它不格式化任何
东西，也不复制 payload；两者都留在分区上。

在一张已在 LBA 2048 处有 FAT 分区的卡上构建并写入该 boot prefix：

```sh
cargo xtask image
cargo xtask payload
cargo xtask sd-image --partition-sectors 1843200
sudo dd if=target/rawboot-sd-mbr.img of=/dev/sdX bs=512 conv=notrunc,fsync
cp target/rawboot-payload.img /media/$USER/<partition>/payload.bin
```

`--partition-sectors` 必须是分区实际的扇区大小（从 `fdisk -l /dev/sdX` 读取）；`--start-lba`
默认 2048，并同时改变 boot prefix 长度和 MBR 条目；`--loader` 覆盖默认的
`target/rawboot-boot-sd.bin`。该 boot prefix 不包含 FAT 卷，也不包含 payload：格式化整个设备
（`mkfs.vfat /dev/sdX`）或在其上写入完整磁盘镜像会破坏扇区 16 中的 loader。只格式化和复制分区
（`mkfs.vfat /dev/sdX1`，挂载它，复制 `payload.bin`），或者之后重新运行 boot prefix 的 `dd`。

包装脚本 `scripts/flash-sd.sh` 执行上述步骤，检测分区起始和大小，并拒绝已挂载的设备、
GPT/多分区卡或承载 `/` 的卡。没有 `--device` 时，它选择唯一的可移动/USB/MMC 磁盘，存在多个时
会询问；`--list` 会显示它们。以你的用户身份运行，而不是通过 `sudo`：它只对 `dd`、`mkfs` 和
`mount` 提权，因此 cargo 保持 `target/` 归用户所有。

```sh
# see which cards/disks the script can pick
scripts/flash-sd.sh --list
# update the loader; keep the partition and its payload
scripts/flash-sd.sh --copy-payload
# recreate the partition as FAT32 (destroys its contents), then copy the payload
scripts/flash-sd.sh --device /dev/sdX --format --yes
```

`--dry-run` 只打印计划；`--skip-build` 复用现有镜像；`--partition`、`--start-lba`、
`--partition-sectors`、`--loader`、`--payload`、`--boot-cfg` 和 `--payload-name` 覆盖检测结果和
默认值。相同的值也可以来自 `SD_DEV`、`SD_PART`、`START_LBA`、`PARTITION_SECTORS`、
`LOADER_IMG`、`PAYLOAD_IMG`、`BOOT_CFG`、`FORMAT_PARTITION` 和 `COPY_PAYLOAD`。除非传入
`--yes`，脚本会在每个破坏性操作前询问（`--format --yes` 示例会跳过两个提示），拒绝扇区 16 中的
`rawboot-fel.bin`，除非给出 `--format` 否则要求分区已经是 FAT，并且只格式化分区，绝不格式化
整个磁盘。

要捕获自启动运行，请在断电重启**之前**启动捕获，然后上电（BROM strap 在冷上电时采样；参见
[捕获自启动的开发板](#capture-a-self-booting-board-no-fel)）：

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

当日志到达 `BOOT: handoff to ...` 且 PD22 随后显示 payload 的短-长节奏（亮 120 ms、灭 180 ms、
亮 600 ms、灭 900 ms）时，运行通过。稳定的 50 ms 翻转意味着 loader 仍在其 `S=0x` 循环中；
静默则指向扇区 16 中的镜像有误。

从受测的代码分支运行 HIL。验证说明和 runbook 更新可以在运行前提交；仅在成功后才记录 PASS。
`cargo xtask image` 会写出 `target/rawboot-fel.bin.build` 和
`target/rawboot-boot-sd.bin.build`，其中包含源码 commit、分支和 dirty 标志；`hil-check.sh` 会
拒绝 commit 不是 `HEAD` 的 FEL 镜像，因此切换分支后要重新构建（`HIL_SKIP_PROVENANCE=1` 是逃生
通道）。

通过冷断电重启（断电、按住 FEL/BOOT、上电）将开发板置于 FEL，而不是用 `xfel reset` 或普通
reset：strap 仅在上电时采样，而 `xfel reset` 通过看门狗复位，看门狗保持启用，因此 loader 随后
会循环启动。上一次运行遗留并仍在运行的 loader 也会继续在 USART0 上打印，因此每次运行前用断电
重启清除它。在终端上脚本会打印此提示并等待 Enter；管道运行或 `HIL_NO_PROMPT=1` 会跳过提示。

`xfel version` 成功**并不**证明开发板在 FEL 中空闲：上一次运行的 loader 在 USART0 上打印时，
FEL USB gadget 仍可能应答。脚本的 UART 静默预检才是权威。

如果上一次运行的 loader 仍在打印，`HIL_SKIP_PREFLIGHT=1` 会排空 USART0 并用受测镜像替换它。
此时预检警告是预期内的：旧的 `S=` 行是过期输出，而不是复位循环。出于同样原因，捕获过程会放弃
复位循环检查，因此在替换旧 loader 时捕获到的过期 `Hello` 行和片段不会再使运行失败；其他所有
断言仍然适用，因此真正缺失的启动行仍会被报告。当 `xfel` 仍能应答时，用它代替反复断电重启；
不能应答时则断电重启。

该脚本：

1. 将镜像构建戳与 `HEAD` 核对；
2. 最多等待 3 分钟以获得有响应的 FEL（`xfel version` 成功）；
3. 要求 UART 在一小段时间内保持静默，这样仍在运行的 loader 会被报告为开发板忙，而不是在后面
   看起来像复位循环（`HIL_SKIP_PREFLIGHT=1` 将此检查降级为警告）；
4. 在加载前排空 UART 输入缓冲区；
5. 将镜像写到 `0x00028000` 并跳转到它；使用 `HIL_FEL_PAYLOAD=1` 时，还会在跳转前将 payload
   镜像和 mailbox 写到 SRAM；
6. 捕获 USART0 数秒，打印唯一行并断言 `Hello`、`CLK:`、`DRAM simple test OK`，以及 loader
   循环（`S=0x`）或 payload handoff（`BOOT: handoff to ...`）之一；使用 `HIL_REQUIRE_SD=1`
   时还会断言 `SD/MMC card found` 和非零的 `SD: read N block(s)`；使用
   `HIL_FEL_PAYLOAD=1` 时要求出现 `FEL: payload` 行；使用 `HIL_REQUIRE_HANDOFF=1` 时要求出现
   handoff 行；使用 `HIL_REQUIRE_PAYLOAD=1`（或 `HIL_FEL_PAYLOAD=1`）时要求 handoff 与主机
   payload 镜像匹配。当出现多个 `Hello` 时失败（复位循环或过期捕获）；使用
   `HIL_SKIP_PREFLIGHT=1` 时会放弃该检查，从而忽略被替换的 loader 留下的输出；
7. 使用 `HIL_REQUIRE_LED=1` 时，请人工确认 PD22 显示 payload 的短-长节奏（亮 120 ms、灭
   180 ms、亮 600 ms、灭 900 ms，循环），而不是 loader 稳定的 50 ms 翻转；在交互式 TTY 上
   （除非 `HIL_NO_PROMPT=1`）它会读取答案，否则要求 `HIL_LED_CONFIRMED=1`。没有该标志时，它
   只打印人工检查提示。

原始捕获保存在 `/tmp/rawboot-hil-<label>.log`。

### 手动加载（无脚本）

```sh
xfel write 0x00028000 target/rawboot-fel.bin
xfel exec 0x00028000
```

对于手动 FEL payload 启动，请在跳转前写入镜像和 mailbox：

```sh
size=$(stat -c %s target/rawboot-payload.img)
xfel write 0x00028000 target/rawboot-fel.bin
xfel write 0x00039000 target/rawboot-payload.img
xfel write32 0x00044004 0x00039000
xfel write32 0x00044008 "$size"
xfel write32 0x0004400c 0
xfel write32 0x00044000 0x4d464252
xfel exec 0x00028000
```

不要先执行 `xfel ddr t113-s3`。它会初始化 DRAM，而 loader 的第二次初始化会在 PLL_DDR 上挂起
（issue #23）。在 loader 自行初始化 DRAM 之前，它只需要 SRAM。

如果手动捕获 UART，请在 `xfel exec` 前排空它（例如使用
`python3 -c 'import serial; s=serial.Serial("/dev/ttyUSB0",115200); s.reset_input_buffer()'`），
并以 115200 8N1 捕获。

### 树内 FEL 传输（无 `xfel`）

`cargo fel` 直接使用 BROM 的 FEL 协议通信；它需要上面的 udev 规则，但不需要外部 `xfel`。这会
端到端检查 issue #40：

```sh
cargo xtask image
cargo fel version        # must print `SoC:        T113-S3 (chip id 0x1859)`
```

向 SRAM 写入一个模式并读回（读取会打印十六进制转储；`--output` 保存字节以供 `cmp` 使用）：

```sh
size=$(stat -c %s target/rawboot-payload.img)
cargo fel write 0x00039000 target/rawboot-payload.img
cargo fel read 0x00039000 "$size" --output /tmp/rawboot-readback.img
cmp target/rawboot-payload.img /tmp/rawboot-readback.img
```

对于 exec，上传一个返回到 BROM 的 stub（`bx lr`），运行它，然后再次读取内存：

```sh
printf '\x1e\xff\x2f\xe1' > /tmp/rawboot-return.bin   # bx lr
cargo fel write 0x00030000 /tmp/rawboot-return.bin
cargo fel exec 0x00030000
cargo fel read 0x00030000 4
```

`cargo fel exec` 仅在 stub 将控制权交还 BROM 时返回；不返回的 stub 会以传输超时结束。
`scripts/hil-check.sh` 仍使用 `xfel`；此 CLI 用于上面的手动检查。

### DDR-init stub 与 flasher（无 SD）

Issue #41 将 FEL 路径拆分为可返回的 DDR-init stub 和链接到 DRAM 的 flasher；issue #42 增加了
SD/FAT job（见下一节）。构建两个镜像并用树内传输运行整个 spike：

```sh
cargo xtask flasher
cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin
```

`bringup` 识别 SoC，将 stub 写到 `0x00028000`，执行它并读取其 SRAM 结果（`0x0002d000`：magic
`RBDD`，然后是 DRAM 大小（MiB）），在 `0x40002000` 写入并读回一个模式，然后在 `0x40000000`
放置一个 `NOOP` job，在 `0x40010000` 放置 flasher，执行它并读取 `0x40000100` 处的状态。预期
尾部为：

```
DRAM:       128 MiB
pattern:    OK at 0x40002000
status:     sequence 1, step 2 (done), result 0 (ok)
```

地址以及 `FlashJob`/`FlashStatus` 布局位于 `rawboot-image/src/flash.rs`（ADR-0013）；CLI 打印
相同的值。此路径刻意**不**使用 `xfel ddr`：stub 才是初始化 DRAM 的东西。

- 如果 stub 不返回，`bringup` 会在下一次 FEL 读取时阻塞直到传输超时，并且开发板不再应答 FEL。
  记录失败的步骤并将 #41 退回 triage；不要用 `xfel ddr` 作为变通重试。
- `DDR-init stub did not report success` 表示 `exec` 返回了，但 SRAM 结果缺失或驱动返回大小
  `0`；开发板仍可用，检查日志后重新运行。
- `DRAM read-back ... returned ...` 指出第一个不匹配的字。不要继续到 flasher；该 DRAM 窗口不可
  信。
- `result` 不是 `0 (ok)` 的状态仍是一次成功的往返，但 job 失败了；CLI 在打印步骤和结果后以
  非零退出。

stub 和 flasher 不带 eGON 头，并且不能与 loader 镜像互换。

### 就地通过 FEL 刷写卡（#42）

卡留在开发板中。通过冷断电重启将开发板置于 FEL，保持卡插入，然后运行：

```sh
cargo xtask fel-flash --yes
```

`--dry-run` 执行相同的构建和验证，打印计划并在不打开 FEL 或不接触卡的情况下退出；用它来检查
产物和路径。实际写入需要 `--yes`。

不带 `--skip-build` 时，命令首先运行 `cargo xtask image`、`payload` 和 `flasher`，然后读取
`target/rawboot-boot-sd.bin`、`target/rawboot-payload.img`、`target/rawboot-ddr.bin` 和
`target/rawboot-flasher.bin`。它会检查 loader 是密封的 SD/SPI 镜像，并拒绝 FEL 变体
（`rawboot-fel.bin`、`variant=fel` 构建戳或匹配的字节），解析 payload 镜像并将 `boot.cfg` 限制
为 4095 字节。然后识别 T113-S3，运行 DDR-init stub，用模式检查 DRAM，在 DRAM 中暂存 flash
计划和三个区域，并执行 flasher。flasher 初始化 USART0 并在那里打印 SD/FAT 驱动诊断，因此失败
的 CMD24 或 FAT 步骤既会出现在状态中，也会出现在 UART 日志中。

选项：

- `--payload <path>` — payload 镜像（默认 `target/rawboot-payload.img`）；它必须是封装后的
  镜像，而不是原始的 `payload.bin`。
- `--boot-cfg <path>` — 还将此文件写为 `boot.cfg`；没有它时，现有的 `boot.cfg` 保持不变。
- `--loader <path>`、`--flasher <path>`、`--ddr-stub <path>` — 覆盖构建出的镜像。
- `--sequence <n>` — job/status 序列（默认 1）。
- `--skip-build` — 复用 `target/` 中已有的镜像；切换分支后要重新构建。
- `--dry-run` — 构建/验证并打印计划；绝不打开 FEL。
- `--yes` — 实际写入所需；没有它时命令验证后停止。

`scripts/hil-flash.sh` 封装了捕获：它启动 UART 记录器，运行 `cargo xtask fel-flash --yes`，
打印主机输出和 UART 尾部，并要求 `step 2 (done), result 0 (ok)`。它不会断电重启；请先冷启动
开发板。

flasher 检查并写入的内容：

1. MBR 具有 `55 AA` 签名和 FAT 分区表条目；没有 MBR 的裸 FAT 卷、没有 FAT 条目的 MBR，以及
   起始于扇区 16 或更早的分区都会被拒绝；
2. 扇区 16 处的 loader 在分区之前结束，且分区位于卡内；
3. `payload.bin`（以及提供时的 `boot.cfg`）在 FAT 根目录中被替换并读回；
4. 只有在这之后，密封的 loader 才被写入扇区 16 并读回；
5. 扇区 0 绝不被写入；flasher 会重新读取 MBR，如果不同则报告 `MBR changed`。

成功时的预期尾部：

```
SoC:        T113-S3
DRAM:       128 MiB
staged:     payload N bytes, boot.cfg 0 bytes, loader N bytes
status:     sequence 1, step 2 (done), result 0 (ok)
card:       payload files and loader written; MBR unchanged
```

**DDR-init 与重复尝试。** stub 不是幂等的：每次运行都会再次调用 `DramPara::init_dram()` 并
等待 PLL_DDR 锁定，因此重复运行可能像 loader 之前的 `xfel ddr` 一样挂起。这与 #23 是不同的
情况，#23 特指在 loader 之前运行外部 `xfel ddr`。HIL #52 测量了重复条件：在同一个电源周期内
运行三次 `bringup`、成功一次后再运行第二次 `fel-flash`，以及 SD 错误后重跑，都返回到了 BROM
并以 `step 2 (done), result 0 (ok)` 结束。然而，后来的一次重复挂起了：在一个电源周期内成功
运行 `raw-write-test` 后，下一次 `fel-flash` 在等待 DDR stub 返回时超时。设备仍在 USB 上枚举，
但 `cargo fel version` 没有应答，开发板需要冷启动。因此重复运行 stub 不是经过验证的路径：每次
冷启动只运行一个 stub，并在重试前断电重启。

| 条件 | 过程 | 预期 | 观察到的 |
| --- | --- | --- | --- |
| 冷启动，一次 `bringup` | 冷断电重启进入 FEL，运行 `cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin` | `DRAM: 128 MiB`、`pattern: OK at 0x40002000`、`status: sequence 1, step 2 (done), result 0 (ok)` | PASS，HIL #52 |
| 同一个电源周期内三次 `bringup` | 不移除电源运行 `bringup` 三次 | 三次相同的成功 | PASS，HIL #46；HIL #52 再次确认 |
| 成功的 `fel-flash` 之后重复 `fel-flash` | 在一个电源周期内运行 `cargo xtask fel-flash --yes` 两次 | 第二次运行报告 `step 2 (done), result 0 (ok)` | HIL #52 上通过一次（在 FEL 中重新插入卡之后）；后来的重复挂起，因此不要依赖它 |
| SD 错误后重复 `fel-flash` | 让 flasher 在状态之前失败（移除卡，或使用起始于扇区 16 或更早的分区），然后在同一电源周期内重新运行 `fel-flash --yes` | 第二次运行报告干净的状态 | HIL #52 上通过一次（第一次运行 `SD card offline`，第二次运行正常）；后来的重复挂起 |
| 四次冷启动，适配器连接，可启动卡 | 断电重启四次，不拔掉适配器也不移除卡 | 只要到达 FEL，DDR-init 就会成功；BROM 可能从卡启动而不是进入 FEL | HIL #52：FEL 到达 4/4；DDR 4/4 PASS；卡 `found` 1/4，`SD card offline` 3/4 |
| 四次冷启动，适配器连接，扇区 16 已失效 | 运行一次 `cargo xtask raw-write-test --yes`，然后断电重启四次，不拔掉适配器也不移除卡 | FEL 是确定性的；DDR-init 每次都成功；SD 可能交替（#43） | HIL #52：DDR 4/4 PASS；SD 奇数次启动 FAIL（2/2），偶数次启动 PASS（2/2） |

仅在所有者于测试台上确认后，才在表中记录结果；不要将未测量的行标记为 PASS。任何开发板或卡变更
后，重做冷启动行。

**连接适配器时的冷启动。** 保持 USB-UART 适配器连接会向开发板反灌电流，因此卡并未真正断电
重启，而扇区 16 中带有可启动 loader 的卡还可能使 BROM 从 SD 启动而不是进入 FEL。在 HIL #52
中，使用可启动卡时，四次此类启动中只有一次枚举了卡；不应答 `cargo fel version` 的启动是从卡
启动了，而不是 DDR-init 失败。在开始一行矩阵之前检查 `cargo fel version`。

要在 BROM 不抢先启动的情况下测量卡的交替，请先使扇区 16 失效：
`cargo xtask raw-write-test --yes` 在那里写入一个 `RBRW` 测试块，保持扇区 0 和 FAT 分区不变，
并使 FEL 具有确定性。使用该卡时，HIL #52 在连接适配器的情况下进行了四次冷启动：DDR-init
通过 4/4，而 SD 交替是严格的（奇数次启动 `SD card offline`，偶数次启动 `done`/`ok`）。之后用
`fel-flash` 恢复 loader。这种交替是 #43（卡电源状态），不是 DDR-init 结果。要进行真正的冷
启动，请在断电前拔掉适配器；当 FEL 进入必须确定时，请移除卡。

flasher 会在状态中报告失败的步骤和原因，`fel-flash` 会在以非零退出前打印它。常见情况：

- `SD init: SD card offline` — 没有卡、未枚举的卡，或 #43 测试台问题；插入卡并冷启动开发板进入
  FEL。
- `read MBR: read failed` — 无法读取卡的第一个块；将卡视为离线，并在冷启动后重试。
- `layout: no MBR` / `layout: no FAT partition` — 卡未准备好；`fel-flash` 不会格式化或创建
  分区。先准备好它（不在范围内），然后重新运行。
- `layout: unsafe card layout` — FAT 分区起始于扇区 16 或更早，或者 loader 会伸入其中。
- `layout: card too small` — MBR 分区或 loader 放不进卡。
- `write payload` / `write boot.cfg` 的 `create file failed`、`truncate failed`、
  `file write failed` 或 `file flush failed` — 卡未接受 FAT 写入；MBR 和分区保持不变，但在
  重跑通过之前不要信任已更新的文件。在 SDSC 测试台卡上这曾是 #47，已在 PR #49 中修复；如果
  再次出现，请将其视为新的写入路径缺陷，而不是 DDR-init 问题。
- `write loader` 的 `write failed` — 写入扇区 16 失败；旧 loader 可能仍在，但在重跑通过之前
  不要从卡启动。
- `verify FAT files` / `verify loader` 的 `read-back mismatch` — 卡没有返回写入的内容；重新
  运行，如果持续出现则更换卡。
- `verify loader` 的 `MBR changed` — 停止，不要从此卡启动；检查它。
- `plan: bad plan or staged image` — 暂存的 plan/loader/payload 未通过验证；用
  `cargo xtask image`/`payload`/`flasher` 重新构建并重跑（不要带 `--skip-build`）。
- 状态之前的 `USB device disconnected` 或 FEL 超时 — stub 或 flasher 没有返回 BROM。将开发板
  冷断电重启进入 FEL 并重跑；不要退回到 `xfel ddr`。优先进行全新的冷启动，而不是在同一电源
  周期内重跑 stub，上表将后者留作未测量。

成功运行后，移除 FEL strap 并从卡冷启动。用 `hil_serial.py` 捕获它（在上电前启动捕获，如
[捕获自启动的开发板](#capture-a-self-booting-board-no-fel)中所述）并断言 handoff：

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

当日志显示 `SD/MMC card found`、`FAT: mounted` 和 `BOOT: handoff to ...`，且 PD22 随后显示
payload 的短-长节奏时，卡路径通过。稳定的 50 ms 翻转意味着 loader 仍在其 `S=0x` 循环中；
静默则指向扇区 16 中的镜像有误。要带上所有断言进行完整 HIL 运行，请使用上面的冷启动流程并
设置 `HIL_REQUIRE_SD=1 HIL_REQUIRE_HANDOFF=1 HIL_REQUIRE_PAYLOAD=1 HIL_REQUIRE_LED=1`。

### 原始 SD 写入诊断（无 MBR/FAT）

当卡写入失败但读取正常时，通过冷 FEL 启动将块写入路径与 FAT 和分区状态区分开，并执行：

```sh
cargo xtask raw-write-test --yes
```

它运行 DDR-init stub，将一个已知的 512 字节块写到 LBA 16，读回它并打印步骤/结果。它需要
`--yes`，并且会覆盖扇区 16（已准备好卡上的 SD loader）；扇区 0 绝不被写入。成功运行以以下
内容结束：

```
status:     sequence 1, step 2 (done), result 0 (ok)
raw test:   sector 16 written and read back identical
```

报告 `step 12 (raw write), result 9 (write failed)` 的写入意味着驱动无法完成 CMD24；
`step 13 (raw verify)` 将读回失败与写入失败区分开。

### 捕获自启动的开发板（无 FEL）

当开发板自行启动（从 SD 或 SPI，没有 `xfel` 步骤）时，在断电重启**之前**启动捕获，并让
`hil_serial.py` 判定。它会排空端口一秒钟（`--drain`）并丢弃读取到的内容，因此请仅在该窗口
之后上电。然后它记录带时间戳的日志，并打印唯一行及断言：

```sh
setsid sg dialout -c 'python3 scripts/hil_serial.py /tmp/boot.log --seconds 10 \
    --require-handoff --payload target/rawboot-payload.img' &
```

仅当 shell 尚未获取 `dialout` 组时才需要 `sg dialout`。在繁忙的开发板上原始日志增长很快
（可能达到数兆字节）；请阅读 `hil_serial.py` 摘要，而不是转储文件。

## 预期日志

当前的 `master` 会打印（插入卡，没有有效的 `payload.bin`）：

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
```

使用有效的 `payload.bin` 时，loader 会打印挂载和 handoff 行，而不是进入其循环：

```
FAT: mounted Fat32 at LBA 0 (987758592 bytes)
ARGS: no boot.cfg
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x00000000
```

当卡上有 `boot.cfg` 时，参数行会指出文件名及其 DRAM 地址，handoff 携带相同的指针：

```
FAT: mounted Fat32 at LBA 0 (987758592 bytes)
ARGS: boot.cfg 13 bytes at 0x47ffefd0
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x47ffefd0
```

使用 `HIL_FEL_PAYLOAD=1` 时，它跳过 SD 初始化并改为打印 mailbox 请求：

```
FEL: payload image 0x00039000, 192 bytes, args 0x00000000
BOOT: handoff to 0x42000000: 192 bytes at 0x42000000, checksum 0x17e889cb, BootInfo 0x47feffe0, SP 0x48000000, args 0x00000000
```

handoff 之后，除非 payload 打印，否则 UART 静默，因此捕获在 `BOOT: handoff to` 行之后结束。
payload 随后闪烁 PD22：亮 120 ms、灭 180 ms、亮 600 ms、灭 900 ms，循环。loader 自己的循环
每 50 ms 翻转一次，因此节奏可以区分已完成的 handoff 与从未离开循环的 loader。地址、payload
大小和校验和取决于镜像头和检测到的 DRAM 大小。

- `CLK: PLL_ddr` 在 `dram.init()` 之前读取，因此显示 DRAM 之前的值；随后的 DRAM 初始化会设置
  为 792 MHz。参见 `research/clk-port-vs-upstream.md`。
- `SD/MMC card found` 和 `SD: read ...` 行证明 SDHCI/IDMAC 路径。前几个字节是块 0 的内容；
  空的测试卡返回零。使用 `HIL_REQUIRE_SD=1` 时，这两行变为断言。
- `BOOT: handoff to ...` 在 payload 被放置、其校验和在目标位置验证并且 `BootInfo` 写入后
  打印。`HIL_REQUIRE_HANDOFF=1` 使其成为强制要求；`HIL_REQUIRE_PAYLOAD=1` 还会将其入口、
  加载地址、大小和校验和与 `target/rawboot-payload.img` 匹配。其 `args` 字段是 `boot.cfg`
  指针，或零。
- `ARGS: boot.cfg ...`（SD 路径）证明 loader 读取了可选文件并保留了以 NUL 结尾的副本；
  `ARGS: no boot.cfg` 表示 payload 得到 `args = 0`。loader 不解析该文件：其含义由 payload
  所有。
- `FEL: payload ...` 证明 loader 读取了 FEL mailbox 并选择了上传的镜像。
  `HIL_FEL_PAYLOAD=1` 使其成为强制要求，并打印来自 mailbox 的参数指针（`args 0x...`）；SD
  路径在 handoff 行上并通过 `ARGS:` 报告相同字段。
- 未插入卡时预期出现 `SD/MMC card not found`。它不会使时钟或 DRAM 检查失效。完整的 SD/MMC
  卡 parity（#20）由 `research/sdcard-port-vs-upstream.md` 覆盖。
- 当 loader 自己的 LED 循环运行时，`S=0x...` 会重复，伴随 50 ms 翻转。脚本在几秒后停止。
  带有有效 `payload.bin` 的卡则会 handoff，打印 `BOOT: handoff to ...` 并闪烁短-长的 payload
  节奏。

## 故障排查

- `xfel` 显示 `No FEL device found`：开发板不在 FEL，或 USB 线是纯数据线且未连接到 OTG/FEL
  端口。重新进入 FEL 并断电重启。
- `xfel` 显示 `Can't connect to device`：用户无法写入设备节点。检查 udev 规则和
  `ls -l /dev/bus/usb/<bus>/<dev>`。
- 预检显示 `UART is not quiet`：上一次运行的 loader 仍在打印（或线路浮空），即使
  `xfel version` 有应答。使用 `HIL_SKIP_PREFLIGHT=1` 重跑以排空它并替换 loader，或断电重启
  进入 FEL（不要用 `xfel reset`）后重跑。
- `image commit ... does not match HEAD`：镜像早于分支切换或提交。在受测分支上用
  `cargo xtask image` 重新构建。
- `... is linked at 0x..., expected 0x...`：loader 使用了错误的链接脚本。`cargo xtask image`
  会构建两个变体，并在此处失败而不是生成无法启动的镜像；检查 `rawboot/build.rs` 和
  `rawboot-rt/link*.x`。
- 从 SD 冷上电后静默且 PD22 保持常亮：扇区 16 中是 FEL 镜像（`0x00028000`），而不是
  `target/rawboot-boot-sd.bin`（`0x00020000`）。这两个镜像不能互换；用 `cargo xtask image`
  和 `cargo xtask sd-image --partition-sectors <n>` 重新构建并重写 boot prefix，随后执行
  `dd ... conv=notrunc`。
- HIL 失败并显示 `missing SD/MMC card found` 或 `missing a successful SD read`：没有卡、卡
  不可读，或没有卡却设置了 `HIL_REQUIRE_SD=1`。插入卡并重跑，或取消该标志以进行无卡测试。
- 在已插入卡的情况下冷上电后出现 `SMHC: cmd1 timeout (rint=0x00000100, flag=0x00000004)`，
  随后是 `SD/MMC card not found`：CMD1 是 MMC 回退，因此 SD 路径已经失败（CMD8 或 ACMD41）。
  首先检查卡是否真的断电：保持连接的 USB-UART 适配器可能向开发板反灌电流并让卡保持有状态，
  这会使未修改的 `master` 上冷启动交替 PASS/FAIL（issue #43）。在开发板断电时拔掉适配器，
  然后重新运行上面的真正冷启动流程。通过运行仍可能在 `SD/MMC card found` 之前记录较早的
  `cmd1 timeout`，因为驱动会重试。扇区 16 中带有有效 boot prefix 的卡还会使 BROM 在冷上电时
  偶尔从 SD 启动而不是采样 FEL；当你需要确定性的 FEL 进入时，请移除卡。
- HIL 失败并显示 `missing a payload handoff`：设置了 `HIL_REQUIRE_HANDOFF=1`，但 loader 仍
  停留在其 `S=0x` 循环中。检查 `payload.bin` 存在且有效，并确认卡路径成功。
- `BOOT: boot.cfg: is N bytes, but the buffer holds 4095`：可选参数文件超出 4 KiB 窗口
  （包含 NUL 终止符）。缩短它；loader 拒绝用截断的参数进行 handoff。
- HIL 失败并显示 `payload mismatch`：卡上的 `payload.bin` 不是 HIL 比较的镜像。用
  `cargo xtask payload` 重新构建，并将 `target/rawboot-payload.img` 复制覆盖 FAT 分区上的
  `payload.bin`，或通过 `HIL_PAYLOAD_IMG` 传入你实际写入的镜像。
- 日志显示 `BOOT: handoff to ...`，但 PD22 保持 loader 稳定的 50 ms 翻转：卡上是旧的 payload。
  用 `cargo xtask payload` 重新构建，并将 `target/rawboot-payload.img` 复制覆盖 FAT 分区上的
  `payload.bin`。
- HIL 失败并显示 `LED pattern not confirmed` 或
  `HIL_REQUIRE_LED=1 needs an interactive prompt`：确认短-长节奏并重跑，或在检查开发板后设置
  `HIL_LED_CONFIRMED=1`。
- HIL 失败并显示 `payload image not found`：运行 `cargo xtask payload`，或将 `HIL_PAYLOAD_IMG`
  指向你写入卡的镜像。
- 在 `HIL_FEL_PAYLOAD=1` 下 HIL 失败并显示 `missing FEL: payload`：mailbox 的 magic/地址没有
  到达 loader，或 loader 早于此路径。用 `cargo xtask image` 重新构建，并对照 mailbox 表检查
  `xfel write32` 的值。
- loader 打印 `BOOT: FEL payload image ... is outside the SRAM staging window`：mailbox 的
  镜像地址/大小错误，或镜像超出 44 KiB 窗口。将镜像保持在 `0x00039000`，如果它变大则用
  `cargo xtask payload` 重新构建。
- HIL 在同时设置 `HIL_REQUIRE_SD` 时拒绝 `HIL_FEL_PAYLOAD=1`：FEL 路径跳过 SD 卡；FEL 运行时
  取消 `HIL_REQUIRE_SD`。
- UART 静默：检查 USART0 PE2/PE3、115200 8N1，以及 CH340 是否为 `/dev/ttyUSB0`。
- 日志以重复的 `Hello`/`S=` 行开头：tty 缓冲区仍保存着上一次运行的内容，或开发板处于复位
  循环。脚本会排空缓冲区，并在出现多个 `Hello` 行时使运行失败；断电重启开发板（不要用
  `xfel reset`）并重跑，或设置 `HIL_SKIP_PREFLIGHT=1` 以排空并替换正在运行的 loader 并忽略
  那些过期行。
- loader 打印 `CLK: init timeout at ...`：某个时钟 PLL 未锁定；捕获完整的行和寄存器地址以供
  诊断。
