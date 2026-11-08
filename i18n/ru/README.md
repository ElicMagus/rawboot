[English](../../README.md) | [简体中文](../zh-CN/README.md) | [Русский](README.md)

# rawboot

Загрузчик второй ступени для Allwinner T113-S3 (ARMv7-A, два ядра Cortex-A7),
написанный на 100% Rust и портированный из [`szemzoa/awboot`](https://github.com/szemzoa/awboot).
Он инициализирует SoC, читает образ payload с FAT-раздела SD-карты через SDHCI,
загружает его в DRAM и передаёт управление по версионированному контракту
`BootInfo`. Payload — наша собственная прошивка, а не Linux: без FDT, PSCI и
initrd.

## Workspace

Члены Cargo-workspace:

- `rawboot` — сам загрузчик (инициализация SoC, работа с накопителем, передача управления)
- `rawboot-rt` — runtime (заголовок eGON, `_start`, linker script)
- `sunxi-pl` — общий платформенный слой (MMIO, GPIO, таймер) для загрузчика, runtime и payload
- `sunxi-drivers` — общие драйверы поверх `sunxi-pl` (часы, DRAM, UART, SDHCI, SD/MMC, FAT)
- `payload` — тестовый payload с мигающим светодиодом, который загружает загрузчик
- `rawboot-image` — тестируемые на хосте образы payload/eGON, SD-префикс загрузки и контракт
  прошивки (`build`, `parse`, `seal`, `verify_sealed`, `build_sd_image`, `FlashJob`,
  `FlashStatus`, `FlashPlan`, `check_sd_layout`)
- `rawboot-fel` — хостовый транспорт FEL по USB и CLI (`version`, `read`, `write`, `exec`,
  `bringup`), а также `flash_sd_card` для обновления карты в плате
- `rawboot-flasher` — вызываемый из FEL stub инициализации DDR и flasher, слинкованный в DRAM;
  он записывает файлы payload и SD-загрузчик через FEL
- `xtask` — хостовые инструменты сборки (`image`, `payload`, `flasher`, `sd-image`, `fel-flash`)

Цель: `armv7a-none-eabi` (soft-float), Rust edition 2024, зафиксирована в
`rust-toolchain.toml`.

## Сборка и тесты

```sh
cargo build --release
```

Хостовые тесты (цель по умолчанию — bare-metal, поэтому хостовый triple указан
явно):

```sh
cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers \
  --target x86_64-unknown-linux-gnu
```

Сборка и запечатывание образов загрузчика:

```sh
cargo xtask image
```

Команда пишет `target/rawboot-fel.bin` для FEL и `target/rawboot-boot-sd.bin`
для загрузки с SD/SPI. `scripts/check.sh` запускает полный локальный набор
проверок: форматирование, lint, сборки, хостовые тесты и дымовые сборки образов.

Чтобы обновить карту, оставленную в плате, переведите плату в FEL и запустите
`cargo xtask fel-flash --yes`; команда соберёт или переиспользует образы,
запишет `payload.bin` и загрузчик, не тронет MBR и FAT-раздел и прочитает всё
обратно для проверки. `cargo xtask fel-flash --dry-run` проверяет и печатает
план, не открывая FEL.

Терминология — в `GLOSSARY.md`, архитектурные решения — в `docs/adr/`.

## Лицензия

Проект распространяется под
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
(SPDX: `CC-BY-4.0`). Полный юридический текст — в [`LICENSE`](../../LICENSE).
Повторное использование и изменение разрешены при указании авторства.

## Атрибуция

Проект разработан с помощью ИИ-агента DeepSeek V4.1 Flash и набора скиллов
[matt-pocock-skills](https://github.com/mattpocock/skills). Вдохновение —
[`szemzoa/awboot`](https://github.com/szemzoa/awboot).
