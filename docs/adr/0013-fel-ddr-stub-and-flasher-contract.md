# FEL flasher: a returning DDR-init stub and a job/status contract in DRAM

The FEL SRAM window (roughly 44 KiB for the image) is smaller than the sealed loader, and the
flasher's data does not fit in SRAM. DRAM cannot be initialized externally via `xfel ddr`: after it
the loader re-initializes DRAM and hangs it on PLL_DDR (#23); this statement is about `xfel ddr`, not
about the returning stub. So the host must obtain DRAM itself by loading a small stub that
initializes DRAM and **returns to BROM**, leaving FEL alive. After that the host places a job and
the flasher into DRAM, executes it and reads the status; SD is not touched yet in this task (#41).

Decision: two separate images without an eGON header, a shared contract in `rawboot-image` and a
host API in `rawboot-fel`.

## Addresses and windows

| What | Address | Size |
| --- | --- | --- |
| DDR-init stub in SRAM | `0x00028000` | `0x5000` (20 KiB) |
| Stub result (magic + MiB) | `0x0002d000` | 8 bytes |
| `FlashJob` in DRAM | `0x40000000` | 36 bytes |
| `FlashStatus` in DRAM | `0x40000100` | 28 bytes |
| DRAM pattern check | `0x40002000` | 4 KiB |
| Flasher in DRAM | `0x40010000` | up to `0x3c000` (240 KiB) |
| Flasher stack | `0x4004c000..0x40050000` | 16 KiB, grows downward |

The stub window ends exactly where the result begins; the actual image is currently about 9 KiB.
The flasher window is 256 KiB, but the host accepts an image of no more than 240 KiB: the top
16 KiB are reserved for the stack, and the linker checks that `.bss` does not run into it. The
result address is chosen inside the range occupied by the external `xfel`'s DDR payload
(`0x28000..0x2de54`), in which BROM has been proven to leave its stack intact: the stub returns and
the next FEL request is answered. The flasher gets its own stack in DRAM and saves BROM's `sp`/`lr`
on entry, so its size is not limited by the BROM stack.

## The job/status contract

`FlashJob` and `FlashStatus` are `#[repr(C)]` structures made of `u32` fields only, but the
contract is the little-endian byte layout, not the Rust structure. Both blocks have:

- `magic` (`RBFJ` / `RBFS`), `version` (`1`) and `size` (36 / 28);
- `checksum` — the sum of the little-endian 32-bit words of the whole block with the checksum field
  zeroed out;
- `sequence` — a monotonic task number so the host does not accept a foreign/stale status.

The job further has `command`, `arg0..arg2`; the status has `step` and `result`. In #41 the `NOOP`
command is implemented, with status `DONE`/`OK` (or `BAD_JOB`/`UNKNOWN_COMMAND`); SD write commands
will appear in #42 without changing the layout. `rawboot-image::flash` is checked by host tests:
round-trip, every corrupted part (magic, version, size, checksum, truncation) and the absence of
address overlaps (`const` asserts).

## Images

`rawboot-flasher` is a separate bare-metal package with two `[[bin]]` targets:

- `rawboot-ddr-init` links into SRAM at `0x28000`, calls the shared `sunxi_drivers::dram` and
  returns to BROM; the result (magic `RBDD`, size in MiB) remains in SRAM. The image's BSS is
  zeroed before the driver call; the stack is BROM's, as with the external `xfel`'s DDR payload.
- `rawboot-flasher` links into DRAM at `0x40010000`, has NOLOAD sections `.bss` and `.stack`,
  switches to the DRAM stack, reads the job, writes the status and returns to BROM after restoring
  the stack.

Neither image carries an eGON header or is a second stage. `cargo xtask flasher` builds both,
checks the VMA of `.text` against the contract addresses, the absence of `eGON` at the start of the
flat binary and the window size. The build is part of `scripts/check.sh`.

## Non-return and panic

Both images return to BROM only on normal completion. On panic the `#[panic_handler]` parks:
control does not return to BROM, the FEL connection stops responding, and the next host request
hits a transport timeout. This is the observable fault signal. Writing `FlashStatus` from the panic
handler is pointless: the host reads the status only after returning from `exec`, and a parked
image no longer hands control back to BROM. The host does not try to distinguish a panic from
another non-return; the reaction is the same — a cold start and retry.

## Host

`rawboot-fel` gains `FelDevice::{run_ddr_stub, verify_dram, run_flash_job, bringup}`; CLI:

```sh
cargo xtask flasher
cargo fel bringup target/rawboot-ddr.bin target/rawboot-flasher.bin
```

`bringup` identifies the T113, runs the stub, reads its result, writes and re-reads a pattern in
DRAM, places a NOOP job and the flasher, executes it and checks the status's
magic/version/size/checksum and `sequence`. If the stub did not return, the next FEL read hits a
transport timeout: this is the go/no-go signal, not a silent fallback.

## Considered Options

- **A returning stub + DRAM job/status** (accepted): FEL stays available, the data is not limited
  to 44 KiB, the host API is reused in #42, and the contract is tested without a board.
- **`xfel ddr`** (rejected): an external tool; after it the loader re-initializes DRAM and hangs on
  PLL_DDR (#23), and this is not our architecture. This does not apply to re-running the returning
  stub; its conditions are described in ADR-0014 and the runbook.
- **A single non-returning flasher** (rejected): the status would have to be served by a
  device-side USB/FEL server, which is outside the task.
- **The job in SRAM** (rejected): after the stub the host has DRAM anyway; the SRAM window must be
  preserved for the stub and for compatibility with the loader's FEL path.
- **Writing SD already now** (rejected): a go/no-go spike first; SD/FAT is #42.

## Consequences

- The new bare-metal package `rawboot-flasher` was part of `default-members` at the time of the
  decision, so `cargo build --release`, `cargo clippy` and `cargo xtask flasher` checked both
  images in the normal set; later it was moved out of there (ADR-0014), and `cargo xtask flasher`
  builds it with a separate cargo invocation.
- `rawboot-image` publishes the addresses and `FlashJob`/`FlashStatus`; `rawboot-fel` depends on
  them.
- `rawboot-fel` gains the `bringup` command and errors for the stub, the DRAM pattern, the contract
  and the sequence.
- The loader, its FEL mailbox and the SD path are unchanged.
- HIL: the stub must return, the DRAM pattern must match, and `bringup` must print the DRAM size and
  `result 0 (ok)`. If the stub does not return, the fact is recorded in #41, and the task returns to
  triage; falling back to `xfel ddr` is not considered a solution.
- `GLOSSARY.md` gains "DDR-init stub", "flasher" and "flash job/status".

> Update #42: the SD command was added without changing the job/status layout, but with a separate
> `FlashPlan` and staging areas in DRAM; `rawboot-flasher` is no longer part of
> `default-members`, so that `sunxi-drivers/write` is not unified into the loader. See ADR-0014.
> The composition of `default-members`, the `write` feature and the place of the driver host tests
> are clarified in ADR-0015.
