# `rawboot` DRAM port vs current upstream `szemzoa/awboot` `dram.c`

Researched: 2026, status updated after issue #18. Target: Allwinner T113-S3 (DDR3, 792 MHz
default). Upstream claims are cited to the exact primary source (pinned SHA plus line numbers).
Local code is cited by symbol, per `docs/agents/domain.md`; the local file changes, so line
numbers would rot.

Reference numbers in square brackets map to the **Sources** section at the end.

---

## 1. Executive summary

- Upstream is pinned to `szemzoa/awboot` `main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`
  (2025-11-21, "Merge pull request #41 ... psci-rebase") [12]. The latest commit touching
  `arch/arm32/mach-t113s3/dram.c` is `385481ffab45089d90410cb67b93981a870edd7c`
  (2025-11-15) [4][9]. At that SHA `dram.c` is 1453 lines / 40,623 B [1].
- Our Rust port is not derived from any revision of `dram.c`. It is a near line-for-line port of
  the pre-rework `arch/arm32/mach-t113s3/mctl_hal.c`, which upstream *deleted* in commit
  `381606b0d68f` ("import Andre Przywara's DRAM init rework code", 2023-01-17) when it added
  `dram_sun20i_d1.c` [5][6][7]. The tell-tale names `auto_cal_timing`, `auto_set_timing_para`,
  `dram_vol_set`, `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test` exist only in
  `mctl_hal.c`, not in `dram.c` [6][1].
- The interesting divergences are therefore upstream *changes made after the fork point*: the
  2023-01-17 Przywara rework, the 2023-03-13 `auto_scan_dram_size` rewrite, and the 2025-11-15
  refactor.

### Status after the parity tickets

- **`auto_scan_dram_size`** is now the per-rank scanner from `9c8b8e40`: separate `rank1_addr`,
  per-rank test pattern, row/bank/column probes, and the `dram_para2` rank-count test. Fixed in
  issue #18.
- **Forced `vol = 25`** is removed from `dram_voltage_set`; the per-type value is used
  (DDR2 → 47, DDR3 → 25, default → 0). Fixed in issue #18.
- **MR-write gating** on the upper 16 bits is removed; the computed `mr0..mr3` are written
  directly, as upstream does. Fixed in issue #18.
- **VTFCR** is read from VTFCR before OR-ing `0x300`; the old ZQCR read is gone. Fixed in
  issue #15.
- **Function and constant names** follow upstream snake_case (`dram_voltage_set`,
  `dram_enable_all_master`, `dramc_get_dram_size`, `dramc_simple_wr_test`, `ns_to_t`,
  `mctl_set_timing_params`, `ccu_set_pll_ddr_clk`, `MCTL_PHY_BASE`, `MCTL_COM_BASE`). The
  `DramPara` type stays Rust-idiomatic, and the receiver is `&mut self` so scan results and
  `dram_clk` propagate. Fixed in issue #18.
- **`dram_get_top()` / `dram_top`** remain deliberately absent: the payload contract does not
  need a top address, and the parent issue defers the API until it does [14].
- **T113-S4 chip-ID exception** in `mctl_phy_ac_remapping` is deliberately not ported: the
  target is T113-S3 only [14].

### Verdict

The DRAM module now matches the pinned upstream for every in-scope behavioural delta: auto-scan,
voltage selection, mode-register writes, VTFCR, and vocabulary. The remaining differences are
deliberate: no `dram_get_top`, no T113-S4 path, and the older (numerically equivalent for DDR3)
timing-routine structure with its dead `dram_tpr0/1/2` reconstruction.

---

## 2. Upstream revision & history

### 2.1 Pinned revision

| Item | Value |
|---|---|
| Upstream repo | `szemzoa/awboot` |
| Branch | `main` |
| Pinned SHA | `5380c00fc67c975433f25c57fb481aa2b91aebf8` |
| HEAD date / subject | 2025-11-21 / "Merge pull request #41 from cornera-org/psci-rebase" [12] |
| `dram.c` | 1453 lines, 40,623 B [1] |
| `dram.h` | 78 lines, 2,276 B [2] |
| `reg-dram.h` | 122 lines, 4,053 B [3] |
| Latest `dram.c` commit | `385481ffab45089d90410cb67b93981a870edd7c`, 2025-11-15 [4][9] |

The current upstream file header explicitly says it is "a reworked version, to match the U-Boot
coding style", with the baseline "lifted from awboot ... de-compilation of some original Allwinner
code bits" [1:1-15].

### 2.2 History of the DRAM source

GitHub's per-path history gives six commits for `arch/arm32/mach-t113s3/dram.c` [4], and four for
its immediate predecessor `arch/arm32/mach-t113s3/dram_sun20i_d1.c` [5]. Including the removal of
`mctl_hal.c`, the relevant sequence is:

| Commit | Date | Subject | What changed |
|---|---|---|---|
| `23e5c393` | 2023-01-17 (parent) | — | Last upstream tree containing `mctl_hal.c` (39265 B) [6][7] |
| `381606b0` | 2023-01-17 | import Andre Przywara's DRAM init rework code | **Deletes `mctl_hal.c`**, adds `dram_sun20i_d1.c` (1392 lines) [7] |
| `63a45cbf` | 2023-02-16 | add DRAM controller register header and fix printf issue | 463/451 lines changed; adds `reg-dram.h` [5] |
| `9deb6cee` | 2023-02-17 | add new ac remap and SUNXI_R_CPUCFG_BASE define | Re-formats AC tables to hex (same values), adds register defines [5] |
| `7ade271d` | 2023-03-01 | add v851s files ... | Renames `dram_sun20i_d1.c` → `dram.c` [4] |
| `9c8b8e40` | 2023-03-13 | add t113-s4 support 256M SiP DRAM, no ac remap. **update t113-s3 auto_scan_dram_size func** | Rewrites `auto_scan_dram_size`; adds chip-ID/fuse branches to AC remap [8] |
| `e6c37db4`, `ed1be708` | 2023-03-13 | "replace with tab", "fix tab" | Whitespace only [4] |
| `78a943aa` | 2023-12-09 | printf format checking, u32/uint32_t, warnings | Mostly `%d` → `%" PRIu32 "` and type spelling [4] |
| `385481ff` | 2025-11-15 | refactor sdmmc driver, create library for fatfs, fdt, loaders | Adds `dram_top`/`dram_get_top`; large style rework (213/204 lines) [9] |

### 2.3 Which revision does our port match?

Historically, none of the `dram.c` revisions. It matches `mctl_hal.c` at the last tree containing
it (`23e5c393`, parent of `381606b0`) [6][7]. Evidence:

- Function inventory: `auto_cal_timing`, `auto_set_timing_para`, `dram_vol_set`,
  `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test`, and `mctl_hal.c`'s
  `auto_scan_dram_size` body — all present in `mctl_hal.c` and absent from every `dram.c`
  revision before #18 [6][1].
- The old `vol = 25` override and the MR-write gating existed only in `mctl_hal.c`, not in
  `dram.c` [6][1:84-105,480-484].
- The VTFCR write read register `0x140` (ZQCR) exactly as `mctl_hal.c` did; `dram.c` reads
  VTFCR [6:1597-1600][1:1397-1398]. Fixed in #15.
- The Rust timing routine keeps the old SP-slot comments (`s4`, `s8`, `80(sp)`, `s5`) and the
  dead `dram_tpr0/1/2` reconstruction, both of which exist in `mctl_hal.c` and were removed by
  the rework [6:183-355][1:190-505]. Retained deliberately in #18; see §4.5.

The exact `mctl_hal.c` revision used by the port cannot be proven from the local tree because the
Rust code carries no provenance comment; `23e5c393` is the **closest surviving upstream ancestor**.
Exact match is **UNVERIFIED**.

---

## 3. Function mapping table

Upstream names are from `dram.c` at `5380c00` [1]; local names are from `rawboot/src/dram/mod.rs`.

| Upstream C (`dram.c` @5380c00) | Local Rust | Status |
|---|---|---|
| `sunxi_dram_init` [1:1420] | `DramPara::default` + `DramPara::init_dram` | merged; no `dram_top` side effect |
| `init_DRAM` [1:1306] | `DramPara::init_dram` | present |
| `dram_get_top` [1:117] | — | **deliberately absent** until the payload needs it [14] |
| `dram_disable_all_master` [1:122] | `dram_disable_all_master` | present |
| `dram_enable_all_master` [1:107] | `dram_enable_all_master` | present, renamed in #18 |
| `dram_voltage_set` [1:84] | `dram_voltage_set` | present, no forced 25 (#18) |
| `sid_read_ldoB_cal` [1:60] | `sid_read_ldo_b_cal` | present |
| `mctl_sys_init` [1:555] | `mctl_sys_init` | present |
| `ccu_set_pll_ddr_clk` [1:512] | `ccu_set_pll_ddr_clk` | present, renamed in #18 |
| `mctl_com_init` [1:595] | `mctl_com_init` | present |
| `mctl_core_init` [1:1070] | `mctl_core_init` | present, same call order |
| `mctl_channel_init` [1:773] | `mctl_channel_init` | present |
| `mctl_vrefzq_init` [1:1053] | `mctl_vrefzq_init` | present |
| `mctl_phy_ac_remapping` [1:697] | `mctl_phy_ac_remapping` | present; T113-S4 branch intentionally omitted |
| `mctl_set_timing_params` [1:190] | `mctl_set_timing_params` | present, older structure; MR writes aligned in #18 |
| `ns_to_t` [1:47] | `ns_to_t` | aligned in #18 |
| `calculate_rank_size` [1:932] | inlined in `dramc_get_dram_size` | present, inlined |
| `DRAMC_get_dram_size` [1:949] | `dramc_get_dram_size` | present, renamed in #18 |
| `auto_scan_dram_config` [1:1288] | `auto_scan_dram_config` | present |
| `auto_scan_dram_rank_width` [1:1257] | `auto_scan_dram_rank_width` | present |
| `auto_scan_dram_size` [1:1096] | `auto_scan_dram_size` | present, **per-rank algorithm since #18** |
| `dqs_gate_detect` [1:975] | `dqs_gate_detect` | present |
| `eye_delay_compensation` [1:130] | `eye_delay_compensation` | present |
| `dramc_simple_wr_test` [1:1016] | `dramc_simple_wr_test` | present, renamed in #18 |
| `abort` [1:54] | — | **MISSING** (the Rust code never calls it) |

`ns_to_t` computes `DIV_ROUND_UP((dram_clk/2) * ns, 1000)` [1:47-52]; the Rust helper computes the
same ceiling division from `dram_clk / 2`.

---

## 4. Algorithmic divergences

### 4.1 `auto_scan_dram_size`: aligned to the per-rank scanner (#18)

**Old port** was the `mctl_hal.c` algorithm [6:1287-1452]:

- Rank count from `(dram_para2 & 0xf000) != 0`.
- Test pattern written once at `SDRAM_BASE`.
- Row scan over `i = 11..17`, fixed base `SDRAM_BASE + (1 << (i + 11))`.
- A22 bank probe storing a 0/1 flag.
- Column scan over `i = 9..14` from `SDRAM_BASE`.
- Rank 1 programmed at `0x3202000`/`0x3202004`.

**Current local** (`auto_scan_dram_size`) transcribes upstream HEAD [1:1096-1249], itself the
`9c8b8e40` rewrite [8]:

- `rank_count = 2` only when `((dram_para2 >> 12) & 0xf) == 0x1`.
- A per-rank loop with `mc_work_mode = MCTL_COM_BASE + MCTL_COM_WORK_MODE0 + 4 * current_rank`.
- For rank 1, reprogram `MCTL_COM_WORK_MODE0` and `MCTL_COM_WORK_MODE1` to 16-row/4-bank/512 B
  page and set `rank1_addr = SDRAM_BASE + (1 << 27)` (128 MiB).
- Write the test pattern per rank; compare rank 1 at `rank1_addr`.
- Row scan `i = 11..17` with `ret = SDRAM_BASE + (1 << (i + 2 + 9))` and reads at `ret + j*4`.
- Bank probe is a single A11 iteration; the loop index (0/1) is stored in `dram_para1`.
- Column scan `i = 9..14` and page-size encoding as upstream.
- Dual-rank comparison of the two `dram_para1` halves, setting `dram_para2` bit 8 when different.

The Rust transcription keeps the `addr_line` accumulator out: upstream computes it but never
reads it. The rank-1 page-mode block is transcribed verbatim even though both upstream calls
target `mc_work_mode`; that is upstream's current behaviour and changing it silently would be a
new divergence.

**Receiver change.** `DramPara` no longer derives `Copy`, and the mutating methods take
`&mut self`. Without that, the old `mut self` methods operated on a copy, so every scan result
(and `dram_clk` set by `mctl_sys_init`) was discarded when the method returned. Removing `Copy`
is part of making the new scanner effective, not an unrelated refactor.

**Verdict: fixed in #18.** Hardware confirmation is tracked in §4.7.

### 4.2 AC remapping: T113-S4 chip-ID exception intentionally omitted

Local `mctl_phy_ac_remapping` matches `mctl_hal.c`:

- Reads fuse `(SID+0x28 & 0xf00) >> 8` and an unused `mark_id`.
- If `dram_tpr13` bits 18-19 are set → `cfg7`; otherwise fuse 8→`cfg2`, 9→`cfg3`, 10→`cfg5`,
  11→`cfg4`, 12→`cfg1`, 13/14→zeros.
- DDR2: fuse 15 returns; otherwise `cfg6`.
- Builds four 32-bit `REMAP0..3` words from the 22-byte table and sets bit 0 of REMAP0.

Upstream HEAD additionally reads `chipid` and, for DDR3 fuse 10, selects "no remap" when
`chipid == 0x6800` (T113-S4) [1:697-755]. On T113-S3 the chip ID is not `0x6800`, so the branch
is inert; the parent issue puts non-T113-S3 SoCs out of scope [14]. **Deliberate difference.**

### 4.3 Voltage / LDO handling (#18)

The old `vol_set` computed the per-type value and then unconditionally overwrote it with
`vol = 25`. `dram_voltage_set` now uses the per-type value directly:

| `dram_type` | value |
|---|---|
| DDR2 (2) | 47 |
| DDR3 (3) | 25 |
| other | 0 |

The LDO mask (`0x20ff00` cleared, `vol << 8`) matches upstream, and the write is followed by
`udelay(1)` before `sid_read_ldo_b_cal`, as upstream does [1:84-105]. `sid_read_ldo_b_cal`
reads SID `0x300621c` bits 8-15 and applies the same DDR3 adjustment (`reg > 0x20` → `reg -=
0x16`); its nested `if` is behaviourally equivalent to upstream's early return.

**Verdict: fixed in #18.** The old override could only change DDR2 (47 → 25); T113-S3 is DDR3.

### 4.4 VTFCR write reads the wrong register (fixed in #15)

The old `init` did:

```rust
let addr  = MCTL_PHY_BASE + ZQCR;  // 0x140
let addr1 = MCTL_PHY_BASE + VTFCR; // 0xb8
write32(addr, read32(addr) | (1 << 31));
if (self.dram_tpr13 & (1 << 8)) != 0 {
    write32(addr1, read32(addr) | 0x300);
}
```

That is exactly `mctl_hal.c:1597-1600`. Upstream reads VTFCR itself:
`writel(readl(VTFCR) | 0x300, VTFCR)` [1:1397-1398]. The branch is live with the shipped default
`dram_tpr13 = 0x34000100` (bit 8 set), so the old code wrote `read32(ZQCR) | 0x300` into VTFCR on
every boot. Issue #15 changed the second read to VTFCR; `init_dram` now does
`write32(addr1, read32(addr1) | 0x300)`.

**Status: fixed in #15.** On-hardware confirmation was part of #15's HIL run.

### 4.5 Timing parameters and mode-register writes (#18)

`mctl_set_timing_params` is still the old `mctl_hal.c` routine [6:183-617], not upstream's
`mctl_set_timing_params` [1:190-505]. Differences in form:

- It keeps the assembly-derived SP-slot comments and reconstructs `dram_tpr0/1/2` from the
  computed values; upstream no longer writes those fields, and they are not read later. The
  reconstruction is dead but harmless; it stays until a separate cleanup.
- It computes timing with `ns_to_t(nanoseconds)` using `dram_clk / 2`, equivalent to upstream's
  ceiling division [1:47-52].
- For DDR3 (the T113-S3 default) the resulting values are numerically equivalent at 792 MHz:
  `tfaw = ns(50)`, `trcd = ns(15)`, `trp = ns(15)`, `trc = ns(53)`, `tras = ns(38)`,
  `twtr = ns(8)+2`, `trrd = max(ns(10),2)`, `txp = max(ns(10),2)`, `tcl = 6`, `tcwl = 4`,
  `t_rdata_en = 4`, `mr0 = 0x1c70`, `mr2 = 0x18`, `trd2wr = 5`
  [1:252-345].
- **Mode-register writes are aligned (#18):** the old code overwrote `dram_mr0/1/2/3` only when
  the corresponding field's upper 16 bits were zero, then wrote the struct fields. Upstream
  writes the locally computed `mr0..mr3` directly [1:480-484]. `mctl_set_timing_params` now
  writes `mr0..mr3` directly; `mr1`/`mr3` still start from `dram_mr1`/`dram_mr3` as upstream.

`mctl_vrefzq_init`, `mctl_com_init`, `mctl_sys_init`, `ccu_set_pll_ddr_clk`, `mctl_channel_init`,
`eye_delay_compensation`, and `dqs_gate_detect` were spot-checked against the upstream rework and
are register-equivalent (same masks, same addresses, same order). An exhaustive instruction-level
equivalence proof for `mctl_channel_init` was not performed.

### 4.6 Size reporting and `dram_top`

`dramc_get_dram_size` is upstream's `DRAMC_get_dram_size` with `calculate_rank_size` inlined; the
bit math is identical [1:932-969]. What is missing is upstream's `dram_top` side effect:
`sunxi_dram_init` computes `size = init_DRAM(...) * 1 MiB` and stores
`dram_top = SDRAM_BASE + size`, exposing it via `dram_get_top()` [1:115-120,1449-1452].

**Deliberately absent (#18).** The parent issue says to add `dram_get_top` only when the payload
contract actually needs a top address [14]; today it does not.

### 4.7 Hardware confirmation

`scripts/hil-check.sh` on T113-S3 loads `target/rawboot-fel.bin` over FEL and asserts the boot
log. After #18 a clean boot printed the expected sequence, including:

```
trefi:7.8ms
DRAMC read ODT  off.
DRAM ODT value: 0x42.
DRAM simple test OK.
get state
SD/MMC card not found
```

`DRAM simple test OK` and the following SD line mean the new scanner completed and the loader
kept executing. The script's reset-loop guard can still trip when the board is left with a
watchdog armed from an earlier `xfel reset`; the runbook requires a power-cycle into FEL before
a clean capture (`docs/agents/hil.md`). The PD22 LED blink is a human check.

---

## 5. Remaining differences

Ordered by likely impact on T113-S3:

1. **`dram_get_top()` / `dram_top`** — absent by design until the payload needs it [14].
2. **T113-S4 AC-remap exception** — not ported; inert on T113-S3, and non-T113-S3 is out of scope.
3. **Timing routine structure** — old `mctl_hal.c` shape with dead `dram_tpr0/1/2`
   reconstruction; numerically equivalent for the DDR3 default. MR-write gating is fixed in #18.
4. **`abort()`** — not ported; the Rust code never calls it.
5. **Register access** — many raw offsets remain where upstream uses `MCTL_PHY_*` macros; the
   base constants and the names touched by #18 are aligned.

Fixed and no longer differences: the per-rank `auto_scan_dram_size` (#18), forced `vol = 25`
(#18), MR-write gating (#18), VTFCR source register (#15), and the in-scope function/variable
names (#18).

Not missing: ZQ bring-up, PLL/MBUS/DRAM clock sequencing, PHY AC remap tables (`cfg1..cfg7`
match upstream indices 1..7), DQS gate detection, Vref/ZQ init, channel init, eye-delay
compensation, simple read/write test, and the default `dram_para_t` values (792 MHz, type 3, ZQ
`0x7b7bfb`, `tpr11 = 0x00340000`, `tpr12 = 0x46`, `tpr13 = 0x34000100`)
[1:1420-1447].

---

## 6. Open questions

1. **Dual-rank T113-S3 behaviour.** The per-rank scanner is now in place, but no dual-rank
   module is available for an A/B hardware test. Upstream's `9c8b8e40` message calls it an
   update, not a bug fix, and the code has suspicious parts (one-iteration bank loop; rank-1
   pattern still written at `SDRAM_BASE` while compares use `rank1_addr`) [1:1096-1249][8].
   **UNVERIFIED on dual-rank hardware** (the HIL board is 128 MiB single-rank).
2. **`dram_get_top` integration.** Does the `BootInfo` contract ever need a DRAM top address, or
   is the returned MiB size sufficient? Deferred by the parent issue [14].
3. **Exhaustive equivalence of `mctl_channel_init`.** Spot checks show the same
   registers/masks, but no full line-by-line proof was done because the Rust uses mixed literal
   and computed addresses. **UNVERIFIED** for every write.
4. **Licensing.** Upstream `dram.c` is GPL-2.0+ [1:1]; `mctl_hal.c` carried no explicit SPDX
   header in the fetched file [6]. The project transcribes facts and behaviour under Unlicense
   (ADR-0001); if any `mctl_hal.c` text was copied verbatim, the licence position should be
   re-checked before redistribution.

---

## Sources

1. Upstream `arch/arm32/mach-t113s3/dram.c` at `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.c
2. Upstream `arch/arm32/mach-t113s3/dram.h` at the same SHA —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/dram.h
3. Upstream `arch/arm32/mach-t113s3/reg-dram.h` at the same SHA —
   https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/reg-dram.h
4. GitHub API, commit history for `arch/arm32/mach-t113s3/dram.c` —
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram.c&per_page=100
5. GitHub API, commit history for `arch/arm32/mach-t113s3/dram_sun20i_d1.c` —
   https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/dram_sun20i_d1.c&per_page=100
6. Upstream `arch/arm32/mach-t113s3/mctl_hal.c` at `23e5c39396588c831e45c77415aed89f12a66a54`
   (last tree before the rework) —
   https://raw.githubusercontent.com/szemzoa/awboot/23e5c39396588c831e45c77415aed89f12a66a54/arch/arm32/mach-t113s3/mctl_hal.c
7. GitHub API, commit `381606b0d68f` ("import Andre Przywara's DRAM init rework code", deletes
   `mctl_hal.c`, adds `dram_sun20i_d1.c`) —
   https://api.github.com/repos/szemzoa/awboot/commits/381606b0d68f
8. Upstream `arch/arm32/mach-t113s3/dram.c` at `9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22`
   ("add t113-s4 support ... update t113-s3 auto_scan_dram_size func") —
   https://raw.githubusercontent.com/szemzoa/awboot/9c8b8e40d28bd73bafaeaa0e43c06c4ded2c1e22/arch/arm32/mach-t113s3/dram.c
9. GitHub API, commit `385481ffab45089d90410cb67b93981a870edd7c` (2025-11-15 refactor; adds
   `dram_top`/`dram_get_top`) —
   https://api.github.com/repos/szemzoa/awboot/commits/385481ffab45089d90410cb67b93981a870edd7c
10. Local `rawboot/src/dram/mod.rs` — <workspace>/rust/awboot/rawboot/src/dram/mod.rs
11. Local `embed/app/src/dram/mod.rs` (vendored pre-port reference) —
    <workspace>/rust/awboot/embed/app/src/dram/mod.rs
12. GitHub API, `main` HEAD at research time (resolves to `5380c00fc67c975433f25c57fb481aa2b91aebf8`) —
    https://api.github.com/repos/szemzoa/awboot/commits/main
13. Prior research note (style reference) — <workspace>/rust/awboot/research/awboot-rust-migration.md
14. Issue #18 "Parity DRAM: dram" and parent issue #14 "Выравнивание лоадера rawboot на upstream
    awboot: parity поведения".
