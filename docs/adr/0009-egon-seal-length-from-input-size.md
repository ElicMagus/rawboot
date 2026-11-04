# eGON sealer derives the checksum length from the input size, not the header field

`seal` (`rawboot-image`) computes the padded image length from the actual byte length of the loader
binary and writes that value into the eGON header's `length` field before summing the seeded
checksum. Upstream `mksunxi` instead reads `length` from the header it was given, aligns it, and
sums that many words. The two agree only when the header already holds the true size.

We diverge on purpose. Our runtime lays down a fixed placeholder header (`length = 32768`) and the
real loader image already exceeds it (the sealed FEL loader is about 57 KiB), so trusting the header
field would sum and declare fewer bytes than BROM must load — the tail of the image would be
ignored. Deriving the length from the buffer cannot get out of step with the bytes actually
present, and BROM still accepts the result because it sums exactly the `length` the header ends up
declaring — the value `seal` itself wrote.

This is a deliberate exception to the parity rule of ADR-0007: the observable behaviour (a
BROM-valid image) matches upstream, the algorithm's input does not. The alternative — embedding the
real size in `rawboot-rt` through a linker symbol, as upstream `start.S` does with `__spl_size`, and
summing the header field — was rejected as more machinery for no observable difference. If a second
sealer or a flash tool ever has to produce byte-identical output to `mksunxi`, revisit this.

Consequences: `seal` never reads the header's `length`; callers must not treat the placeholder as
meaningful. The eGON header layout and its constants stay the single shared definition in
`rawboot-image::egon`, embedded by `rawboot-rt`.
