//! Readable checks for how the game reads a Map's light atlas, an OpenEXR
//! picture (#294): the file Blender writes, the forms of the format the game
//! reads, and the ones it refuses in a sentence. Basis: Rule (the OpenEXR
//! file format: scanline blocks, `ZIP` as deflate over differences of
//! interleaved halves) and Observed (a file Blender 5.2.2 wrote).

mod common;

use common::exr::{Packing, Picture, Sample, half, write};
use opendrone::light_atlas::read_exr;

/// The three numbers of each texel: all exact in a half float.
fn texel(x: usize, y: usize) -> [f32; 3] {
    [x as f32 + 0.25, y as f32 * 0.5, 3.0 + (x + y) as f32]
}

fn picture(width: usize, height: usize, sample: Sample, packing: Packing) -> Vec<u8> {
    write(&Picture {
        width,
        height,
        sample,
        packing,
        texel: &texel,
    })
}

/// Every texel of `bytes` read back is what `texel` gave, in half floats.
fn assert_reads_back(bytes: &[u8], width: usize, height: usize) {
    let read = read_exr(bytes).expect("the file reads");
    assert_eq!((read.width as usize, read.height as usize), (width, height));
    for y in 0..height {
        for x in 0..width {
            let [r, g, b] = texel(x, y);
            assert_eq!(
                read.texels[y * width + x],
                [half(r), half(g), half(b), 0x3c00],
                "texel ({x}, {y}), alpha is 1"
            );
        }
    }
}

#[test]
fn the_file_blender_writes_reads_with_the_top_row_first() {
    // Basis: Observed. Blender 5.2.2 wrote a 5 x 20 picture, half floats,
    // ZIP, in which the texel at x across and y up from the bottom is
    // (x + 0.25, y / 2, 3 + x + y): see `tests/fixtures/`.
    let bytes = std::fs::read("tests/fixtures/blender-atlas-5x20.exr").unwrap();
    let read = read_exr(&bytes).expect("Blender's file reads");
    assert_eq!((read.width, read.height), (5, 20));
    for row in 0..20 {
        let up = (19 - row) as f32;
        for x in 0..5 {
            let at = row * 5 + x;
            let want = [x as f32 + 0.25, up * 0.5, 3.0 + x as f32 + up];
            assert_eq!(
                read.texels[at],
                [half(want[0]), half(want[1]), half(want[2]), 0x3c00],
                "row {row} from the top, texel {x}"
            );
        }
    }
}

#[test]
fn an_uncompressed_picture_reads_back_as_written() {
    assert_reads_back(&picture(7, 5, Sample::Half, Packing::None), 7, 5);
}

#[test]
fn a_zip_picture_reads_back_as_written_across_blocks_of_sixteen_lines() {
    // 37 lines: two whole blocks and a short one of five.
    assert_reads_back(&picture(9, 37, Sample::Half, Packing::Zip), 9, 37);
}

#[test]
fn a_zips_picture_with_a_block_for_each_line_reads_back_as_written() {
    assert_reads_back(&picture(6, 4, Sample::Half, Packing::Zips), 6, 4);
}

#[test]
fn a_picture_of_32_bit_numbers_is_read_into_half_floats() {
    assert_reads_back(&picture(4, 18, Sample::Float, Packing::Zip), 4, 18);
    assert_reads_back(&picture(4, 3, Sample::Float, Packing::None), 4, 3);
}

#[test]
fn a_file_the_game_cant_read_says_what_is_wrong() {
    let good = picture(4, 4, Sample::Half, Packing::None);
    let says = |bytes: &[u8]| read_exr(bytes).unwrap_err();
    assert!(says(b"not a picture").contains("isn't an OpenEXR picture"));
    assert!(says(&good[..good.len() - 5]).contains("cut short"));
    assert!(says(&good[..30]).contains("cut short"));
    let mut tiled = good.clone();
    tiled[5] |= 0x02;
    assert!(says(&tiled).contains("tiles"));
    let mut two = good.clone();
    two[5] |= 0x10;
    assert!(says(&two).contains("more than one picture"));
    // Compression PIZ, which the game doesn't read.
    let mut piz = good.clone();
    let at = good
        .windows(12)
        .position(|w| w == b"compression\0")
        .unwrap();
    // After "compression\0compression\0", the size (4 bytes), then the code.
    piz[at + 12 + 12 + 4] = 4;
    assert!(says(&piz).contains("doesn't read"), "{}", says(&piz));
    // A channel that isn't R, G and B.
    let mut renamed = good.clone();
    let red = good.windows(2).position(|w| w == b"R\0").unwrap();
    renamed[red] = b'X';
    assert!(says(&renamed).contains("has no R channel"));
}
