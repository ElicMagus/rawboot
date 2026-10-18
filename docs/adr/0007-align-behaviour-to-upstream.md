# Align loader behaviour to upstream awboot, keep our own Rust expression

The loader's hardware logic is aligned to the *behaviour* of `szemzoa/awboot`, anchored to a pinned
upstream commit, not to its code: we match step order, constants, branch structure and lookup
tables, but keep our own expression under Unlicense (ADR-0001). Names adopt upstream's
`snake_case` so the shared vocabulary carries over; types, traits, `Result`/`?` and register
accessors stay idiomatic Rust.

Out of scope: upstream's Linux/FDT/PSCI/secure-monitor/FatFs paths and non-T113-S3 SoC paths
(T113-S4, V851s), none of which our custom-firmware contract (ADR-0002) uses. Only drivers we
already have are realigned; upstream drivers we lack (DMA, SPI-NAND, watchdog, cache/exception
helpers) are deferred until a feature needs them. Parity is verified by hardware-in-the-loop smoke
on T113-S3 against the divergence checklist.
