# FEL flasher：返回式 DDR-init stub 与 DRAM 中的 job/status 契约

FEL 的 SRAM 窗口（镜像约 44 KiB）小于 sealed loader，而 flasher 的数据无法放入 SRAM。不能通过 `xfel ddr` 从外部初始化 DRAM：之后 loader 会再次初始化 DRAM 并使其挂起在 PLL_DDR 上（#23）；这是关于 `xfel ddr` 的断言，而不是关于返回式 stub 的。因此主机必须自行获得 DRAM，方法是加载一个小 stub，它初始化 DRAM 并**返回 BROM**，让 FEL 保持存活。之后主机将 job 和 flasher 放入 DRAM，执行它并读取状态；本任务（#41）中尚未触及 SD。

决策：两个不带 eGON 头的独立镜像，`rawboot-image` 中的共享契约以及 `rawboot-fel` 中的主机 API。

## 地址与窗口

| 项目 | 地址 | 大小 |
| --- | --- | --- |
| SRAM 中的 DDR-init stub | `0x00028000` | `0x5000`（20 KiB） |
| stub 的结果（magic + MiB） | `0x0002d000` | 8 字节 |
| DRAM 中的 `FlashJob` | `0x40000000` | 36 字节 |
| DRAM 中的 `FlashStatus` | `0x40000100` | 28 字节 |
| DRAM 模式校验 | `0x40002000` | 4 KiB |
| DRAM 中的 flasher | `0x40010000` | 至 `0x3c000`（240 KiB） |
| flasher 的栈 | `0x4004c000..0x40050000` | 16 KiB，向下增长 |

stub 的窗口恰好在结果开始处结束；实际镜像现在约为 9 KiB。flasher 窗口为 256 KiB，但主机接受的镜像不超过 240 KiB：顶部 16 KiB 保留给栈，链接器检查 `.bss` 不会进入其中。
结果地址选在外部 `xfel` 的 DDR payload 所占用的范围（`0x28000..0x2de54`）内，并且 BROM 已证明在其中保持自己的栈完整：stub 返回，下一个 FEL 请求得到应答。flasher 在 DRAM 中获得自己的栈，并在入口保存 BROM 的 `sp`/`lr`，因此其体积不受 BROM 栈的限制。

## job/status 契约

`FlashJob` 和 `FlashStatus` 是仅由 `u32` 组成的 `#[repr(C)]` 结构，但契约是小端字节布局，而不是 Rust 结构。两个块都有：

- `magic`（`RBFJ` / `RBFS`）、`version`（`1`）和 `size`（36 / 28）；
- `checksum`——整个块的小端 32 位字之和，其中校验和字段置零；
- `sequence`——任务的连续编号，以免主机接受别人的/过期的状态。

job 之后是 `command`、`arg0..arg2`；status 是 `step` 和 `result`。在 #41 中实现了命令 `NOOP`、状态 `DONE`/`OK`（或 `BAD_JOB`/`UNKNOWN_COMMAND`）；SD 写入命令将在 #42 中出现，布局不变。`rawboot-image::flash` 由 host 测试检查：round-trip、每个损坏部分（magic、version、size、checksum、截断）以及不存在地址重叠（`const` 断言）。

## 镜像

`rawboot-flasher`——一个独立的 bare-metal 包，包含两个 `[[bin]]`：

- `rawboot-ddr-init` 链接到 SRAM 的 `0x28000`，调用共享的 `sunxi_drivers::dram` 并返回 BROM；结果（magic `RBDD`、以 MiB 为单位的大小）留在 SRAM 中。镜像的 BSS 在调用驱动之前清零；栈是 BROM 的，与外部 `xfel` 的 DDR payload 一样。
- `rawboot-flasher` 链接到 DRAM 的 `0x40010000`，具有 NOLOAD 段 `.bss` 和 `.stack`，切换到 DRAM 栈，读取 job，写入 status，并在恢复栈后返回 BROM。

两个镜像都不带 eGON 头，也不是第二 stage。`cargo xtask flasher` 构建两者，检查 `.text` 的 VMA 是否在契约地址上、flat 二进制开头是否没有 `eGON`，以及窗口大小。构建包含在 `scripts/check.sh` 中。

## 不返回与 panic

两个镜像仅在正常终止时才返回 BROM。发生 panic 时，`#[panic_handler]` 会停驻：控制权不会返回 BROM，FEL 连接停止应答，主机的下一个请求会遇到传输超时。这就是可观察的故障信号。从 panic handler 写入 `FlashStatus` 没有意义：主机仅在从 `exec` 返回后才读取状态，而停驻的镜像已不再将控制权交还 BROM。主机不试图区分 panic 与其他不返回情况；反应只有一个——冷启动并重试。

## 主机

`rawboot-fel` 获得 `FelDevice::{run_ddr_stub, verify_dram, run_flash_job, bringup}`；CLI：

```sh
cargo xtask flasher
cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin
```

`bringup` 确认 T113、执行 stub、读取其结果、在 DRAM 中写入并回读模式、放置 NOOP job 和 flasher、执行它，并检查状态的 magic/version/size/checksum 和 `sequence`。如果 stub 没有返回，下一个 FEL read 会遇到传输超时：这就是 go/no-go 信号，而不是静默回退。

## 考虑过的选项

- **返回式 stub + DRAM job/status**（已采用）：FEL 保持可用，数据不受 44 KiB 限制，主机 API 在 #42 中复用，契约无需板子即可测试。
- **`xfel ddr`**（已拒绝）：外部工具，之后 loader 会再次初始化 DRAM 并挂起在 PLL_DDR 上（#23），而且这不是我们的架构。这与重新运行返回式 stub 无关；其条件在 ADR-0014 和 runbook 中描述。
- **一个不返回的 flasher**（已拒绝）：状态将不得不由设备端 USB/FEL 服务器提供，这超出了任务范围。
- **SRAM 中的 job**（已拒绝）：stub 之后主机无论如何都有 DRAM；SRAM 窗口需要留给 stub 以及与 loader 的 FEL 路径兼容。
- **现在就写 SD**（已拒绝）：先做 go/no-go 探针；SD/FAT 是 #42。

## 后果

- 新的 bare-metal 包 `rawboot-flasher` 在决策时属于 `default-members`，因此 `cargo build --release`、`cargo clippy` 和 `cargo xtask flasher` 在常规集合中检查两个镜像；后来它从中移出（ADR-0014），`cargo xtask flasher` 通过单独的 cargo 调用构建它。
- `rawboot-image` 发布地址和 `FlashJob`/`FlashStatus`；`rawboot-fel` 依赖它们。
- `rawboot-fel` 获得命令 `bringup` 以及 stub、DRAM 模式、契约和 sequence 的错误。
- loader、其 FEL mailbox 和 SD 路径未改变。
- HIL：stub 必须返回，DRAM 模式必须匹配，而 `bringup` 必须打印 DRAM 大小和 `result 0 (ok)`。如果 stub 不返回，该事实记录在 #41 中，任务返回 triage；回退到 `xfel ddr` 不视为解决方案。
- `GLOSSARY.md` 中新增“DDR-init stub”、“flasher”和“flash job/status”。

> 更新 #42：SD 命令已添加，job/status 布局不变，但带有单独的 `FlashPlan` 和 DRAM 中的 staging 区域；`rawboot-flasher` 不再属于 `default-members`，以免 `sunxi-drivers/write` 统一到 loader 中。参见 ADR-0014。
> `default-members` 的组成、`write` feature 以及驱动 host 测试的位置在 ADR-0015 中进一步明确。
