//! A tiny OpenEXR writer for the light atlas checks: just enough of the
//! format to hold a picture of red, green and blue, in scanlines, so each
//! check builds the exact file it needs without Blender.

#![allow(dead_code)]

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

/// How each number of a texel is stored.
#[derive(Clone, Copy)]
pub enum Sample {
    Half,
    Float,
}

/// How a block of lines is stored.
#[derive(Clone, Copy, PartialEq)]
pub enum Packing {
    None,
    Zips,
    Zip,
}

/// A picture to write: `texel(x, y)` gives red, green and blue, `y` counted
/// from the top of the file.
pub struct Picture<'a> {
    pub width: usize,
    pub height: usize,
    pub sample: Sample,
    pub packing: Packing,
    pub texel: &'a dyn Fn(usize, usize) -> [f32; 3],
}

fn attribute(out: &mut Vec<u8>, name: &str, kind: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.extend_from_slice(kind.as_bytes());
    out.push(0);
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}

/// A number as the bits of a half float, for the numbers the checks use,
/// which are all exact in a half float (the writer doesn't round).
pub fn half(value: f32) -> u16 {
    if value == 0.0 {
        return 0;
    }
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    assert!(
        (1..31).contains(&exponent),
        "{value} isn't a plain half float"
    );
    assert_eq!(bits & 0x1fff, 0, "{value} isn't exact in a half float");
    sign | ((exponent as u16) << 10) | ((bits >> 13) & 0x3ff) as u16
}

pub fn write(picture: &Picture<'_>) -> Vec<u8> {
    let bytes_per = match picture.sample {
        Sample::Half => 2,
        Sample::Float => 4,
    };
    let kind: u32 = match picture.sample {
        Sample::Half => 1,
        Sample::Float => 2,
    };
    let mut out = vec![0x76, 0x2f, 0x31, 0x01, 2, 0, 0, 0];
    let mut channels = Vec::new();
    for name in ["B", "G", "R"] {
        channels.extend_from_slice(name.as_bytes());
        channels.push(0);
        channels.extend_from_slice(&kind.to_le_bytes());
        channels.extend_from_slice(&[0, 0, 0, 0]);
        channels.extend_from_slice(&1u32.to_le_bytes());
        channels.extend_from_slice(&1u32.to_le_bytes());
    }
    channels.push(0);
    attribute(&mut out, "channels", "chlist", &channels);
    let (code, per_block) = match picture.packing {
        Packing::None => (0u8, 1),
        Packing::Zips => (2, 1),
        Packing::Zip => (3, 16),
    };
    attribute(&mut out, "compression", "compression", &[code]);
    let mut window = Vec::new();
    for n in [0, 0, picture.width as i32 - 1, picture.height as i32 - 1] {
        window.extend_from_slice(&n.to_le_bytes());
    }
    attribute(&mut out, "dataWindow", "box2i", &window);
    attribute(&mut out, "displayWindow", "box2i", &window);
    attribute(&mut out, "lineOrder", "lineOrder", &[0]);
    out.push(0);

    let blocks = picture.height.div_ceil(per_block);
    let mut bodies = Vec::new();
    for block in 0..blocks {
        let first = block * per_block;
        let lines = per_block.min(picture.height - first);
        let mut raw = Vec::new();
        for y in first..first + lines {
            // Each line holds a whole row of each channel in turn: B, G, R.
            for channel in [2, 1, 0] {
                for x in 0..picture.width {
                    let value = (picture.texel)(x, y)[channel];
                    match picture.sample {
                        Sample::Half => raw.extend_from_slice(&half(value).to_le_bytes()),
                        Sample::Float => raw.extend_from_slice(&value.to_le_bytes()),
                    }
                }
            }
        }
        assert_eq!(raw.len(), lines * picture.width * 3 * bytes_per);
        let stored = if picture.packing == Packing::None {
            raw
        } else {
            // The writer's way: the even bytes then the odd ones, each as the
            // difference from the byte before it, then deflated.
            let half = raw.len().div_ceil(2);
            let mut split = vec![0u8; raw.len()];
            for (i, byte) in raw.iter().enumerate() {
                split[if i % 2 == 0 { i / 2 } else { half + i / 2 }] = *byte;
            }
            let mut deltas = split.clone();
            for i in (1..split.len()).rev() {
                deltas[i] = split[i].wrapping_sub(split[i - 1]).wrapping_add(128);
            }
            let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
            zlib.write_all(&deltas).unwrap();
            zlib.finish().unwrap()
        };
        let mut body = Vec::new();
        body.extend_from_slice(&(first as i32).to_le_bytes());
        body.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        body.extend_from_slice(&stored);
        bodies.push(body);
    }
    let table_end = out.len() + blocks * 8;
    let mut at = table_end;
    for body in &bodies {
        out.extend_from_slice(&(at as u64).to_le_bytes());
        at += body.len();
    }
    for body in bodies {
        out.extend_from_slice(&body);
    }
    out
}
