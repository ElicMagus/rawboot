# Port awboot to 100% Rust under Unlicense

`szemzoa/awboot` is a C bootloader for the T113-S3; our successor is a clean 100% Rust rewrite
(no C, not even for DRAM init, which is already in Rust). We transcribe only *facts* from awboot
and the datasheet — addresses, register offsets, bit positions, constant names — and write the
behaviour ourselves, idiomatically (own types, traits, `&raw` accessors). The port is released as
public domain under `Unlicense`, so we deliberately avoid copying code bodies from awboot's
GPL-2.0+/GPL-2.0-only and unlicensed files, which our own licence would not make safe.
