# FEL payload mailboxes live in SRAM, not DRAM

The FEL fast-iteration path lets the host upload both the loader and a payload image over USB,
without an SD card. The host writes a four-word mailbox before `xfel exec`; the loader reads it
after its own DRAM init and boots the staged image through the same placement, checksum and
`BootInfo` handoff as the SD path.

The mailbox and payload image live in the free part of FEL-mode SRAM (mailbox at `0x00044000`,
payload from `0x00039000`, optional NUL-terminated arguments in `0x00038000..0x00039000`), with
magic `RBFM`. Upstream
awboot places its Linux mailboxes at `0x43100000` in DRAM, after `xfel ddr`. We cannot do that:
the host must write the request before the loader runs, and the loader is the component that
initialises DRAM. Running `xfel ddr` first currently makes the loader's own DRAM init hang (issue
#23), so a DRAM mailbox would either require pre-initialised DRAM or a fix to that bug.

Consequences: the FEL payload image must fit in the roughly 44 KiB SRAM window, which is ample for
the blink image and quick tests; the SD path remains the primary boot medium for larger payloads.
`BootInfo.boot_medium` is `2` for this path (`1` remains SD), and `BootInfo.args` carries the
mailbox argument pointer or zero. Once DRAM initialisation is idempotent, a DRAM staging window can
be added behind a `BootInfo` version bump or a mailbox flag without changing the current contract.
