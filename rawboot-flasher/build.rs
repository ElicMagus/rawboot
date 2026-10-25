// SPDX-License-Identifier: Unlicense

use std::env;
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    for (bin, script) in [
        ("rawboot-ddr-init", "link-ddr.x"),
        ("rawboot-flasher", "link-flasher.x"),
    ] {
        let script = manifest.join(script);
        println!("cargo:rustc-link-arg-bin={bin}=-T{}", script.display());
        println!("cargo:rustc-link-arg-bin={bin}=--gc-sections");
        println!("cargo:rerun-if-changed={}", script.display());
    }

    Ok(())
}
