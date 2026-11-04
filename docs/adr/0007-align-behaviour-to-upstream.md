# Align loader behaviour to upstream awboot, keep our own Rust expression

The loader's hardware logic is aligned to the *behaviour* of `szemzoa/awboot`, anchored to a pinned
upstream commit, not to its code: we match step order, constants, branch structure and lookup
tables, but keep our own expression under CC-BY-4.0 (ADR-0001). Names adopt upstream's
`snake_case` so the shared vocabulary carries over; types, traits, `Result`/`?` and register
accessors stay idiomatic Rust.

Out of scope: upstream's Linux/FDT/PSCI/secure-monitor/FatFs paths and non-T113-S3 SoC paths
(T113-S4, V851s), none of which our custom-firmware contract (ADR-0002) uses. Only drivers we
already have are realigned; upstream drivers we lack (DMA, SPI-NAND, watchdog, cache/exception
helpers) are deferred until a feature needs them. Parity is verified by hardware-in-the-loop smoke
on T113-S3 against the divergence checklist.

## Names

Upstream names apply to functions, constants, register fields and behaviour. Rust types, struct
fields and enum variants are ours: `SdhciCmd.idx` is the idiomatic Rust spelling of upstream
`cmd.idx`, and a wrapper field may be named for its meaning rather than copied from the C struct.
Renaming a field is not a parity violation when the port's type shape changes.

## Ticket boundary

A parity ticket may adapt the immediate callers of the driver it realigns to the new API, and fix
blocking bugs needed to make its own acceptance criteria testable. Full parity of those callers
stays with its own ticket; such adaptation is not scope creep.
