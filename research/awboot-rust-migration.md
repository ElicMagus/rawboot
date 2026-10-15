# Migrating `szemzoa/awboot` (Allwinner T113-S3, ARMv7-A) to Rust — Primary-Source Feasibility Report

Researched: 2026. Target: Allwinner T113-S3 specifically. All claims are cited inline to the exact
primary source (source file, official docs, crate metadata, or first-party repo). Items that could
not be verified against a primary source are marked **UNVERIFIED**.

Reference numbers in square brackets map to the **Sources** section at the end.

---

## 1. Executive summary

- awboot is a small, bare-metal C bootloader for T113-S3/T113-S4/V851s built with an
  `arm-none-eabi` toolchain, `-mcpu=cortex-a7`, hard-float, `-Os`, producing four FEL/SPI/SD/eMMC
  images, each wrapped in an Allwinner "eGON" BROM header [1][2].
- The boot chain is: BROM → loads a tiny stage from SD sector 16 (8 KB) or, as a fallback on
  newer SoCs explicitly including T113, sector 256 (128 KB), from SPI NOR/NAND, or via USB FEL;
  BROM validates the eGON header checksum (`0x5F0A6C39` seed) [1][4][40]. awboot emits the eGON
  header directly from `start.S` and uses `mksunxi` only to recompute the checksum, fix the length,
  and pad the image [3][4].
- Rust bare-metal on ARMv7-A is viable **on stable**: `armv7a-none-eabi` and `armv7a-none-eabihf`
  are Tier-2 `no_std` targets present in rustc 1.99.0 (2026-09-28) and installable via rustup
  [20][21]. There is **no** `armv7a-unknown-none-eabi` target; that name does not exist [21].
  The `rust-embedded/aarch32` project provides `aarch32-rt` 0.5.0 for startup/exception vectors and
  `aarch32-cpu` 0.4.0 for CPU support (MIT/Apache-2.0, MSRV 1.93) [22][23].
- **No existing Rust project targets the ARMv7-A Allwinner T113.** The Allwinner Rust ecosystem is
  entirely RISC-V (D1/D1s) or PAC-only: `rustsbi-d1` and oreboot's Allwinner port are T-Head C906
  **RISC-V** [24][25][28]; `smaeul/sun20i_d1_spl` is C [42]. oreboot supports ARMv7-A only in
  parked QEMU emulation [28].
- Reusable Rust crates exist for the "easy" peripherals and filesystems: `fatfs` 0.3.6 (no_std,
  MIT) [31][32], `embedded-sdmmc` 0.10.0 (no_std, SD/MMC + FAT, MIT/Apache) [36], and the
  `fdt` crate (no_std, MPL-2.0) [34][35]. For the Synopsys DesignWare MSHC (the `sunxi_sdhci`
  IP), `dwmmc-host` (Apache-2.0, no_std, FIFO/IDMAC) and `sdhci-host` (ADMA2) are recent,
  directly relevant backends for `sdmmc-protocol` [37][38].
- **The dominant risk is the DRAM controller initialisation** (`dram.c`, ~40 KB). It is undocumented
  register/timing code, self-described as decompilation-derived, whose algorithms (auto rank/width
  and size detection, ZQ, DQS gating, PHY AC remapping) are not trivially translatable [14]. U-Boot
  mainline carries the same code reworked [41]. The pragmatic recommendation is to keep this one
  file (and the secure-monitor/PSCI asm) in C behind FFI for the first port, and port the rest.

### Feasibility verdict

**Feasible, with a phased C/Rust hybrid.** The CPU/ABI/toolchain story is solved, the peripheral
drivers and FAT/FDT pieces are largely reusable, and the risk concentrates in two isolated,
well-bounded modules (DRAM init; secure-monitor/PSCI). A full "100% Rust" rewrite is feasible but
disproportionately expensive because of the DRAM microcode and the secure-state GIC/CPUCFG dance.

---

## 2. T113-S3 boot chain

### 2.1 SoC and toolchain

- awboot targets "Allwinner T113-s3, T113-s4, V851s" [1]. Its build flags are
  `-mcpu=cortex-a7 -mthumb-interwork -mthumb -mno-unaligned-access -mfpu=neon-vfpv4
  -mfloat-abi=hard`, i.e. an ARMv7-A Cortex-A7 with NEON/VFPv4 and the hard-float ABI [2].
  (The T113-S3 datasheet itself was not directly retrieved; the Cortex-A7 identification rests on
  awboot's own `-mcpu=cortex-a7` and the shared R528/T113 DRAM code — see §4 — and is
  **UNVERIFIED against the vendor datasheet**.)
- The machine tree lives under `arch/arm32/mach-t113s3/` [19]. Separate `mach-t113s4` and
  `mach-v851s` trees exist, sharing identical `sdmmc.h`, `dram.h` and `memcpy.S` blobs
  (confirmed by identical git blob SHAs in the tree listing) [19].

### 2.2 FEL / USB

- FEL boot uploads awboot with `xfel` to the FEL load address and jumps to it; `fel.sh` sets
  `BOOT_ADDR=0x00028000`, runs `xfel ddr t113-s3`, writes the kernel to `0x42000000`, the DTB just
  under the top of 128 MiB SDRAM, and optional initrd, then `xfel exec` [5].
- awboot's FEL image is linked at `__RAM_BASE=0x00028000` (128 KB SRAM limit), while the persistent
  boot image is linked at `0x00020000` (160 KB in boot mode) [2][6]. `link.ld` declares a 128 KB
  SRAM region plus a 128 MiB DDR region at `0x40000000` [6].
- The kernel/DTB/initrd hand-off uses four "mailboxes" at `0x43100000`: initrd size (+0x0), initrd
  start (+0x4), DTB address (+0x8), kernel address (+0xC). `fel.sh` writes them with `xfel write32`
  and `main.c` reads them back in `apply_fel_mailboxes()` [5][8][7].
- `xfel` is a C program (repo language "C", MIT) supporting T113 and the `spinor`/`spinand`
  commands used for SPI flashing [29][30].

### 2.3 eGON header (what awboot actually encodes)

- awboot's `start.S` **emits the eGON header itself** as the first bytes of `.text`: the ARM branch
  word `0xea000016`, ASCII magic `eGON.BT0`, a placeholder checksum `0x12345678`, the SPL size
  symbol `__spl_size`, boot-header size `0x30`, boot-header version word `0x30303033`
  ("3030"/"300"), return value `0x00020000`, run address `0x00028000`, and the platform string
  bytes `34 2e 30 00` ("4.0\0") [3].
- `tools/mksunxi.c` defines the same 56-byte `boot_head_t` layout (`instruction`, `magic[8]`,
  `checksum`, `length`, `spl_signature[4]`, `fel_script_address`, `fel_uenv_length`,
  `dt_name_offset`, `reserved1`, `boot_media`, `string_pool[13]`) [4].
- `mksunxi` only: reads the binary, aligns `length` to the padding (512 for block devices, 8192 for
  flash), seeds the checksum with `0x5F0A6C39`, sums all 32-bit words over the aligned length, writes
  the checksum back, and (optionally) expands each 2 KB page into `pagesize` (for large-page SPI
  flash the BROM assumes 2 KB pages) [4].
- The Makefile invokes `mksunxi <bin> 512` for FEL/SD/eMMC and `mksunxi <bin> 8192` plus
  `8192 4096` for the SPI variants [2].

### 2.4 SD card and SPI placement

- awboot README: SD boot writes `awboot-boot-sd.bin` with `dd ... bs=1024 seek=8` — MBR offset
  8 KB (sector 16) — or 128 KB (sector 256) for GPT [1].
- The linux-sunxi wiki confirms the mechanics: the 8 KB offset "is dictated by the BROM, it will
  check for a valid eGON/TOC0 header at this location", and "Newer SoCs (tested on H2+, A64, H5,
  H6, T113) can also load the SPL from sector 256 (128KB) ... if no valid eGON/TOC0 signature is
  found at 8KB" [40]. This is the primary-source corroboration for awboot's 8 KB/128 KB claim.
- SPI: the SPI image is written three times (pages 0, 32 and 64 at 2 KB stride), together with the
  `4k` page-size variant produced by the second `mksunxi` invocation, to tolerate BROM's 2 KB page
  assumption and flash geometry [2][4].
- The BROM general boot order (SD0 → NAND → SD2 → SPI → FEL for older sunxi, with a FEL/BSP pin
  override) is documented on linux-sunxi [39]; the exact T113 order is **UNVERIFIED** in a
  T113-specific primary source, but the T113 supports SD/SPI/FEL boot [40].
- Kernel source used is `smaeul/linux` branch `d1/all` [1][45].

### 2.5 awboot runtime boot flow

`main()` performs, in order: `sunxi_clk_init()`, `board_init()` (UART on USART0, PE2/PE3 mux 6),
`sunxi_dram_init()`, `sunxi_wdg_set(10)`, then either SDHCI→SD/MMC→FatFs or SPI-NAND load, then
`boot_image_setup()` (validates the Linux zImage magic and computes the entry), then FDT mutation
(bootargs, `/memory` reg, initrd start/end), then disables MMU/D-cache/I-cache/interrupts and calls
`boot_linux_psci()` [7][8][9]. `boot_linux_psci()` runs `armv7_init_nonsec()` and then the
`_do_nonsec_entry` trampoline with the kernel entry in r0..r2 [7][12].

---

## 3. Bare-metal Rust on ARMv7-A Cortex-A7 (no_std) in 2026

### 3.1 rustc targets and tier status

- `armv7a-none-eabi` and `armv7a-none-eabihf` (and the `thumbv7a-none-eabi{,hf}` variants) are
  documented on a single rustc book page as **Tier 2**, core+alloc, bare-metal `#![no_std]`;
  `-eabi` is soft-float, `-eabihf` hard-float (needs VFPv3-D16 minimum) [20].
- Verified on the local toolchain, `rustc 1.99.0 (b940084d7 2026-09-28)`: `rustc --print target-list`
  contains `armv7a-none-eabi` and `armv7a-none-eabihf`; `rustup target list` offers both for
  install [21].
- The name `armv7a-unknown-none-eabi` **does not exist** and is not a valid target [21]. The correct
  bare-metal names have no `unknown` vendor component.
- Tier 2 means rustup-distributed precompiled `core`/`alloc` (and `std` where applicable) but no
  host tools; a custom target JSON is **not required** for the Cortex-A7 (a JSON is only needed if
  you want a non-shipped CPU feature default). Target features such as `+vfp4`, `+d32`, `+neon`
  are enabled via `-C target-feature=` or `.cargo/config.toml` [20].
- `asm!`/`global_asm!` are stable; no nightly feature is required for startup code. (Nightly is only
  needed for exotic `#![feature(...)]` set-ups.)

### 3.2 Startup, linker scripts, cache/MMU

- awboot's start-up work — vector base (`VBAR`), endianness, disabling caches/MMU, TLB/invalidate,
  enabling VFP/NEON, stack, BSS clear, `CNTFRQ=24 MHz`, branch to `main` — is ordinary ARMv7-A asm
  that is expressible in Rust `global_asm!` or the `aarch32-rt` runtime [3][23].
- The `rust-embedded/aarch32` project maintains `aarch32-rt` 0.5.0 (runtime/startup, features
  `eabi-fpu`, `el2-mode`, `fpu-d32`, `svc-stack-interrupt`) and `aarch32-cpu` 0.4.0 (CPU/register
  support), both MIT/Apache-2.0, MSRV 1.93, first released 2025-10-31 and actively updated through
  2026-09 [22][23]. These are the recommended foundation for the port's CR0/CR1 coprocessor and
  exception-vector handling.
- The linker script can be ported directly; it is plain GNU ld (`OUTPUT_FORMAT("elf32-littlearm")`,
  `OUTPUT_ARCH(arm)`, MEMORY/SECTIONS with `__spl_size` computed as `__spl_end-__spl_start`) [6].

---

## 4. Prior art & reusable crates

### 4.1 Prior art — what exists, and why none of it is a T113 bootloader

| Project | Arch | What it does | License | Reusability for T113-S3 |
|---|---|---|---|---|
| `rustsbi/rustsbi-d1` | **RISC-V** (T-Head C906) | SBI firmware/SPL for D1 boards; uses riscv hart/PLIC/trap CSRs and a `magic.rs` DRAM blob | no license declared in repo metadata | **Not applicable** to ARMv7-A: the `see/` and `spl/` crates are RISC-V (`riscv_spec.rs`, `hart_csr_utils.rs`, `plic.rs`) [24][25] |
| `rustsbi/rustsbi-allwinner-nezha` | RISC-V | "Bootloader environment for Allwinner Nezha ... XuanTie C906 RISC-V core" | none; repo is **archived and empty** (size 0) [26] | Not applicable (RISC-V, dead) |
| `oreboot` | x86, RISC-V, ARM in QEMU | "coreboot without C", LinuxBoot payloads; Allwinner D1 port is RISC-V; ARMv7-A is parked (`qemu-armv7` in `src.broken`) | GPL-2.0 | No ARM sunxi/T113 mainboard; architecture patterns and DWMMC knowledge only [27][28] |
| `smaeul/sun20i_d1_spl` | RISC-V | "Mainline-friendly SPL for D1" | none | C, RISC-V; not reusable [42] |
| `xboot/xfel` | host tool | FEL/flash tool supporting T113 | MIT | C; usable as-is, not a Rust port target [29][30] |
| `Razican/aw-fel` + `fel-cli` | host tool | Pure-Rust FEL library/CLI | — | Last release 0.5.2, 2018; **T113 support UNVERIFIED** [43] |
| `duskmoon314/aw-pac` | D1/R528/V853 | SVD-generated peripheral access crates (`d1-pac`, `r528-pac`, `v853-pac`) | — | R528 is the ARM sibling of T113; register offsets may transfer, but PACs are D1/RISC-V oriented — ARMv7-A usability **UNVERIFIED** [43] |

Conclusion: for T113-S3 specifically, there is **no prior Rust bootloader**; a port starts from
first principles for the SoC-specific parts.

### 4.2 Reusable crates (verified metadata)

- **CPU/runtime:** `aarch32-cpu` 0.4.0 and `aarch32-rt` 0.5.0 (MIT/Apache-2.0, MSRV 1.93)
  [22][23].
- **FAT:** `fatfs` 0.3.6 (MIT, no_std via `default-features = false`; last published 2023-01-17)
  [31][32]. Note: `embedded-fatfs` on crates.io is **not** a real crate — it is a 3-line `0.0.0`
  name-squat placeholder published 2023 [33]. The active embedded FAT project is the `fatfs` crate
  itself.
- **SD/MMC + FAT:** `embedded-sdmmc` 0.10.0 (MIT/Apache-2.0, no_std, MSRV 1.87), which bundles an
  SD/MMC protocol driver and a FAT filesystem behind a `BlockDevice` trait [36].
- **FDT:** `fdt` 0.1.5 stable / 0.2.0-alpha3 (MPL-2.0, pure `#![no_std]`, actively maintained
  2026-10) [34][35]. See §7 for the read-vs-write caveat.
- **DWMSHC/SDHCI:** `dwmmc-host` 0.4.2 (Apache-2.0, no_std, "FIFO/IDMAC", DesignWare Mobile Storage
  Host Controller backend for `sdmmc-protocol`) and `sdhci-host` 0.5.2 (Apache-2.0, no_std,
  FIFO/ADMA2), both from `rcore-os/tgoskits`, edition 2024, published/updated through 2026-09
  [37][38]. The sunxi `sunxi_sdhci` register map (IDMAC `dmac/dlba/idst/idie`, FIFO at 0x200,
  version at 0x300) is the DesignWare MSHC layout, so `dwmmc-host` is directly relevant — but it
  targets a Rust OS/`no_std` context and the sunxi clock/PHY quirks are not included
  [15][37].
- **FEL tooling:** `aw-fel`/`fel-cli` are Rust, but stale (2018) [43].

---

## 5. Area-by-area analysis

### 5.1 DRAM init (`dram.c`, ~40 KB) — **highest risk**

- `dram.c` is 40,623 bytes [19]. Its header states it is "a reworked version" whose "baseline ... was
  lifted from awboot, which seems to be based on some form of de-compilation of some original
  Allwinner code bits", and it covers "Allwinner D1/D1s/R528/T113-sx DRAM initialisation" [14].
- It is **not** a flat microcode table: it implements algorithms — `auto_scan_dram_rank_width`,
  `auto_scan_dram_size`, `dqs_gate_detect`, `mctl_set_timing_params` (DDR2/DDR3/LPDDR2/LPDDR3
  timing from `ns_to_t`), `mctl_phy_ac_remapping` (fuse/chip-id-selected 22-line remap tables),
  `mctl_channel_init` (PIR/PGSR handshakes), `ccu_set_pll_ddr_clk`, ZQ calibration, and a
  read/write size test [14]. This is undocumented, timing-sensitive register code.
- The same logic is in U-Boot mainline `drivers/ram/sunxi/dram_sun20i_d1.c`, explicitly credited to
  awboot, with `sunxi_dram_init()` usable both standalone and via the driver model
  [41]. That is a cleaner reference for a port than awboot's variant, and it confirms the code is
  portable C with no ARM/RISC-V core dependency (the controller is shared between the RISC-V D1 and
  the ARM R528/T113).
- **Portability:** the code is self-contained C using `read32/write32`/`udelay` shims; it has no
  threading, allocation, or libc beyond `abort`/`memcpy`-free arithmetic [14]. It can be reused
  verbatim.
- **Rust prior art:** none found for Allwinner DDR3 init in Rust (searches of crates.io and the
  Allwinner package space returned only RISC-V SVD PACs and the FEL tools) [43].
- **C-in-Rust interim:** entirely realistic. Compile `dram.c` (and its `reg-*.h` siblings) as a
  normal C object/`staticlib` and expose `extern "C"` `sunxi_dram_init()`/`dram_get_top()`; Rust
  calls it behind a safe wrapper. This is the single most valuable "keep in C" candidate.

### 5.2 SDHCI (`sunxi_sdhci.c`, 28.9 KB)

- Register layout is Synopsys DesignWare MSHC: `gctrl/clkcr/timeout/width/blksz/bytecnt/cmd/arg/
  resp*`, IDMAC block (`dmac/dlba/idst/idie`), FIFO at offset `0x200`, version at `0x300`, plus
  sunxi-specific delay registers (`drv_dl`, `samp_dl`, skew registers) [15].
- Drivers instantiate `sdhci0` at `0x04020000` (SD/MMC, PF pins) and `sdhci2` at `0x04022000`
  (eMMC, PC pins) with a `sdhci_idma_desc_t` descriptor ring [9][15].
- **Reusable Rust:** `dwmmc-host` is a no_std DWMSHC backend with FIFO/IDMAC support and an
  `sdmmc-protocol` high layer; `sdhci-host` is the ADMA2 sibling. `embedded-sdmmc` also offers an
  SD/MMC layer + FAT with a `BlockDevice` abstraction [37][38][36]. Integration effort is
  **medium**: the sunxi clock/voltage/timing tables (`odly/sdly`, `set_clock`, `set_voltage`) and
  the eMMC hardware-reset (`hwrst`) and pinmux still need SoC glue.

### 5.3 SPI NOR / SPI NAND (`sunxi_spi.c`, 23.9 KB)

- awboot's SPI path detects and reads SPI NAND (`spi_nand_detect`, `spi_nand_read`), using **fixed
  flash offsets** rather than a filesystem: DTB at `128*2048`, kernel at `256*2048` [17][8].
- **Reusable Rust:** only small/experimental crates exist for SPI-NOR (`norbert`, `rflasher`), none
  for the sunxi SPI controller or SPI-NAND [43]. Expect a **medium-to-high** from-scratch driver,
  or keep in C initially. It is only needed for the `spi` variant; for T113-S3 SD/eMMC boot it can be
  deferred.

### 5.4 DMA (`sunxi_dma.c`, 7.4 KB)

- awboot has a small DMA engine used by the SPI-NAND path (`dma_init`, `dma_test`) [7]. No
  dedicated sunxi DMA Rust crate exists [43]. **Low-to-medium** effort, but only needed if SPI-NAND
  is ported.

### 5.5 CCU clocks (`sunxi_clk.c`, 7.5 KB), GPIO (`sunxi_gpio.c`, 2.7 KB)

- `sunxi_clk_init()` and `sunxi_clk_get_fail_addr()` run first in `main()`; GPIO handles pinmux/pull
  for UART, MMC, SPI and the board LED [7][8][9]. Register definitions live in `reg-ccu.h`
  (12.4 KB) and `reg-r-ccu.h`.
- No sunxi CCU/GPIO Rust crates exist; the R528/D1 PACs are the closest register references but are
  RISC-V-oriented [43]. These are **low-effort** (mostly register writes) and make good early Rust
  port targets. Note the DRAM code touches CCU (DDR PLL, MBUS/DRAM gates) internally, so CCU and
  DRAM are coupled [14].

### 5.6 UART (`sunxi_usart.c`, 2.3 KB), watchdog (`sunxi_wdg.c`, 2.8 KB), timers (`arch_timer.c`)

- UART is a simple polled init at 115200 on USART0 (and alternatives on USART3/5) [9]; watchdog is
  a simple `sunxi_wdg_set(seconds)` [7]. `arch_timer.c` (1.3 KB) uses the ARM generic timer and
  `CNTFRQ`. These are the **lowest-risk** modules and the natural first Rust port (UART gives you
  debug output immediately). `synopsys-dw-uart` exists but the sunxi UART is a simple DesignWare
  APB UART and awboot's own driver is tiny, so direct porting is trivial [43]. `aarch32-cpu`
  provides the generic-timer/CP15 primitives [22].

### 5.7 FDT (`lib/fdt.c`, 17.3 KB) — read **and write**

- awboot's FDT code is not just a parser: it mutates the blob in place — `fdt_update_bootargs`
  (`/chosen:bootargs`), `fdt_update_initrd` (`linux,initrd-start/end`, with cells handling),
  `fdt_update_memory` (`/memory:reg`, `device_type`), including inserting new properties and
  growing/moving `dt_struct`/`dt_strings` (`of_blob_move_dt_struct`, `of_add_property`) [16].
- The Rust `fdt` crate is explicitly "for parsing Flattened Devicetrees"; its README demonstrates
  read-only access (`model`, `compatible`, `memory`, `chosen.bootargs`, `find_node`) and does not
  advertise mutation [35]. So **a read-only `fdt` port leaves a gap**: awboot must *write* bootargs,
  memory size, and initrd range before boot. Options: (a) use a writer-oriented crate
  (`vm-fdt` "crate for writing FDT blobs" [43]); (b) hand-write the FDT mutation (bounded, ~a few
  hundred lines); or (c) keep `lib/fdt.c` behind FFI. This is a **medium** effort with a licensing
  caveat: `lib/fdt.c` carries an Atmel BSD notice [16], whereas the `fdt` crate is MPL-2.0 [34].

### 5.8 FAT filesystem

- awboot uses ChaN FatFs (`lib/fatfs/ff.c` is 259,696 bytes; `ffunicode.c` is 1,980,820 bytes;
  `ffconf.h` 12 KB) over an `f_read`/`f_lseek(CREATE_LINKMAP)` streaming adapter with a 32 KB read
  chunk [17][19]. This is by far the largest source component.
- Rust replacement `fatfs` 0.3.6 is no_std, MIT, supports FAT12/16/32 + LFN, and is the de-facto
  embedded Rust FAT library [31][32]. Integration requires a `Read/Write/Seek` block-device adapter
  (or use `embedded-sdmmc`, which already couples SD/MMC and FAT) [36][32]. **Low-to-medium** effort
  and a clear win over the 2 MB generated `ffunicode.c`.

### 5.9 PSCI / secure-to-nonsecure / exception vectors — **high risk**

- awboot's PSCI is lifted from U-Boot/Allwinner: `psci.c` (8 KB) implements `psci_cpu_on/off`,
  `psci_fiq_enter` (GIC FIQ in monitor mode), `psci_arch_init` (GICD/GICC group/priority/FIQEn,
  SCR), and the R528 CPUCFG path (soft entry `0x1c8`, C0 reset/status, PRCM power gating) [10].
- The monitor-mode transition is in asm: `nonsec_virt.S` (5.4 KB) sets up `_monitor_vectors`,
  handles `smc #0`, programs SCR (NS/AW/FW bits), optionally HYP mode (`CONFIG_ARMV7_VIRT`), resets
  `CNTVOFF`, sets `MVBAR`, and does the GIC/NSACR/CNTFRQ setup in `_nonsec_init`; `_do_nonsec_entry`
  is the entry trampoline [11]. `armv7_nonsec.c` drives it (SEC extension check via ID_PFR1, GICD
  enable, secure-section relocation) [12]. `psci_vectors.S` (8.5 KB) holds the monitor vectors and
  exception save/restore; `start.S` holds the IRQ/FIQ/abort vector table and `save_regs`/`restore_regs`
  macros [13][3].
- Rust can express this: `global_asm!` for the monitor vectors/save-restore and `asm!` for the SCR/
  SMC/MVBAR operations. `aarch32-cpu` supplies many of the register primitives [22]. But note awboot
  puts the secure code in a relocated `._secure.text`/`._secure.data` section (`relocate_secure_section`)
  and the PSCI stack in a dedicated `NOLOAD .psci_stack` region [6][12]. Reproducing section
  placement and the relocation correctly in Rust is subtle. The exception *stubs* are trivial
  (`exception.c`, XBOOT MIT, just prints registers and halts) [13].

### 5.10 Boot chain, eGON header and tooling

- The eGON header can be emitted from Rust `global_asm!` exactly as `start.S` does, or produced by a
  Rust host tool. `mksunxi.c` is ~3.5 KB and its job (checksum over aligned length with
  `0x5F0A6C39`, padding 512/8192, optional 2 KB→page expansion) is trivial to reimplement as a Rust
  `build.rs`/xtask [3][4].
- `xfel` remains the practical FEL/flash tool and is C/MIT; there is no complete Rust equivalent for
  T113 (the 2018 `aw-fel` predates T113) [29][30][43]. Tooling should be **ported or wrapped**, not
  assumed.
- awboot's own licensing is **ambiguous**: the GitHub repo reports no license, while individual
  files carry mixed notices (GPL-2.0+ in `dram.c`, BSD in `lib/fdt.c`, MIT in `exception.c`)
  [18][14][16][13]. This must be resolved before wholesale code reuse in a redistributed Rust port.

---

## 6. Risk & effort ranking

| Area | Effort | Risk | Notes / reuse |
|---|---|---|---|
| UART, GPIO, WDG, generic timer, exception stubs | S | Low | Trivial registers; port first for bring-up [9][13] |
| eGON header + `mksunxi` + build/tooling | S | Low | Rust asm/xtask; keep xfel external [3][4][29] |
| CCU clocks (non-DRAM) | S–M | Low–Med | Register writes; coupled to DRAM PLL [8][14] |
| FAT (replace ChaN FatFs, 2 MB) | M | Low | `fatfs` / `embedded-sdmmc` [31][36] |
| FDT (replace `lib/fdt.c`) | M | Med | `fdt` parses; **writing** bootargs/initrd/memory needs care [16][35] |
| SDHCI / DWMSHC + SD/MMC | M | Med | `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc` [15][37][36] |
| SPI NOR/NAND (+ DMA) | M–L | Med–High | No mature sunxi Rust driver; defer unless SPI boot needed [17][43] |
| Startup asm / linker / cache-MMU | M | Med | `aarch32-rt`; port link.ld [6][23] |
| PSCI / secure monitor / relocation | L | High | asm + secure section relocation [10][11][12] |
| **DRAM init (`dram.c`, ~40 KB)** | **L** | **Highest** | Undocumented timing code; keep in C via FFI, or port from U-Boot [14][41] |

---

## 7. Recommended incremental migration strategy

1. **Toolchain spike (days).** Build a `no_std` Rust blob for `armv7a-none-eabi`/`eabihf` linked at
   `0x00020000`, drive it through `mksunxi` + `xfel`, and toggle a GPIO / print on USART0. This
   proves target, linker and eGON integration before any real porting [2][3][6][20][21].
2. **Port the leaf drivers first (UART, GPIO, WDG, timer).** These are small and give observable
   output; use `aarch32-cpu`/`aarch32-rt` for CPU/exception plumbing [22][23][9][13].
3. **Keep DRAM and secure-monitor in C behind FFI.** Compile `dram.c` + `reg-*.h` and
   `nonsec_virt.S`/`psci_vectors.S` as objects; expose `extern "C"` `sunxi_dram_init()`,
   `dram_get_top()`, `boot_linux_psci()`. Rust owns the control flow; C owns the two risky modules.
   This de-risks the project immediately [14][11][41].
4. **Port boot flow + eGON + FDT + FAT.** Move `main.c`'s orchestration to Rust; use the `fatfs` or
   `embedded-sdmmc` crate for FAT; use `fdt` for parsing and a small writer (or retain `lib/fdt.c`)
   for mutation. Reimplement `mksunxi` in Rust (small) but keep `xfel` external [16][31][36][35][4].
5. **Port SDHCI/card protocol** using `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc` plus sunxi
   clock/voltage/pinmux glue [15][37][36].
6. **Then port DRAM** (lowest priority for a full-Rust goal), using U-Boot's cleaned-up
   `dram_sun20i_d1.c` as the reference and treating it as a byte-for-byte behavioral port with
   hardware-in-the-loop regression testing [41].
7. **Finally port PSCI/secure monitor** into `global_asm!`, carefully reproducing the relocated
   secure section and the monitor vector table [10][11][12][6].

Any full-Rust end state should resolve the **licensing** question first, because awboot mixes
GPL-2.0+, BSD and MIT notices and has no top-level LICENSE [18][14][16][13].

---

## 8. Open questions for the user

1. **Scope:** Is the goal a *full* Rust rewrite, or a Rust/C hybrid that keeps the DRAM
   controller (and possibly the monitor-mode asm) in C? (Strongly recommend the hybrid.)
2. **Boot medium priority:** Which path must ship first — SD card (SDHCI+FAT), eMMC, SPI NOR/NAND,
   or FEL? This reorders the SPI/DMA work entirely.
3. **The user's existing port:** Which old awboot revision and which rustc edition/target did it
   use? Was it `armv7a-none-eabi` or `eabihf`? What already compiles/? (Needed to avoid redoing
   work and to audit licensing of any copied code.)
4. **Licensing:** What license will the Rust port carry? This constrains reuse of GPL-2.0+ `dram.c`
   and of MPL-2.0 `fdt` / Apache-2.0 `dwmmc-host`.
5. **T113-S3 vs T113-S4 vs V851s:** Keep all three machine trees, or T113-S3 only (as stated)?
6. **Hardware-in-the-loop:** Is a T113-S3 board available for continuous DRAM/PSCI regression
   testing? (Mandatory for a DRAM port.)
7. **Target feature set:** Confirm Cortex-A7 + NEON/VFPv4 + hard-float parity is required, or whether
   soft-float (`-eabi`) is acceptable for a bootloader.
8. **Tooling:** Acceptable to keep `xfel` (C) as the FEL/flash host tool, or must all host tooling be
   Rust?

---

## Sources

1. awboot README — https://raw.githubusercontent.com/szemzoa/awboot/main/README.md
2. awboot Makefile — https://raw.githubusercontent.com/szemzoa/awboot/main/Makefile
3. awboot `start.S` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/start.S
4. awboot `tools/mksunxi.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/tools/mksunxi.c
5. awboot `tools/fel.sh` — https://raw.githubusercontent.com/szemzoa/awboot/main/tools/fel.sh
6. awboot `link.ld` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/link.ld
7. awboot `main.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/main.c
8. awboot `board.h` — https://raw.githubusercontent.com/szemzoa/awboot/main/board.h
9. awboot `board.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/board.c
10. awboot `psci.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/psci.c
11. awboot `nonsec_virt.S` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/nonsec_virt.S
12. awboot `armv7_nonsec.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/armv7_nonsec.c
13. awboot `exception.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/exception.c
14. awboot `dram.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/dram.c
15. awboot `sunxi_sdhci.h` — https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/sunxi_sdhci.h
16. awboot `lib/fdt.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/lib/fdt.c
17. awboot `lib/loaders.c` — https://raw.githubusercontent.com/szemzoa/awboot/main/lib/loaders.c
18. awboot repo metadata (no license) — https://api.github.com/repos/szemzoa/awboot
19. awboot full tree (file sizes) — https://api.github.com/repos/szemzoa/awboot/git/trees/main?recursive=1
20. rustc book: `{arm,thumb}v7a-none-eabi{,hf}` (Tier 2) — https://doc.rust-lang.org/rustc/platform-support/armv7a-none-eabi.html
21. Local toolchain `rustc 1.99.0 (b940084d7 2026-09-28)`: `rustc --print target-list`, `rustup target list`
22. `aarch32-cpu` crate metadata — https://crates.io/api/v1/crates/aarch32-cpu
23. `aarch32-rt` crate metadata — https://crates.io/api/v1/crates/aarch32-rt
24. `rustsbi/rustsbi-d1` repo — https://api.github.com/repos/rustsbi/rustsbi-d1
25. `rustsbi/rustsbi-d1` tree (RISC-V `see/`, `spl/`) — https://api.github.com/repos/rustsbi/rustsbi-d1/git/trees/main?recursive=1
26. `rustsbi/rustsbi-allwinner-nezha` repo (archived, empty) — https://api.github.com/repos/rustsbi/rustsbi-allwinner-nezha
27. `oreboot` repo — https://api.github.com/repos/oreboot/oreboot
28. `oreboot` README (D1 RISC-V, parked `qemu-armv7`) — https://raw.githubusercontent.com/oreboot/oreboot/main/README.md
29. `xboot/xfel` repo (C, MIT) — https://api.github.com/repos/xboot/xfel
30. `xfel` README — https://raw.githubusercontent.com/xboot/xfel/master/README.md
31. `fatfs` crate metadata — https://crates.io/api/v1/crates/fatfs
32. `rust-fatfs` README (no_std usage, MIT) — https://raw.githubusercontent.com/rafalh/rust-fatfs/master/README.md
33. `embedded-fatfs` crate metadata (0.0.0 placeholder) — https://crates.io/api/v1/crates/embedded-fatfs
34. `fdt` crate metadata — https://crates.io/api/v1/crates/fdt
35. `fdt` README (parse-only example) — https://raw.githubusercontent.com/repnop/fdt/main/README.md
36. `embedded-sdmmc` crate metadata — https://crates.io/api/v1/crates/embedded-sdmmc
37. `dwmmc-host` crate metadata — https://crates.io/api/v1/crates/dwmmc-host
38. `sdhci-host` crate metadata — https://crates.io/api/v1/crates/sdhci-host
39. linux-sunxi wiki: BROM (via archive) — https://web.archive.org/web/2024id_/https://linux-sunxi.org/BROM
40. linux-sunxi wiki: Bootable SD card, 8 KB / 128 KB and T113 (via archive) — https://web.archive.org/web/2023id_/https://linux-sunxi.org/Bootable_SD_card
41. U-Boot mainline `dram_sun20i_d1.c` (credits awboot) — https://raw.githubusercontent.com/u-boot/u-boot/master/drivers/ram/sunxi/dram_sun20i_d1.c
42. `smaeul` repos (sun20i_d1_spl is C; no Rust T113) — https://api.github.com/users/smaeul/repos
43. crates.io searches used for gap analysis: `sunxi`, `allwinner`, `sdhci`, `dwmmc`, `designware`, `spi-nor`, `devicetree` — e.g. https://crates.io/api/v1/crates?q=allwinner and https://crates.io/api/v1/crates?q=devicetree
44. `rcore-os/tgoskits` (source repo for `dwmmc-host`/`sdhci-host`/`sdmmc-protocol`) — https://github.com/rcore-os/tgoskits
45. `smaeul/linux` `d1/all` branch (kernel used by awboot) — https://github.com/smaeul/linux/tree/d1/all

### Note on the brief's pre-gathered facts
- "~40 KB dram.c register microcode": size confirmed (40,623 B) [19]; it is **not** purely a
  microcode table — it is algorithmic register/timing C [14].
- `ff.c` size confirmed 259,696 B; `ffunicode.c` 1,980,820 B [19].
- `psci.c`/`psci_vectors.S`/`nonsec_virt.S`/`memcpy.S`/`link.ld`/`reg-*.h` all confirmed present and
  sized [19].
- Target name in the brief, `armv7a-unknown-none-eabi`, is incorrect; the valid targets are
  `armv7a-none-eabi` / `armv7a-none-eabihf` [20][21].
- `armv7_nonsec.c` is present (3,970 B); the brief omitted it [19][12].

---

## Addendum — audit of the user's existing Rust SPL (`embed/app` + `embed/rt`)

Source: local working copy `<workspace>/rust/awboot/embed/` (the mirror of
`https://github.com/ElicMagus/rawboot`, history through commit `2f35515 "Tested Dram"`).
This materially changes two conclusions above.

- **The port is already well past "hello world."** Two crates: `rt` (custom runtime — eGON header
  emitted from Rust statics in `.head.head`, `_start` in `.head.start`, BSS/data init, `link.x`
  linker script, `cpu.rs` with `wfe`/CPSR/SP/generic-timer/`udelay`) and `app` (SPL: `clk`, `ccu`,
  `gpio`, `uart`, `io`, `print`, `dram`, `sdhci`, `sdcard`). `План.txt` marks LED, UART, CLK,
  pre-DRAM magic and DRAM as done; SDHCI/SD-card work is present but **uncommitted** (steps 6–9:
  mount FS, load kernel, jump = still todo).
- **It builds on stable**, contrary to any nightly assumption: `rustup target add armv7a-none-eabi`
  + `cargo build --release` succeeds on `rustc 1.99.0 (b940084d7 2026-09-28)` with only
  `static_mut_refs` warnings (Rust 2024 lint) in `rt/src/lib.rs`. Output ELF ≈ 65 KB.
- **DRAM is already in Rust, and the "decompilation" is NOT lost.** `app/src/dram/mod.rs` (working
  copy, 63 KB, 1,926 lines, 22 functions) is a readable Rust port — `auto_scan_dram_size`,
  `auto_scan_dram_rank_width`, `dqs_gate_detect`, `mctl_channel_init`, `auto_set_timing_para`,
  `mctl_phy_ac_remapping`, `ccm_set_pll_ddr_clk`, `eye_delay_compensation`, `auto_cal_timing`, etc.
  `app/123.txt` (1,381 lines) is an earlier readable snapshot of the same code. No `include_bytes!`
  or oversized literal array replaced readable code in any of the five committed `dram/mod.rs`
  revisions (checked `cde25bd` skeleton → `c014bb9`/`d7b3cda`/`c85111e`/`2f35515`). The only
  "blob"-like datum is the seven 22-byte `cfg1..cfg7` AC-remapping tables — legitimately tabular,
  and present in mainline U-Boot too [41].
- **Consequence:** the report's "single biggest risk — keep `dram.c` in C via FFI" is **already
  de-risked**. A substantially-full-Rust end state is more attractive than the hybrid baseline
  suggested in §7, with DRAM effort reclassified from L/High to done.
- Remaining port surface is what §5 still flags: **SDHCI DWMSHC + SD/MMC card protocol** (in
  progress locally), **FAT** (replace the 2 MB ChaN FatFs — use the `fatfs` crate), **FDT
  read+write**, **secure-monitor/PSCI asm**, and **build packaging** (`build.bat` shelled out to
  C tools `checksum.exe`/`xfel.exe`/`dd.exe`; reimplement `mksunxi` checksum as a Rust xtask).
- **Open architecture question surfaced by the code:** out-of-date patterns to modernise — the
  `rt` crate still uses `#[no_mangle]` statics + `static mut` (Rust 2024 `static_mut_refs`), the
  `cc` build-dependency is unused (`build.rs` has the `asm.s` compile commented out), `asm.s` is
  0 bytes, and `.cargo/config` pins a raw `-Tlink.x`. Worth resolving before scaling the port.
