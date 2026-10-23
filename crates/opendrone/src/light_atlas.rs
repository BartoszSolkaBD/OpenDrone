//! Reading a Map's light atlas: the OpenEXR picture `cargo xtask maps --bake`
//! writes (#293), into the half floats the graphics card holds it as (#294).
//!
//! The game reads the file itself, in a few lines, and not with Bevy's `exr`
//! feature: that brings in the `exr` crate, which turns on `num-complex`'s
//! `std` for the whole workspace, which the core must never have (ADR-0003,
//! `cargo xtask walls`).
//!
//! It reads what a bake writes and what Blender and most tools save by
//! default: one picture in scanlines, with the red, green and blue channels
//! as half or 32-bit floats, uncompressed or compressed with `ZIP` or `ZIPS`.
//! A file in any other form is refused with a sentence that says so.

use std::io::Read;

use flate2::read::ZlibDecoder;

/// The first bytes of every OpenEXR file.
const MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];

/// 1.0 as 16 bits of a half float: the alpha of every texel.
const HALF_ONE: u16 = 0x3c00;

/// A picture of half floats: the bits of red, green, blue and alpha of each
/// texel, a row at a time, the top row first.
#[derive(Clone, Debug, PartialEq)]
pub struct HalfPicture {
    pub width: u32,
    pub height: u32,
    pub texels: Vec<[u16; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sample {
    Half,
    Float,
}

#[derive(Clone, Debug)]
struct Channel {
    name: String,
    sample: Sample,
}

/// Reads an OpenEXR file into half floats, or says why it can't.
pub fn read_exr(bytes: &[u8]) -> Result<HalfPicture, String> {
    if !bytes.starts_with(&MAGIC) {
        return Err("isn't an OpenEXR picture".to_string());
    }
    let mut at = Cursor { bytes, at: 4 };
    let version = at.u32()?;
    if version & 0xff != 2 {
        return Err("is an OpenEXR version the game doesn't read".to_string());
    }
    if version & 0x0200 != 0 {
        return Err("is made of tiles, and the game reads scanlines".to_string());
    }
    if version & 0x1800 != 0 {
        return Err("has more than one picture, and the game reads one".to_string());
    }
    let mut channels = Vec::new();
    let mut compression = None;
    let mut window = None;
    loop {
        let name = at.text()?;
        if name.is_empty() {
            break;
        }
        let _kind = at.text()?;
        let size = at.u32()? as usize;
        let value = at.take(size)?;
        match name.as_str() {
            "channels" => channels = read_channels(value)?,
            "compression" => compression = value.first().copied(),
            "dataWindow" => {
                if value.len() < 16 {
                    return Err("has a data window that is cut short".to_string());
                }
                let n = |i: usize| i32::from_le_bytes(value[i * 4..i * 4 + 4].try_into().unwrap());
                window = Some((n(0), n(1), n(2), n(3)));
            }
            _ => {}
        }
    }
    let compression = compression.ok_or("has no compression in its header")?;
    let lines_per_block = match compression {
        0 | 2 => 1,
        3 => 16,
        _ => {
            return Err(
                "is compressed in a way the game doesn't read: save it uncompressed or with ZIP"
                    .to_string(),
            );
        }
    };
    let (x_min, y_min, x_max, y_max) = window.ok_or("has no data window in its header")?;
    if x_max < x_min || y_max < y_min {
        return Err("has an empty data window".to_string());
    }
    let (width, height) = ((x_max - x_min + 1) as usize, (y_max - y_min + 1) as usize);
    if width.saturating_mul(height) > 1 << 28 {
        return Err("is bigger than the game reads".to_string());
    }
    let find = |name: &str| {
        channels
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| format!("has no {name} channel"))
    };
    let (red, green, blue) = (find("R")?, find("G")?, find("B")?);
    let line_bytes: usize = channels.iter().map(|c| width * c.sample.size()).sum();
    let blocks = height.div_ceil(lines_per_block);
    // Where each block is, in the table that follows the header.
    let mut offsets = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        offsets.push(at.u64()? as usize);
    }
    let mut texels = vec![[0, 0, 0, HALF_ONE]; width * height];
    for offset in offsets {
        let mut block = Cursor { bytes, at: offset };
        let first = i64::from(block.u32()? as i32) - i64::from(y_min);
        let stored = block.u32()? as usize;
        let data = block.take(stored)?;
        if first < 0 || first as usize >= height {
            return Err("has a block of lines outside its picture".to_string());
        }
        let first = first as usize;
        let lines = lines_per_block.min(height - first);
        let wanted = lines * line_bytes;
        let raw = if stored == wanted {
            data.to_vec()
        } else if compression == 0 {
            return Err("has a block whose size isn't its lines' size".to_string());
        } else {
            undo_zip(data, wanted)?
        };
        for line in 0..lines {
            let mut place = line * line_bytes;
            for (number, channel) in channels.iter().enumerate() {
                let row = &raw[place..place + width * channel.sample.size()];
                place += row.len();
                let slot = [red, green, blue].iter().position(|c| *c == number);
                let Some(slot) = slot else { continue };
                for (x, texel) in texels[(first + line) * width..][..width]
                    .iter_mut()
                    .enumerate()
                {
                    texel[slot] = channel.sample.half_bits(row, x);
                }
            }
        }
    }
    Ok(HalfPicture {
        width: width as u32,
        height: height as u32,
        texels,
    })
}

impl Sample {
    fn size(self) -> usize {
        match self {
            Sample::Half => 2,
            Sample::Float => 4,
        }
    }

    /// Texel `x` of a row of this kind of sample, as the bits of a half float.
    fn half_bits(self, row: &[u8], x: usize) -> u16 {
        match self {
            Sample::Half => u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]),
            Sample::Float => {
                let at = x * 4;
                crate::map_drawing::half_bits(f32::from_le_bytes([
                    row[at],
                    row[at + 1],
                    row[at + 2],
                    row[at + 3],
                ]))
            }
        }
    }
}

fn read_channels(value: &[u8]) -> Result<Vec<Channel>, String> {
    let mut at = Cursor {
        bytes: value,
        at: 0,
    };
    let mut channels = Vec::new();
    loop {
        let name = at.text()?;
        if name.is_empty() {
            return Ok(channels);
        }
        let sample = match at.u32()? {
            1 => Sample::Half,
            2 => Sample::Float,
            _ => return Err(format!("has a {name} channel that isn't decimal numbers")),
        };
        // Whether it is linear, three bytes kept for later, and how many
        // samples it skips along and down.
        let _ = at.take(4)?;
        let (across, down) = (at.u32()?, at.u32()?);
        if across != 1 || down != 1 {
            return Err(format!("has a {name} channel with samples left out"));
        }
        channels.push(Channel { name, sample });
    }
}

/// A `ZIP` or `ZIPS` block back to the bytes of its lines: inflated, then the
/// running differences added back and the two halves interleaved again, as
/// the format's writer split them to compress better.
fn undo_zip(data: &[u8], wanted: usize) -> Result<Vec<u8>, String> {
    let mut inflated = Vec::with_capacity(wanted);
    ZlibDecoder::new(data)
        .read_to_end(&mut inflated)
        .map_err(|e| format!("has a block that doesn't inflate: {e}"))?;
    if inflated.len() != wanted {
        return Err("has a block that inflates to the wrong size".to_string());
    }
    for i in 1..inflated.len() {
        inflated[i] = inflated[i - 1].wrapping_add(inflated[i]).wrapping_sub(128);
    }
    let half = inflated.len().div_ceil(2);
    let (first, second) = inflated.split_at(half);
    let mut bytes = Vec::with_capacity(wanted);
    for (i, byte) in first.iter().enumerate() {
        bytes.push(*byte);
        if let Some(next) = second.get(i) {
            bytes.push(*next);
        }
    }
    Ok(bytes)
}

/// A reader over bytes, which says "cut short" instead of panicking.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len());
        let end = end.ok_or("is cut short")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// Text up to its zero byte.
    fn text(&mut self) -> Result<String, String> {
        let rest = &self.bytes[self.at.min(self.bytes.len())..];
        let end = rest.iter().position(|b| *b == 0).ok_or("is cut short")?;
        let text = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.at += end + 1;
        Ok(text)
    }
}
