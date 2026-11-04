// SPDX-License-Identifier: CC-BY-4.0

use std::{env, error::Error, fs::File, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");

    File::create(out_dir.join("link.x"))?.write_all(include_bytes!("link.x"))?;

    Ok(())
}
