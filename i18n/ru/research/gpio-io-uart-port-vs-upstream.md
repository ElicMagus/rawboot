# Порт GPIO/IO/UART в `rawboot` против текущего upstream `szemzoa/awboot`

Исследовано: 2026. Цель: Allwinner T113-S3. Upstream закреплён на `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21). Сравниваемые файлы:
`arch/arm32/mach-t113s3/sunxi_gpio.c`, `arch/arm32/mach-t113s3/sunxi_gpio.h`,
`arch/arm32/mach-t113s3/sunxi_usart.c`, `arch/arm32/mach-t113s3/sunxi_usart.h`,
`include/io.h`, `arch/arm32/mach-t113s3/arch_timer.c` (`sdelay`), а также таблицы выводов
платы в `board.c` и настройка выводов SD в `sunxi_sdhci.c` [1]-[8]. Локальная сторона —
`sunxi-pl/src/gpio.rs`, `sunxi-pl/src/io.rs`, `sunxi-pl/src/cpu.rs`, `rawboot/src/uart/mod.rs`,
`rawboot/src/sdhci/mod.rs` и `rawboot/src/main.rs`; локальный код цитируется по символам, а не
по строкам (см. `docs/agents/domain.md`). Обновлено после issue #29, который перенёс общий
аппаратный слой в crate `sunxi-pl` (ADR-0010).

Эта заметка — аналог `research/dram-port-vs-upstream.md` и `research/clk-port-vs-upstream.md`
для GPIO/IO/UART; это статический контрольный список parity для issue #17.

---

## 1. Сравнение функция за функцией

| Upstream | Локально (после #17) | Статус |
| --- | --- | --- |
| `_port_base_get` [1:24] | `port_base_get` | Тот же switch, базы `0x0200_0030`/`0060`/`0090`/`00c0`/`00f0`/`0120` (именованные константы). |
| `_pin_num` [1:52] | `pin_num` | Тот же `pin & ((1 << 5) - 1)`. |
| `sunxi_gpio_init` [1:57] | `sunxi_gpio_init` | Тот же `CFG0 + ((pin >> 3) << 2)`, маскируемое 4-битное поле, защита `if cfg`. |
| `sunxi_gpio_write` [1:72] | `sunxi_gpio_write` | Тот же read-modify-write `DAT`, `!!value`. |
| `sunxi_gpio_read` [1:84] | `sunxi_gpio_read` | Тот же `!!(DAT & bit)`. |
| `sunxi_gpio_set_pull` [1:94] | `sunxi_gpio_set_pull` | Тот же `PUL0 + ((pin >> 4) << 2)` и карта `v` из upstream (up→1, down→2, none→0). |
| `sunxi_gpio_set_drive_lvl` [1:126] | `sunxi_gpio_set_drive_lvl` | Тот же `DRV0 + ((pin >> 4) << 2)`, `lvl &= 3`. |
| `read8/16/32/64`, `write8/16/32/64` [5] | те же имена | Добавлены недостающие разрядности; `read32`/`write32` без изменений. |
| `sdelay` [6:60] | `sdelay` | Тот же цикл декремента `subs`/`bne`. |
| `get_arch_counter` [6:16] | `get_arch_counter` | Тот же `mrrc p15, 0, …, c14`, 64-битный счётчик. |
| `time_ms` [6:26] | `time_ms` | То же: счётчик / 24000. |
| `time_us` [6:34] | `time_us` | То же: счётчик / 24. |
| `udelay` [6:39] | `udelay` | Расходится: upstream крутится на `time_us()`, наш вариант сравнивает сырой счётчик с дедлайном (раздел 3). |
| `sunxi_usart_init` [3:24] | `sunxi_usart_init` | Тот же порядок: mux TX/RX, подтяжки, gate, reset, `div = 1500000/baud`, `LCR/DLL/LCR/FCR`. |
| `sunxi_usart_putc` [3:61] | `sunxi_usart_putc` | То же ожидание TFNF, запись THR, ожидание BUSY. |
| `sunxi_usart_put` [3:72] | `sunxi_usart_put` | Тот же цикл по байтам. |
| `sunxi_usart_get` [3:80] | `sunxi_usart_get` | То же ожидание RFNE, чтение RBR. |
| `sunxi_usart_t` [4:10] | `SunxiUsart` | Те же поля: `id`, `gpio_tx`, `gpio_rx`. |
| `usart0_dbg` (board) [7:10] | `usart0_dbg` | Те же выводы/mux: TX PE2 MUX6, RX PE3 MUX6. |
| `board_init_led` PD22 [7:76,99] | `main` PD22 | Тот же `GPIO_OUTPUT`, затем запись 0. |
| Выводы SDHCI PF2/3/1/0/5/4 MUX2 [7:49-54] | `sdhci_t::default` | Те же выводы/mux; тот же порядок `init`+`set_pull(up)` в `sdhci_t::init` [8:942-958]. |

## 2. Расхождения, исправленные в #17

1. **Вывод RX UART был неверным.** Старый `UartHandler::default` задавал `rx` как
   `gpio::Port::E, pin: 2` (`PE2`, вывод TX), хотя номинально выбирал `pe3::UART0_RX` (см.
   `85e3e1e`), тогда как `usart0_dbg.gpio_rx` в upstream — `GPIO_PIN(PORTE, 3)` [7:13].
   Исправлено в `usart0_dbg`: `gpio_rx.pin = gpio_pin(PORTE, 3)`.
2. **API и имена GPIO.** Старый порт использовал структуру `Gpio` плюс модули mux для каждого
   вывода (`pd22`, `pe2`, `pe3`, `pf0..pf5`) и объединял mux+pull в одном `init`. Upstream
   использует упакованный `gpio_t` (`port << 5 | pin`), `gpio_mux_t`, константы
   `GPIO_INPUT`/`GPIO_OUTPUT`/`GPIO_PERIPH_MUXn`/`GPIO_DISABLED` и раздельные `sunxi_gpio_init` /
   `sunxi_gpio_set_pull` [2]. Заменено словарём функций/упаковки upstream, с идиоматичными для
   Rust типами (`GpioPin`, `GpioMux`, `GpioPull`) согласно ADR-0007.
3. **Кодирование подтяжки было неверным.** Старые `Pull::UP = 0x01`, `Pull::DOWN = 0x10` (позиция
   бита, а не значение), и `init` очищал всё 4-битное поле подтяжки. Upstream отображает
   константу подтяжки в `v` и только маскирует/устанавливает биты `v`, поэтому pull-down пишет
   `0b10` [1:101-123]. Заменено на `GpioPull` (`Up`/`Down`/`None`) и отображение upstream в
   `sunxi_gpio_set_pull`.
4. **Последовательность инициализации UART.** Старый `init` жёстко задавал UART0 и делитель 13,
   писал IER/MCR и `FCR = 0xf7`, а LCR собирал из старого значения. Upstream принимает
   `baudrate`, вычисляет `div = 1500000 / baudrate` (с округлением вниз, минимум 1) и делает
   `LCR = (1<<7)|3; DLL = div; LCR = 3; FCR = 1` [3:48-58]. Заменено этой последовательностью в
   `sunxi_usart_init`.
5. **Разрядности `io`.** Старый модуль имел только `read32`/`write32`. Upstream `include/io.h`
   предоставляет аксессоры на 8/16/32/64 бита [5]; добавлены недостающие `read16`/`read64`/
   `write8`/`write16`/`write64`.
6. **`sdelay`.** Старый `sdelay` был циклом прокрутки на `write_volatile` (см. `85e3e1e`).
   Upstream `arch_timer.c` использует цикл `subs`/`bne` [6:60-67]; воспроизведено в `io::sdelay`.
7. **Mux выводов платы.** Выход LED PD22 и USART0 PE2/PE3 на MUX6 теперь соответствуют
   `board.c` в upstream; SDHCI сохраняет свой набор PF MUX2 и порядок init/pull из upstream
   [7][8].

## 3. Намеренные различия

- **Печать.** Upstream печатает через `debug(...)`/xformat; порт сохраняет `core::fmt` и
  направляет `print!` через `UartConsole` на `usart0_dbg`. Текст и значения не изменены.
- **Нет путей для отдельных SoC.** Таблицы выводов T113-S4/V851s не воспроизводятся; в область
  входит только T113-S3 (ADR-0007).
- **Типы.** Имена функций, констант и глобальных переменных следуют upstream (`sunxi_gpio_*`,
  `sunxi_usart_*`, `usart0_dbg`, `GPIO_PERIPH_MUX*`); типы Rust и их конструкторы идиоматичны
  (`GpioPin`, `GpioMux`, `GpioPull`, `SunxiUsart`, `gpio_pin`) согласно ADR-0007, а значения
  регистров и порядок шагов остаются как в upstream.
- **`udelay` считает сырой счётчик.** Upstream крутится на `time_us()` [6:39-46]; наш вариант
  читает счётчик один раз, вычисляет дедлайн и ждёт, пока счётчик его пройдёт. Длительность
  ожидания та же, но наш вариант сравнивает абсолютные значения, поэтому не переживает
  переполнение 64-битного счётчика, тогда как вычитание в upstream переживает. Перенесено без
  изменений из порта до #29; исправление — отдельный вопрос parity.

## 4. Подтверждение на оборудовании

HIL на T113-S3 (FEL, USART0 115200 8N1; `scripts/hil-check.sh target/rawboot-fel.bin iss17`)
после изменений #17 вывел:

```
Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
trefi:7.8ms
DRAMC read ODT  off.
DRAM ODT value: 0x42.
DRAM simple test OK.
get state
SD/MMC card not found
S=0x10000
```

Скрипт сообщил `PASS: UART assertions` (`Hello`, `CLK:`, `DRAM simple test OK`, `S=0x`), один
`Hello` и отсутствие цикла сброса. Мигание LED PD22 проверяется вручную.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/sunxi_gpio.c` на `5380c00`:
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_gpio.c>
2. Upstream `arch/arm32/mach-t113s3/sunxi_gpio.h` на `5380c00`.
3. Upstream `arch/arm32/mach-t113s3/sunxi_usart.c` на `5380c00`.
4. Upstream `arch/arm32/mach-t113s3/sunxi_usart.h` на `5380c00`.
5. Upstream `include/io.h` на `5380c00`.
6. Upstream `arch/arm32/mach-t113s3/arch_timer.c` на `5380c00`.
7. Upstream `board.c` на `5380c00`.
8. Upstream `arch/arm32/mach-t113s3/sunxi_sdhci.c` на `5380c00`.
9. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
10. Issue #17, "Parity GPIO/IO/UART".
