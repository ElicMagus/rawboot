# SD boot prefix：MBR 和 sector 16 中的 loader，不含自己的 FAT

BROM 从 sector 16 加载 SD/SPI 镜像到地址 `0x00020000`，而 loader 从 FAT 分区读取 `payload.bin`。此前没有将 loader 放到卡上的主机工具：`cargo xtask image` 只生成 sealed 镜像，而卡的布局是手动组装的。在 bring-up #37 中得到了一个可用的 prefix：1 MiB（2048 个 sector）、带一条类型为 `0x0c`、位于 LBA 2048、大小为 1843200 个 sector 的记录的 MBR、disk signature `0`、CHS `FE FF FF`、偏移 8192 处的 loader。它不可复现。

决策：`cargo xtask sd-image --partition-sectors <n> [--start-lba <n>] [--loader <path>]` 生成 `target/rawboot-sd-mbr.img`——**SD boot prefix**。这是一个恰好为 `--start-lba` 个 sector（默认 2048，即 1 MiB）的文件，除两个区域外均填充为零：

- sector 0——MBR：disk signature `00000000`，446 处一条记录，boot flag `0`、类型 `0x0c`、CHS start/end `FE FF FF`、start LBA `--start-lba` 和 sector 数 `--partition-sectors`；510 处签名为 `55 AA`；
- 偏移 8192（sector 16）——sealed loader `target/rawboot-boot-sd.bin` 的字节原样放置。

prefix 中没有 FAT、BPB 和 payload。MBR 只按其地址引用已存在的 FAT 分区；格式化和复制 payload 仍由外部工具完成。`cargo xtask sd-image` 写入的是文件，而不是块设备；用 `dd ... conv=notrunc` 将其放到卡上。

`cargo xtask sd-image` 仍然是文件工具。对于 HIL，有 `scripts/flash-sd.sh`：它调用 xtask，自行查找磁盘和分区，拒绝写入已挂载的磁盘、带根文件系统的磁盘、GPT/多分区卡以及没有 MBR 的卡，检查大小并通过 `dd` 写入 prefix。根据标志，它只格式化分区（`mkfs.vfat -F 32`）并复制 payload。对块设备、权限和 `sudo` 的操作存在于 shell 包装脚本中，而不是 xtask 中：构建工具保持可测试且不需要权限。

## 考虑过的选项

- **SD boot prefix**（已采用）：卡保留现有分区及其 `payload.bin`；产物很小（1 MiB），在主机上构建，并通过 host 测试逐字节检查。prefix 大小和分区地址由标志指定，因此该工具不会硬编码特定卡。风险是写入错误的设备；HIL 包装脚本的检查（挂载状态、根磁盘、MBR、单一分区）以及写入前的确认限制了这一点。写入会擦除介质的第一个 MiB，因此只能使用 `conv=notrunc`，且只能写入已有分区就绪的卡。
- **带空 FAT 的完整镜像**（已拒绝）：该工具将不得不格式化 FAT 并创建 BPB，也就是重复 `mkfs.vfat`。对于 1843200 个 sector 的分区，镜像将重约 900 MiB，每次 payload 更改后都需要重新构建，并且每张卡都需要单独的大小。格式化和复制 payload 不是引导加载程序的任务。
- **直接写入块设备**（xtask 已拒绝）：xtask 将不得不具备设备查找、权限检查和平台逻辑；选错磁盘的错误是不可逆的。而文件可以用任何熟悉的工具检查、比较和写入，包括在所有者控制下的 `dd`。对于 HIL，这一角色由单独的 shell 包装脚本 `scripts/flash-sd.sh` 承担，它调用 xtask 并添加设备检查；xtask 本身仍然是文件工具。

## 后果

- `build_sd_image` 位于 `rawboot-image`，并由 host 测试检查：MBR 字节、loader 放置、prefix 长度、空或无效 eGON、零大小、溢出以及 loader 进入分区时的拒绝。sealed eGON 的校验已移至 `rawboot-image::egon::verify_sealed`；`cargo xtask image` 和 `cargo xtask sd-image` 都会调用它。
- `cargo xtask sd-image` 需要 `--partition-sectors`（分区的实际大小），并在 `scripts/check.sh` 中作为 smoke 构建调用。
- HIL runbook：将 prefix `dd` 到已有 FAT 分区的卡上，仅在分区上执行 `mkfs.vfat` 并复制 payload。格式化整个设备会破坏 sector 16 中的 loader。
- `GLOSSARY.md` 中新增术语“SD boot prefix”，以免将产物与完整磁盘镜像混淆。
