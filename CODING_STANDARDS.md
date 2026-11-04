# Coding standards

Read this during code review. `scripts/check.sh` enforces the mechanical rules (format, clippy,
host tests, SPDX, branch docs, feature graph, smoke builds); this file is for the judgement calls
that a linter cannot decide. The repo's domain vocabulary is `GLOSSARY.md`; decisions are in
`docs/adr/`; the loader contract is `docs/spec-rawboot-v1.md`.

## Errors and status

- Preserve the failing stage. If the caller needs to act differently on create, truncate, write
  and flush, do not collapse them into one `failed` result.
- Return context the caller can print: which image, file, step or range failed.
- Error names are domain names (`LoaderNotSealed`, `FlashLayoutError::PartitionTooEarly`), not a
  bare string when the caller or tests branch on the case.
- No fallible side effect in `Drop`. If cleanup can fail, expose an explicit `flush`/`unmount`
  that returns the error. A best-effort `Drop` is allowed only when it is documented at the type
  and cannot change a success report.

## Addresses, sizes and constants

- Use the owning module's constant for addresses, signatures, sector sizes, clock values and
  limits. No literal `0x55aa`, `512`, `0x4000_0000` or clock number at a call site.
- Cross-crate contract addresses live in `rawboot-image` (`flash`, `fel`, `payload`); drivers use
  their own modules (`fat::partition`, `sdhci`, `sdcard`).
- A new default workspace member must not enable `sunxi-drivers/write`. Run
  `scripts/check-features.sh` before blaming the linker; the loader's 64 KiB SRAM budget is a hard
  contract.

## API shape

- More than two same-typed arguments, or a group that travels together, becomes a named
  request/plan struct (`FlashSdRequest`, `FlashPlan`). Positional slices invite transposition.
- Keep the `no_std`/no-alloc boundary explicit: caller-owned buffers, no hidden allocation in
  driver or flasher code.
- Prefer a shared constructor over repeating a struct literal; if a type has an invariant, the
  constructor is the place to state it.

## Writes and read-back

- A write is not successful until the written bytes are read back and compared. Report a short or
  broken write with the step that failed.
- Never write the MBR or a partition outside the intended target. A layout check runs before the
  first card write.
- Preserve order when a partial failure matters: payload files before the boot loader, MBR last
  (or never).

## Tests

- Extract a shared fixture or step-builder when two tests assemble the same sequence. A copy that
  must be edited twice is a bug waiting to happen.
- Test refusals before the device is touched: oversized input, unsealed loader, unsafe layout,
  missing DRAM.
- For byte contracts, test every field and every rejected bit: magic, version, size, checksum,
  truncation, alignment.
- Host tests cover pure logic and scripted transports; HIL covers the card, LED and cold boot.
  Do not claim a HIL result without the cold-start run.

## Contracts and docs

- When adding or renaming a step/result constant, update the name table, host printer, ADR list
  and runbook in the same change.
- Contract docs (`docs/spec`, `GLOSSARY`, ADR about public API or `default-members`) ride the code
  branch. Runbooks, README and AGENTS go directly to master.
- Keep one source of truth: link to the runbook instead of restating its procedure in AGENTS.

## Manual HIL

- Prefer one DDR-init stub run per cold boot: the stub re-runs DRAM init and is not idempotent,
  so a repeat on the same power cycle is not a verified path. HIL #46 ran three `bringup`s on one
  power cycle successfully; the condition table lives in `docs/agents/hil.md`. After a failed or
  successful `fel-flash`, power-cycle before retrying.
- Ask one question per physical step and include an explicit confirmation; do not batch cycles.
- Capture UART for the flasher path (`scripts/hil-flash.sh`); the status step alone does not name
  driver-level failures such as `CMD24` timeouts.
