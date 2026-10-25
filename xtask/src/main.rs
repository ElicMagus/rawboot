// SPDX-License-Identifier: Unlicense

//! `cargo xtask` — host build tooling for rawboot.
//!
//! `image` builds the loader twice, converts each ELF to a flat binary with the
//! toolchain's `llvm-objcopy`, and eGON-seals it for BROM: the FEL variant at
//! `0x00028000` and the SD/SPI boot variant at `0x00020000`.
//!
//! `payload` builds the payload crate and wraps its flat binary in the rawboot
//! image header, producing the image the loader parses.
//!
//! `sd-image` wraps the sealed SD/SPI loader in an MBR boot prefix that points at
//! an existing FAT partition, so sector 16 holds a loader BROM can start without
//! disturbing the partition or its `payload.bin`.
//!
//! `fel-flash` builds or reuses those artifacts, checks the SD loader, and writes
//! the payload files and loader to a card left in the board over FEL.

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use rawboot_fel::FelDevice;
use rawboot_image::{
    DDR_RESULT_ADDR, DDR_STUB_LOAD_ADDR, DDR_STUB_MAX_SIZE, DEFAULT_LOAD_ADDR, DEFAULT_START_LBA,
    FLASH_ARGS_MAX_SIZE, FLASH_PAYLOAD_MAX_SIZE, FLASHER_LOAD_ADDR, FLASHER_MAX_SIZE,
    FLASHER_STACK_SIZE, FLASHER_STACK_TOP, ImageOptions, SdImageOptions, SealOptions,
    build_sd_image, parse as parse_payload, result_name, seal, step_name, verify_sealed,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The loader's target triple, matching the workspace default target.
const TARGET: &str = "armv7a-none-eabi";

/// FEL load address of the loader (matches `rawboot-rt`'s FEL link script).
const FEL_LOAD_ADDR: u32 = 0x0002_8000;

/// SD/SPI load address BROM uses for a boot image (matches the boot link script).
const SD_LOAD_ADDR: u32 = 0x0002_0000;

const USAGE: &str = "usage: cargo xtask image | cargo xtask payload | cargo xtask flasher | cargo xtask sd-image --partition-sectors <n> [--start-lba <n>] [--loader <path>] | cargo xtask fel-flash [--payload <path>] [--boot-cfg <path>] [--loader <path>] [--flasher <path>] [--ddr-stub <path>] [--sequence <n>] [--skip-build]";

/// One of the two loader images: same code, different link address.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoaderVariant {
    /// Loaded with `xfel write 0x00028000`.
    Fel,
    /// Written to sector 16 for BROM to load at `0x00020000`.
    BootSd,
}

impl LoaderVariant {
    const ALL: [Self; 2] = [Self::Fel, Self::BootSd];

    fn image_name(self) -> &'static str {
        match self {
            Self::Fel => "rawboot-fel.bin",
            Self::BootSd => "rawboot-boot-sd.bin",
        }
    }

    fn load_addr(self) -> u32 {
        match self {
            Self::Fel => FEL_LOAD_ADDR,
            Self::BootSd => SD_LOAD_ADDR,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Fel => "fel",
            Self::BootSd => "boot-sd",
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("image") => image(),
        Some("payload") => payload(),
        Some("flasher") => flasher(),
        Some("sd-image") => sd_image(args),
        Some("fel-flash") => fel_flash(args),
        Some(other) => Err(format!("unknown command `{other}`\n{USAGE}").into()),
        None => Err(format!("missing command\n{USAGE}").into()),
    }
}

/// Build the loader for both load addresses and seal each into its image.
fn image() -> Result<()> {
    let root = workspace_root();

    for variant in LoaderVariant::ALL {
        let (elf, raw) = release_bin(&root, "rawboot");

        build_loader(&root, variant)?;
        verify_elf_section_vma(&elf, ".head", variant.load_addr())?;
        objcopy(&elf, &raw)?;

        let image = fs::read(&raw)?;
        let sealed = seal(&image, SealOptions::BLOCK)
            .map_err(|e| format!("sealing `{}`: {e:?}", raw.display()))?;
        verify_sealed(&sealed)
            .map_err(|e| format!("assembled loader image is not BROM-valid: {e}"))?;

        let sealed_path = root.join("target").join(variant.image_name());
        fs::write(&sealed_path, &sealed)
            .map_err(|e| format!("writing `{}`: {e}", sealed_path.display()))?;
        write_build_stamp(&root, &sealed_path, variant)?;

        println!(
            "sealed {} bytes -> {} (linked at {:#010x})",
            sealed.len(),
            sealed_path.display(),
            variant.load_addr()
        );
        if variant == LoaderVariant::BootSd {
            println!("sd: write the image to sector 16 of the boot medium");
        } else {
            println!("load: {}", fel_load_command(&sealed_path));
        }
    }
    Ok(())
}

/// Record the tree an image came from, so a HIL run can refuse an image that no
/// longer matches `HEAD` (for example after switching branches).
fn write_build_stamp(root: &Path, image: &Path, variant: LoaderVariant) -> Result<()> {
    let commit = git_line(root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let branch =
        git_line(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = match git_line(root, &["status", "--porcelain"]) {
        Some(status) => (!status.is_empty()).to_string(),
        None => "unknown".into(),
    };
    let stamp = format!(
        "commit={commit}\nbranch={branch}\ndirty={dirty}\nvariant={}\n",
        variant.tag()
    );

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

fn build_loader(root: &Path, variant: LoaderVariant) -> Result<()> {
    match variant {
        LoaderVariant::Fel => build_bin(root, "rawboot", "rawboot", &[]),
        LoaderVariant::BootSd => build_bin(root, "rawboot", "rawboot", &["--features", "boot-sd"]),
    }
}

/// Fail the build if the linker placed `section` anywhere but `expected`, so a
/// wrong or missing link script never reaches the board.
fn verify_elf_section_vma(elf: &Path, section: &str, expected: u32) -> Result<()> {
    let output = Command::new(llvm_objdump()?)
        .args(["-h"])
        .arg(elf)
        .output()
        .map_err(|e| format!("running llvm-objdump: {e}"))?;
    if !output.status.success() {
        return Err(format!("llvm-objdump failed: {}", output.status).into());
    }

    let sections = String::from_utf8_lossy(&output.stdout);
    let actual = parse_section_vma(&sections, section)
        .ok_or_else(|| format!("cannot read `{section}` VMA from `{}`", elf.display()))?;
    if actual != expected {
        return Err(format!(
            "`{}` has `{section}` at {actual:#010x}, expected {expected:#010x}",
            elf.display()
        )
        .into());
    }
    Ok(())
}

/// Fail the build if `symbol` is not at `expected`.
fn verify_elf_symbol(elf: &Path, symbol: &str, expected: u32) -> Result<()> {
    let actual = elf_symbol_value(elf, symbol)?;
    if actual != expected {
        return Err(format!(
            "`{}` has `{symbol}` at {actual:#010x}, expected {expected:#010x}",
            elf.display()
        )
        .into());
    }
    Ok(())
}

/// Fail the build if `symbol` lies above `limit`.
fn verify_elf_symbol_at_most(elf: &Path, symbol: &str, limit: u32) -> Result<()> {
    let actual = elf_symbol_value(elf, symbol)?;
    if actual > limit {
        return Err(format!(
            "`{}` has `{symbol}` at {actual:#010x}, above {limit:#010x}",
            elf.display()
        )
        .into());
    }
    Ok(())
}

fn elf_symbol_value(elf: &Path, symbol: &str) -> Result<u32> {
    let output = Command::new(llvm_objdump()?)
        .args(["-t"])
        .arg(elf)
        .output()
        .map_err(|e| format!("running llvm-objdump: {e}"))?;
    if !output.status.success() {
        return Err(format!("llvm-objdump failed: {}", output.status).into());
    }

    let symbols = String::from_utf8_lossy(&output.stdout);
    parse_symbol_value(&symbols, symbol)
        .ok_or_else(|| format!("cannot read `{symbol}` from `{}`", elf.display()).into())
}

/// The value of one symbol in `llvm-objdump -t` output.
fn parse_symbol_value(symbols: &str, symbol: &str) -> Option<u32> {
    symbols.lines().find_map(|line| {
        if line.split_whitespace().last()? != symbol {
            return None;
        }
        let mut fields = line.split_whitespace();
        u32::from_str_radix(fields.next()?, 16).ok()
    })
}

/// The VMA of one section in `llvm-objdump -h` output.
fn parse_section_vma(sections: &str, section: &str) -> Option<u32> {
    sections.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        fields.next()?;
        if fields.next()? != section {
            return None;
        }
        let _size = fields.next()?;
        u32::from_str_radix(fields.next()?, 16).ok()
    })
}

/// Build the payload crate and wrap its flat binary in the rawboot image header.
fn payload() -> Result<()> {
    let root = workspace_root();
    let (elf, raw) = release_bin(&root, "payload");

    build_bin(&root, "payload", "payload", &[])?;
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

/// Build the two FEL-callable flasher images: the returning DDR-init stub and
/// the DRAM-linked minimal flasher. Neither is an eGON image.
fn flasher() -> Result<()> {
    let root = workspace_root();

    for image in [
        FlasherImage {
            bin: "rawboot-ddr-init",
            output: "rawboot-ddr.bin",
            load_addr: DDR_STUB_LOAD_ADDR,
            max_size: DDR_STUB_MAX_SIZE,
            stack_start: None,
            stack_top: None,
            bss_limit: Some(DDR_RESULT_ADDR),
        },
        FlasherImage {
            bin: "rawboot-flasher",
            output: "rawboot-flasher.bin",
            load_addr: FLASHER_LOAD_ADDR,
            max_size: FLASHER_MAX_SIZE,
            stack_start: Some(FLASHER_STACK_TOP - FLASHER_STACK_SIZE),
            stack_top: Some(FLASHER_STACK_TOP),
            bss_limit: Some(FLASHER_STACK_TOP - FLASHER_STACK_SIZE),
        },
    ] {
        let (elf, _) = release_bin(&root, image.bin);
        build_bin(&root, "rawboot-flasher", image.bin, &[])?;
        verify_elf_section_vma(&elf, ".text", image.load_addr)?;
        verify_elf_symbol(&elf, "_start", image.load_addr)?;
        if let Some(stack_start) = image.stack_start {
            verify_elf_symbol(&elf, "_sstack", stack_start)?;
        }
        if let Some(stack_top) = image.stack_top {
            verify_elf_symbol(&elf, "_estack", stack_top)?;
        }
        if let Some(bss_limit) = image.bss_limit {
            verify_elf_symbol_at_most(&elf, "_ebss", bss_limit)?;
        }

        let output = root.join("target").join(image.output);
        objcopy(&elf, &output)?;

        let bytes = fs::read(&output)?;
        if bytes.starts_with(b"eGON") {
            return Err(format!(
                "`{}` starts with an eGON header; a FEL-callable image must not",
                output.display()
            )
            .into());
        }
        if bytes.len() > image.max_size as usize {
            return Err(format!(
                "`{}` is {} bytes, larger than its {}-byte window",
                output.display(),
                bytes.len(),
                image.max_size
            )
            .into());
        }

        println!(
            "{} {} bytes -> {} (linked at {:#010x}, no eGON header)",
            image.bin,
            bytes.len(),
            output.display(),
            image.load_addr
        );
    }

    Ok(())
}

struct FlasherImage {
    bin: &'static str,
    output: &'static str,
    load_addr: u32,
    max_size: u32,
    stack_start: Option<u32>,
    stack_top: Option<u32>,
    bss_limit: Option<u32>,
}

/// Build the SD boot prefix: an MBR pointing at the existing FAT partition and
/// the sealed SD/SPI loader at sector 16.
fn sd_image(args: impl Iterator<Item = String>) -> Result<()> {
    let root = workspace_root();
    let args = parse_sd_image_args(args)?;
    let loader_path = args.loader.unwrap_or_else(|| default_sd_loader(&root));

    let loader = read_loader(&loader_path)?;
    let image = build_sd_image(&loader, args.options)
        .map_err(|e| format!("building `{}`: {e}", loader_path.display()))?;

    let image_path = root.join("target").join("rawboot-sd-mbr.img");
    fs::write(&image_path, &image)
        .map_err(|e| format!("writing `{}`: {e}", image_path.display()))?;

    println!(
        "sd prefix: {} bytes -> {} ({} loader bytes at sector 16, partition LBA {} +{} sectors)",
        image.len(),
        image_path.display(),
        loader.len(),
        args.options.start_lba,
        args.options.partition_sectors
    );
    Ok(())
}

fn default_sd_loader(root: &Path) -> PathBuf {
    root.join("target").join("rawboot-boot-sd.bin")
}

fn default_fel_loader(root: &Path) -> PathBuf {
    root.join("target").join("rawboot-fel.bin")
}

fn default_payload(root: &Path) -> PathBuf {
    root.join("target").join("rawboot-payload.img")
}

fn default_flasher(root: &Path) -> PathBuf {
    root.join("target").join("rawboot-flasher.bin")
}

fn default_ddr_stub(root: &Path) -> PathBuf {
    root.join("target").join("rawboot-ddr.bin")
}

/// Build or reuse the artifacts and write them to a card left in the board.
fn fel_flash(args: impl Iterator<Item = String>) -> Result<()> {
    let root = workspace_root();
    let args = parse_fel_flash_args(args)?;

    if !args.skip_build {
        image()?;
        payload()?;
        flasher()?;
    }

    let loader_path = args.loader.unwrap_or_else(|| default_sd_loader(&root));
    let payload_path = args.payload.unwrap_or_else(|| default_payload(&root));
    let flasher_path = args.flasher.unwrap_or_else(|| default_flasher(&root));
    let stub_path = args.ddr_stub.unwrap_or_else(|| default_ddr_stub(&root));

    let loader = read_loader(&loader_path)?;
    check_sd_loader(&loader, &root, &loader_path)?;
    let payload = read_payload(&payload_path)?;
    let boot_cfg = match args.boot_cfg {
        Some(path) => read_boot_cfg(&path)?,
        None => Vec::new(),
    };
    let flasher = read_flasher_image(&flasher_path, "flasher")?;
    let stub = read_flasher_image(&stub_path, "DDR-init stub")?;

    let mut fel = FelDevice::open()?;
    let (_, soc) = fel.identify()?;
    println!("SoC:        {}", soc.name());

    let report = fel.flash_sd_card(&stub, &flasher, &payload, &boot_cfg, &loader, args.sequence)?;

    println!("DRAM:       {} MiB", report.dram_size_mib);
    println!(
        "staged:     payload {} bytes, boot.cfg {} bytes, loader {} bytes",
        payload.len(),
        boot_cfg.len(),
        loader.len()
    );
    println!(
        "status:     sequence {}, step {} ({}), result {} ({})",
        report.status.sequence,
        report.status.step,
        step_name(report.status.step),
        report.status.result,
        result_name(report.status.result)
    );

    if !report.status.is_ok() {
        return Err(format!(
            "flasher stopped at step {} ({}): {} ({})",
            report.status.step,
            step_name(report.status.step),
            report.status.result,
            result_name(report.status.result)
        )
        .into());
    }

    println!("card:       payload files and loader written; MBR unchanged");
    Ok(())
}

#[derive(Debug)]
struct FelFlashArgs {
    payload: Option<PathBuf>,
    boot_cfg: Option<PathBuf>,
    loader: Option<PathBuf>,
    flasher: Option<PathBuf>,
    ddr_stub: Option<PathBuf>,
    sequence: u32,
    skip_build: bool,
}

fn parse_fel_flash_args(mut args: impl Iterator<Item = String>) -> Result<FelFlashArgs> {
    let mut parsed = FelFlashArgs {
        payload: None,
        boot_cfg: None,
        loader: None,
        flasher: None,
        ddr_stub: None,
        sequence: 1,
        skip_build: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--payload" => parsed.payload = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            "--boot-cfg" => parsed.boot_cfg = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            "--loader" => parsed.loader = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            "--flasher" => parsed.flasher = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            "--ddr-stub" => parsed.ddr_stub = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            "--sequence" => parsed.sequence = parse_u32(&arg, take_value(&mut args, &arg)?)?,
            "--skip-build" => parsed.skip_build = true,
            other => return Err(format!("unexpected argument `{other}`\n{USAGE}").into()),
        }
    }

    Ok(parsed)
}

fn check_sd_loader(loader: &[u8], root: &Path, path: &Path) -> Result<()> {
    if loader.is_empty() {
        return Err(format!("`{}` is empty", path.display()).into());
    }
    let fel = fs::read(default_fel_loader(root)).ok();
    let named_fel = path
        .file_name()
        .is_some_and(|name| name == "rawboot-fel.bin");
    let stamped_fel = fs::read_to_string(build_stamp_path(path))
        .is_ok_and(|stamp| stamp.lines().any(|line| line == "variant=fel"));
    if named_fel || stamped_fel || fel.is_some_and(|fel| loader == fel) {
        return Err(format!(
            "`{}` is the FEL loader; use the SD/SPI image (`target/rawboot-boot-sd.bin`)",
            path.display()
        )
        .into());
    }
    verify_sealed(loader).map_err(|err| {
        format!(
            "`{}` is not a sealed SD/SPI loader: {err}; run `cargo xtask image`",
            path.display()
        )
    })?;
    Ok(())
}

fn read_payload(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(|err| match err.kind() {
        ErrorKind::NotFound => {
            format!(
                "`{}` not found; run `cargo xtask payload` first",
                path.display()
            )
        }
        _ => format!("reading `{}`: {err}", path.display()),
    })?;

    parse_payload(&bytes).map_err(|err| {
        format!(
            "`{}` is not a wrapped payload image: {err}; use `target/rawboot-payload.img`",
            path.display()
        )
    })?;
    if bytes.len() > FLASH_PAYLOAD_MAX_SIZE as usize {
        return Err(format!(
            "`{}` is {} bytes, larger than the {}-byte staging window",
            path.display(),
            bytes.len(),
            FLASH_PAYLOAD_MAX_SIZE
        )
        .into());
    }
    Ok(bytes)
}

fn read_boot_cfg(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(|err| format!("reading `{}`: {err}", path.display()))?;
    if bytes.len() > FLASH_ARGS_MAX_SIZE as usize {
        return Err(format!(
            "`{}` is {} bytes, but boot.cfg is limited to {FLASH_ARGS_MAX_SIZE} bytes",
            path.display(),
            bytes.len()
        )
        .into());
    }
    Ok(bytes)
}

fn read_flasher_image(path: &Path, what: &str) -> Result<Vec<u8>> {
    fs::read(path)
        .map_err(|err| match err.kind() {
            ErrorKind::NotFound => format!(
                "`{}` ({what}) not found; run `cargo xtask flasher` first",
                path.display()
            ),
            _ => format!("reading `{}`: {err}", path.display()),
        })
        .map_err(Into::into)
}

fn read_loader(path: &Path) -> Result<Vec<u8>> {
    Ok(fs::read(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => format!(
            "`{}` not found; run `cargo xtask image` first",
            path.display()
        ),
        _ => format!("reading `{}`: {e}", path.display()),
    })?)
}

#[derive(Debug)]
struct SdImageArgs {
    options: SdImageOptions,
    loader: Option<PathBuf>,
}

fn parse_sd_image_args(mut args: impl Iterator<Item = String>) -> Result<SdImageArgs> {
    let mut start_lba = DEFAULT_START_LBA;
    let mut partition_sectors = None;
    let mut loader = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--start-lba" => {
                start_lba = parse_u32(&arg, take_value(&mut args, &arg)?)?;
            }
            "--partition-sectors" => {
                partition_sectors = Some(parse_u32(&arg, take_value(&mut args, &arg)?)?);
            }
            "--loader" => loader = Some(PathBuf::from(take_value(&mut args, &arg)?)),
            other => return Err(format!("unexpected argument `{other}`\n{USAGE}").into()),
        }
    }

    let partition_sectors =
        partition_sectors.ok_or_else(|| format!("missing `--partition-sectors`\n{USAGE}"))?;

    Ok(SdImageArgs {
        options: SdImageOptions::new(start_lba, partition_sectors),
        loader,
    })
}

fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| format!("`{flag}` needs a value\n{USAGE}").into())
}

fn parse_u32(flag: &str, value: String) -> Result<u32> {
    value.parse::<u32>().map_err(|_| {
        format!("`{flag}` expects an unsigned 32-bit integer, got `{value}`\n{USAGE}").into()
    })
}

/// The release ELF of a workspace binary and the flat binary `objcopy` produces.
fn release_bin(root: &Path, bin: &str) -> (PathBuf, PathBuf) {
    let out_dir = root.join("target").join(TARGET).join("release");
    (out_dir.join(bin), out_dir.join(format!("{bin}.bin")))
}

/// Build a workspace binary for the loader's target.
fn build_bin(root: &Path, package: &str, bin: &str, extra_args: &[&str]) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--release",
            "--package",
            package,
            "--bin",
            bin,
            "--target",
            TARGET,
        ])
        .args(extra_args)
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
    llvm_tool(&["rust-objcopy", "llvm-objcopy"])
}

/// Locate `llvm-objdump` next to `llvm-objcopy`.
fn llvm_objdump() -> Result<PathBuf> {
    llvm_tool(&["llvm-objdump"])
}

fn llvm_tool(candidates: &[&str]) -> Result<PathBuf> {
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
    for name in candidates {
        let candidate = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "none of {candidates:?} in {}; install the `llvm-tools` rustup component",
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

    #[test]
    fn both_variants_target_their_origins() {
        assert_eq!(LoaderVariant::Fel.load_addr(), 0x0002_8000);
        assert_eq!(LoaderVariant::BootSd.load_addr(), 0x0002_0000);
        assert_eq!(LoaderVariant::Fel.image_name(), "rawboot-fel.bin");
        assert_eq!(LoaderVariant::BootSd.image_name(), "rawboot-boot-sd.bin");
    }

    #[test]
    fn parses_the_head_vma_from_objdump_output() {
        let output = "\
Sections:
Idx Name            Size     VMA      LMA      Type
  0                 00000000 00000000 00000000
  1 .head           00000080 00020000 00020000 TEXT
  2 .text           0000c550 00020080 00020080 TEXT
";

        assert_eq!(parse_section_vma(output, ".head"), Some(0x0002_0000));
        assert_eq!(parse_section_vma(output, ".text"), Some(0x0002_0080));
        assert_eq!(parse_section_vma("Sections:\n", ".head"), None);
    }

    #[test]
    fn parses_a_symbol_value_from_objdump_output() {
        let output = "\
SYMBOL TABLE:
40010000 g     F .text\t0000001c _start
40050000 g       .stack\t00000000 _estack
40010580 g       .bss\t00000000 _sbss
";

        assert_eq!(parse_symbol_value(output, "_start"), Some(0x4001_0000));
        assert_eq!(parse_symbol_value(output, "_estack"), Some(0x4005_0000));
        assert_eq!(parse_symbol_value(output, "_missing"), None);
    }

    fn args(items: &[&str]) -> impl Iterator<Item = String> {
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn sd_image_defaults_to_lba_2048_and_no_loader_override() {
        let parsed = parse_sd_image_args(args(&["--partition-sectors", "1843200"])).unwrap();

        assert_eq!(parsed.options.start_lba, DEFAULT_START_LBA);
        assert_eq!(parsed.options.partition_sectors, 1_843_200);
        assert_eq!(parsed.loader, None);
    }

    #[test]
    fn default_sd_loader_points_at_the_boot_sd_image() {
        assert_eq!(
            default_sd_loader(Path::new("repo")),
            PathBuf::from("repo/target/rawboot-boot-sd.bin")
        );
    }

    #[test]
    fn missing_loader_hints_at_the_image_command() {
        let err = read_loader(Path::new("target/does-not-exist-rawboot-boot-sd.bin"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("cargo xtask image"), "{err}");
    }

    #[test]
    fn sd_image_accepts_overrides() {
        let parsed = parse_sd_image_args(args(&[
            "--start-lba",
            "4096",
            "--loader",
            "custom.bin",
            "--partition-sectors",
            "100",
        ]))
        .unwrap();

        assert_eq!(parsed.options, SdImageOptions::new(4096, 100));
        assert_eq!(parsed.loader, Some(PathBuf::from("custom.bin")));
    }

    #[test]
    fn sd_image_requires_partition_sectors() {
        let err = parse_sd_image_args(args(&[])).unwrap_err().to_string();

        assert!(err.contains("--partition-sectors"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn sd_image_rejects_a_flag_without_a_value() {
        let err = parse_sd_image_args(args(&["--partition-sectors"]))
            .unwrap_err()
            .to_string();

        assert!(err.contains("`--partition-sectors` needs a value"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn sd_image_rejects_a_non_numeric_size() {
        let err = parse_sd_image_args(args(&["--partition-sectors", "many"]))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unsigned 32-bit integer"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn sd_image_rejects_an_unknown_argument() {
        let err = parse_sd_image_args(args(&["--partition-sectors", "1", "--wat"]))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected argument `--wat`"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn fel_flash_defaults_to_sequence_one_and_the_build_steps() {
        let parsed = parse_fel_flash_args(args(&[])).unwrap();

        assert_eq!(parsed.sequence, 1);
        assert!(!parsed.skip_build);
        assert_eq!(parsed.payload, None);
        assert_eq!(parsed.boot_cfg, None);
        assert_eq!(parsed.loader, None);
        assert_eq!(parsed.flasher, None);
        assert_eq!(parsed.ddr_stub, None);
    }

    #[test]
    fn fel_flash_accepts_overrides() {
        let parsed = parse_fel_flash_args(args(&[
            "--payload",
            "payload.img",
            "--boot-cfg",
            "boot.cfg",
            "--loader",
            "loader.bin",
            "--flasher",
            "flasher.bin",
            "--ddr-stub",
            "ddr.bin",
            "--sequence",
            "9",
            "--skip-build",
        ]))
        .unwrap();

        assert_eq!(parsed.payload, Some(PathBuf::from("payload.img")));
        assert_eq!(parsed.boot_cfg, Some(PathBuf::from("boot.cfg")));
        assert_eq!(parsed.loader, Some(PathBuf::from("loader.bin")));
        assert_eq!(parsed.flasher, Some(PathBuf::from("flasher.bin")));
        assert_eq!(parsed.ddr_stub, Some(PathBuf::from("ddr.bin")));
        assert_eq!(parsed.sequence, 9);
        assert!(parsed.skip_build);
    }

    #[test]
    fn fel_flash_rejects_a_sequence_that_is_not_a_number() {
        let err = parse_fel_flash_args(args(&["--sequence", "soon"]))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unsigned 32-bit integer"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn fel_flash_rejects_an_unknown_argument() {
        let err = parse_fel_flash_args(args(&["--wat"]))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected argument `--wat`"), "{err}");
        assert!(err.contains("usage:"), "{err}");
    }

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rawboot-xtask-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn refuses_the_fel_image_as_a_loader() {
        let root = temp_path("fel-root");
        fs::create_dir_all(root.join("target")).unwrap();
        let fel = b"the fel image";
        fs::write(root.join("target/rawboot-fel.bin"), fel).unwrap();

        let err = check_sd_loader(fel, &root, Path::new("loader.bin"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("FEL loader"), "{err}");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_a_loader_with_a_fel_build_stamp() {
        let root = temp_path("stamped-fel-root");
        fs::create_dir_all(&root).unwrap();
        let loader = root.join("loader.bin");
        fs::write(&loader, b"the fel image").unwrap();
        fs::write(
            build_stamp_path(&loader),
            "commit=abc\nbranch=master\ndirty=false\nvariant=fel\n",
        )
        .unwrap();

        let err = check_sd_loader(b"the fel image", &root, &loader)
            .unwrap_err()
            .to_string();
        assert!(err.contains("FEL loader"), "{err}");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_a_sealed_loader_that_is_not_the_fel_image() {
        use rawboot_image::EgonHeader;

        let sealed = seal(&EgonHeader::PLACEHOLDER.to_bytes(), SealOptions::BLOCK).unwrap();
        let root = temp_path("sealed-root");
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target/rawboot-fel.bin"), b"a different image").unwrap();

        check_sd_loader(&sealed, &root, Path::new("loader.bin")).unwrap();

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_an_unsealed_loader() {
        let root = temp_path("unsealed-root");
        fs::create_dir_all(&root).unwrap();

        let err = check_sd_loader(b"not sealed", &root, Path::new("loader.bin"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a sealed SD/SPI loader"), "{err}");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn read_payload_accepts_a_wrapped_image_and_rejects_a_raw_binary() {
        let image = rawboot_image::build(b"payload", ImageOptions::default()).unwrap();
        let good = temp_path("payload.img");
        fs::write(&good, &image).unwrap();
        assert_eq!(read_payload(&good).unwrap(), image);

        let raw = temp_path("payload-raw.bin");
        fs::write(&raw, b"not an image").unwrap();
        let err = read_payload(&raw).unwrap_err().to_string();
        assert!(err.contains("not a wrapped payload image"), "{err}");

        fs::remove_file(good).unwrap();
        fs::remove_file(raw).unwrap();
    }

    #[test]
    fn read_boot_cfg_enforces_the_loader_window() {
        let path = temp_path("boot.cfg");
        fs::write(&path, vec![b'x'; FLASH_ARGS_MAX_SIZE as usize]).unwrap();
        assert_eq!(
            read_boot_cfg(&path).unwrap().len(),
            FLASH_ARGS_MAX_SIZE as usize
        );

        fs::write(&path, vec![b'x'; FLASH_ARGS_MAX_SIZE as usize + 1]).unwrap();
        let err = read_boot_cfg(&path).unwrap_err().to_string();
        assert!(err.contains("boot.cfg is limited"), "{err}");

        fs::remove_file(path).unwrap();
    }
}
