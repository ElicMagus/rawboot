# SD boot prefix: MBR and loader in sector 16, without its own FAT

BROM loads the SD/SPI image from sector 16 to address `0x00020000`, and the loader reads
`payload.bin` from the FAT partition. There was no host tool that placed the loader on the card:
`cargo xtask image` produced only sealed images, and the card layout was assembled by hand. During
bring-up #37 a working prefix was produced: 1 MiB (2048 sectors), an MBR with a single entry of type
`0x0c` at LBA 2048 with a size of 1843200 sectors, disk signature `0`, CHS `FE FF FF`, and the
loader at offset 8192. It is not reproducible.

Decision: `cargo xtask sd-image --partition-sectors <n> [--start-lba <n>] [--loader <path>]` builds
`target/rawboot-sd-mbr.img` — the **SD boot prefix**. It is a file of exactly `--start-lba` sectors
(2048 by default, that is 1 MiB), filled with zeros except for two regions:

- sector 0 — the MBR: disk signature `00000000`, a single entry at 446 with boot flag `0`, type
  `0x0c`, CHS start/end `FE FF FF`, start LBA `--start-lba` and sector count `--partition-sectors`;
  signature `55 AA` at 510;
- offset 8192 (sector 16) — the bytes of the sealed loader `target/rawboot-boot-sd.bin` as is.

The prefix contains no FAT, no BPB and no payload. The MBR only references the already existing FAT
partition by its address; formatting and copying the payload remain external tools. `cargo xtask
sd-image` writes a file, not a block device; it is placed on the card with `dd ... conv=notrunc`.

`cargo xtask sd-image` remains a file tool. For HIL there is `scripts/flash-sd.sh`: it invokes the
xtask, finds the disk and partition itself, refuses to write to a mounted disk, a disk holding the
root filesystem, a GPT/multi-partition card or a card without an MBR, checks the sizes and writes
the prefix via `dd`. With flags it formats only the partition (`mkfs.vfat -F 32`) and copies the
payload. Work with the block device, permissions and `sudo` lives in the shell wrapper, not in the
xtask: the build tool stays testable and requires no privileges.

## Considered Options

- **SD boot prefix** (accepted): the card keeps its existing partition and its `payload.bin`; the
  artifact is small (1 MiB), is built on the host and is checked by host tests byte by byte. The
  prefix size and partition address are set by flags, so the tool does not hard-code a particular
  card. The risk is writing to the wrong device; it is limited by the HIL wrapper's checks
  (mounted state, root disk, MBR, single partition) and by confirmation before writing. Writing
  overwrites the first MiB of the medium, so only `conv=notrunc` and only onto a card with an
  already prepared partition.
- **A full image with an empty FAT** (rejected): the tool would have to format FAT and create a
  BPB, that is, duplicate `mkfs.vfat`. With a partition of 1843200 sectors the image would weigh
  about 900 MiB, be rebuilt after every payload change and require a separate size for each card.
  Formatting and copying the payload is not the bootloader's job.
- **Writing directly to a block device** (rejected for the xtask): the xtask would have to be given
  device discovery, permission checks and platform logic; a wrong disk choice is irreversible. A
  file, by contrast, can be checked, compared and written with any familiar tool, including `dd`
  under the owner's control. For HIL this role is taken by a separate shell wrapper
  `scripts/flash-sd.sh`, which invokes the xtask and adds device checks; the xtask itself remains
  file-based.

## Consequences

- `build_sd_image` lives in `rawboot-image` and is checked by host tests: the MBR bytes, loader
  placement, prefix length, rejection of an empty or invalid eGON, zero sizes, overflow and the
  loader running into the partition. Sealed eGON validation moved into
  `rawboot-image::egon::verify_sealed`; it is called by both `cargo xtask image` and `cargo xtask
  sd-image`.
- `cargo xtask sd-image` requires `--partition-sectors` (the real partition size) and is invoked in
  `scripts/check.sh` as a smoke build.
- HIL runbook: `dd` the prefix onto a card with an existing FAT partition, run `mkfs.vfat` and copy
  the payload only on the partition. Formatting the whole device destroys the loader in sector 16.
- `GLOSSARY.md` gains the term "SD boot prefix" so the artifact is not confused with a full disk
  image.
