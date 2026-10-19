// SPDX-License-Identifier: Unlicense

//! `cargo xtask` — host build tooling for rawboot.
//!
//! `image` builds the loader, converts the ELF to a flat binary with the
//! toolchain's `llvm-objcopy`, and eGON-seals it for BROM. The result is a
//! FEL/SD loader image, loadable with `xfel`.
//!
//! `payload` builds the payload crate and wraps its flat binary in the rawboot
//! image header, producing the image the loader parses.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rawboot_image::{DEFAULT_LOAD_ADDR, ImageOptions, SealOptions, seal};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The loader's target triple, matching the workspace default target.
const TARGET: &str = "armv7a-none-eabi";

/// FEL load address of the loader (matches `rawboot-rt`'s `RAM` origin).
const FEL_LOAD_ADDR: u32 = 0x0002_8000;

const USAGE: &str = "usage: cargo xtask image | cargo xtask payload";

fn main() {
    if let Err(err) = run() {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("image") => image(),
        Some("payload") => payload(),
        Some(other) => Err(format!("unknown command `{other}`\n{USAGE}").into()),
        None => Err(format!("missing command\n{USAGE}").into()),
    }
}

/// Build the loader and seal it into a FEL/SD image.
fn image() -> Result<()> {
    let root = workspace_root();
    let (elf, raw) = release_bin(&root, "rawboot");

    build_loader(&root)?;
    objcopy(&elf, &raw)?;

    let image = fs::read(&raw)?;
    let sealed = seal(&image, SealOptions::BLOCK)
        .map_err(|e| format!("sealing `{}`: {e:?}", raw.display()))?;
    xtask::verify_sealed_egon(&sealed)
        .map_err(|e| format!("assembled loader image is not BROM-valid: {e}"))?;

    let sealed_path = root.join("target").join("rawboot-fel.bin");
    fs::write(&sealed_path, &sealed)
        .map_err(|e| format!("writing `{}`: {e}", sealed_path.display()))?;
    write_build_stamp(&root, &sealed_path)?;

    println!("sealed {} bytes -> {}", sealed.len(), sealed_path.display());
    println!("load: {}", fel_load_command(&sealed_path));
    Ok(())
}

/// Record the tree the FEL image came from, so `hil-check.sh` can refuse an
/// image that no longer matches `HEAD` (for example after switching branches).
fn write_build_stamp(root: &Path, image: &Path) -> Result<()> {
    let commit = git_line(root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let branch =
        git_line(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = match git_line(root, &["status", "--porcelain"]) {
        Some(status) => (!status.is_empty()).to_string(),
        None => "unknown".into(),
    };
    let stamp = format!("commit={commit}\nbranch={branch}\ndirty={dirty}\n");

    let stamp_path = build_stamp_path(image);
    fs::write(&stamp_path, stamp)
        .map_err(|e| format!("writing `{}`: {e}", stamp_path.display()))?;
    Ok(())
}

fn build_stamp_path(image: &Path) -> PathBuf {
    PathBuf::from(format!("{}.build", image.display()))
}

fn git_line(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn fel_load_command(image: &Path) -> String {
    format!(
        "xfel write {FEL_LOAD_ADDR:#010x} {} && xfel exec {FEL_LOAD_ADDR:#010x}",
        image.display()
    )
}

fn build_loader(root: &Path) -> Result<()> {
    build_bin(root, "rawboot")
}

/// Build the payload crate and wrap its flat binary in the rawboot image header.
fn payload() -> Result<()> {
    let root = workspace_root();
    let (elf, raw) = release_bin(&root, "payload");

    build_bin(&root, "payload")?;
    objcopy(&elf, &raw)?;

    let raw_bytes = fs::read(&raw)?;
    let image = xtask::wrap_payload(&raw_bytes, ImageOptions::default())
        .map_err(|e| format!("wrapping `{}`: {e}", raw.display()))?;

    let image_path = root.join("target").join("rawboot-payload.img");
    fs::write(&image_path, &image)
        .map_err(|e| format!("writing `{}`: {e}", image_path.display()))?;

    println!(
        "wrapped {} bytes -> {} (load/entry {DEFAULT_LOAD_ADDR:#010x})",
        image.len(),
        image_path.display()
    );
    Ok(())
}

/// The release ELF of a workspace binary and the flat binary `objcopy` produces.
fn release_bin(root: &Path, bin: &str) -> (PathBuf, PathBuf) {
    let out_dir = root.join("target").join(TARGET).join("release");
    (out_dir.join(bin), out_dir.join(format!("{bin}.bin")))
}

/// Build a workspace binary for the loader's target.
fn build_bin(root: &Path, bin: &str) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--bin", bin, "--target", TARGET])
        .status()
        .map_err(|e| format!("running cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build failed: {status}").into());
    }
    Ok(())
}

fn objcopy(elf: &Path, raw: &Path) -> Result<()> {
    let status = Command::new(llvm_objcopy()?)
        .args(["-O", "binary"])
        .arg(elf)
        .arg(raw)
        .status()
        .map_err(|e| format!("running llvm-objcopy: {e}"))?;
    if !status.success() {
        return Err(format!("llvm-objcopy failed: {status}").into());
    }
    Ok(())
}

/// Locate `llvm-objcopy` in the active toolchain's `llvm-tools` component, so
/// the build works on any host without a separately installed `cargo-binutils`.
fn llvm_objcopy() -> Result<PathBuf> {
    let sysroot = rustc_output(&["--print", "sysroot"])?;
    let version = rustc_output(&["-vV"])?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc -vV did not report a host triple")?;
    let bin = Path::new(sysroot.trim())
        .join("lib")
        .join("rustlib")
        .join(host)
        .join("bin");
    for name in ["rust-objcopy", "llvm-objcopy"] {
        let candidate = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "no rust-objcopy in {}; install the `llvm-tools` rustup component",
        bin.display()
    )
    .into())
}

fn rustc_output(args: &[&str]) -> Result<String> {
    let output = Command::new("rustc")
        .args(args)
        .output()
        .map_err(|e| format!("running rustc {args:?}: {e}"))?;
    if !output.status.success() {
        return Err(format!("rustc {args:?} failed: {}", output.status).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// The workspace root: the `xtask` crate's parent directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate lives in a workspace")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fel_load_command_does_not_run_xfel_ddr() {
        let command = fel_load_command(Path::new("target/rawboot-fel.bin"));

        assert!(!command.contains("xfel ddr"), "{command}");
        assert!(command.contains("xfel write 0x00028000"), "{command}");
        assert!(command.contains("xfel exec 0x00028000"), "{command}");
    }

    #[test]
    fn build_stamp_sits_next_to_the_image() {
        assert_eq!(
            build_stamp_path(Path::new("target/rawboot-fel.bin")),
            PathBuf::from("target/rawboot-fel.bin.build")
        );
    }
}
