# AGENTS

## Проект

`rawboot` — загрузчик второй ступени для Allwinner T113-S3 (ARMv7-A, два ядра Cortex-A7),
написанный на 100% Rust и портированный из `szemzoa/awboot`. Он инициализирует SoC, читает сырой
образ payload с FAT-раздела SD-карты через SDHCI, загружает его в DRAM и передаёт управление по
версионированному контракту `BootInfo`. Payload — наша собственная прошивка, а не Linux (без FDT,
без PSCI).

Cargo workspace из девяти членов: `rawboot` (loader), `rawboot-rt` (runtime), `sunxi-pl` (общий
платформенный слой: MMIO, GPIO, таймер), `sunxi-drivers` (общий драйверный крейт поверх `sunxi-pl`:
clock, DRAM, UART, SDHCI, SD/MMC, FAT), `payload` (тестовый образ payload с миганием, который
загружает loader), `rawboot-image` (тестируемые на хосте инструменты payload/eGON, SD boot prefix и
flash job/status), `rawboot-fel` (хостовый транспорт FEL по USB и CLI), `rawboot-flasher`
(вызываемый из FEL DDR-init stub и слинкованный в DRAM flasher) и `xtask` (хостовый инструментарий
сборки). Зависимость `fatfs` `core_io` для `no_std` пропатчена на `vendor/core_io` — минимальный
стабильно-совместимый шим. Цель `armv7a-none-eabi` (soft-float), Rust edition 2024.

`embed/` — игнорируемый снимок более старого Rust-порта, сохранённый для сравнения до портирования;
закреплённый C upstream скачивается скриптом `scripts/fetch-upstream.sh` в `target/upstream/`.
Исследуйте крейты workspace, а не эти деревья.

Поиск: используйте инструменты агента Grep/Glob; `rg` на этом хосте не установлен.

Стандартные команды:

- Сборка: `cargo build --release` (`xtask` и его хостовый инструментарий образов держатся вне
  bare-metal `default-members`; `rawboot-image` всё равно собирается как no-alloc-зависимость
  loader. `rawboot-flasher` тоже вне `default-members`: он включает `sunxi-drivers/write`, что
  привело бы к унификации с loader; его собирает `cargo xtask flasher`)
- Тест (хост, инструментарий): `cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu`
  (цель workspace по умолчанию — `armv7a-none-eabi`, поэтому хостовым тестам нужен явный
  `--target`; область пакетов держит аппаратные bare-metal члены вне хостовой сборки. У
  `sunxi-drivers` есть хостовые юнит-тесты блочного адаптера и записи FAT поверх mock-диска; им не
  нужна плата.)
- Lint: `cargo clippy -- -D warnings` (у `no_std`-крейтов нет хостовой тестовой цели, поэтому
  `--all-targets` не может собрать libtest-обвязку на `armv7a-none-eabi`; `sunxi-drivers` не
  является членом по умолчанию, поэтому два его набора фич линтуются явно командами
  `cargo clippy -p sunxi-drivers -- -D warnings` и
  `cargo clippy -p sunxi-drivers --no-default-features -- -D warnings`; flasher линтуется отдельно
  командой `cargo clippy -p rawboot-flasher -- -D warnings`, потому что он не член по умолчанию и
  включает фичу write; хостовый инструментарий и хостовые тесты драйверов линтуются командой
  `cargo clippy -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu --all-targets -- -D warnings`;
  шим `core_io` линтуется командой
  `cargo clippy --manifest-path vendor/core_io/Cargo.toml --target-dir target/core_io-shim -- -D warnings`)
- Форматирование: `cargo fmt` (CI запускает `cargo fmt --all -- --check`)
- Дизассемблирование: `scripts/objdump.sh -d target/armv7a-none-eabi/release/rawboot` (оборачивает
  `llvm-objdump` из компонента `llvm-tools`; `cargo objdump` не существует)
- Скачать закреплённый upstream: `scripts/fetch-upstream.sh` (записывает дерево C, используемое
  исследовательскими заметками parity, в `target/upstream/awboot-<sha>/`)
- Проверить всё: `scripts/check.sh` (fmt, ранний guard графа фич `scripts/check-features.sh`,
  сборка, проходы clippy для bare-metal, flasher, драйверов и хоста, хостовые тесты, заголовки
  SPDX, правило документов веток, guard символов no-write у loader, дымовые сборки
  `cargo xtask image`/`payload`/`flasher`, `cargo xtask fel-flash --dry-run` и `cargo xtask sd-image`)
- Запечатать образ: `cargo xtask image` (дважды собирает loader и пишет запечатанный
  `target/rawboot-fel.bin` для FEL по адресу `0x00028000` и `target/rawboot-boot-sd.bin` для
  загрузки с SD/SPI по адресу `0x00020000`; псевдоним `xtask` закрепляет хостовый triple в
  `.cargo/config.toml`)
- Публикация публичного репозитория: переписанная история `master` через `v0.1.0` плюс тег уходят на
  GitHub (ADR-0016); старый одно-коммитный `scripts/export-public.sh` выведен из обращения. Разовая
  миграция и ручная процедура исправления публикации описаны в `docs/agents/export-public.md`
- Собрать образ payload: `cargo xtask payload` (собирает крейт `payload` и пишет
  `target/rawboot-payload.img` — его плоский бинарник, обёрнутый в image header payload)
- Собрать образы FEL flasher: `cargo xtask flasher` (пишет `target/rawboot-ddr.bin` — возвращающийся
  DDR-init stub по адресу `0x00028000`, и `target/rawboot-flasher.bin`, слинкованный по адресу
  `0x40010000` в DRAM; оба не несут заголовка eGON; ADR-0013)
- Прошить карту в самой плате: `cargo xtask fel-flash [--payload <path>] [--boot-cfg <path>]
  [--loader <path>] [--flasher <path>] [--ddr-stub <path>] [--sequence <n>] [--skip-build]
  [--dry-run] [--yes]` (собирает или переиспользует образы, проверяет запечатанный SD loader и
  отвергает FEL-вариант, записывает файлы payload и loader через FEL, не трогая MBR, и читает всё
  обратно. `--dry-run` проверяет и печатает план, не открывая FEL; реальная запись требует `--yes`;
  ADR-0014; runbook `docs/agents/hil.md`)
- FEL CLI: `cargo fel version|read|write|exec|bringup ...` (хостовый транспорт FEL по USB без
  системного `libusb`; `version` идентифицирует T113-S3, `read <addr> <len>` делает hex-дамп или
  пишет в `--output`, `write <addr> <file>` загружает байты, `exec <addr>` совершает переход,
  `bringup <ddr-stub> <flasher>` запускает возвращающийся stub, проверяет шаблон DRAM и читает
  статус flasher; псевдоним `fel` закрепляет хостовый triple)
- Собрать SD boot prefix: `cargo xtask sd-image --partition-sectors <n> [--start-lba <n>]
  [--loader <path>]` (после `cargo xtask image`; пишет `target/rawboot-sd-mbr.img` — MBR,
  указывающий на существующий FAT-раздел, плюс SD loader в секторе 16, для `dd conv=notrunc` на
  карту; ничего не форматирует и не копирует payload; ADR-0011)
- Подготовить SD-карту: `scripts/flash-sd.sh [--device /dev/sdX] [--copy-payload] [--format]`
  (запускайте от своего пользователя, не через `sudo`; автоматически обнаруживает единственный
  съёмный/USB/MMC-диск, `--list` показывает кандидатов, собирает и пишет boot prefix, при желании
  форматирует/перезаполняет только раздел; запускается с `--dry-run`, ничего не трогая; отказывается
  от смонтированных, корневых, GPT- или многраздельных дисков; runbook `docs/agents/hil.md`)
- HIL: `docs/agents/hil.md` — авторитетный runbook. После `code-review`, на ветке кода:
  `scripts/hil-check.sh target/rawboot-fel.bin <label>`; нестабильность SD (#43) ->
  `scripts/hil-sd-retry.sh`; обновление карты в плате -> `scripts/hil-flash.sh` (один холодный старт
  на попытку: повторный DDR-init stub может зависнуть; см. таблицу условий в runbook). Блокер
  `CMD24` из HIL #42 (#47) исправлен (PR #49). `scripts/check.sh` покрывает только хостовые
  проверки.

## CI

`.gitea/workflows/ci.yml` запускает `scripts/check.sh` на каждый push и pull request. Нужен
`act_runner`, зарегистрированный с меткой `ubuntu-latest`; пока его нет, запускайте скрипт вручную
перед пушем.

## Рабочие соглашения

- **Язык git-истории — русский, публичный трекер двуязычный.** Заголовки, тела issue,
  комментарии и сообщения коммитов веди на русском. При публикации в GitHub-архив английский
  перевод идёт сверху, русский оригинал — в `<details>`. Общение с владельцем проекта (в том
  числе вопросы, уточнения и комментарии в трекере) — на русском.
- **Публичный GitHub — зеркало с настоящей историей (ADR-0016).** Gitea остаётся каноническим
  трекером и источником истины; на GitHub публикуются история `master` с тегами, а также весь
  код, окружение и решения. Внутренними остаются `.gitea/`, `scripts/export-public.sh`,
  `scripts/issue-view.sh`, `target/` и `embed/`. Незамерженные ветки не публикуются: сначала
  мерж, потом отдельный ручной заход.
- **Документация: английский — первоисточник.** Английские файлы лежат на текущих путях;
  переводы зеркалят дерево в `i18n/ru/` и `i18n/zh-CN/` с теми же относительными путями. Агент
  читает и правит только английские файлы, переводы не читает; при расхождении прав английский.
  Имена API, регистров и термины глоссария не переводятся; черновики переводов агент делает сам,
  без отдельного ревью.
- **Ветку с решением issue — пушь, PR — не создавай.** Решив задачу, создай ветку
  (`feat/<N>-<slug>` / `fix/<N>-<slug>`), закоммить и запушь её в `origin`. Pull request открывает
  только владелец проекта. Инструментальные правки без issue, включая ретро, веди прямо в
  `master`; отдельная ветка нужна только для issue или когда запрошен PR.
- **Доки коммить прямо, без ветки.** ADR, `GLOSSARY.md`, `research/` и `docs/` коммить в текущую
  ветку (обычно `master`); ветка (`feat/`/`fix/`) — для кода. Исключение: доки, без которых merge
  меняет контракт (spec/GLOSSARY/ADR о границе модуля, публичном API или `default-members`), веди
  в code branch. Перед коммитом сверь новые доки со старыми ADR, глоссарием и кодом; нашёл
  противоречие — сначала грилинг (`grill-with-docs`), потом коммит.
- **Перед пушем прогони ревью — skill `code-review` (Standards + Spec).**
- **Порядок HIL и доков.** Инструкции по проверке (runbook, команды) можно коммитить до HIL;
  заявления о результате (PASS, подтверждённый LED) — только после подтверждения HIL.
- **Ручные HIL-шаги веди диалогом.** Если тест требует действий человека (power-cycle, вставка
  карты, FEL/BOOT, подтверждение LED), задавай один вопрос через инструмент `question` перед
  каждым шагом или циклом и жди ответа. Не запускай следующий цикл и не переходи к следующему
  шагу без подтверждения; не объединяй несколько циклов в один вопрос. В вопросе давай точную
  физическую процедуру и ожидаемый результат, после прогона — PASS/FAIL, ключевые строки лога и
  следующий шаг. В одном вызове `question` задай два вопроса-вкладки: сам шаг и контрольный
  «Уверен?» с вариантами «Да» и «Нет». Выполняй действие только если шаг подтверждён и на
  «Уверен?» выбрано «Да»; иначе вернись к вопросу о шаге. Это защита от случайного нажатия.
- **После пуша выдавай инструкцию по проверке** — шаги сборки, загрузки через FEL и/или SD и
  наблюдения за LED payload'а.
- **Перед merge перечисли acceptance issue.** Невыполненный пункт либо оставляет issue открытым,
  либо переносится в follow-up issue с native blocking link и явной пометкой. PASS одной части
  (например, чтения) не заменяет PASS другой (записи).
- **После подтверждения — влей ветку в `master`, закрой issue, удали ветку.**
- **Найденные недостатки оформляй отдельным issue** с меткой `needs-triage`.

## Навыки агента

### Трекер issue

Issue живут в Gitea по адресу `ElicMagus/rawboot-internal`. См. `docs/agents/issue-tracker.md`.

### Метки триажа

Две категорийные и пять статусных ролей, сопоставленных со строками меток этого репозитория. См.
`docs/agents/triage-labels.md`.

### Доменные документы

Единый контекст. Спека в `docs/spec-rawboot-v1.md`, исследовательские заметки в `research/`,
глоссарий в `GLOSSARY.md`, решения в `docs/adr/`. См. `docs/agents/domain.md`.

Суждения ревьюера живут в `CODING_STANDARDS.md`; читайте его во время `code-review`.
