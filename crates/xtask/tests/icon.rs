//! Readable checks for the app icon (#297): `cargo xtask icon` draws it,
//! and the committed files in `crates/opendrone/icon/` are what it draws, in
//! the sizes each system shows. Sizes in pixels.

use std::path::Path;
use std::process::Command;

fn icon_file(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../opendrone/icon")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A PNG's width and height, from its header.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "a PNG");
    assert_eq!(&png[12..16], b"IHDR");
    let at = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
    (at(16), at(20))
}

#[test]
fn the_committed_app_icon_is_the_one_cargo_xtask_icon_draws() {
    // Basis: Rule (#297): the icon is made by a script, with no outside
    // picture, so the committed files can always be made again from it.
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["icon", "--check"])
        .output()
        .expect("xtask runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_windows_icon_holds_every_size_explorer_shows_from_16_to_256_pixels() {
    // Basis: Rule (#297): Explorer picks the size it needs from the .ico the
    // game's build script builds into OpenDrone.exe; each is a PNG, which
    // Windows reads inside an .ico since Vista.
    let ico = icon_file("opendrone.ico");
    let u16_at = |i: usize| u16::from_le_bytes(ico[i..i + 2].try_into().unwrap());
    let u32_at = |i: usize| u32::from_le_bytes(ico[i..i + 4].try_into().unwrap()) as usize;
    assert_eq!((u16_at(0), u16_at(2)), (0, 1), "an icon file");
    let mut sizes = Vec::new();
    for i in 0..usize::from(u16_at(4)) {
        let entry = 6 + 16 * i;
        let (size, at) = (u32_at(entry + 8), u32_at(entry + 12));
        let (width, height) = png_size(&ico[at..at + size]);
        let side = |b: u8| if b == 0 { 256 } else { u32::from(b) };
        assert_eq!((side(ico[entry]), side(ico[entry + 1])), (width, height));
        sizes.push(width);
    }
    assert_eq!(sizes, [16, 24, 32, 48, 64, 128, 256]);
}

#[test]
fn the_macos_icon_holds_finders_sizes_from_16_to_1024_pixels() {
    // Basis: Rule (#297): Finder and the Dock pick the size they need from
    // the app's .icns, at twice the pixels on a Retina screen.
    let icns = icon_file("opendrone.icns");
    assert_eq!(&icns[..4], b"icns");
    let length = |i: usize| u32::from_be_bytes(icns[i..i + 4].try_into().unwrap()) as usize;
    assert_eq!(length(4), icns.len());
    let mut pictures = Vec::new();
    let mut at = 8;
    while at < icns.len() {
        let kind = String::from_utf8_lossy(&icns[at..at + 4]).into_owned();
        let end = at + length(at + 4);
        pictures.push((kind, png_size(&icns[at + 8..end]).0));
        at = end;
    }
    let want: Vec<(String, u32)> = [
        ("icp4", 16),
        ("icp5", 32),
        ("ic11", 32),
        ("ic12", 64),
        ("ic07", 128),
        ("ic13", 256),
        ("ic08", 256),
        ("ic14", 512),
        ("ic09", 512),
        ("ic10", 1024),
    ]
    .map(|(k, s)| (k.to_string(), s))
    .to_vec();
    assert_eq!(pictures, want);
}

#[test]
fn the_linux_icon_is_a_512_pixel_png() {
    // Basis: Rule (#297): the desktop entry add-to-app-menu.sh writes
    // names it, and desktops scale it down for their menus.
    assert_eq!(png_size(&icon_file("opendrone.png")), (512, 512));
}
