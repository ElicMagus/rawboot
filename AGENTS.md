# AGENTS

## Project

`rawboot` — a second-stage bootloader for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
written in 100% Rust and ported from `szemzoa/awboot`. It brings up the SoC, reads a raw payload
image from a FAT partition on an SD card over SDHCI, loads it into DRAM, and hands off through a
versioned `BootInfo` contract. The payload is our own firmware, not Linux (no FDT, no PSCI).

Cargo workspace with five members: `rawboot` (the loader), `rawboot-rt` (the runtime),
`payload` (the stub payload image the loader boots), `rawboot-image` (host-testable
payload/eGON image tooling) and `xtask` (host build tooling).
Target `armv7a-none-eabi` (soft-float), Rust edition 2024.

`embed/` is an ignored snapshot of the older Rust port kept for pre-port comparison; the
pinned C upstream is fetched by `scripts/fetch-upstream.sh` into `target/upstream/`.
Explore the workspace crates, not those trees.

Standard commands:

- Build: `cargo build --release` (`xtask` is a host tool, kept out of the bare-metal
  `default-members`, so it is not part of this build)
- Test (host, tooling): `cargo test -p rawboot-image -p xtask --target x86_64-unknown-linux-gnu`
  (the workspace default target is `armv7a-none-eabi`, so host tests need an explicit
  `--target` and the package scope keeps the bare-metal members out of the host build)
- Lint: `cargo clippy -- -D warnings` (the `no_std` crates have no host test
  target, so `--all-targets` cannot build the libtest harness on
  `armv7a-none-eabi`; the host tooling is linted with
  `cargo clippy -p rawboot-image -p xtask --target x86_64-unknown-linux-gnu --all-targets -- -D warnings`)
- Format: `cargo fmt` (CI runs `cargo fmt --all -- --check`)
- Disassemble: `scripts/objdump.sh -d target/armv7a-none-eabi/release/rawboot` (wraps
  `llvm-objdump` from the `llvm-tools` component; there is no `cargo objdump`)
- Fetch pinned upstream: `scripts/fetch-upstream.sh` (writes the C tree used by the parity
  research notes to `target/upstream/awboot-<sha>/`)
- Check everything: `scripts/check.sh` (fmt, build, both clippy passes, host tests, SPDX
  headers, branch-docs rule, and `cargo xtask image`/`payload` smoke builds)
- Seal an image: `cargo xtask image` (builds the loader and writes the sealed
  `target/rawboot-fel.bin`; the `xtask` alias pins the host triple in `.cargo/config.toml`)
- Build the payload image: `cargo xtask payload` (builds the `payload` crate and writes
  `target/rawboot-payload.img`, its flat binary wrapped in the payload image header)
- HIL: `scripts/hil-check.sh target/rawboot-fel.bin <label>` loads the image over FEL, captures
  USART0 and asserts the boot log; run it after `code-review`, on the code branch, before
  committing docs or merging. The script checks the image build stamp against `HEAD` and
  requires a quiet UART (board idle in FEL). Set `HIL_REQUIRE_SD=1` when the ticket also
  accepts the card path. Runbook: `docs/agents/hil.md`. The loader
  initialises DRAM itself, so the load is `xfel write 0x00028000 ... && xfel exec 0x00028000`
  with no separate DDR step.

## CI

`.gitea/workflows/ci.yml` runs `scripts/check.sh` on every push and pull request. It needs an
`act_runner` registered with the `ubuntu-latest` label; until one is, run the script by hand before
pushing.

## Working agreements

- **Язык трекера и git-истории — русский.** Веди на русском языке заголовки и тела issue,
  комментарии в трекере и сообщения коммитов. Общение с владельцем проекта (в том числе вопросы,
  уточнения и комментарии в трекере) — тоже на русском.
- **Ветку с решением issue — пушь, PR — не создавай.** Решив задачу, создай ветку
  (`feat/<N>-<slug>` / `fix/<N>-<slug>`), закоммить и запушь её в `origin`. Pull request открывает
  только владелец проекта.
- **Доки коммить прямо, без ветки.** ADR, `GLOSSARY.md`, `research/` и `docs/` коммить в текущую
  ветку (обычно `master`); ветка (`feat/`/`fix/`) — для кода. Перед коммитом сверь новые доки со
  старыми ADR, глоссарием и кодом; нашёл противоречие — сначала грилинг (`grill-with-docs`), потом
  коммит.
- **Перед пушем прогони ревью — skill `code-review` (Standards + Spec).**
- **После пуша выдавай инструкцию по проверке** — шаги сборки, загрузки через FEL и/или SD и
  наблюдения за LED payload'а.
- **После подтверждения — влей ветку в `master`, закрой issue, удали ветку.**
- **Найденные недостатки оформляй отдельным issue** с меткой `needs-triage`.

## Agent skills

### Issue tracker

Issues live in Gitea at `ElicMagus/rawboot-internal`. See `docs/agents/issue-tracker.md`.

### Triage labels

Two category + five state roles, mapped to this repo's label strings. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. Spec at `docs/spec-rawboot-v1.md`, research notes at `research/`, glossary at
`GLOSSARY.md`, decisions at `docs/adr/`. See `docs/agents/domain.md`.
