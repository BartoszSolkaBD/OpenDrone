//! The app icon (#297): a Quad seen from above, on a dark rounded square,
//! drawn by this script from shapes, with no outside picture. Like every
//! asset an agent makes, the pictures are CC0, and this script is code, so
//! it stays MIT/Apache-2.0 (ADR-0014).
//!
//! `cargo xtask icon` writes the three files every download takes its icon
//! from, and `cargo xtask icon --check` fails when a committed one isn't
//! what this script draws (a readable check runs it):
//!
//! - [`PNG`], 512 pixels square: the Linux download's icon, for its app menu
//!   entry;
//! - [`ICO`]: the Windows program's icon, which the game's build script
//!   (`crates/opendrone/build.rs`) builds into `OpenDrone.exe`;
//! - [`ICNS`]: the macOS app's icon, in `OpenDrone.app/Contents/Resources/`.
//!
//! Each holds PNG pictures, which Windows (since Vista) and macOS (since
//! 10.7) read inside their own icon files. The PNG writer here is a few
//! lines on top of `flate2`, which xtask already uses, so no library is
//! added.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use flate2::Compression;
use flate2::write::ZlibEncoder;

/// The Linux icon, from the repo's root.
pub const PNG: &str = "crates/opendrone/icon/opendrone.png";
/// The Windows icon, from the repo's root.
pub const ICO: &str = "crates/opendrone/icon/opendrone.ico";
/// The macOS icon, from the repo's root.
pub const ICNS: &str = "crates/opendrone/icon/opendrone.icns";

/// The PNG icon's size, in pixels.
const PNG_SIZE: u32 = 512;
/// The sizes the Windows icon holds: what Explorer shows, from a list's
/// 16 pixels to a large view's 256.
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// The pictures the macOS icon holds, each by its type and size: Finder's
/// sizes from 16 to 512 points, each also at twice the pixels for a Retina
/// screen.
const ICNS_PICTURES: [(&[u8; 4], u32); 10] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
];

pub fn run(args: &[String]) -> ExitCode {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            eprintln!("Usage: cargo xtask icon [--check]");
            return ExitCode::from(2);
        }
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = [(PNG, png_file()), (ICO, ico_file()), (ICNS, icns_file())];
    let mut stale = Vec::new();
    for (path, bytes) in &files {
        let at = root.join(path);
        if check {
            if fs::read(&at).ok().as_deref() != Some(bytes.as_slice()) {
                stale.push(*path);
            }
        } else {
            let written = fs::create_dir_all(at.parent().expect("a folder"))
                .and_then(|()| fs::write(&at, bytes));
            if let Err(e) = written {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
            println!("Wrote {path} ({} bytes).", bytes.len());
        }
    }
    if !stale.is_empty() {
        eprintln!(
            "These icon files aren't what `cargo xtask icon` draws: {}. Run `cargo xtask icon` \
             and commit them.",
            stale.join(", ")
        );
        return ExitCode::FAILURE;
    }
    if check {
        println!("The icon files are what `cargo xtask icon` draws.");
    }
    ExitCode::SUCCESS
}

// ----- Drawing ---------------------------------------------------------------

/// A colour, each part from 0 to 1, with its opacity.
#[derive(Clone, Copy)]
struct Paint {
    rgb: [f64; 3],
    alpha: f64,
}

const fn hex(rgb: u32, alpha: f64) -> Paint {
    Paint {
        rgb: [
            ((rgb >> 16) & 0xff) as f64 / 255.0,
            ((rgb >> 8) & 0xff) as f64 / 255.0,
            (rgb & 0xff) as f64 / 255.0,
        ],
        alpha,
    }
}

const SKY_TOP: Paint = hex(0x2c3e5c, 1.0);
const SKY_BOTTOM: Paint = hex(0x111827, 1.0);
const FRAME: Paint = hex(0xdfe4ec, 1.0);
const ORANGE: Paint = hex(0xff8a1f, 1.0);
const PROP_BLUR: Paint = hex(0xff8a1f, 0.28);
const LENS: Paint = hex(0x111827, 1.0);

/// The four motors, as fractions of the icon's width from its top left.
const MOTORS: [[f64; 2]; 4] = [[0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]];

fn length(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// How far a point is outside a box centred on `c` with half-sizes `h` and
/// corners rounded to `r`: below zero inside.
fn rounded_box(p: [f64; 2], c: [f64; 2], h: [f64; 2], r: f64) -> f64 {
    let qx = (p[0] - c[0]).abs() - (h[0] - r);
    let qy = (p[1] - c[1]).abs() - (h[1] - r);
    length(qx.max(0.0), qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// How far a point is outside a bar of radius `r` from `a` to `b`.
fn bar(p: [f64; 2], a: [f64; 2], b: [f64; 2], r: f64) -> f64 {
    let (px, py) = (p[0] - a[0], p[1] - a[1]);
    let (bx, by) = (b[0] - a[0], b[1] - a[1]);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    length(px - t * bx, py - t * by) - r
}

fn circle(p: [f64; 2], c: [f64; 2], r: f64) -> f64 {
    length(p[0] - c[0], p[1] - c[1]) - r
}

/// Lays `paint` over `under`, covering the fraction `cover` of the pixel.
fn over(under: Paint, paint: Paint, cover: f64) -> Paint {
    let a = paint.alpha * cover;
    let out = a + under.alpha * (1.0 - a);
    if out <= 0.0 {
        return under;
    }
    let mix = |i: usize| (paint.rgb[i] * a + under.rgb[i] * under.alpha * (1.0 - a)) / out;
    Paint {
        rgb: [mix(0), mix(1), mix(2)],
        alpha: out,
    }
}

/// The icon at `size` pixels square, as rows of RGBA bytes, top row first.
/// Each shape's edge is smoothed over one pixel, from how far the pixel's
/// centre is from it.
pub fn draw(size: u32) -> Vec<u8> {
    let pixel = 1.0 / f64::from(size);
    let cover = |distance: f64| (0.5 - distance / pixel).clamp(0.0, 1.0);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for column in 0..size {
            let p = [
                (f64::from(column) + 0.5) * pixel,
                (f64::from(row) + 0.5) * pixel,
            ];
            let mut here = Paint {
                rgb: [0.0; 3],
                alpha: 0.0,
            };
            // The rounded square, on macOS's grid: 824 of 1024 pixels wide,
            // its corners rounded over 185 of them.
            let t = ((p[1] - 0.1) / 0.8).clamp(0.0, 1.0);
            let sky = Paint {
                rgb: [0, 1, 2].map(|i| SKY_TOP.rgb[i] * (1.0 - t) + SKY_BOTTOM.rgb[i] * t),
                alpha: 1.0,
            };
            here = over(
                here,
                sky,
                cover(rounded_box(p, [0.5, 0.5], [0.4023, 0.4023], 0.1807)),
            );
            for motor in MOTORS {
                here = over(here, PROP_BLUR, cover(circle(p, motor, 0.135)));
                here = over(here, ORANGE, cover(circle(p, motor, 0.135).abs() - 0.009));
            }
            here = over(here, FRAME, cover(bar(p, MOTORS[0], MOTORS[3], 0.034)));
            here = over(here, FRAME, cover(bar(p, MOTORS[1], MOTORS[2], 0.034)));
            for motor in MOTORS {
                here = over(here, FRAME, cover(circle(p, motor, 0.045)));
            }
            here = over(
                here,
                ORANGE,
                cover(rounded_box(p, [0.5, 0.5], [0.08, 0.11], 0.035)),
            );
            here = over(here, LENS, cover(circle(p, [0.5, 0.425], 0.027)));
            let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba.extend(here.rgb.map(byte));
            rgba.push(byte(here.alpha));
        }
    }
    rgba
}

// ----- Files -------------------------------------------------------------------

/// The CRC-32 PNG puts after each chunk.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The icon at `size` pixels square, as a PNG file: 8-bit RGBA, each row
/// unfiltered.
pub fn png(size: u32) -> Vec<u8> {
    let rgba = draw(size);
    let mut rows = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks_exact((size * 4) as usize) {
        rows.push(0);
        rows.extend_from_slice(row);
    }
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::best());
    zlib.write_all(&rows).expect("in memory");
    let pixels = zlib.finish().expect("in memory");

    let mut header = Vec::new();
    header.extend(size.to_be_bytes());
    header.extend(size.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]);
    let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", header), (b"IDAT", pixels), (b"IEND", Vec::new())] {
        file.extend((data.len() as u32).to_be_bytes());
        let mut chunk = kind.to_vec();
        chunk.extend(data);
        file.extend(&chunk);
        file.extend(crc32(&chunk).to_be_bytes());
    }
    file
}

fn png_file() -> Vec<u8> {
    png(PNG_SIZE)
}

/// The Windows icon: a header, one entry per size, then each size's PNG.
fn ico_file() -> Vec<u8> {
    let pictures: Vec<(u32, Vec<u8>)> = ICO_SIZES.iter().map(|&s| (s, png(s))).collect();
    let mut file = Vec::new();
    file.extend(0_u16.to_le_bytes());
    file.extend(1_u16.to_le_bytes());
    file.extend((pictures.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pictures.len() as u32;
    for (size, picture) in &pictures {
        // 256 is written as 0: the field is one byte.
        let side = (*size % 256) as u8;
        file.extend([side, side, 0, 0]);
        file.extend(1_u16.to_le_bytes());
        file.extend(32_u16.to_le_bytes());
        file.extend((picture.len() as u32).to_le_bytes());
        file.extend(offset.to_le_bytes());
        offset += picture.len() as u32;
    }
    for (_, picture) in &pictures {
        file.extend(picture);
    }
    file
}

/// The macOS icon: `icns`, its whole length, then each picture with its type
/// and length.
fn icns_file() -> Vec<u8> {
    let mut pictures = Vec::new();
    for (kind, size) in ICNS_PICTURES {
        let picture = png(size);
        pictures.extend(kind);
        pictures.extend((picture.len() as u32 + 8).to_be_bytes());
        pictures.extend(picture);
    }
    let mut file = b"icns".to_vec();
    file.extend((pictures.len() as u32 + 8).to_be_bytes());
    file.extend(pictures);
    file
}
