# AGENTS

## Project

`rawboot` — a second-stage bootloader for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
written in 100% Rust and ported from `szemzoa/awboot`. It brings up the SoC, reads a raw payload
image from a FAT partition on an SD card over SDHCI, loads it into DRAM, and hands off through a
versioned `BootInfo` contract. The payload is our own firmware, not Linux (no FDT, no PSCI).

Cargo workspace with two members: `rawboot` (the loader) and `rawboot-rt` (the runtime). Target
`armv7a-none-eabi` (soft-float), Rust edition 2024.

Standard commands:

- Build: `cargo build --release`
- Test (host, tooling): `cargo test`
- Lint: `cargo clippy -- -D warnings` (the `no_std` crates have no host test
  target, so `--all-targets` cannot build the libtest harness on
  `armv7a-none-eabi`)
- Format: `cargo fmt`
- Seal an image: `cargo xtask image`

## Working agreements

- **Язык трекера и git-истории — русский.** Веди на русском языке заголовки и тела issue,
  комментарии в трекере и сообщения коммитов. Общение с владельцем проекта (в том числе вопросы,
  уточнения и комментарии в трекере) — тоже на русском.
- **Ветку с решением issue — пушь, PR — не создавай.** Решив задачу, создай ветку
  (`feat/<N>-<slug>` / `fix/<N>-<slug>`), закоммить и запушь её в `origin`. Pull request открывает
  только владелец проекта.
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

Single-context. Glossary at `GLOSSARY.md`, decisions at `docs/adr/`. See `docs/agents/domain.md`.
