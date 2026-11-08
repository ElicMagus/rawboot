# Порт DRAM в `rawboot` против текущего upstream `szemzoa/awboot` `dram.c`

Исследовано: 2026, статус обновлён после issue #18. Цель: Allwinner T113-S3 (DDR3, 792 МГц
по умолчанию). Утверждения об upstream цитируются по точному первоисточнику (закреплённый SHA
плюс номера строк). Локальный код цитируется по символам, согласно `docs/agents/domain.md`;
локальный файл меняется, поэтому номера строк устарели бы.

Номера ссылок в квадратных скобках соответствуют разделу **Sources** в конце.

---

## 1. Краткое резюме

- Upstream закреплён на `szemzoa/awboot` `main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`
  (2025-11-21, "Merge pull request #41 ... psci-rebase") [12]. Последний коммит, затрагивающий
  `arch/arm32/mach-t113s3/dram.c`, — `385481ffab45089d90410cb67b93981a870edd7c`
  (2025-11-15) [4][9]. На этом SHA `dram.c` занимает 1453 строки / 40 623 Б [1].
- Наш порт на Rust не происходит ни от какой ревизии `dram.c`. Это почти построчный порт
  `arch/arm32/mach-t113s3/mctl_hal.c` до переработки, который upstream *удалил* в коммите
  `381606b0d68f` ("import Andre Przywara's DRAM init rework code", 2023-01-17), когда добавил
  `dram_sun20i_d1.c` [5][6][7]. Характерные имена `auto_cal_timing`, `auto_set_timing_para`,
  `dram_vol_set`, `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test` существуют
  только в `mctl_hal.c`, но не в `dram.c` [6][1].
- Поэтому интересные расхождения — это изменения upstream, *сделанные после точки ответвления*:
  переработка Przywara от 2023-01-17, переписывание `auto_scan_dram_size` от 2023-03-13 и
  рефакторинг от 2025-11-15.

### Состояние после тикетов parity

- **`auto_scan_dram_size`** теперь представляет собой поразрядный сканер из `9c8b8e40`:
  отдельный `rank1_addr`, шаблон теста для каждого ранга, пробы строки/банка/столбца и проверка
  числа рангов через `dram_para2`. Исправлено в issue #18.
- **Принудительный `vol = 25`** удалён из `dram_voltage_set`; используется значение для
  конкретного типа (DDR2 → 47, DDR3 → 25, по умолчанию → 0). Исправлено в issue #18.
- **Условие записи MR** по старшим 16 битам удалено; вычисленные `mr0..mr3` записываются
  напрямую, как делает upstream. Исправлено в issue #18.
- **VTFCR** читается из VTFCR перед OR с `0x300`; прежнее чтение ZQCR убрано. Исправлено в
  issue #15.
- **Имена функций и констант** следуют upstream snake_case (`dram_voltage_set`,
  `dram_enable_all_master`, `dramc_get_dram_size`, `dramc_simple_wr_test`, `ns_to_t`,
  `mctl_set_timing_params`, `ccu_set_pll_ddr_clk`, `MCTL_PHY_BASE`, `MCTL_COM_BASE`). Тип
  `DramPara` остаётся идиоматичным для Rust, а получатель — `&mut self`, чтобы результаты
  сканирования и `dram_clk` распространялись. Исправлено в issue #18.
- **`dram_get_top()` / `dram_top`** намеренно отсутствуют: контракту payload не нужен верхний
  адрес, и родительский issue откладывает API до тех пор, пока он не понадобится [14].
- **Исключение по chip-ID для T113-S4** в `mctl_phy_ac_remapping` намеренно не портировано:
  цель — только T113-S3 [14].

### Вердикт

Модуль DRAM теперь соответствует закреплённому upstream по всем входящим в область
поведенческим расхождениям: автосканирование, выбор напряжения, записи регистров режима, VTFCR
и терминология. Оставшиеся различия намеренны: нет `dram_get_top`, нет пути для T113-S4, и
сохраняется более старая (численно эквивалентная для DDR3) структура процедуры таймингов с её
мёртвой реконструкцией `dram_tpr0/1/2`.

---

## 2. Ревизия upstream и история

### 2.1 Закреплённая ревизия

| Пункт | Значение |
|---|---|
| Репозиторий upstream | `szemzoa/awboot` |
| Ветка | `main` |
| Закреплённый SHA | `5380c00fc67c975433f25c57fb481aa2b91aebf8` |
| Дата / тема HEAD | 2025-11-21 / "Merge pull request #41 from cornera-org/psci-rebase" [12] |
| `dram.c` | 1453 строки, 40 623 Б [1] |
| `dram.h` | 78 строк, 2 276 Б [2] |
| `reg-dram.h` | 122 строки, 4 053 Б [3] |
| Последний коммит `dram.c` | `385481ffab45089d90410cb67b93981a870edd7c`, 2025-11-15 [4][9] |

Заголовок текущего файла upstream явно говорит, что это "a reworked version, to match the
U-Boot coding style", с основой "lifted from awboot ... de-compilation of some original Allwinner
code bits" [1:1-15].

### 2.2 История исходника DRAM

История GitHub по пути даёт шесть коммитов для `arch/arm32/mach-t113s3/dram.c` [4] и четыре для
его непосредственного предшественника `arch/arm32/mach-t113s3/dram_sun20i_d1.c` [5]. С учётом
удаления `mctl_hal.c` релевантная последовательность такова:

| Коммит | Дата | Тема | Что изменилось |
|---|---|---|---|
| `23e5c393` | 2023-01-17 (родитель) | — | Последнее дерево upstream, содержащее `mctl_hal.c` (39 265 Б) [6][7] |
| `381606b0` | 2023-01-17 | import Andre Przywara's DRAM init rework code | **Удаляет `mctl_hal.c`**, добавляет `dram_sun20i_d1.c` (1392 строки) [7] |
| `63a45cbf` | 2023-02-16 | add DRAM controller register header and fix printf issue | Изменено 463/451 строк; добавляет `reg-dram.h` [5] |
| `9deb6cee` | 2023-02-17 | add new ac remap and SUNXI_R_CPUCFG_BASE define | Переформатирует таблицы AC в hex (те же значения), добавляет define'ы регистров [5] |
| `7ade271d` | 2023-03-01 | add v851s files ... | Переименовывает `dram_sun20i_d1.c` → `dram.c` [4] |
| `9c8b8e40` | 2023-03-13 | add t113-s4 support 256M SiP DRAM, no ac remap. **update t113-s3 auto_scan_dram_size func** | Переписывает `auto_scan_dram_size`; добавляет ветки chip-ID/fuse в AC remap [8] |
| `e6c37db4`, `ed1be708` | 2023-03-13 | "replace with tab", "fix tab" | Только пробельные символы [4] |
| `78a943aa` | 2023-12-09 | printf format checking, u32/uint32_t, warnings | В основном `%d` → `%" PRIu32 "` и написание типов [4] |
| `385481ff` | 2025-11-15 | refactor sdmmc driver, create library for fatfs, fdt, loaders | Добавляет `dram_top`/`dram_get_top`; крупная стилистическая переработка (213/204 строк) [9] |

### 2.3 Какой ревизии соответствует наш порт?

Исторически — ни одной из ревизий `dram.c`. Он соответствует `mctl_hal.c` в последнем дереве,
где тот присутствовал (`23e5c393`, родитель `381606b0`) [6][7]. Доказательства:

- Инвентарь функций: `auto_cal_timing`, `auto_set_timing_para`, `dram_vol_set`,
  `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test` и тело
  `auto_scan_dram_size` из `mctl_hal.c` — все присутствуют в `mctl_hal.c` и отсутствуют в любой
  ревизии `dram.c` до #18 [6][1].
- Старое переопределение `vol = 25` и условие записи MR существовали только в `mctl_hal.c`, но
  не в `dram.c` [6][1:84-105,480-484].
- Запись VTFCR читала регистр `0x140` (ZQCR) точно так же, как это делал `mctl_hal.c`; `dram.c`
  читает VTFCR [6:1597-1600][1:1397-1398]. Исправлено в #15.
- Процедура таймингов на Rust сохраняет старые комментарии о слотах SP (`s4`, `s8`, `80(sp)`,
  `s5`) и мёртвую реконструкцию `dram_tpr0/1/2`, которые обе есть в `mctl_hal.c` и были
  удалены при переработке [6:183-355][1:190-505]. Сохранены намеренно в #18; см. §4.5.

Точную ревизию `mctl_hal.c`, использованную портом, невозможно доказать по локальному дереву,
поскольку код на Rust не несёт комментария о происхождении; `23e5c393` — **ближайший
сохранившийся предок upstream**. Точное соответствие **НЕ ПРОВЕРЕНО**.

---

## 3. Таблица сопоставления функций

Имена upstream взяты из `dram.c` на `5380c00` [1]; локальные имена — из
`rawboot/src/dram/mod.rs`.

| Upstream C (`dram.c` @5380c00) | Локальный Rust | Статус |
|---|---|---|
| `sunxi_dram_init` [1:1420] | `DramPara::default` + `DramPara::init_dram` | объединён; без побочного эффекта `dram_top` |
| `init_DRAM` [1:1306] | `DramPara::init_dram` | присутствует |
| `dram_get_top` [1:117] | — | **намеренно отсутствует**, пока payload не понадобится [14] |
| `dram_disable_all_master` [1:122] | `dram_disable_all_master` | присутствует |
| `dram_enable_all_master` [1:107] | `dram_enable_all_master` | присутствует, переименовано в #18 |
| `dram_voltage_set` [1:84] | `dram_voltage_set` | присутствует, без принудительного 25 (#18) |
| `sid_read_ldoB_cal` [1:60] | `sid_read_ldo_b_cal` | присутствует |
| `mctl_sys_init` [1:555] | `mctl_sys_init` | присутствует |
| `ccu_set_pll_ddr_clk` [1:512] | `ccu_set_pll_ddr_clk` | присутствует, переименовано в #18 |
| `mctl_com_init` [1:595] | `mctl_com_init` | присутствует |
| `mctl_core_init` [1:1070] | `mctl_core_init` | присутствует, тот же порядок вызовов |
| `mctl_channel_init` [1:773] | `mctl_channel_init` | присутствует |
| `mctl_vrefzq_init` [1:1053] | `mctl_vrefzq_init` | присутствует |
| `mctl_phy_ac_remapping` [1:697] | `mctl_phy_ac_remapping` | присутствует; ветка T113-S4 намеренно опущена |
| `mctl_set_timing_params` [1:190] | `mctl_set_timing_params` | присутствует, более старая структура; записи MR выровнены в #18 |
| `ns_to_t` [1:47] | `ns_to_t` | выровнено в #18 |
| `calculate_rank_size` [1:932] | встроено в `dramc_get_dram_size` | присутствует, встроено |
| `DRAMC_get_dram_size` [1:949] | `dramc_get_dram_size` | присутствует, переименовано в #18 |
| `auto_scan_dram_config` [1:1288] | `auto_scan_dram_config` | присутствует |
| `auto_scan_dram_rank_width` [1:1257] | `auto_scan_dram_rank_width` | присутствует |
| `auto_scan_dram_size` [1:1096] | `auto_scan_dram_size` | присутствует, **поразрядный алгоритм с #18** |
| `dqs_gate_detect` [1:975] | `dqs_gate_detect` | присутствует |
| `eye_delay_compensation` [1:130] | `eye_delay_compensation` | присутствует |
| `dramc_simple_wr_test` [1:1016] | `dramc_simple_wr_test` | присутствует, переименовано в #18 |
| `abort` [1:54] | — | **ОТСУТСТВУЕТ** (код на Rust никогда его не вызывает) |

`ns_to_t` вычисляет `DIV_ROUND_UP((dram_clk/2) * ns, 1000)` [1:47-52]; помощник на Rust
вычисляет то же деление с округлением вверх от `dram_clk / 2`.

---

## 4. Алгоритмические расхождения

### 4.1 `auto_scan_dram_size`: выровнен на поразрядный сканер (#18)

**Старый порт** был алгоритмом из `mctl_hal.c` [6:1287-1452]:

- Число рангов из `(dram_para2 & 0xf000) != 0`.
- Тестовый шаблон записывается один раз по `SDRAM_BASE`.
- Сканирование строк по `i = 11..17`, фиксированная база `SDRAM_BASE + (1 << (i + 11))`.
- Проба банка по A22, сохраняющая флаг 0/1.
- Сканирование столбцов по `i = 9..14` от `SDRAM_BASE`.
- Ранг 1 программируется по `0x3202000`/`0x3202004`.

**Текущий локальный** (`auto_scan_dram_size`) воспроизводит upstream HEAD [1:1096-1249],
который сам является переписыванием `9c8b8e40` [8]:

- `rank_count = 2` только когда `((dram_para2 >> 12) & 0xf) == 0x1`.
- Цикл по рангам с `mc_work_mode = MCTL_COM_BASE + MCTL_COM_WORK_MODE0 + 4 * current_rank`.
- Для ранга 1 перепрограммировать `MCTL_COM_WORK_MODE0` и `MCTL_COM_WORK_MODE1` под
  страницу 16 строк/4 банка/512 Б и задать `rank1_addr = SDRAM_BASE + (1 << 27)` (128 МиБ).
- Записывать тестовый шаблон для каждого ранга; сравнивать ранг 1 по `rank1_addr`.
- Сканирование строк `i = 11..17` с `ret = SDRAM_BASE + (1 << (i + 2 + 9))` и чтениями по
  `ret + j*4`.
- Проба банка — единственная итерация по A11; индекс цикла (0/1) сохраняется в `dram_para1`.
- Сканирование столбцов `i = 9..14` и кодирование размера страницы, как в upstream.
- Сравнение двух половин `dram_para1` для двух рангов, устанавливающее бит 8 `dram_para2`,
  если они различаются.

Транскрипция на Rust опускает аккумулятор `addr_line`: upstream его вычисляет, но никогда не
читает. Блок режима страницы для ранга 1 воспроизведён дословно, хотя оба вызова upstream
нацелены на `mc_work_mode`; это текущее поведение upstream, и молчаливое его изменение было бы
новым расхождением.

**Изменение получателя.** `DramPara` больше не выводит `Copy`, а мутирующие методы принимают
`&mut self`. Без этого старые методы с `mut self` работали с копией, поэтому каждый результат
сканирования (и `dram_clk`, устанавливаемый `mctl_sys_init`) отбрасывался при возврате из
метода. Удаление `Copy` — часть введения нового сканера в действие, а не посторонний
рефакторинг.

**Вердикт: исправлено в #18.** Подтверждение на оборудовании отслеживается в §4.7.

### 4.2 AC remapping: исключение по chip-ID T113-S4 намеренно опущено

Локальный `mctl_phy_ac_remapping` соответствует `mctl_hal.c`:

- Читает fuse `(SID+0x28 & 0xf00) >> 8` и неиспользуемый `mark_id`.
- Если биты 18-19 `dram_tpr13` установлены → `cfg7`; иначе fuse 8→`cfg2`, 9→`cfg3`, 10→`cfg5`,
  11→`cfg4`, 12→`cfg1`, 13/14→нули.
- DDR2: fuse 15 возвращает; иначе `cfg6`.
- Собирает четыре 32-битных слова `REMAP0..3` из 22-байтовой таблицы и устанавливает бит 0
  REMAP0.

Upstream HEAD дополнительно читает `chipid` и для DDR3 fuse 10 выбирает "без remap", когда
`chipid == 0x6800` (T113-S4) [1:697-755]. На T113-S3 chip ID не равен `0x6800`, поэтому ветка
не действует; родительский issue выводит SoC, отличные от T113-S3, за область [14].
**Намеренное различие.**

### 4.3 Обработка напряжения / LDO (#18)

Старый `vol_set` вычислял значение для конкретного типа, а затем безусловно перезаписывал его
на `vol = 25`. `dram_voltage_set` теперь использует значение для конкретного типа напрямую:

| `dram_type` | значение |
|---|---|
| DDR2 (2) | 47 |
| DDR3 (3) | 25 |
| прочее | 0 |

Маска LDO (`0x20ff00` очищается, `vol << 8`) соответствует upstream, и за записью следует
`udelay(1)` перед `sid_read_ldo_b_cal`, как делает upstream [1:84-105].
`sid_read_ldo_b_cal` читает биты 8-15 SID `0x300621c` и применяет ту же поправку для DDR3
(`reg > 0x20` → `reg -= 0x16`); его вложенный `if` поведенчески эквивалентен раннему возврату в
upstream.

**Вердикт: исправлено в #18.** Старое переопределение могло изменить только DDR2 (47 → 25);
T113-S3 — это DDR3.

### 4.4 Запись VTFCR читает не тот регистр (исправлено в #15)

Старый `init` делал:

```rust
let addr  = MCTL_PHY_BASE + ZQCR;  // 0x140
let addr1 = MCTL_PHY_BASE + VTFCR; // 0xb8
write32(addr, read32(addr) | (1 << 31));
if (self.dram_tpr13 & (1 << 8)) != 0 {
    write32(addr1, read32(addr) | 0x300);
}
```

Это в точности `mctl_hal.c:1597-1600`. Upstream читает сам VTFCR:
`writel(readl(VTFCR) | 0x300, VTFCR)` [1:1397-1398]. Ветка активна при поставляемом значении по
умолчанию `dram_tpr13 = 0x34000100` (бит 8 установлен), поэтому старый код записывал
`read32(ZQCR) | 0x300` в VTFCR при каждой загрузке. Issue #15 изменил второе чтение на VTFCR;
теперь `init_dram` делает `write32(addr1, read32(addr1) | 0x300)`.

**Статус: исправлено в #15.** Подтверждение на оборудовании было частью прогона HIL для #15.

### 4.5 Параметры таймингов и записи регистров режима (#18)

`mctl_set_timing_params` по-прежнему остаётся старой процедурой из `mctl_hal.c` [6:183-617], а
не `mctl_set_timing_params` из upstream [1:190-505]. Различия по форме:

- Она сохраняет комментарии о слотах SP, полученные из ассемблера, и реконструирует
  `dram_tpr0/1/2` из вычисленных значений; upstream больше не пишет эти поля, и позже они не
  читаются. Реконструкция мёртвая, но безвредная; она остаётся до отдельной очистки.
- Она вычисляет тайминг через `ns_to_t(nanoseconds)`, используя `dram_clk / 2`, что
  эквивалентно делению с округлением вверх в upstream [1:47-52].
- Для DDR3 (значение по умолчанию для T113-S3) получающиеся значения численно эквивалентны на
  792 МГц: `tfaw = ns(50)`, `trcd = ns(15)`, `trp = ns(15)`, `trc = ns(53)`, `tras = ns(38)`,
  `twtr = ns(8)+2`, `trrd = max(ns(10),2)`, `txp = max(ns(10),2)`, `tcl = 6`, `tcwl = 4`,
  `t_rdata_en = 4`, `mr0 = 0x1c70`, `mr2 = 0x18`, `trd2wr = 5`
  [1:252-345].
- **Записи регистров режима выровнены (#18):** старый код перезаписывал `dram_mr0/1/2/3` только
  когда старшие 16 бит соответствующего поля были нулевыми, а затем записывал поля структуры.
  Upstream записывает локально вычисленные `mr0..mr3` напрямую [1:480-484].
  `mctl_set_timing_params` теперь записывает `mr0..mr3` напрямую; `mr1`/`mr3` по-прежнему
  начинаются с `dram_mr1`/`dram_mr3`, как в upstream.

`mctl_vrefzq_init`, `mctl_com_init`, `mctl_sys_init`, `ccu_set_pll_ddr_clk`, `mctl_channel_init`,
`eye_delay_compensation` и `dqs_gate_detect` были выборочно сверены с переработкой upstream и
эквивалентны по регистрам (те же маски, те же адреса, тот же порядок). Исчерпывающее
доказательство эквивалентности `mctl_channel_init` на уровне инструкций не проводилось.

### 4.6 Отчёт о размере и `dram_top`

`dramc_get_dram_size` — это `DRAMC_get_dram_size` из upstream со встроенным `calculate_rank_size`;
битовая арифметика идентична [1:932-969]. Не хватает побочного эффекта `dram_top` из upstream:
`sunxi_dram_init` вычисляет `size = init_DRAM(...) * 1 MiB` и сохраняет
`dram_top = SDRAM_BASE + size`, предоставляя его через `dram_get_top()` [1:115-120,1449-1452].

**Намеренно отсутствует (#18).** Родительский issue говорит добавлять `dram_get_top` только
когда контракту payload действительно нужен верхний адрес [14]; сегодня это не так.

### 4.7 Подтверждение на оборудовании

`scripts/hil-check.sh` на T113-S3 загружает `target/rawboot-fel.bin` через FEL и проверяет
журнал загрузки. После выключения и включения в FEL образ из #18 выдал `hello=1`,
`PASS: UART assertions` и такую последовательность:

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

`DRAM simple test OK` и следующая строка SD означают, что новый сканер завершился и loader
продолжил выполнение; мигание LED PD22 (проверка человеком) также было подтверждено. Чистый
прогон `hil-check.sh` с одной загрузкой требует предварительного выключения и включения в FEL:
более ранний `xfel reset` оставляет watchdog включённым, и цикл сброса срабатывает на защиту
скрипта от множественных `Hello` (`docs/agents/hil.md`).

---

## 5. Оставшиеся различия

Упорядочены по вероятному влиянию на T113-S3:

1. **`dram_get_top()` / `dram_top`** — отсутствуют по замыслу, пока payload не понадобится [14].
2. **Исключение AC-remap для T113-S4** — не портировано; не действует на T113-S3, а SoC, отличные
   от T113-S3, вне области.
3. **Структура процедуры таймингов** — старая форма из `mctl_hal.c` с мёртвой реконструкцией
   `dram_tpr0/1/2`; численно эквивалентна для DDR3 по умолчанию. Условие записи MR исправлено в
   #18.
4. **`abort()`** — не портировано; код на Rust никогда его не вызывает.
5. **Доступ к регистрам** — многие сырые смещения остаются там, где upstream использует макросы
   `MCTL_PHY_*`; базовые константы и имена, затронутые в #18, выровнены.

Исправлено и больше не является различиями: поразрядный `auto_scan_dram_size` (#18),
принудительный `vol = 25` (#18), условие записи MR (#18), исходный регистр VTFCR (#15) и имена
функций/переменных в области (#18).

Не отсутствует: подъём ZQ, последовательность тактирования PLL/MBUS/DRAM, таблицы AC remap для
PHY (`cfg1..cfg7` соответствуют индексам upstream 1..7), обнаружение DQS gate, инициализация
Vref/ZQ, инициализация канала, компенсация eye-delay, простой тест чтения/записи и значения по
умолчанию `dram_para_t` (792 МГц, тип 3, ZQ `0x7b7bfb`, `tpr11 = 0x00340000`, `tpr12 = 0x46`,
`tpr13 = 0x34000100`) [1:1420-1447].

---

## 6. Открытые вопросы

1. **Поведение двухрангового T113-S3.** Поразрядный сканер теперь на месте, но двухрангового
   модуля для аппаратного A/B-теста нет. Сообщение `9c8b8e40` в upstream называет это
   обновлением, а не исправлением ошибки, и в коде есть подозрительные места (цикл по банкам с
   одной итерацией; шаблон для ранга 1 по-прежнему пишется по `SDRAM_BASE`, тогда как сравнения
   используют `rank1_addr`) [1:1096-1249][8]. **НЕ ПРОВЕРЕНО на двухранговом оборудовании**
   (плата для HIL — 128 МиБ, один ранг).
2. **Интеграция `dram_get_top`.** Нужен ли когда-либо контракту `BootInfo` верхний адрес DRAM,
   или достаточен возвращаемый размер в МиБ? Отложено родительским issue [14].
3. **Исчерпывающая эквивалентность `mctl_channel_init`.** Выборочные проверки показывают те же
   регистры/маски, но полное построчное доказательство не проводилось, поскольку в Rust
   смешаны литеральные и вычисляемые адреса. **НЕ ПРОВЕРЕНО** для каждой записи.
4. **Лицензирование.** `dram.c` в upstream — GPL-2.0+ [1:1]; `mctl_hal.c` не имел явного
   заголовка SPDX в загруженном файле [6]. Проект воспроизводит факты и поведение под
   CC-BY-4.0 (ADR-0001); если какой-либо текст `mctl_hal.c` был скопирован дословно, лицензионную
   позицию следует перепроверить перед распространением.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/dram.c` на `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.c
2. Upstream `arch/arm32/mach-t113s3/dram.h` на том же SHA —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.h
3. Upstream `arch/arm32/mach-t113s3/reg-dram.h` на том же SHA —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/reg-dram.h
4. GitHub API, история коммитов для `arch/arm32/mach-t113s3/dram.c` —
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram.c&per_page=100
5. GitHub API, история коммитов для `arch/arm32/mach-t113s3/dram_sun20i_d1.c` —
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram_sun20i_d1.c&per_page=100
6. Upstream `arch/arm32/mach-t113s3/mctl_hal.c` на `23e5c39396588c831e45c77415aed89f12a66a54`
   (последнее дерево перед переработкой) —
   https://raw.githubusercontent.com/szemzoa/awboot/23e5c39396588c831e45c77415aed89f12a66a54/arch/arm32/mach-t113s3/mctl_hal.c
7. GitHub API, коммит `381606b0d68f` ("import Andre Przywara's DRAM init rework code", удаляет
   `mctl_hal.c`, добавляет `dram_sun20i_d1.c`) —
   https://api.github.com/repos/szemzoa/awboot/commits/381606b0d68f
8. Upstream `arch/arm32/mach-t113s3/dram.c` на `9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22`
   ("add t113-s4 support ... update t113-s3 auto_scan_dram_size func") —
   https://raw.githubusercontent.com/szemzoa/awboot/9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22/arch/arm32/mach-t113s3/dram.c
9. GitHub API, коммит `385481ffab45089d90410cb67b93981a870edd7c` (рефакторинг 2025-11-15;
   добавляет `dram_top`/`dram_get_top`) —
   https://api.github.com/repos/szemzoa/awboot/commits/385481ffab45089d90410cb67b93981a870edd7c
10. Локальный `rawboot/src/dram/mod.rs` — <workspace>/rust/awboot/rawboot/src/dram/mod.rs
11. Локальный `embed/app/src/dram/mod.rs` (встроенный эталон до порта) —
    <workspace>/rust/awboot/embed/app/src/dram/mod.rs
12. GitHub API, `main` HEAD на момент исследования (разрешается в
    `5380c00fc67c975433f25c57fb481aa2b91aebf8`) —
    https://api.github.com/repos/szemzoa/awboot/commits/main
13. Предыдущая исследовательская заметка (эталон стиля) — <workspace>/rust/awboot/research/awboot-rust-migration.md
14. Issue #18 "Parity DRAM: dram" и родительский issue #14 "Выравнивание лоадера rawboot на upstream
    awboot: parity поведения".
