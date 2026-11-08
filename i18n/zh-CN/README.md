[English](../../README.md) | [简体中文](README.md) | [Русский](../ru/README.md)

# rawboot

面向全志 T113-S3（ARMv7-A，双核 Cortex-A7）的第二阶段引导加载程序，使用
100% Rust 编写，移植自 [`szemzoa/awboot`](https://github.com/szemzoa/awboot)。
它负责初始化 SoC，通过 SDHCI 从 SD 卡上的 FAT 分区读取原始 payload 镜像，
将其加载到 DRAM，并通过带版本号的 `BootInfo` 契约移交控制权。payload 是我们
自己的固件，而不是 Linux：没有 FDT、没有 PSCI，也没有 initrd。

## 工作区

Cargo 工作区成员：

- `rawboot` — 引导加载程序（SoC 初始化、存储、移交）
- `rawboot-rt` — 运行时（eGON 头、`_start`、链接脚本）
- `sunxi-pl` — 共享平台层（MMIO、GPIO、定时器），供加载器、运行时和 payload 使用
- `sunxi-drivers` — 位于 `sunxi-pl` 之上的共享驱动（时钟、DRAM、UART、SDHCI、SD/MMC、FAT）
- `payload` — 加载器启动的 LED 闪烁测试 payload 镜像
- `rawboot-image` — 可在主机上测试的 payload/eGON 镜像、SD 启动前缀和 flash 契约
  （`build`、`parse`、`seal`、`verify_sealed`、`build_sd_image`、`FlashJob`、`FlashStatus`、
  `FlashPlan`、`check_sd_layout`）
- `rawboot-fel` — 主机端 FEL USB 传输与 CLI（`version`、`read`、`write`、`exec`、
  `bringup`），以及用于板载更新路径的 `flash_sd_card`
- `rawboot-flasher` — 可由 FEL 调用的 DDR 初始化 stub 和链接到 DRAM 的 flasher；
  通过 FEL 写入 payload 文件和 SD 加载器
- `xtask` — 主机端构建工具（`image`、`payload`、`flasher`、`sd-image`、`fel-flash`）

目标平台：`armv7a-none-eabi`（软浮点），Rust edition 2024，由
`rust-toolchain.toml` 固定。

## 构建与测试

```sh
cargo build --release
```

运行主机端测试（工作区默认目标是裸机，因此需显式指定主机三元组）：

```sh
cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers \
  --target x86_64-unknown-linux-gnu
```

生成并封装加载器镜像：

```sh
cargo xtask image
```

该命令写出用于 FEL 的 `target/rawboot-fel.bin` 和用于 SD/SPI 启动的
`target/rawboot-boot-sd.bin`。`scripts/check.sh` 运行完整的本地检查：格式、
lint、构建、主机测试和镜像冒烟构建。

要更新留在板上的 SD 卡，请将板子置于 FEL 模式并运行
`cargo xtask fel-flash --yes`；它会构建或复用镜像，写入 `payload.bin` 和
加载器，保留 MBR 与 FAT 分区，并回读校验所有内容。
`cargo xtask fel-flash --dry-run` 会在不打开 FEL 的情况下验证并打印计划。

术语见 `GLOSSARY.md`，架构决策见 `docs/adr/`。

## 许可

本项目采用
[知识共享署名 4.0 国际许可协议](https://creativecommons.org/licenses/by/4.0/)
（SPDX：`CC-BY-4.0`）发布。完整法律文本见 [`LICENSE`](../../LICENSE)。允许在署名
的前提下复用和修改。

## 致谢

本项目在 AI 智能体 DeepSeek V4.1 Flash 与
[matt-pocock-skills](https://github.com/mattpocock/skills) 技能集的协助下开发，
并受到 [`szemzoa/awboot`](https://github.com/szemzoa/awboot) 的启发。
