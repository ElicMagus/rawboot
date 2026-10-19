# `rawboot` DRAM Port vs Current Upstream `szemzoa/awboot` `dram.c` — Primary-Source Comparison

Researched: 2026. Target: Allwinner T113-S3 (DDR3, 792 MHz default). All claims are cited inline to
the exact primary source (upstream file at a pinned SHA with line numbers, local Rust file with line
numbers, or GitHub API metadata). Items that could not be verified against a primary source are marked
**UNVERIFIED**.

Reference numbers in square brackets map to the **Sources** section at the end.

> Local `rawboot/...:line` references below predate the symbol-citation rule in
> `docs/agents/domain.md`; migrate them to symbol citations when touching this note.

---

## 1. Executive summary

- Upstream is pinned to `szemzoa/awboot` `main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`
  (2025-11-21, "Merge pull request #41 ... psci-rebase") [12]. The latest commit touching
  `arch/arm32/mach-t113s3/dram.c` is `385481ffab45089d90410cb67b93981a870edd7c`
  (2025-11-15) [4][9]. At that SHA `dram.c` is 1453 lines / 40,623 B [1].
- **Our Rust port is not derived from any revision of `dram.c`.** It is a near line-for-line port of
  the pre-rework `arch/arm32/mach-t113s3/mctl_hal.c`, which upstream *deleted* in commit
  `381606b0d68f` ("import Andre Przywara's DRAM init rework code", 2023-01-17) when it added
  `dram_sun20i_d1.c` [5][6][7]. The tell-tale names `auto_cal_timing`, `auto_set_timing_para`,
  `dram_vol_set`, `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test` exist only in
  `mctl_hal.c`, not in `dram.c` [6][1][10].
- Consequently the interesting "divergences" are upstream *changes made after the fork point*: the
  2023-01-17 Przywara rework, the 2023-03-13 `auto_scan_dram_size` rewrite, and the 2025-11-15
  refactor. The single largest algorithmic gap is `auto_scan_dram_size`: our port has the old
  rank/row/bank/page scanner, upstream HEAD has a per-rank scanner with a separate rank-1 address
  (`rank1_addr`) and a different rank-count test [1:1096-1249][10:404-589].
- Upstream added T113-S4/D1s handling to `mctl_phy_ac_remapping` (`chipid == 0x6800` → no remap;
  fuse 10 → D1s table); our port has neither the chip ID read nor the branch [1:697-755][10:1525-1623].
  On a T113-S3 this should be inert, but on T113-S4 it changes the PHY AC remap table.
- Our port retains the old `init_DRAM` VTFCR bug: it reads `ZQCR` (0x140) and writes the result to
  VTFCR (0xb8), whereas upstream reworked code reads VTFCR itself [10:290-295][1:1397-1398].
  This branch is **live with the shipped default** `dram_tpr13 = 0x34000100`, whose bit 8 is set
  [10:188], so the port writes `0x80000300` into VTFCR instead of setting bits 8-9 on the existing
  VTFCR value. **Fixed in issue #15.**
- `embed/app/src/dram/mod.rs` is **functionally identical** to `rawboot/src/dram/mod.rs` at the
  source level: a whitespace-insensitive diff produces only 13 hunks, all line-wrapping, comment
  placement, or `use` reordering; no numeric constant or control-flow difference was found [10][11].
- `dram_get_top()` / the `dram_top` variable are **MISSING IN PORT**; upstream added them in the
  2025-11-15 refactor and sets `dram_top` after `init_DRAM()` [1:115-120,1449-1450][9].

### Verdict

Our DRAM port is a faithful port of an **older code base** (`mctl_hal.c`), not a stale copy of the
current `dram.c`. For the T113-S3 default configuration the register-level behaviour is largely
unchanged, but the port is missing the 2023 `auto_scan_dram_size` update, the T113-S4 AC-remap
exception, the VTFCR read fix (active by default; now fixed in issue #15), and the `dram_get_top` API. These are all **older-than-upstream**, not
intentional divergences, with the exception of the forced `vol = 25` and the MR-write gating, which
are inherited verbatim from `mctl_hal.c` but were deliberately removed upstream.

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

**None of the `dram.c` revisions.** It matches `mctl_hal.c` at the last tree containing it
(`23e5c393`, parent of `381606b0`) [6][7]. Evidence:

- Function inventory: `auto_cal_timing`, `auto_set_timing_para`, `dram_vol_set`,
  `sid_read_ldoB_cal`, `DRAMC_get_dram_size`, `dramc_simple_wr_test`, `mctl_hal.c`'s
  `auto_scan_dram_size` body (variables `rval, i, j, rank, maxrank, offs, shft, ptr, mc_work_mode,
  chk`) — all present in `mctl_hal.c` and absent from every `dram.c` revision [6][1][10].
- The Rust `auto_scan_dram_size` body, including the `while read32(mc_work_mode) != rval {}` spin
  loops, the `11..17` row scan using `chk = RAM_BASE + (1 << (i + 11))`, the A22 bank probe, the
  `9..14` page scan, the rank-1 `0x3202000/0x3202004` reprogramming, and the final
  `maxrank == 2` comparison, is a direct translation of `mctl_hal.c:1287-1452`
  [10:404-589][6:1287-1452].
- The Rust `vol = 25; // XXX` line is present only in `mctl_hal.c`, not in `dram.c`
  [10:342][6:54][1:84-105].
- The Rust VTFCR write reads register `0x140` (ZQCR), exactly as `mctl_hal.c` does; `dram.c` reads
  VTFCR [10:290-295][6:1597-1600][1:1397-1398].
- The Rust timing routine keeps the old SP-slot comments (`s4`, `s8`, `80(sp)`, `s5`) and the dead
  `dram_tpr0/1/2` reconstruction, both of which exist in `mctl_hal.c` and were removed by the rework
  [10:1041-1235][6:183-355][1:190-505].

The exact `mctl_hal.c` revision used by the port cannot be proven from the local tree because the
Rust code carries no provenance comment; `23e5c393` is the **closest surviving upstream ancestor**.
Exact match is **UNVERIFIED**.

---

## 3. Function mapping table

Upstream names are from `dram.c` at `5380c00` [1]; local lines are from `rawboot/src/dram/mod.rs`
[10].

| Upstream C (`dram.c` @5380c00) | Local Rust (`rawboot/src/dram/mod.rs`) | Status |
|---|---|---|
| `sunxi_dram_init` [1:1420] | `init` [10:194] | merged with `init_DRAM`; no `dram_top` side effect |
| `init_DRAM` [1:1306] | `init` [10:194] | present |
| `dram_get_top` [1:117] | — | **MISSING IN PORT** |
| `dram_disable_all_master` [1:122] | `dram_disable_all_master` [10:1873] | present |
| `dram_enable_all_master` [1:107] | `enable_all_master` [10:1866] | present |
| `dram_voltage_set` [1:84] | `vol_set` [10:324] | present, forced 25 |
| `sid_read_ldoB_cal` [1:60] | `sid_read_ldo_b_cal` [10:359] | present |
| `mctl_sys_init` [1:555] | `mctl_sys_init` [10:1734] | present |
| `ccu_set_pll_ddr_clk` [1:512] | `ccm_set_pll_ddr_clk` [10:1786] | present (renamed) |
| `mctl_com_init` [1:595] | `mctl_com_init` [10:1625] | present |
| `mctl_core_init` [1:1070] | `mctl_core_init` [10:673] | present, same call order |
| `mctl_channel_init` [1:773] | `mctl_channel_init` [10:686] | present |
| `mctl_vrefzq_init` [1:1053] | `mctl_vrefzq_init` [10:1716] | present |
| `mctl_phy_ac_remapping` [1:697] | `mctl_phy_ac_remapping` [10:1525] | present, older table selection |
| `mctl_set_timing_params` [1:190] | `auto_set_timing_para` [10:1041] | present, older structure |
| `ns_to_t` [1:47] | `auto_cal_timing` [10:1515] | equivalent formula, renamed |
| `calculate_rank_size` [1:932] | inlined in `get_size` [10:1830] | present, inlined |
| `DRAMC_get_dram_size` [1:949] | `get_size` [10:1830] | present (renamed) |
| `auto_scan_dram_config` [1:1288] | `auto_scan_dram_config` [10:381] | present |
| `auto_scan_dram_rank_width` [1:1257] | `auto_scan_dram_rank_width` [10:596] | present |
| `auto_scan_dram_size` [1:1096] | `auto_scan_dram_size` [10:404] | present, **old algorithm** |
| `dqs_gate_detect` [1:975] | `dqs_gate_detect` [10:631] | present |
| `eye_delay_compensation` [1:130] | `eye_delay_compensation` [10:942] | present |
| `dramc_simple_wr_test` [1:1016] | `simple_wr_test` [10:1880] | present (renamed) |
| `abort` [1:54] | — | **MISSING IN PORT** (Rust code never calls it) |

`ns_to_t` computes `DIV_ROUND_UP((dram_clk/2) * ns, 1000)` [1:47-52]; `auto_cal_timing`
computes `time*freq/1000 + ((time*freq % 1000) != 0)` and is called with `freq = dram_clk >> 1`,
which is the same ceiling division [10:1515-1520][6:173-176].

---

## 4. Algorithmic divergences

### 4.1 `auto_scan_dram_size`: old linear scanner vs upstream per-rank scanner

**Our port** (`rawboot/src/dram/mod.rs:404-589`) is the `mctl_hal.c` algorithm [6:1287-1452]:

- Rank count is `maxrank = (dram_para2 & 0xf000) ? 2 : 1` [10:424-428].
- A test pattern is written **once** at `RAM_BASE` (`0x40000000`) [10:432-439].
- Row scan walks `i = 11..17`, probing `chk = RAM_BASE + (1 << (i + 11))` and comparing 64
  consecutive words against the pattern at `RAM_BASE` [10:451-452].
- Bank scan probes the single address line A22 (`RAM_BASE + (1 << 22)`) and stores `j` (0/1) into
  the bank field [10:493-511].
- Page scan walks `i = 9..14`, probing `RAM_BASE + (1 << i)` [10:534-535].
- For rank 1 it programs `0x3202000`/`0x3202004` (the *wrong* base — see open question) and then
  advances `mc_work_mode += 4` [10:562-575].
- Final dual-rank comparison uses the low/high halves of `dram_para1` [10:578-587].

**Upstream HEAD** (`dram.c:1096-1249`) [1] is the `9c8b8e40` rewrite [8:1089-1245]:

- Rank count is `rank_count = 2` only when `((dram_para2 >> 12) & 0xf) == 0x1` [1:1110-1112].
- Each rank is scanned in a `for (current_rank = 0; current_rank < rank_count; ...)` loop with
  `mc_work_mode = MCTL_COM_WORK_MODE0 + 4*current_rank` [1:1114-1115].
- For rank 1 it first reprograms **both** `MCTL_COM_WORK_MODE0` and `MCTL_COM_WORK_MODE1` to
  16-row/4-bank/512 B page (`0xf0c` ← `0x6f0`) and sets
  `rank1_addr = CONFIG_SYS_SDRAM_BASE + (1 << 27)` [1:1118-1123].
- The test pattern is written **per rank**, and every compare uses `rank1_addr + 4*j` (rank 0:
  `SDRAM_BASE`; rank 1: `SDRAM_BASE + 128 MiB`) [1:1125-1145].
- Row scan address is `ret = SDRAM_BASE + (1 << (i + 2 + 9))`, i.e. the same `i + 11` exponent, but
  it reads `ret + j*4` (advancing the read address) rather than a fixed base [1:1140-1150].
- The bank probe is no longer A22; it is a one-iteration loop over
  `ret = SDRAM_BASE + (1 << (i + 2 + 9))` with `i = 0` (A11), and the stored bank field value is
  `i` (0), not the old 0/1 flag [1:1163-1188].
- The final dual-rank check is the same comparison and the same `dram_para2` bit 8 update
  [1:1239-1248].

**Verdict: our port looks OLDER.** This is exactly the "update t113-s3 auto_scan_dram_size func"
commit `9c8b8e40` (2023-03-13) [8]. Upstream's commit message frames it as an update/fix for
T113-S3; the rewrite changes rank enumeration, rank-1 address base, and bank detection. Whether the
new scanner fixes a concrete dual-rank bug on T113-S3 hardware is **UNVERIFIED** (no board test and
no upstream commit body detail). The upstream code also looks internally suspicious (one-iteration
bank loop, per-rank pattern still written at `SDRAM_BASE` despite using `rank1_addr` for compares)
[1:1118-1188], so a byte-for-byte port of the new scanner should be hardware-tested before adoption.

### 4.2 AC remapping: missing T113-S4 chip-ID exception (and D1s fuse 10)

Our port (`rawboot/src/dram/mod.rs:1525-1623`) matches `mctl_hal.c:795-859` [6]:

- Reads fuse `(SID+0x28 & 0xf00) >> 8` and an unused `mark_id` from `0x3006200`
  [10:1531-1535].
- If `dram_tpr13` bits 18-19 are set → `cfg7`; otherwise fuse 8→`cfg2`, 9→`cfg3`, 10→`cfg5`,
  11→`cfg4`, 12→`cfg1`, 13/14→zeros [10:1537-1560].
- DDR2: fuse 15 returns; otherwise `cfg6` [10:1572-1578].
- Builds four 32-bit `REMAP0..3` words from the 22-byte table and finally sets bit 0 of REMAP0
  [10:1580-1620].

Upstream HEAD (`dram.c:697-755`) additionally reads `chipid = SUNXI_SID_BASE & 0xffff`
[1:709-713], and in the DDR3 fuse-10 case selects "no remap" (`ac_remapping_tables[0]`) when
`chipid == 0x6800` (T113-S4), otherwise `ac_remapping_tables[5]` [1:733-737]. It also contains a
dead `if (fuse == 10) cfg = ac_remapping_tables[0];` which is immediately overwritten by
`cfg = ac_remapping_tables[6]` in the DDR2 path [1:718-720].

The 22-byte table contents are otherwise identical between our `cfg1..cfg7` and upstream
`ac_remapping_tables[1..7]`; upstream also carries tables `[8]`/`[9]` (H133) that its selector never
references [1:668-695][10:105-129].

**Verdict: our port looks OLDER** — missing the T113-S4 no-remap exception added by `9c8b8e40`
[8:693-737]. On T113-S3 the chip ID is not `0x6800`; for fuse 10 upstream therefore still selects table[5] like
our port, and for all other fuse values the branch never triggers, so T113-S3 behaviour is
unchanged. The D1s fuse-10
line in upstream is dead code and does not represent a behavioural gap.

### 4.3 Voltage / LDO handling

Our `vol_set` matches the old logic and then **unconditionally forces `vol = 25`**:

```rust
match self.dram_type { 2 => vol = 47, 3 => vol = 25, _ => vol = 0 }
vol = 25; // XXX
```

[10:324-345], identical to `mctl_hal.c:40-60` [6]. Upstream's reworked `dram_voltage_set` keeps the
per-type values without the override (DDR2 → 47, DDR3 → 25, default → 0)
[1:84-105]. The LDO write mask is the same in both (`0x20ff00` cleared, `vol << 8`), expressed in
Rust as `& 0xffdf00ff | (vol << 8)` [10:345][1:100].

`sid_read_ldo_b_cal` reads SID `0x300621c` bits 8-15; Rust's `(read32(...) << 16) >> 24` is
equivalent to upstream's `(readl(...) & 0xff00) >> 8` [10:362][1:62]. Upstream restructured the
function to early-return on zero and use `clrsetbits` [1:60-79]; our nested `if reg != 0` is
behaviourally equivalent for the default path [10:364-378].

**Verdict: intentionally/lineage different, not an upstream fix we need for T113-S3.** For DDR3
(both `dram_type == 3`) 25 is the correct value; the override only changes DDR2 from 47 to 25
[10:328-342][1:84-100]. It is inherited verbatim from `mctl_hal.c`, so it is "older", but there is
no T113-S3 impact.

### 4.4 VTFCR write reads the wrong register (ZQCR instead of VTFCR)

Our `init` does:

```rust
let addr  = SUNXI_DRAM_CTL0_BASE + 0x140; // ZQCR
let addr1 = SUNXI_DRAM_CTL0_BASE + 0x0b8; // VTFCR
write32(addr, read32(addr) | (1 << 31));
if (self.dram_tpr13 & (1 << 8)) != 0 {
    write32(addr1, read32(addr) | 0x300);
}
```

[10:290-295]. That is exactly `mctl_hal.c:1597-1600` [6]. Upstream's reworked `init_DRAM` reads
VTFCR itself: `writel(readl(VTFCR) | 0x300, VTFCR)` [1:1397-1398].

**Verdict: our port looks OLDER** — upstream fixed a source-register bug during the 2023 rework.
The branch is **active in the shipped configuration**: `dram_tpr13 = 0x34000100` has bit 8 set
[10:188], so `read32(0x140) | 0x300` (with bit 31 already set on ZQCR) is written to VTFCR on every
boot. Upstream instead ORs `0x300` into the current VTFCR value [1:1397-1398]. This is the most
concrete live behavioural divergence found; its observable effect on T113-S3 (if any) is
**UNVERIFIED** without hardware.

**Status (issue #15): fixed.** `rawboot/src/dram/mod.rs` now reads VTFCR before OR-ing `0x300`, matching
upstream [1:1397-1398]. On-hardware confirmation (UART log + working LED payload) is tracked in #15.

### 4.5 Timing parameters and mode-register writes

Our `auto_set_timing_para` (`rawboot/src/dram/mod.rs:1041-1513`) is the old `mctl_hal.c` routine
[6:183-617], not upstream's `mctl_set_timing_params` [1:190-505]. Differences in form:

- It keeps the assembly-derived SP-slot comments and reconstructs `dram_tpr0/1/2` from the computed
  values [10:1219-1230]; upstream no longer writes those fields, and they are not read later
  [6:348-350][1:190-505].
- It implements the MCTL timing math with `auto_cal_timing(time, dram_clk/2)` instead of
  `ns_to_t(para, time)`, but the arithmetic is the same ceiling division [10:1515-1520][1:47-52].
- For DDR3 (the T113-S3 default) the resulting values are numerically equivalent at 792 MHz: both
  use `tfaw = ns(50)`, `trcd = ns(15)`, `trp = ns(15)`, `trc = ns(53)`, `tras = ns(38)`,
  `twtr = ns(8)+2`, `trrd = max(ns(10),2)`, `txp = max(ns(10),2)`, `tcl = 6`, `tcwl = 4`,
  `t_rdata_en = 4`, `mr0 = 0x1c70`, `mr2 = 0x18`, and `trd2wr = 5`
  [10:1089-1124,1306-1345][1:252-345]. Rust derives `t_rdata_en` from `tcwl`, which happens to
  equal upstream's explicit 4 [10:1335-1341][1:305-310]; the dead `trd2wr = tcwl + 2 + twr` is
  overwritten with 5/6 in both [10:1325-1344][1:326-345].
- **Mode-register gating differs:** Rust only overwrites `dram_mr0/1/2/3` when the corresponding
  field's upper 16 bits are zero [10:1446-1458], then writes the struct fields
  [10:1461-1464]. Upstream writes the locally computed `mr0..mr3` directly
  [1:480-484]. With the shipped defaults `mr0 = 0x1c70`, `mr1 = 0x42`, `mr2 = 0x18`,
  `mr3 = 0` [10:171-174], both paths write the same values; the difference is only visible for a
  caller supplying a non-zero upper half.

`mctl_vrefzq_init`, `mctl_com_init`, `mctl_sys_init`, `ccm_set_pll_ddr_clk`, `mctl_channel_init`,
`eye_delay_compensation`, and `dqs_gate_detect` were spot-checked against the upstream rework and
are register-equivalent (same masks, same addresses, same order); the rework mostly replaces raw
addresses with `MCTL_*` macros and folds the `eye_delay_compensation` loops into indexed writes
[10:942-1040,1625-1828,686-941][1:130-190,555-697,773-932]. An exhaustive instruction-level
equivalence proof for `mctl_channel_init` was not performed — **UNVERIFIED** beyond the spot checks.

### 4.6 Size reporting and `dram_top`

Our `get_size` is upstream's `DRAMC_get_dram_size` with `calculate_rank_size` inlined; the bit
math is identical [10:1830-1864][1:932-969]. What is missing is upstream's `dram_top` side effect:
`sunxi_dram_init` computes `size = init_DRAM(...) * 1 MiB` and stores
`dram_top = SDRAM_BASE + size`, exposing it via `dram_get_top()` [1:115-120,1449-1452]. Our `init`
returns the size in MiB and never records a top address [10:194-322]; there is no `get_top` /
`dram_get_top` equivalent [10].

**Verdict: our port looks OLDER** — `dram_top`/`dram_get_top` were added by the 2025-11-15 refactor
[9]. Whether our own `BootInfo` hand-off needs it is an integration question, not a DRAM-logic bug.

---

## 5. What our port is missing / older

Ordered by likely impact on T113-S3:

1. **`auto_scan_dram_size` rewrite** (`9c8b8e40`, 2023-03-13) — old scanner in the port; upstream
   uses a per-rank scanner with `rank1_addr` and a different rank-count test
   [10:404-589][1:1096-1249][8]. **OLDER; potentially a correctness fix for dual-rank parts.**
2. **T113-S4 AC-remap exception** — missing `chipid == 0x6800` no-remap branch
   [10:1525-1623][1:697-755][8]. **OLDER; inert on T113-S3, wrong table on T113-S4.**
3. **VTFCR source-register fix** — port read ZQCR and wrote it to VTFCR; branch is live with
   default `tpr13` bit 8 set [10:290-295,188][1:1397-1398]. **OLDER; live behavioural divergence.**
   **FIXED in issue #15** (hardware confirmation pending).
4. **`dram_get_top()` / `dram_top`** — not exposed [1:115-120,1449-1452][10]. **OLDER; API gap
   introduced by the 2025 refactor.**
5. **Forced `vol = 25`** — inherited from `mctl_hal.c`; upstream removed it
   [10:342][6:54][1:84-100]. **OLDER/intentional-lineage; only affects DDR2.**
6. **MR-write gating on upper 16 bits** — inherited from `mctl_hal.c`; upstream always writes the
   computed MR values [10:1446-1464][1:480-484]. **OLDER; no effect with shipped defaults.**
7. **Dead `dram_tpr0/1/2` reconstruction** — retained from `mctl_hal.c`; harmless because nothing
   downstream reads them after the timing routine [10:1219-1230][1:190-505]. **OLDER; cosmetic.**
8. **Structural/API renames** — `ns_to_t`→`auto_cal_timing`, `DRAMC_get_dram_size`→`get_size`,
   `dramc_simple_wr_test`→`simple_wr_test`, `ccu_set_pll_ddr_clk`→`ccm_set_pll_ddr_clk`,
   `dram_voltage_set`→`vol_set`, `dram_enable_all_master`→`enable_all_master`
   [10][1]. **Intentionally different / idiomatic Rust.**

Not missing: ZQ bring-up, PLL/MBUS/DRAM clock sequencing, PHY AC remap tables (`cfg1..cfg7` match
upstream indices 1..7), DQS gate detection, Vref/ZQ init, channel init, eye-delay compensation,
simple read/write test, and the default `dram_para_t` values (792 MHz, type 3, ZQ `0x7b7bfb`,
`tpr11 = 0x00340000`, `tpr12 = 0x46`, `tpr13 = 0x34000100`)
[10:165-191][1:1420-1447].

---

## 6. Open questions

1. **Dual-rank T113-S3 behaviour.** Does the old `auto_scan_dram_size` actually mis-detect any
   shipping dual-rank T113-S3 module? Upstream's `9c8b8e40` message calls it an update, not a bug
   fix, and the new code has suspicious parts (one-iteration bank loop; rank-1 pattern still written
   at `SDRAM_BASE`). Hardware A/B testing is required before porting the new scanner
   [1:1096-1249][8]. **UNVERIFIED.**
2. **Rank-1 register base.** Our port writes rank-1 mode registers at `0x3202000`/`0x3202004`
   [10:564-572], while `mctl_hal.c` itself does the same and upstream's rework uses
   `MCTL_COM_BASE + MCTL_COM_WORK_MODE0/1` (= `0x3102000`/`0x3102004`) [6:1420-1437][1:1118-1123].
   Is `0x3202000` a valid alias on T113-S3? **UNVERIFIED** against a register manual.
3. **`chipid == 0x6800` T113-S4 detection.** Is the chip-ID field at `SUNXI_SID_BASE & 0xffff`
   really `0x6800` on T113-S4, and should our port carry the exception even though the project targets
   T113-S3? Only upstream source evidence exists; no datasheet was retrieved [1:709-737].
   **UNVERIFIED** against a primary vendor document.
4. **`dram_get_top` integration.** Does our `rawboot` `BootInfo` contract need a DRAM top address,
   or is the returned MiB size sufficient? The upstream API exists but our hand-off is different
   [1:115-120][10:194].
5. **Exhaustive equivalence of `mctl_channel_init`.** Spot checks show the same registers/masks, but
   no full line-by-line proof was done because the Rust uses mixed literal and computed addresses
   [10:686-941][1:773-932]. **UNVERIFIED** for every write.
6. **Licensing.** Upstream `dram.c` is GPL-2.0+ [1:1]; `mctl_hal.c` carried no explicit SPDX header in
   the fetched file [6]. If any `mctl_hal.c` text was copied verbatim, the port's licence position
   should be re-checked before redistribution.

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
10. Local `rawboot/src/dram/mod.rs` (1919 lines) — <workspace>/rust/awboot/rawboot/src/dram/mod.rs
11. Local `embed/app/src/dram/mod.rs` (1926 lines) — <workspace>/rust/awboot/embed/app/src/dram/mod.rs
12. GitHub API, `main` HEAD at research time (resolves to `5380c00fc67c975433f25c57fb481aa2b91aebf8`) —
    https://api.github.com/repos/szemzoa/awboot/commits/main
13. Prior research note (style reference) — <workspace>/rust/awboot/research/awboot-rust-migration.md
