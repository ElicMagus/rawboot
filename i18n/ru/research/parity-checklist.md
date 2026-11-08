# Контрольный список parity с upstream (модули loader)

Исследовано: 2026. Статический индекс для issue #14: одна строка на каждый входящий в область
модуль loader, указывающая на контрольный список расхождений функция за функцией. Parity
означает поведенческую эквивалентность с upstream — тот же порядок шагов, константы, ветки и
таблицы — при том что выражение остаётся нашим идиоматичным Rust (ADR-0007).

Якорь upstream: `szemzoa/awboot` `main` `5380c00fc67c975433f25c57fb481aa2b91aebf8`
(2025-11-21). Загрузить его можно через `scripts/fetch-upstream.sh`; цитировать upstream по
файлу и строке, наш код — по символу (`docs/agents/domain.md`). Этот индекс ссылается только на
файлы; ссылки на уровне строк находятся в каждом связанном контрольном списке.

## Модули в области

| Модуль | Исходник upstream | Локальный код | Контрольный список расхождений | Тикет |
| --- | --- | --- | --- | --- |
| `ccu` | `arch/arm32/mach-t113s3/reg-ccu.h` | `rawboot/src/ccu/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `clk` | `arch/arm32/mach-t113s3/sunxi_clk.c` | `rawboot/src/clk/mod.rs` | `research/clk-port-vs-upstream.md` | #16 |
| `gpio` | `arch/arm32/mach-t113s3/sunxi_gpio.[ch]`, таблицы выводов `board.c` | `sunxi-pl/src/gpio.rs` (места вызова в `rawboot/src/sdhci/mod.rs`, `rawboot/src/main.rs`) | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `io` | `include/io.h`, `arch/arm32/mach-t113s3/arch_timer.c` (только `sdelay`) | `sunxi-pl/src/io.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `cpu` (таймер) | `arch/arm32/mach-t113s3/arch_timer.c` | `sunxi-pl/src/cpu.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17, #29 |
| `uart` | `arch/arm32/mach-t113s3/sunxi_usart.[ch]`, `board.c` (`usart0_dbg`) | `rawboot/src/uart/mod.rs` | `research/gpio-io-uart-port-vs-upstream.md` | #17 |
| `dram` | `arch/arm32/mach-t113s3/dram.[ch]`, `reg-dram.h` | `rawboot/src/dram/mod.rs` | `research/dram-port-vs-upstream.md` | #15, #18 |
| `sdhci` | `arch/arm32/mach-t113s3/sunxi_sdhci.[ch]` | `rawboot/src/sdhci/mod.rs` | `research/sdhci-port-vs-upstream.md` | #19 |
| `sdcard` | `arch/arm32/mach-t113s3/sdmmc.[ch]` | `rawboot/src/sdcard/mod.rs` | `research/sdcard-port-vs-upstream.md` | #20 |

Каждый контрольный список содержит поверхность функций upstream и оставшиеся намеренные
различия; заметки #17/#19/#20 также перечисляют расхождения, исправленные их тикетами. Заметка
по DRAM дополнительно фиксирует историю до переработки (точку ответвления `mctl_hal.c`), стоящую
за старым портом.

## Намеренные различия

Часть решения о parity, а не пробелы, которые нужно "исправить"; подробности во владеющем
разделе:

- **`dram_get_top` / `dram_top` отсутствуют** — `research/dram-port-vs-upstream.md` (оставшиеся
  различия). Добавлять только когда payload понадобится верхний адрес (issue #14).
- **Пути T113-S4/V851s отсутствуют** — ADR-0007. Цель — только T113-S3.
- **`abort()` отсутствует** — `research/dram-port-vs-upstream.md` (оставшиеся различия).
- **Раскладка модулей и идиомы C остаются нашими** — ADR-0007. Upstream `common.h`, `xformat`
  и макросный `debug.h` не воспроизводятся.
- **`ccu` сохраняет имена констант `PLL_*_REG`.** Значения были сверены с `reg-ccu.h`; строгое
  переименование с префиксом `CCU_*` — механическое продолжение, а не поведенческий пробел
  (`research/clk-port-vs-upstream.md`, намеренные различия).
- **Отложенные драйверы upstream** — #21 отслеживает движок DMA, SPI/SPI-NAND, watchdog и
  помощники кэша/исключений.

## Проверка

- Статический оракул: контрольные списки выше плюс закреплённые исходники upstream.
- Шов выполнения: дымовой тест HIL на T113-S3 (`docs/agents/hil.md`) — журнал инициализации UART
  плюс работающий blink payload.
