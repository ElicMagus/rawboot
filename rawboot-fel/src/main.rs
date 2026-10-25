// SPDX-License-Identifier: Unlicense

//! `rawboot-fel` — manual FEL bring-up without an external `xfel`.
//!
//! Talks to an Allwinner T113-S3 in FEL mode over USB: identify it, read and
//! write memory, and execute uploaded code.

use std::env;
use std::error::Error as StdError;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use rawboot_fel::{FelDevice, UsbTransport};

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

const USAGE: &str = "usage:
  rawboot-fel version
  rawboot-fel read <address> <length> [--output <path>]
  rawboot-fel write <address> <file>
  rawboot-fel exec <address>

<address> is decimal or 0x-hex; <length> is decimal or 0x-hex.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rawboot-fel: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("version") => version(args),
        Some("read") => read(args),
        Some("write") => write(args),
        Some("exec") => exec(args),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n{USAGE}").into()),
        None => Err(format!("missing command\n{USAGE}").into()),
    }
}

fn version(mut args: impl Iterator<Item = String>) -> Result<()> {
    no_extra(&mut args)?;

    let (version, soc) = open()?.identify()?;
    println!(
        "SoC:        {} (chip id {:#06x})",
        soc.name(),
        version.chip_id()
    );
    println!(
        "signature:  {}",
        String::from_utf8_lossy(&version.signature).trim_end_matches('\0')
    );
    println!("firmware:   {:#010x}", version.firmware);
    println!("protocol:   {}", version.protocol);
    println!(
        "flags:      {:#04x}, length {:#04x}",
        version.dflag, version.dlength
    );
    println!("scratchpad: {:#010x}", version.scratchpad);
    Ok(())
}

fn read(mut args: impl Iterator<Item = String>) -> Result<()> {
    let address = parse_number(&next_arg(&mut args, "address")?)?;
    let length = parse_number(&next_arg(&mut args, "length")?)? as usize;
    let mut output = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" | "-o" => {
                output = Some(PathBuf::from(next_arg(&mut args, "--output path")?));
            }
            other => return Err(format!("unexpected argument `{other}`\n{USAGE}").into()),
        }
    }

    let data = open()?.read(address, length)?;
    match output {
        Some(path) => {
            fs::write(&path, &data)
                .map_err(|error| format!("writing `{}`: {error}", path.display()))?;
            println!(
                "read {} bytes from {address:#010x} -> {}",
                data.len(),
                path.display()
            );
        }
        None => hexdump(address, &data),
    }
    Ok(())
}

fn write(mut args: impl Iterator<Item = String>) -> Result<()> {
    let address = parse_number(&next_arg(&mut args, "address")?)?;
    let path = PathBuf::from(next_arg(&mut args, "file")?);
    no_extra(&mut args)?;

    let data = fs::read(&path).map_err(|error| format!("reading `{}`: {error}", path.display()))?;
    open()?.write(address, &data)?;
    println!("wrote {} bytes to {address:#010x}", data.len());
    Ok(())
}

fn exec(mut args: impl Iterator<Item = String>) -> Result<()> {
    let address = parse_number(&next_arg(&mut args, "address")?)?;
    no_extra(&mut args)?;

    open()?.exec(address)?;
    println!("executed at {address:#010x}");
    Ok(())
}

fn open() -> Result<FelDevice<UsbTransport>> {
    Ok(FelDevice::open()?)
}

fn next_arg(args: &mut impl Iterator<Item = String>, what: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| format!("missing {what}\n{USAGE}").into())
}

fn no_extra(args: &mut impl Iterator<Item = String>) -> Result<()> {
    match args.next() {
        Some(extra) => Err(format!("unexpected argument `{extra}`\n{USAGE}").into()),
        None => Ok(()),
    }
}

fn parse_number(text: &str) -> Result<u32> {
    let (digits, radix) = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => (hex, 16),
        None => (text, 10),
    };
    u32::from_str_radix(digits, radix)
        .map_err(|_| format!("invalid number `{text}` (use decimal or 0x-hex)").into())
}

fn hexdump(address: u32, data: &[u8]) {
    for (offset, line) in data.chunks(16).enumerate() {
        let line_address = address.wrapping_add((offset * 16) as u32);
        print!("{line_address:#010x}:");
        for index in 0..16 {
            match line.get(index) {
                Some(byte) => print!(" {byte:02x}"),
                None => print!("   "),
            }
        }
        print!("  |");
        for &byte in line {
            let character = if byte.is_ascii_graphic() || byte == b' ' {
                byte as char
            } else {
                '.'
            };
            print!("{character}");
        }
        println!("|");
    }
}
