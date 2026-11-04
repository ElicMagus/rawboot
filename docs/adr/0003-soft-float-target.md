# Target armv7a-none-eabi (soft-float), not the hard-float ABI

The original C uses `-mfloat-abi=hard`, but a bootloader does no floating-point maths, so
hard-float buys nothing. We measured both: `.text` is 22 067 B for `armv7a-none-eabi` (soft) versus
22 167 B for `armv7a-none-eabihf` (hard) — hard-float is 100 B *larger*, and it would force VFP
enablement and a hard-float ABI for no benefit. We therefore standardise on `armv7a-none-eabi`.
