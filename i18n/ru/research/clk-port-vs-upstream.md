# Порт тактирования в `rawboot` против текущего upstream `szemzoa/awboot` `sunxi_clk.c`

Исследовано: 2026. Цель: Allwinner T113-S3. Upstream закреплён на `szemzoa/awboot`
`main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8` (2025-11-21), файл
`arch/arm32/mach-t113s3/sunxi_clk.c` [1]. Локальная сторона — `rawboot/src/clk/mod.rs` и
`rawboot/src/ccu/mod.rs` [2][3].

Эта заметка — аналог `research/dram-port-vs-upstream.md` для тактирования; она существует,
чтобы при проверке parity не приходилось заново загружать исходник upstream.

---

## 1. Сравнение функция за функцией

| Upstream `sunxi_clk.c` (5380c00) | `rawboot/src/clk/mod.rs` | Статус |
| --- | --- | --- |
| `wait_for_bit_set(addr, mask)` | `wait_for_bit_set` | То же: 2 000 000 прокруток, `sdelay(1)`, запоминается первый адрес с ошибкой. |
| `sunxi_clk_get_fail_addr()` | `sunxi_clk_get_fail_addr` | То же, через `AtomicU32` вместо статической переменной. |
| `set_pll_cpux_axi()` | `set_pll_cpux_axi` | Тот же порядок: источник PLL_PERI(1x), `sdelay(10)`, gating/LDO, `41 << 8`, lock/enable, ожидание бита 28, `sdelay(20)`, отключение gating, финальный `CPU_AXI_CFG` с `DIV2=1`. Финальная запись опускает явный `0 << 16`, что эквивалентно upstream. |
| `set_pll_periph0()` | `set_pll_periph0` | То же, включая ранний возврат при установленном бите 31. |
| `set_ahb()` | `set_ahb` | Тот же `PSI_CLK_REG = (2 << 0) | (0 << 8) | (0x03 << 24)`. |
| `set_apb()` | `set_apb` | Тот же `APB0_CLK_REG = (2 << 0) | (1 << 8) | (0x03 << 24)`. |
| `set_dma()` | `set_dma` | Та же последовательность reset/gate. |
| `set_mbus()` | `set_mbus` | Те же reset и маска gating `0x00000d87`. |
| `set_module(addr)` | `set_module` | Та же последовательность enable/lock/wait. |
| `sunxi_clk_init()` | `sunxi_clk_init` | Тот же порядок: CPUX/AXI, PERI0, AHB, APB0, DMA, MBUS, затем PERI0, VIDEO0, VIDEO1, VE, AUDIO0, AUDIO1. |
| `sunxi_clk_get_peri1x_rate()` | `sunxi_clk_get_peri1x_rate` | Та же формула `(24 * plln / (pllm * p0)) >> 1 * 1000 * 1000`. |
| `sunxi_clk_dump()` | `sunxi_clk_dump` | Тот же выбор источника CPU (биты 24-26 `CPU_AXI_CFG`), формула PLL CPU, ветки PLL_PERI 2X/1X/800M и PLL_DDR. |

## 2. Намеренные различия

- **Защита дампа и место вызова.** Upstream компилирует `sunxi_clk_dump` под
  `CONFIG_ENABLE_CPU_FREQ_DUMP`. Порт компилирует его всегда, и `main` вызывает его до
  `dram.init()`. В результате `CLK: PLL_ddr=` сообщает значение до инициализации DRAM (432 МГц
  в журнале HIL); последующая инициализация DRAM устанавливает 792 МГц. Это выбор порядка
  диагностики, а не различие инициализации. Перенос дампа после `dram.init()` заставил бы эту
  строку сообщать итоговый PLL_DDR, если это когда-нибудь станет важно.
- **Печать.** Upstream использует `debug(...)`; порт использует `print!` на USART0. Текст и
  значения совпадают.
- **Имена констант CCU.** Upstream использует имена `CCU_*` из `reg-ccu.h`. Модуль `ccu` в порту
  сохраняет существующие в репозитории имена `PLL_*_REG`/`*_REG`; значения были сверены с
  `reg-ccu.h` и отображаются один к одному. ADR-0007 требует upstream `snake_case` для общей
  терминологии; имена функций выше его соблюдают. Строгое переименование с префиксом `CCU_*` —
  механическое продолжение, а не поведенческий пробел.

## 3. Подтверждение на оборудовании

HIL на T113-S3 (FEL, USART0 115200, без `xfel ddr`; см. `docs/agents/hil.md`) после слияния
#16 вывел:

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

Строки `CLK: init timeout` нет; UART и DRAM поднимаются, и цикл LED работает.

---

## Sources

1. Upstream `sunxi_clk.c` на `5380c00`:
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_clk.c>
2. `rawboot/src/clk/mod.rs` (этот репозиторий).
3. `rawboot/src/ccu/mod.rs` (этот репозиторий).
4. ADR-0007, `docs/adr/0007-align-behaviour-to-upstream.md`.
5. Issue #16, "Parity тактов: ccu + clk".
