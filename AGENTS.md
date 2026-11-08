# AGENTS

## Project

`rawboot` — a second-stage bootloader for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
written in 100% Rust and ported from `szemzoa/awboot`. It brings up the SoC, reads a raw payload
image from a FAT partition on an SD card over SDHCI, loads it into DRAM, and hands off through a
versioned `BootInfo` contract. The payload is our own firmware, not Linux (no FDT, no PSCI).

Cargo workspace with nine members: `rawboot` (the loader), `rawboot-rt` (the runtime),
`sunxi-pl` (the shared platform layer: MMIO, GPIO, timer), `sunxi-drivers` (the shared
driver crate above `sunxi-pl`: clock, DRAM, UART, SDHCI, SD/MMC, FAT), `payload` (the blink
test payload image the loader boots), `rawboot-image` (host-testable payload/eGON, SD boot
prefix and flash job/status tooling), `rawboot-fel` (host-side FEL USB transport and CLI),
`rawboot-flasher` (the FEL-callable DDR-init stub and DRAM-linked flasher) and `xtask` (host
build tooling). `fatfs`'s `no_std` `core_io` dependency is patched to `vendor/core_io`, a
minimal stable-compatible shim. Target `armv7a-none-eabi` (soft-float), Rust edition 2024.

`embed/` is an ignored snapshot of the older Rust port kept for pre-port comparison; the
pinned C upstream is fetched by `scripts/fetch-upstream.sh` into `target/upstream/`.
Explore the workspace crates, not those trees.

Search: use the agent's Grep/Glob tools; `rg` is not installed on this host.

Standard commands:

- Build: `cargo build --release` (`xtask` and its host-only image tooling are kept out of the
  bare-metal `default-members`; `rawboot-image` is still built as the loader's no-alloc
  dependency. `rawboot-flasher` is also out of `default-members`: it enables
  `sunxi-drivers/write`, which would unify into the loader; `cargo xtask flasher` builds it)
- Test (host, tooling): `cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu`
  (the workspace default target is `armv7a-none-eabi`, so host tests need an explicit
  `--target`; the package scope keeps the hardware-only bare-metal members out of the host
  build. `sunxi-drivers` has host unit tests for the block adapter and FAT writes over a
  mock disk; they need no board.)
- Lint: `cargo clippy -- -D warnings` (the `no_std` crates have no host test
  target, so `--all-targets` cannot build the libtest harness on
  `armv7a-none-eabi`; `sunxi-drivers` is not a default member, so its two feature sets
  are linted explicitly with `cargo clippy -p sunxi-drivers -- -D warnings` and
  `cargo clippy -p sunxi-drivers --no-default-features -- -D warnings`; the flasher is
  linted separately with `cargo clippy -p rawboot-flasher -- -D warnings` because it is
  not a default member and enables the write feature; the host tooling and the
  host-testable driver tests are linted with
  `cargo clippy -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu --all-targets -- -D warnings`;
  the `core_io` shim is linted with
  `cargo clippy --manifest-path vendor/core_io/Cargo.toml --target-dir target/core_io-shim -- -D warnings`)
- Format: `cargo fmt` (CI runs `cargo fmt --all -- --check`)
- Check translations: `scripts/check-i18n.sh` (every public Markdown file has RU and
  zh-CN mirrors under `i18n/`, and their relative links resolve)
- Disassemble: `scripts/objdump.sh -d target/armv7a-none-eabi/release/rawboot` (wraps
  `llvm-objdump` from the `llvm-tools` component; there is no `cargo objdump`)
- Fetch pinned upstream: `scripts/fetch-upstream.sh` (writes the C tree used by the parity
  research notes to `target/upstream/awboot-<sha>/`)
- Check everything: `scripts/check.sh` (fmt, the early `scripts/check-features.sh` feature-graph
  guard, build, the bare-metal, flasher, driver and host clippy passes, host tests, SPDX headers,
  branch-docs rule, i18n mirrors, the loader no-write symbol guard, `cargo xtask image`/`payload`/`flasher`
  smoke builds, `cargo xtask fel-flash --dry-run`, and `cargo xtask sd-image`)
- Seal an image: `cargo xtask image` (builds the loader twice and writes the sealed
  `target/rawboot-fel.bin` for FEL at `0x00028000` and `target/rawboot-boot-sd.bin` for
  SD/SPI boot at `0x00020000`; the `xtask` alias pins the host triple in `.cargo/config.toml`)
- Publish the public repository: rewritten `master` history through `v0.1.0` plus the tag goes to
  GitHub (ADR-0016); the old single-commit `scripts/export-public.sh` is retired. The one-time
  migration and the manual fix-publish procedure live in `docs/agents/export-public.md`
- Build the payload image: `cargo xtask payload` (builds the `payload` crate and writes
  `target/rawboot-payload.img`, its flat binary wrapped in the payload image header)
- Build the FEL flasher images: `cargo xtask flasher` (writes `target/rawboot-ddr.bin`, the
  returning DDR-init stub at `0x00028000`, and `target/rawboot-flasher.bin`, linked at
  `0x40010000` in DRAM; both carry no eGON header; ADR-0013)
- Flash the card in the board: `cargo xtask fel-flash [--payload <path>] [--boot-cfg <path>]
  [--loader <path>] [--flasher <path>] [--ddr-stub <path>] [--sequence <n>] [--skip-build]
  [--dry-run] [--yes]` (builds or reuses the images, checks the sealed SD loader and refuses the
  FEL variant, writes the payload files and loader over FEL without touching the MBR, and reads
  everything back. `--dry-run` validates and prints the plan without opening FEL; actual writing
  requires `--yes`; ADR-0014; runbook `docs/agents/hil.md`)
- FEL CLI: `cargo fel version|read|write|exec|bringup ...` (host-side FEL transport over USB
  with no system `libusb`; `version` identifies the T113-S3, `read <addr> <len>` hex-dumps or
  writes to `--output`, `write <addr> <file>` uploads bytes, `exec <addr>` jumps, `bringup
  <ddr-stub> <flasher>` runs the returning stub, checks a DRAM pattern and reads the flasher
  status; the `fel` alias pins the host triple)
- Build the SD boot prefix: `cargo xtask sd-image --partition-sectors <n> [--start-lba <n>]
  [--loader <path>]` (after `cargo xtask image`; writes `target/rawboot-sd-mbr.img`, an MBR
  pointing at the existing FAT partition plus the SD loader at sector 16, for `dd conv=notrunc`
  onto a card; it formats nothing and copies no payload; ADR-0011)
- Prepare an SD card: `scripts/flash-sd.sh [--device /dev/sdX] [--copy-payload] [--format]`
  (run as your user, not via `sudo`; auto-detects a single removable/USB/MMC disk, `--list` shows
  candidates, builds and writes the boot prefix, optionally formats/repopulates only the
  partition; runs `--dry-run` without touching anything; refuses mounted, root, GPT or
  multi-partition disks; runbook `docs/agents/hil.md`)
- HIL: `docs/agents/hil.md` is the authoritative runbook. After `code-review`, on the code
  branch: `scripts/hil-check.sh target/rawboot-fel.bin <label>`; SD flakiness (#43) ->
  `scripts/hil-sd-retry.sh`; in-board card update -> `scripts/hil-flash.sh` (one cold start per
  attempt: a repeat DDR-init stub can hang; see the condition table in the runbook). HIL #42's
  CMD24 blocker (#47) is fixed (PR #49). `scripts/check.sh` covers host checks only.

## CI

`.gitea/workflows/ci.yml` runs `scripts/check.sh` on every push and pull request. It needs an
`act_runner` registered with the `ubuntu-latest` label; until one is, run the script by hand before
pushing.

## Working agreements

- **Git history is Russian; the public tracker is bilingual.** Write issue titles, bodies,
  comments and commit messages in Russian. When publishing to the GitHub archive, put the English
  translation on top and the Russian original in `<details>`. Address the project owner (including
  questions, clarifications and tracker comments) in Russian.
- **The public GitHub repository mirrors the real history (ADR-0016).** Gitea remains the
  canonical tracker and source of truth; GitHub receives the `master` history with tags, plus all
  code, environment and decisions. `.gitea/`, `scripts/export-public.sh`, `scripts/issue-view.sh`,
  `target/` and `embed/` stay internal. Unmerged branches are not published: merge first, then
  publish them in a separate manual pass.
- **Documentation: English is the primary source.** English files live at their current paths;
  translations mirror the tree under `i18n/ru/` and `i18n/zh-CN/` with the same relative paths. The
  agent reads and edits only English files, never reads translations, and English wins any
  disagreement. API and register names and glossary terms are not translated; the agent drafts
  translations on its own, without a separate review.
- **Push the branch that solves an issue; do not open a pull request.** Once the task is solved,
  create a branch (`feat/<N>-<slug>` / `fix/<N>-<slug>`), commit and push it to `origin`. Only the
  project owner opens pull requests. Commit tooling changes that have no issue, including retros,
  straight to `master`; a separate branch is needed only for an issue or when a PR is requested.
- **Commit docs directly, without a branch.** Commit ADRs, `GLOSSARY.md`, `research/` and `docs/` to
  the current branch (usually `master`); a branch (`feat/`/`fix/`) is for code. Exception: docs
  without which a merge would change the contract (spec/GLOSSARY/ADR about a module boundary, a
  public API or `default-members`) go on the code branch. Before committing, check new docs against
  earlier ADRs, the glossary and the code; if you find a contradiction, run the grilling
  (`grill-with-docs`) first, then commit.
- **Review before pushing — the `code-review` skill (Standards + Spec).**
- **HIL and docs order.** Verification instructions (runbook, commands) may be committed before
  HIL; claims about results (PASS, confirmed LED) only after HIL confirmation.
- **Drive manual HIL steps as a dialogue.** If a test needs a human action (power-cycle, insert the
  card, FEL/BOOT, confirm the LED), ask one question through the `question` tool before each step or
  cycle and wait for the answer. Do not start the next cycle or move to the next step without
  confirmation; do not merge several cycles into one question. In the question give the exact
  physical procedure and the expected result; after the run, give PASS/FAIL, the key log lines and
  the next step. In one `question` call ask two tab questions: the step itself and a control
  "Sure?" with the options "Yes" and "No". Perform the action only if the step is confirmed and
  "Sure?" is "Yes"; otherwise return to the step question. This guards against an accidental click.
- **After a push, give the verification instructions** — build steps, FEL and/or SD boot, and what
  to observe on the payload LED.
- **List the issue's acceptance criteria before merging.** An unmet item either keeps the issue open
  or moves to a follow-up issue with a native blocking link and an explicit note. A PASS of one part
  (for example reading) does not replace a PASS of another (writing).
- **After confirmation, merge the branch into `master`, close the issue and delete the branch.**
- **File discovered defects as a separate issue** labelled `needs-triage`.

## Agent skills

### Issue tracker

Issues live in Gitea at `ElicMagus/rawboot-internal`. See `docs/agents/issue-tracker.md`.

### Triage labels

Two category + five state roles, mapped to this repo's label strings. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. Spec at `docs/spec-rawboot-v1.md`, research notes at `research/`, glossary at
`GLOSSARY.md`, decisions at `docs/adr/`. See `docs/agents/domain.md`.

Reviewer judgement calls live in `CODING_STANDARDS.md`; read it during `code-review`.
