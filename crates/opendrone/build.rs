//! Builds the app icon into the Windows program (#297), so Explorer, the
//! taskbar and the window show it.
//!
//! The icon is `icon/opendrone.ico`, beside this script, which
//! `cargo xtask icon` draws. Windows keeps a program's icon as resources inside the `.exe`:
//! one resource per picture, and an icon group listing them, the first
//! group being the program's own icon. Microsoft's linker takes those
//! resources as a compiled resource file (`.res`), the format the resource
//! compiler writes. This script writes that file itself, from the `.ico`, so
//! the build needs no resource compiler and no library: it is a short list
//! of headers around the pictures already in the `.ico`. Every other build
//! leaves it out.

use std::env;
use std::fs;
use std::path::PathBuf;

const ICON: &str = "icon/opendrone.ico";

/// Resource types, as Windows numbers them.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// English (United States), as the resource compiler marks resources by
/// default.
const LANGUAGE: u16 = 0x0409;

fn main() {
    println!("cargo:rerun-if-changed={ICON}");
    println!("cargo:rerun-if-changed=build.rs");
    let target = |key: &str| env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") != "windows" || target("CARGO_CFG_TARGET_ENV") != "msvc" {
        return;
    }
    let ico = fs::read(ICON).unwrap_or_else(|e| panic!("{ICON}: {e}"));
    let res = resources(&ico).unwrap_or_else(|problem| panic!("{ICON}: {problem}"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo gives OUT_DIR")).join("icon.res");
    fs::write(&out, res).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// One resource: its header (its size, the header's size, its type and its
/// number, both as numbers, then its flags and language), then its data,
/// padded to four bytes.
fn resource(res: &mut Vec<u8>, kind: u16, number: u16, flags: u16, data: &[u8]) {
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32_u32.to_le_bytes());
    res.extend(
        [0xffff, kind, 0xffff, number]
            .map(u16::to_le_bytes)
            .concat(),
    );
    res.extend(0_u32.to_le_bytes());
    res.extend(flags.to_le_bytes());
    let language: u16 = if kind == 0 { 0 } else { LANGUAGE };
    res.extend(language.to_le_bytes());
    res.extend(0_u32.to_le_bytes());
    res.extend(0_u32.to_le_bytes());
    res.extend(data);
    while res.len() % 4 != 0 {
        res.push(0);
    }
}

/// The `.res` file holding the `.ico`'s pictures as icon 1, 2, … and the
/// group listing them as icon group 1.
fn resources(ico: &[u8]) -> Result<Vec<u8>, String> {
    if u16_at(ico, 0) != Some(0) || u16_at(ico, 2) != Some(1) {
        return Err("it isn't an icon file".into());
    }
    let count = u16_at(ico, 4).ok_or("it has no header")?;
    // A .res file starts with an empty resource.
    let mut res = Vec::new();
    resource(&mut res, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend([0_u16, 1, count].map(u16::to_le_bytes).concat());
    for i in 0..count {
        let entry = 6 + 16 * usize::from(i);
        let head = ico
            .get(entry..entry + 8)
            .ok_or("its list of pictures is cut short")?;
        let size = u32_at(ico, entry + 8).ok_or("a picture has no size")? as usize;
        let at = u32_at(ico, entry + 12).ok_or("a picture has no place")? as usize;
        let picture = ico
            .get(at..at + size)
            .ok_or("a picture runs past the file's end")?;
        resource(&mut res, RT_ICON, i + 1, 0x1010, picture);
        // Width, height, colours, a reserved byte, planes and bits per
        // pixel, as the .ico has them, then the size and the icon's number.
        group.extend(head);
        group.extend((size as u32).to_le_bytes());
        group.extend((i + 1).to_le_bytes());
    }
    resource(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);
    Ok(res)
}
