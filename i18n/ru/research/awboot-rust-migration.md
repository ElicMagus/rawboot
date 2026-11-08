# Перенос `szemzoa/awboot` (Allwinner T113-S3, ARMv7-A) на Rust — отчёт о выполнимости по первоисточникам

Исследование: 2026. Цель: именно Allwinner T113-S3. Все утверждения подкреплены ссылками по месту
на точный первоисточник (исходный файл, официальная документация, метаданные crate или
first-party-репозиторий). Пункты, которые не удалось сверить с первоисточником, помечены
**UNVERIFIED**.

Номера в квадратных скобках соответствуют разделу **Sources** в конце.

---

## 1. Краткое резюме

- awboot — небольшой bare-metal загрузчик на C для T113-S3/T113-S4/V851s, собираемый тулчейном
  `arm-none-eabi`, с `-mcpu=cortex-a7`, hard-float, `-Os`, и порождающий четыре образа
  FEL/SPI/SD/eMMC, каждый из которых обёрнут в заголовок BROM Allwinner «eGON» [1][2].
- Цепочка загрузки такова: BROM → загружает крошечную стадию с SD-сектора 16 (8 KB) или, как
  запасной вариант на более новых SoC, явно включая T113, с сектора 256 (128 KB), из SPI NOR/NAND
  или через USB FEL; BROM проверяет контрольную сумму заголовка eGON (затравка `0x5F0A6C39`)
  [1][4][40]. awboot формирует заголовок eGON напрямую из `start.S` и использует `mksunxi` лишь
  для пересчёта контрольной суммы, исправления длины и выравнивания образа [3][4].
- Bare-metal Rust на ARMv7-A жизнеспособен **на stable**: `armv7a-none-eabi` и `armv7a-none-eabihf`
  — это цели Tier-2 `no_std`, присутствующие в rustc 1.99.0 (2026-09-28) и устанавливаемые через
  rustup [20][21]. Цели `armv7a-unknown-none-eabi` **не существует**; такого имени нет [21].
  Проект `rust-embedded/aarch32` предоставляет `aarch32-rt` 0.5.0 для старта/векторов исключений и
  `aarch32-cpu` 0.4.0 для поддержки CPU (MIT/Apache-2.0, MSRV 1.93) [22][23].
- **Ни один существующий Rust-проект не нацелен на ARMv7-A Allwinner T113.** Экосистема Rust для
  Allwinner целиком RISC-V (D1/D1s) или только PAC: `rustsbi-d1` и порт Allwinner в oreboot
  относятся к T-Head C906 **RISC-V** [24][25][28]; `smaeul/sun20i_d1_spl` написан на C [42].
  oreboot поддерживает ARMv7-A только в замороженной эмуляции QEMU [28].
- Для «лёгких» периферий и файловых систем есть переиспользуемые Rust-crate: `fatfs` 0.3.6 (no_std,
  MIT) [31][32], `embedded-sdmmc` 0.10.0 (no_std, SD/MMC + FAT, MIT/Apache) [36] и crate `fdt`
  (no_std, MPL-2.0) [34][35]. Для Synopsys DesignWare MSHC (IP `sunxi_sdhci`) есть `dwmmc-host`
  (Apache-2.0, no_std, FIFO/IDMAC) и `sdhci-host` (ADMA2) — свежие, напрямую релевантные бэкенды
  для `sdmmc-protocol` [37][38].
- **Главный риск — инициализация контроллера DRAM** (`dram.c`, ~40 KB). Это недокументированный
  код регистров/таймингов, сам себя описывающий как полученный декомпиляцией, чьи алгоритмы
  (автоопределение ранга/ширины и размера, ZQ, DQS gating, PHY AC remapping) не транслируются
  тривиально [14]. В mainline U-Boot тот же код присутствует в переработанном виде [41].
  Прагматичная рекомендация — на первом этапе оставить этот один файл (и asm secure-monitor/PSCI)
  на C за FFI, а остальное портировать.

### Вердикт о выполнимости

**Выполнимо, с поэтапным гибридом C/Rust.** Вопросы CPU/ABI/тулчейна решены, драйверы периферии и
части FAT/FDT в значительной мере переиспользуемы, а риск сосредоточен в двух изолированных,
хорошо ограниченных модулях (DRAM init; secure-monitor/PSCI). Полный переход «100% Rust» выполним,
но непропорционально дорог из-за микрокода DRAM и танца с GIC/CPUCFG в защищённом состоянии.

---

## 2. Цепочка загрузки T113-S3

### 2.1 SoC и тулчейн

- awboot нацелен на «Allwinner T113-s3, T113-s4, V851s» [1]. Его флаги сборки:
  `-mcpu=cortex-a7 -mthumb-interwork -mthumb -mno-unaligned-access -mfpu=neon-vfpv4
  -mfloat-abi=hard`, то есть ARMv7-A Cortex-A7 с NEON/VFPv4 и ABI hard-float [2].
  (Сам даташит T113-S3 напрямую не получен; идентификация Cortex-A7 опирается на собственный
  `-mcpu=cortex-a7` у awboot и общий код DRAM R528/T113 — см. §4 — и потому
  **UNVERIFIED относительно даташита производителя**.)
- Дерево машины находится в `arch/arm32/mach-t113s3/` [19]. Отдельные деревья `mach-t113s4` и
  `mach-v851s` существуют и используют идентичные блобы `sdmmc.h`, `dram.h` и `memcpy.S`
  (подтверждено одинаковыми git blob SHA в листинге дерева) [19].

### 2.2 FEL / USB

- Загрузка по FEL загружает awboot через `xfel` по адресу загрузки FEL и прыгает на него; `fel.sh`
  задаёт `BOOT_ADDR=0x00028000`, выполняет `xfel ddr t113-s3`, пишет ядро по `0x42000000`, DTB
  чуть ниже верхней границы 128 MiB SDRAM и необязательный initrd, затем `xfel exec` [5].
- Образ FEL у awboot слинкован по `__RAM_BASE=0x00028000` (предел SRAM 128 KB), тогда как
  постоянный загрузочный образ слинкован по `0x00020000` (160 KB в режиме загрузки) [2][6]. `link.ld`
  объявляет регион SRAM на 128 KB плюс регион DDR на 128 MiB по `0x40000000` [6].
- Передача ядра/DTB/initrd использует четыре «почтовых ящика» по `0x43100000`: размер initrd (+0x0),
  начало initrd (+0x4), адрес DTB (+0x8), адрес ядра (+0xC). `fel.sh` пишет их через `xfel write32`,
  а `main.c` читает их обратно в `apply_fel_mailboxes()` [5][8][7].
- `xfel` — программа на C (язык репозитория «C», MIT), поддерживающая T113 и команды
  `spinor`/`spinand`, применяемые для прошивки SPI [29][30].

### 2.3 Заголовок eGON (что фактически кодирует awboot)

- `start.S` в awboot **формирует заголовок eGON сам** как первые байты `.text`: слово ветвления ARM
  `0xea000016`, ASCII-магия `eGON.BT0`, заглушка контрольной суммы `0x12345678`, символ размера SPL
  `__spl_size`, размер boot-заголовка `0x30`, слово версии boot-заголовка `0x30303033`
  («3030»/«300»), возвращаемое значение `0x00020000`, адрес запуска `0x00028000` и байты строки
  платформы `34 2e 30 00` («4.0\0») [3].
- `tools/mksunxi.c` определяет ту же 56-байтную раскладку `boot_head_t` (`instruction`, `magic[8]`,
  `checksum`, `length`, `spl_signature[4]`, `fel_script_address`, `fel_uenv_length`,
  `dt_name_offset`, `reserved1`, `boot_media`, `string_pool[13]`) [4].
- `mksunxi` только: читает бинарник, выравнивает `length` по выравниванию (512 для блочных
  устройств, 8192 для flash), инициализирует контрольную сумму затравкой `0x5F0A6C39`, суммирует
  все 32-битные слова по выровненной длине, записывает контрольную сумму обратно и (при желании)
  разворачивает каждую страницу 2 KB в `pagesize` (для крупностраничной SPI flash BROM
  подразумевает страницы 2 KB) [4].
- Makefile вызывает `mksunxi <bin> 512` для FEL/SD/eMMC и `mksunxi <bin> 8192` плюс `8192 4096`
  для вариантов SPI [2].

### 2.4 Размещение на SD-карте и SPI

- README awboot: SD-загрузка записывает `awboot-boot-sd.bin` через `dd ... bs=1024 seek=8` —
  смещение MBR 8 KB (сектор 16) — либо 128 KB (сектор 256) для GPT [1].
- wiki linux-sunxi подтверждает механику: смещение 8 KB «диктуется BROM, он проверяет наличие
  валидного заголовка eGON/TOC0 в этом месте», и «более новые SoC (проверено на H2+, A64, H5, H6,
  T113) также могут загружать SPL с сектора 256 (128KB) ... если валидная сигнатура eGON/TOC0 не
  найдена по 8KB» [40]. Это первоисточник, подтверждающий утверждение awboot про 8 KB/128 KB.
- SPI: образ SPI записывается трижды (страницы 0, 32 и 64 с шагом 2 KB) вместе с вариантом размера
  страницы `4k`, созданным вторым вызовом `mksunxi`, чтобы учесть предположение BROM о странице
  2 KB и геометрию flash [2][4].
- Общий порядок загрузки BROM (SD0 → NAND → SD2 → SPI → FEL для старых sunxi, с переопределением
  через пин FEL/BSP) документирован на linux-sunxi [39]; точный порядок для T113 в специфичном для
  T113 первоисточнике **UNVERIFIED**, но T113 поддерживает загрузку SD/SPI/FEL [40].
- Используемый исходник ядра — ветка `smaeul/linux` `d1/all` [1][45].

### 2.5 Поток загрузки во время работы awboot

`main()` выполняет по порядку: `sunxi_clk_init()`, `board_init()` (UART на USART0, PE2/PE3 mux 6),
`sunxi_dram_init()`, `sunxi_wdg_set(10)`, затем загрузку либо SDHCI→SD/MMC→FatFs, либо SPI-NAND,
затем `boot_image_setup()` (проверяет магию Linux zImage и вычисляет точку входа), затем
модификацию FDT (bootargs, reg `/memory`, начало/конец initrd), затем отключает MMU/D-cache/I-cache/
прерывания и вызывает `boot_linux_psci()` [7][8][9]. `boot_linux_psci()` запускает
`armv7_init_nonsec()`, а затем трамплин `_do_nonsec_entry` с точкой входа ядра в r0..r2 [7][12].

---

## 3. Bare-metal Rust на ARMv7-A Cortex-A7 (no_std) в 2026

### 3.1 Цели rustc и статус tier

- `armv7a-none-eabi` и `armv7a-none-eabihf` (а также варианты `thumbv7a-none-eabi{,hf}`)
  документированы на одной странице rustc book как **Tier 2**, core+alloc, bare-metal `#![no_std]`;
  `-eabi` — soft-float, `-eabihf` — hard-float (нужен минимум VFPv3-D16) [20].
- Проверено на локальном тулчейне, `rustc 1.99.0 (b940084d7 2026-09-28)`: `rustc --print target-list`
  содержит `armv7a-none-eabi` и `armv7a-none-eabihf`; `rustup target list` предлагает обе для
  установки [21].
- Имени `armv7a-unknown-none-eabi` **не существует**, и это не валидная цель [21]. У правильных
  bare-metal имён нет компонента vendor `unknown`.
- Tier 2 означает распространяемые через rustup предкомпилированные `core`/`alloc` (и `std`, где
  применимо), но без хостовых инструментов; для Cortex-A7 кастомный JSON-файл цели **не требуется**
  (JSON нужен, только если требуется непоставляемый набор возможностей CPU по умолчанию). Возможности
  цели вроде `+vfp4`, `+d32`, `+neon` включаются через `-C target-feature=` или
  `.cargo/config.toml` [20].
- `asm!`/`global_asm!` стабильны; для кода старта ночная фича не требуется. (Nightly нужен только
  для экзотических схем `#![feature(...)]`.)

### 3.2 Старт, линкер-скрипты, cache/MMU

- Работа awboot при старте — база векторов (`VBAR`), порядок байт, отключение кэшей/MMU,
  TLB/invalidate, включение VFP/NEON, стек, очистка BSS, `CNTFRQ=24 MHz`, переход на `main` — это
  обычный ассемблер ARMv7-A, выразимый в Rust `global_asm!` или в рантайме `aarch32-rt` [3][23].
- Проект `rust-embedded/aarch32` поддерживает `aarch32-rt` 0.5.0 (рантайм/старт, фичи `eabi-fpu`,
  `el2-mode`, `fpu-d32`, `svc-stack-interrupt`) и `aarch32-cpu` 0.4.0 (поддержка CPU/регистров), оба
  MIT/Apache-2.0, MSRV 1.93, впервые выпущены 2025-10-31 и активно обновлялись до 2026-09
  [22][23]. Это рекомендуемая основа для работы с сопроцессором CR0/CR1 и векторами исключений
  при переносе.
- Линкер-скрипт можно перенести напрямую; это обычный GNU ld (`OUTPUT_FORMAT("elf32-littlearm")`,
  `OUTPUT_ARCH(arm)`, MEMORY/SECTIONS с `__spl_size`, вычисляемым как `__spl_end-__spl_start`) [6].

---

## 4. Существующие наработки и переиспользуемые crate

### 4.1 Существующие наработки — что есть и почему ничто из этого не является загрузчиком для T113

| Проект | Arch | Что делает | Лицензия | Применимость для T113-S3 |
|---|---|---|---|---|
| `rustsbi/rustsbi-d1` | **RISC-V** (T-Head C906) | Прошивка/SPL SBI для плат D1; использует riscv hart/PLIC/trap CSRs и DRAM-блоб `magic.rs` | лицензия не заявлена в метаданных репозитория | **Не применимо** к ARMv7-A: crate `see/` и `spl/` относятся к RISC-V (`riscv_spec.rs`, `hart_csr_utils.rs`, `plic.rs`) [24][25] |
| `rustsbi/rustsbi-allwinner-nezha` | RISC-V | «Bootloader environment for Allwinner Nezha ... XuanTie C906 RISC-V core» | нет; репозиторий **архивирован и пуст** (размер 0) [26] | Не применимо (RISC-V, мёртв) |
| `oreboot` | x86, RISC-V, ARM в QEMU | «coreboot without C», payload'ы LinuxBoot; порт Allwinner D1 — RISC-V; ARMv7-A заморожен (`qemu-armv7` в `src.broken`) | GPL-2.0 | Нет mainboard ARM sunxi/T113; только архитектурные приёмы и знание DWMMC [27][28] |
| `smaeul/sun20i_d1_spl` | RISC-V | «Mainline-friendly SPL for D1» | нет | C, RISC-V; не переиспользуемо [42] |
| `xboot/xfel` | хостовый инструмент | Инструмент FEL/прошивки, поддерживающий T113 | MIT | C; используется как есть, не цель портирования на Rust [29][30] |
| `Razican/aw-fel` + `fel-cli` | хостовый инструмент | Библиотека/CLI FEL на чистом Rust | — | Последний релиз 0.5.2, 2018; **поддержка T113 UNVERIFIED** [43] |
| `duskmoon314/aw-pac` | D1/R528/V853 | Сгенерированные из SVD crate доступа к периферии (`d1-pac`, `r528-pac`, `v853-pac`) | — | R528 — ARM-собрат T113; смещения регистров могут переноситься, но PAC ориентированы на D1/RISC-V — применимость к ARMv7-A **UNVERIFIED** [43] |

Вывод: именно для T113-S3 **нет предшествующего Rust-загрузчика**; порт начинается с первых
принципов для частей, специфичных для SoC.

### 4.2 Переиспользуемые crate (проверенные метаданные)

- **CPU/рантайм:** `aarch32-cpu` 0.4.0 и `aarch32-rt` 0.5.0 (MIT/Apache-2.0, MSRV 1.93)
  [22][23].
- **FAT:** `fatfs` 0.3.6 (MIT, no_std через `default-features = false`; опубликован 2023-01-17)
  [31][32]. Примечание: `embedded-fatfs` на crates.io — **не** настоящий crate, это 3-строчная
  заглушка-сквоттинг имени `0.0.0`, опубликованная в 2023 [33]. Активный embedded-проект FAT — это
  сам crate `fatfs`.
- **SD/MMC + FAT:** `embedded-sdmmc` 0.10.0 (MIT/Apache-2.0, no_std, MSRV 1.87), который содержит
  драйвер протокола SD/MMC и файловую систему FAT за трейтом `BlockDevice` [36].
- **FDT:** `fdt` 0.1.5 stable / 0.2.0-alpha3 (MPL-2.0, чистый `#![no_std]`, активно поддерживается,
  2026-10) [34][35]. Оговорку про чтение-против-записи см. в §7.
- **DWMSHC/SDHCI:** `dwmmc-host` 0.4.2 (Apache-2.0, no_std, «FIFO/IDMAC», бэкенд DesignWare Mobile
  Storage Host Controller для `sdmmc-protocol`) и `sdhci-host` 0.5.2 (Apache-2.0, no_std,
  FIFO/ADMA2), оба из `rcore-os/tgoskits`, edition 2024, публиковались/обновлялись по 2026-09
  [37][38]. Карта регистров sunxi `sunxi_sdhci` (IDMAC `dmac/dlba/idst/idie`, FIFO по 0x200,
  версия по 0x300) — это раскладка DesignWare MSHC, поэтому `dwmmc-host` напрямую релевантен — но
  он нацелен на контекст Rust OS/`no_std`, а особенности тактирования/PHY sunxi в нём не учтены
  [15][37].
- **Инструментарий FEL:** `aw-fel`/`fel-cli` написаны на Rust, но устарели (2018) [43].

---

## 5. Анализ по областям

### 5.1 DRAM init (`dram.c`, ~40 KB) — **наивысший риск**

- `dram.c` — 40 623 байта [19]. В его заголовке сказано, что это «переработанная версия», чья
  «базовая линия ... была взята из awboot, который, похоже, основан на некоей форме декомпиляции
  неких оригинальных фрагментов кода Allwinner», и что он покрывает «инициализацию DRAM Allwinner
  D1/D1s/R528/T113-sx» [14].
- Это **не** плоская таблица микрокода: он реализует алгоритмы — `auto_scan_dram_rank_width`,
  `auto_scan_dram_size`, `dqs_gate_detect`, `mctl_set_timing_params` (тайминги DDR2/DDR3/LPDDR2/
  LPDDR3 из `ns_to_t`), `mctl_phy_ac_remapping` (таблицы ремапа на 22 линии, выбираемые по
  fuse/chip-id), `mctl_channel_init` (рукопожатия PIR/PGSR), `ccu_set_pll_ddr_clk`, калибровку ZQ
  и тест размера чтения/записи [14]. Это недокументированный, чувствительный к таймингам код
  регистров.
- Та же логика есть в mainline U-Boot `drivers/ram/sunxi/dram_sun20i_d1.c`, явно с указанием
  авторства awboot, причём `sunxi_dram_init()` пригодна и автономно, и через модель драйверов
  [41]. Это более чистый эталон для порта, чем вариант awboot, и он подтверждает, что код — это
  переносимый C без зависимости от ядра ARM/RISC-V (контроллер общий между RISC-V D1 и
  ARM R528/T113).
- **Переносимость:** код — самодостаточный C, использующий шимы `read32/write32`/`udelay`; в нём
  нет потоков, аллокации и libc сверх арифметики без `abort`/`memcpy` [14]. Его можно
  переиспользовать дословно.
- **Предшествующий код на Rust:** для инициализации DDR3 Allwinner на Rust не найдено (поиск по
  crates.io и по пространству пакетов Allwinner вернул только RISC-V SVD PAC и инструменты FEL)
  [43].
- **Промежуточный C-в-Rust:** вполне реалистичен. Скомпилируйте `dram.c` (и его родственные
  `reg-*.h`) как обычный C-объект/`staticlib` и экспортируйте `extern "C"` `sunxi_dram_init()`/
  `dram_get_top()`; Rust вызывает его за безопасной обёрткой. Это единственный наиболее ценный
  кандидат «оставить на C».

### 5.2 SDHCI (`sunxi_sdhci.c`, 28.9 KB)

- Раскладка регистров — Synopsys DesignWare MSHC: `gctrl/clkcr/timeout/width/blksz/bytecnt/cmd/arg/
  resp*`, блок IDMAC (`dmac/dlba/idst/idie`), FIFO по смещению `0x200`, версия по `0x300`, плюс
  специфичные для sunxi регистры задержек (`drv_dl`, `samp_dl`, регистры skew) [15].
- Драйверы создают `sdhci0` по `0x04020000` (SD/MMC, пины PF) и `sdhci2` по `0x04022000`
  (eMMC, пины PC) с кольцом дескрипторов `sdhci_idma_desc_t` [9][15].
- **Переиспользуемый Rust:** `dwmmc-host` — это no_std-бэкенд DWMSHC с поддержкой FIFO/IDMAC и
  высокоуровневым слоем `sdmmc-protocol`; `sdhci-host` — его ADMA2-собрат. `embedded-sdmmc` также
  предлагает слой SD/MMC + FAT с абстракцией `BlockDevice` [37][38][36]. Трудозатраты на интеграцию
  — **средние**: таблицы тактирования/напряжения/таймингов sunxi (`odly/sdly`, `set_clock`,
  `set_voltage`), аппаратный сброс eMMC (`hwrst`) и pinmux всё ещё требуют SoC-обвязки.

### 5.3 SPI NOR / SPI NAND (`sunxi_spi.c`, 23.9 KB)

- Путь SPI в awboot определяет и читает SPI NAND (`spi_nand_detect`, `spi_nand_read`), используя
  **фиксированные смещения flash** вместо файловой системы: DTB по `128*2048`, ядро по `256*2048`
  [17][8].
- **Переиспользуемый Rust:** для SPI-NOR существуют только небольшие/экспериментальные crate
  (`norbert`, `rflasher`), для контроллера SPI sunxi или SPI-NAND — никаких [43]. Ожидайте
  **средне-высокие** трудозатраты на драйвер с нуля либо оставьте на C на начальном этапе. Он нужен
  только для варианта `spi`; для загрузки T113-S3 с SD/eMMC его можно отложить.

### 5.4 DMA (`sunxi_dma.c`, 7.4 KB)

- У awboot есть небольшой движок DMA, используемый путём SPI-NAND (`dma_init`, `dma_test`). Отдельного
  Rust-crate для DMA sunxi не существует [43]. Трудозатраты **низко-средние**, но нужны, только если
  портируется SPI-NAND.

### 5.5 Тактирование CCU (`sunxi_clk.c`, 7.5 KB), GPIO (`sunxi_gpio.c`, 2.7 KB)

- `sunxi_clk_init()` и `sunxi_clk_get_fail_addr()` выполняются первыми в `main()`; GPIO
  обрабатывает pinmux/pull для UART, MMC, SPI и платного LED [7][8][9]. Определения регистров лежат
  в `reg-ccu.h` (12.4 KB) и `reg-r-ccu.h`.
- Rust-crate для CCU/GPIO sunxi не существует; ближайшие справочники регистров — PAC R528/D1, но они
  ориентированы на RISC-V [43]. Это **низкозатратные** цели (в основном записи в регистры) и хорошие
  ранние цели для порта на Rust. Учтите, что код DRAM внутри обращается к CCU (DDR PLL, гейты
  MBUS/DRAM), поэтому CCU и DRAM связаны [14].

### 5.6 UART (`sunxi_usart.c`, 2.3 KB), watchdog (`sunxi_wdg.c`, 2.8 KB), таймеры (`arch_timer.c`)

- UART — простая polled-инициализация на 115200 на USART0 (и альтернативы на USART3/5) [9];
  watchdog — простой `sunxi_wdg_set(seconds)` [7]. `arch_timer.c` (1.3 KB) использует общий таймер
  ARM и `CNTFRQ`. Это **наименее рискованные** модули и естественный первый порт на Rust (UART
  сразу даёт отладочный вывод). `synopsys-dw-uart` существует, но UART sunxi — это простой APB UART
  DesignWare, а собственный драйвер awboot крошечный, так что прямой перенос тривиален [43].
  `aarch32-cpu` предоставляет примитивы общего таймера/CP15 [22].

### 5.7 FDT (`lib/fdt.c`, 17.3 KB) — чтение **и запись**

- Код FDT в awboot — не просто парсер: он изменяет блоб на месте — `fdt_update_bootargs`
  (`/chosen:bootargs`), `fdt_update_initrd` (`linux,initrd-start/end`, с обработкой ячеек),
  `fdt_update_memory` (`/memory:reg`, `device_type`), включая вставку новых свойств и
  рост/перемещение `dt_struct`/`dt_strings` (`of_blob_move_dt_struct`, `of_add_property`) [16].
- Rust-crate `fdt` явно «for parsing Flattened Devicetrees»; его README демонстрирует доступ только
  на чтение (`model`, `compatible`, `memory`, `chosen.bootargs`, `find_node`) и не заявляет
  возможность изменения [35]. Поэтому **порт `fdt` только для чтения оставляет пробел**: awboot
  обязан *записывать* bootargs, размер памяти и диапазон initrd перед загрузкой. Варианты:
  (a) использовать crate, ориентированный на запись (`vm-fdt` «crate for writing FDT blobs» [43]);
  (b) написать модификацию FDT вручную (ограниченно, ~несколько сот строк); или (c) оставить
  `lib/fdt.c` за FFI. Это трудозатраты **средние** с оговоркой по лицензиям: `lib/fdt.c` несёт
  уведомление Atmel BSD [16], тогда как crate `fdt` — MPL-2.0 [34].

### 5.8 Файловая система FAT

- awboot использует ChaN FatFs (`lib/fatfs/ff.c` — 259 696 байт; `ffunicode.c` — 1 980 820 байт;
  `ffconf.h` — 12 KB) поверх потокового адаптера `f_read`/`f_lseek(CREATE_LINKMAP)` с блоком
  чтения 32 KB [17][19]. Это, безусловно, самый крупный компонент исходников.
- Rust-замена `fatfs` 0.3.6 — no_std, MIT, поддерживает FAT12/16/32 + LFN и является де-факто
  embedded-библиотекой FAT для Rust [31][32]. Для интеграции нужен адаптер блочного устройства
  `Read/Write/Seek` (либо используйте `embedded-sdmmc`, который уже связывает SD/MMC и FAT)
  [36][32]. Трудозатраты **низко-средние** и явный выигрыш против сгенерированного
  `ffunicode.c` на 2 MB.

### 5.9 PSCI / переход secure-to-nonsecure / векторы исключений — **высокий риск**

- PSCI в awboot взят из U-Boot/Allwinner: `psci.c` (8 KB) реализует `psci_cpu_on/off`,
  `psci_fiq_enter` (GIC FIQ в режиме monitor), `psci_arch_init` (GICD/GICC group/priority/FIQEn,
  SCR) и путь CPUCFG R528 (soft entry `0x1c8`, сброс/статус C0, power gating PRCM) [10].
- Переход в режим monitor — в asm: `nonsec_virt.S` (5.4 KB) настраивает `_monitor_vectors`,
  обрабатывает `smc #0`, программирует SCR (биты NS/AW/FW), опционально режим HYP
  (`CONFIG_ARMV7_VIRT`), сбрасывает `CNTVOFF`, задаёт `MVBAR` и выполняет настройку
  GIC/NSACR/CNTFRQ в `_nonsec_init`; `_do_nonsec_entry` — трамплин входа [11]. `armv7_nonsec.c`
  управляет им (проверка расширения SEC через ID_PFR1, включение GICD, перемещение secure-секции)
  [12]. `psci_vectors.S` (8.5 KB) содержит векторы monitor и сохранение/восстановление при
  исключениях; `start.S` содержит таблицу векторов IRQ/FIQ/abort и макросы `save_regs`/`restore_regs`
  [13][3].
- Rust может это выразить: `global_asm!` для векторов monitor/сохранения-восстановления и `asm!` для
  операций SCR/SMC/MVBAR. `aarch32-cpu` поставляет многие примитивы регистров [22]. Но учтите, что
  awboot помещает защищённый код в перемещаемую секцию `._secure.text`/`._secure.data`
  (`relocate_secure_section`), а стек PSCI — в выделенный регион `NOLOAD .psci_stack` [6][12].
  Корректно воспроизвести размещение секций и перемещение в Rust тонко. *Заглушки* исключений
  тривиальны (`exception.c`, XBOOT MIT, просто печатает регистры и останавливается) [13].

### 5.10 Цепочка загрузки, заголовок eGON и инструментарий

- Заголовок eGON можно сформировать из Rust `global_asm!` точно так же, как это делает `start.S`,
  либо породить Rust-инструментом на хосте. `mksunxi.c` — ~3.5 KB, и его задача (контрольная сумма
  по выровненной длине с `0x5F0A6C39`, выравнивание 512/8192, опциональное разворачивание
  2 KB→страница) тривиальна для переписывания на Rust в `build.rs`/xtask [3][4].
- `xfel` остаётся практичным инструментом FEL/прошивки и написан на C/MIT; полного Rust-эквивалента
  для T113 нет (`aw-fel` 2018 года предшествует T113) [29][30][43]. Инструментарий следует
  **портировать или обернуть**, а не считать готовым.
- Собственное лицензирование awboot **неоднозначно**: репозиторий на GitHub не сообщает лицензию,
  при этом отдельные файлы несут смешанные уведомления (GPL-2.0+ в `dram.c`, BSD в `lib/fdt.c`, MIT
  в `exception.c`) [18][14][16][13]. Это нужно разрешить до масштабного переиспользования кода в
  распространяемом Rust-порте.

---

## 6. Ранжирование по риску и трудозатратам

| Область | Трудозатраты | Риск | Примечания / переиспользование |
|---|---|---|---|
| UART, GPIO, WDG, общий таймер, заглушки исключений | S | Низкий | Тривиальные регистры; портировать первыми для bring-up [9][13] |
| Заголовок eGON + `mksunxi` + сборка/инструментарий | S | Низкий | Rust asm/xtask; `xfel` держать внешним [3][4][29] |
| Тактирование CCU (не-DRAM) | S–M | Низкий–Сред | Записи в регистры; связаны с DDR PLL [8][14] |
| FAT (замена ChaN FatFs, 2 MB) | M | Низкий | `fatfs` / `embedded-sdmmc` [31][36] |
| FDT (замена `lib/fdt.c`) | M | Сред | `fdt` парсит; **запись** bootargs/initrd/memory требует аккуратности [16][35] |
| SDHCI / DWMSHC + SD/MMC | M | Сред | `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc` [15][37][36] |
| SPI NOR/NAND (+ DMA) | M–L | Сред–Выс | Нет зрелого Rust-драйвера sunxi; отложить, если не нужна загрузка SPI [17][43] |
| Стартовый asm / линкер / cache-MMU | M | Сред | `aarch32-rt`; перенести link.ld [6][23] |
| PSCI / secure monitor / перемещение | L | Высокий | asm + перемещение secure-секции [10][11][12] |
| **DRAM init (`dram.c`, ~40 KB)** | **L** | **Наивысший** | Недокументированный код таймингов; оставить на C через FFI либо портировать из U-Boot [14][41] |

---

## 7. Рекомендуемая стратегия поэтапного переноса

1. **Пробный прогон тулчейна (дни).** Соберите блоб Rust `no_std` для `armv7a-none-eabi`/`eabihf`,
   слинкованный по `0x00020000`, проведите его через `mksunxi` + `xfel` и переключите GPIO /
   напечатайте что-нибудь на USART0. Это доказывает интеграцию цели, линкера и eGON до любого
   настоящего портирования [2][3][6][20][21].
2. **Сначала портируйте листовые драйверы (UART, GPIO, WDG, таймер).** Они малы и дают наблюдаемый
   вывод; используйте `aarch32-cpu`/`aarch32-rt` для обвязки CPU/исключений [22][23][9][13].
3. **Оставьте DRAM и secure-monitor на C за FFI.** Скомпилируйте `dram.c` + `reg-*.h` и
   `nonsec_virt.S`/`psci_vectors.S` как объекты; экспортируйте `extern "C"` `sunxi_dram_init()`,
   `dram_get_top()`, `boot_linux_psci()`. Rust владеет потоком управления; C владеет двумя
   рискованными модулями. Это немедленно снижает риск проекта [14][11][41].
4. **Портируйте поток загрузки + eGON + FDT + FAT.** Перенесите оркестрацию из `main.c` на Rust;
   используйте crate `fatfs` или `embedded-sdmmc` для FAT; используйте `fdt` для парсинга и
   небольшой writer (или сохраните `lib/fdt.c`) для модификации. Перепишите `mksunxi` на Rust
   (немного) но держите `xfel` внешним [16][31][36][35][4].
5. **Портируйте SDHCI/протокол карты** с помощью `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc`
   плюс обвязка тактирования/напряжения/pinmux sunxi [15][37][36].
6. **Затем портируйте DRAM** (наинизший приоритет для цели полного Rust), используя очищенный в
   U-Boot `dram_sun20i_d1.c` как эталон и относясь к нему как к побайтовому поведенческому порту с
   регрессионным тестированием hardware-in-the-loop [41].
7. **В завершение портируйте PSCI/secure monitor** в `global_asm!`, аккуратно воспроизводя
   перемещаемую secure-секцию и таблицу векторов monitor [10][11][12][6].

Любое конечное состояние полного Rust должно сначала разрешить вопрос **лицензирования**, поскольку
awboot смешивает уведомления GPL-2.0+, BSD и MIT и не имеет LICENSE верхнего уровня [18][14][16][13].

---

## 8. Открытые вопросы к пользователю

1. **Объём:** цель — *полная* переписка на Rust или гибрид Rust/C, сохраняющий контроллер DRAM
   (и, возможно, asm режима monitor) на C? (Настоятельно рекомендую гибрид.)
2. **Приоритет носителя загрузки:** какой путь должен выйти первым — SD-карта (SDHCI+FAT), eMMC,
   SPI NOR/NAND или FEL? Это полностью меняет порядок работ по SPI/DMA.
3. **Существующий порт пользователя:** какая старая ревизия awboot и какая редакция/цель rustc
   использовались? Было ли это `armv7a-none-eabi` или `eabihf`? Что уже компилируется? (Нужно,
   чтобы не переделывать работу и проверить лицензирование любого скопированного кода.)
4. **Лицензирование:** какую лицензию будет нести Rust-порт? Это ограничивает переиспользование
   GPL-2.0+ `dram.c` и MPL-2.0 `fdt` / Apache-2.0 `dwmmc-host`.
5. **T113-S3 vs T113-S4 vs V851s:** сохранить все три дерева машины или только T113-S3 (как
   заявлено)?
6. **Hardware-in-the-loop:** доступна ли плата T113-S3 для непрерывного регрессионного
   тестирования DRAM/PSCI? (Обязательно для порта DRAM.)
7. **Набор возможностей цели:** подтвердите, требуется ли паритет Cortex-A7 + NEON/VFPv4 +
   hard-float, или для загрузчика допустим soft-float (`-eabi`).
8. **Инструментарий:** допустимо ли оставить `xfel` (C) как хостовый инструмент FEL/прошивки, или
   весь хостовый инструментарий должен быть на Rust?

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

### Примечание о заранее собранных фактах из задания
- «~40 KB dram.c register microcode»: размер подтверждён (40 623 B) [19]; это **не** чисто таблица
  микрокода — это алгоритмический C регистров/таймингов [14].
- Размер `ff.c` подтверждён: 259 696 B; `ffunicode.c` — 1 980 820 B [19].
- `psci.c`/`psci_vectors.S`/`nonsec_virt.S`/`memcpy.S`/`link.ld`/`reg-*.h` — все подтверждены как
  присутствующие, с известным размером [19].
- Имя цели в задании, `armv7a-unknown-none-eabi`, неверно; валидные цели —
  `armv7a-none-eabi` / `armv7a-none-eabihf` [20][21].
- `armv7_nonsec.c` присутствует (3 970 B); в задании он был опущен [19][12].

---

## Дополнение — аудит существующего Rust SPL пользователя (`embed/app` + `embed/rt`)

Источник: локальная рабочая копия `<workspace>/rust/awboot/embed/` (зеркало
`https://github.com/ElicMagus/rawboot`, история по коммит `2f35515 "Tested Dram"`).
Это существенно меняет два вывода выше.

- **Порт уже далеко ушёл от «hello world».** Два crate: `rt` (собственный рантайм — заголовок eGON,
  формируемый из статиков Rust в `.head.head`, `_start` в `.head.start`, инициализация BSS/data,
  линкер-скрипт `link.x`, `cpu.rs` с `wfe`/CPSR/SP/общим таймером/`udelay`) и `app` (SPL: `clk`,
  `ccu`, `gpio`, `uart`, `io`, `print`, `dram`, `sdhci`, `sdcard`). `План.txt` отмечает LED, UART,
  CLK, pre-DRAM magic и DRAM как выполненные; работа по SDHCI/SD-карте присутствует, но
  **не закоммичена** (шаги 6–9: смонтировать FS, загрузить ядро, прыгнуть = всё ещё todo).
- **Собирается на stable**, вопреки любому допущению о nightly: `rustup target add armv7a-none-eabi`
  + `cargo build --release` проходит на `rustc 1.99.0 (b940084d7 2026-09-28)` лишь с
  предупреждениями `static_mut_refs` (lint Rust 2024) в `rt/src/lib.rs`. Выходной ELF ≈ 65 KB.
- **DRAM уже на Rust, и «декомпиляция» НЕ потеряна.** `app/src/dram/mod.rs` (рабочая копия, 63 KB,
  1 926 строк, 22 функции) — читаемый Rust-порт — `auto_scan_dram_size`,
  `auto_scan_dram_rank_width`, `dqs_gate_detect`, `mctl_channel_init`, `auto_set_timing_para`,
  `mctl_phy_ac_remapping`, `ccm_set_pll_ddr_clk`, `eye_delay_compensation`, `auto_cal_timing` и др.
  `app/123.txt` (1 381 строка) — более ранний читаемый снимок того же кода. Ни `include_bytes!`,
  ни гипертрофированный литеральный массив не заменяли читаемый код ни в одной из пяти
  закоммиченных ревизий `dram/mod.rs` (проверены скелет `cde25bd` → `c014bb9`/`d7b3cda`/`c85111e`/
  `2f35515`). Единственный «блобоподобный» элемент — семь 22-байтных таблиц AC-remap `cfg1..cfg7` —
  законно табличные, и они присутствуют и в mainline U-Boot [41].
- **Следствие:** «единственный крупнейший риск — оставить `dram.c` на C через FFI» из отчёта
  **уже снят**. Существенно полное конечное состояние на Rust привлекательнее гибридной базовой
  линии, предложенной в §7, а трудозатраты DRAM переклассифицируются из L/High в done.
- Оставшаяся поверхность порта — то, что §5 всё ещё отмечает: **протокол карты SDHCI DWMSHC + SD/MMC**
  (в процессе локально), **FAT** (заменить ChaN FatFs на 2 MB — использовать crate `fatfs`), **FDT
  чтение+запись**, **asm secure-monitor/PSCI** и **упаковка сборки** (`build.bat` вызывал C-инструменты
  `checksum.exe`/`xfel.exe`/`dd.exe`; переписать контрольную сумму `mksunxi` как Rust xtask).
- **Открытый архитектурный вопрос, вскрытый кодом:** устаревшие шаблоны для модернизации — crate `rt`
  всё ещё использует статики `#[no_mangle]` + `static mut` (Rust 2024 `static_mut_refs`), build-
  зависимость `cc` не используется (`build.rs` содержит закомментированную компиляцию `asm.s`),
  `asm.s` имеет размер 0 байт, а `.cargo/config` жёстко задаёт сырой `-Tlink.x`. Стоит разрешить до
  масштабирования порта.
