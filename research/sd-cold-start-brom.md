# SD cold start after BROM on T113-S3 (#43)

Researched: 2026. Target: Allwinner T113-S3, SMHC0. Upstream is pinned to
`szemzoa/awboot` `5380c00fc67c975433f25c57fb481aa2b91aebf8`. Local code is cited by
symbol, not by line.

This note records the investigation behind issue #43 and the deliberately chosen
divergence from upstream. It does not claim a HIL pass; the statistical acceptance
(ten cold power-cycles) is tracked in the issue.

---

## 1. Symptom

With power removed, the FEL/BOOT strap held, a working SD card already in the slot,
and power then applied, the loader often prints:

```
SMHC: cmd1 timeout (rint=0x00000100, flag=0x00000004)
SD/MMC card not found
```

Some runs also print `SMHC: cmd8 timeout` before the CMD1 line. The same image works
when the card is inserted after the board is in FEL (hot-plug), or when the loader is
re-executed over FEL without removing power. Baseline `master @ ab49d66` and the
post-#38 tree show the same behaviour, so this is not a #38 regression.

The failing CMD1 is the **MMC fallback**. The SD path (`sd_send_op_cond`) rejects the
card silently first: CMD8 can time out, or ACMD41 can poll without the busy bit ever
being set. Only then does `sdmmc_detect` reset the host and try CMD1, which an SD card
does not answer. The visible CMD1 timeout is therefore a consequence, not the root
failure.

## 2. What the HIL logs rule out

- **Retry budget.** `fix/43-sd-init-retries` raised the attempt count. One cold run
  passed after two CMD1 timeouts; another logged about forty CMD1 timeouts over more
  than five seconds before the loader gave up. More attempts and more delay do not make
  the card answer.
- **A power-only card state.** Re-executing the loader without a power cycle recovers
  the card, so the card is not latched in a state that only a power cycle clears.
- **#38/#39 regressions.** The same timeouts reproduce on the pre-#38 baseline and on
  the #39 branch; the block/FAT layers are not involved.

What remains is the state BROM leaves behind. BROM has already initialised SMHC0 and
the card to read the boot sectors before it hands over to FEL. The loader's first
attempt therefore starts from an active controller and an identified card, unlike the
hot-plug case, where the card has been clocked by BROM's still-running SMHC clock
until the loader replaces it.

## 3. Two missing host steps

The pinned upstream `sunxi_sdhci.c` does neither of the following [1:525-645,
647-651]; mainline Linux `sunxi-mmc.c` does both (`sunxi_mmc_reset_host`,
`sunxi_mmc_request`) [2]. The port transcribed upstream, so both are absent here.

1. **Wait for the controller reset to finish.** `sdhci_reset` writes
   `SMHC_GCTRL_HARDWARE_RESET` and returns immediately. The reset bits are
   self-clearing. Linux `sunxi_mmc_reset_host` polls `REG_GCTRL` until they clear,
   with a 250 ms timeout, before programming the clock. Programming `CLKCR`, the CCU
   gate/reset and the timing registers while the controller is still resetting is a
   race; on a cold start the controller comes from BROM's active state, so the race is
   visible there and not after a loader-to-loader re-exec.
2. **Send the 80-clock init sequence with CMD0.** Linux sets
   `SDXC_SEND_INIT_SEQUENCE` in `REG_CMDR` for `MMC_GO_IDLE_STATE`. The SD physical
   layer requires at least 74 clocks with CMD/DAT high before CMD0. The SMHC hardware
   generates them when that bit is set. Without it, CMD0 is sent immediately after the
   card clock is enabled; after the host reset stopped and restarted the clock, the
   card may not have seen enough clocks to accept the command. CMD8/ACMD41 then fail
   even though the card is healthy.

The older Rust snapshot (`embed/app/src/sdhci/mod.rs`) did set
`SEND_INIT_SEQUENCE`; the upstream-parity rewrite in #19 dropped it because upstream
C does not use it.

## 4. Chosen change

- `sdhci_reset` first enables the controller gate and deasserts its CCU reset, then
  polls `GCTRL` until the reset bits clear (250 ms bound) and reports a timeout
  instead of returning immediately. Enabling the gate first matters because a gated
  SMHC block cannot be polled reliably.
- `sdhci_transfer` sets `SMHC_CMD_SEND_INIT_SEQUENCE` when the command is CMD0.
- `sd_send_op_cond` only accepts ACMD41 when the transfer succeeded and the busy bit
  is set; a transport failure is retried inside the same bounded budget instead of
  being mistaken for a completed SD initialisation.
- `sdmmc_detect` stops the attempt if the reset times out; the stop/busy error paths
  reuse the polling reset.
- `sunxi_sdhci_init` logs the pre-reset `GCTRL`, `CLKCR`, `WIDTH`, `RINT` and
  `STATUS` once, so a cold-start log records what BROM left behind. If the SMHC clock
  gate is off, it logs that instead of touching a gated block.
- The retry budget, the 100 ms inter-attempt pause and the 2 ms post-CMD0 delay are
  unchanged. No timeout is extended to hide the failure.

Both changes are deliberate divergences from the pinned upstream, adopted from
mainline Linux because they are host-initialisation requirements, not card-protocol
policy. They are recorded in `research/sdhci-port-vs-upstream.md`.

## 5. Acceptance still to run

The cause above is consistent with the logs, but the fix is not confirmed until the
issue's HIL criterion is met: at least ten cold power-cycles with the card inserted
before power-on, each reaching `SD/MMC card found` and a successful block read, plus
hot-plug, re-exec and no-card checks. See `docs/agents/hil.md` for the procedure.

## Sources

1. `szemzoa/awboot` at `5380c00fc67c975433f25c57fb481aa2b91aebf8`:
   `arch/arm32/mach-t113s3/sunxi_sdhci.c` [525-645, 647-651];
   `arch/arm32/mach-t113s3/sdmmc.c` [186-241, 375-706, 725-763].
2. Linux `drivers/mmc/host/sunxi-mmc.c`: `sunxi_mmc_reset_host` (GCTRL polling) and
   `sunxi_mmc_request` (`SDXC_SEND_INIT_SEQUENCE` for `MMC_GO_IDLE_STATE`).
