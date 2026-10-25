// SPDX-License-Identifier: Unlicense

//! Selects the loader's link script.
//!
//! The default image is linked for FEL (`0x00028000`); the `boot-sd` feature
//! links the same loader for BROM's SD/SPI origin (`0x00020000`). Both scripts
//! are written to the runtime crate's `OUT_DIR`, whose link search path the
//! runtime build script adds to the final link.

fn main() {
    let script = if std::env::var_os("CARGO_FEATURE_BOOT_SD").is_some() {
        "link-boot.x"
    } else {
        "link.x"
    };
    println!("cargo:rustc-link-arg=-T{script}");
}
