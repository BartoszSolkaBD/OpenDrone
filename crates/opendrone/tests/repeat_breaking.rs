//! Readable checks for surfaces that don't visibly repeat (#304): the numbers
//! the shader makes its picture from, which have a copy in Rust, and what the
//! material library asks the game for. Nothing here opens a window: whether
//! the picture is free of blocks is judged on the pictures `--render-views`
//! saves, which the PR shows.
//! Basis: Rule (the ticket: neighbouring tiles take different shifts, and the
//! large-scale picture differs 20 m apart) and Observed (the committed
//! macro picture's range).

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::math::{Vec2, Vec3};
use opendrone::map_drawing::{MaterialLibrary, Surface};
use opendrone::repeat_breaking::{cell_look, cell_shift, corners_of, wide_lookups};

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The ways one triangle corner can sit next to another on the grid.
const NEIGHBOURS: [[i32; 2]; 3] = [[1, 0], [0, 1], [1, 1]];

#[test]
fn a_corner_looks_the_same_every_time_it_is_asked() {
    for cell in [[0, 0], [3, -7], [-100, 250]] {
        assert_eq!(cell_look(cell), cell_look(cell), "{cell:?}");
    }
}

#[test]
fn neighbouring_corners_shift_the_picture_to_different_places() {
    // A 40 x 40 patch of the grid: no two neighbours take the same shift, and
    // the shifts cover the picture.
    let mut same = 0;
    let (mut low, mut high, mut sum) = ([1.0f32; 2], [0.0f32; 2], [0.0f32; 2]);
    for x in -20..20 {
        for y in -20..20 {
            let here = cell_shift([x, y], [0.0, 0.0]);
            for axis in 0..2 {
                low[axis] = low[axis].min(here[axis]);
                high[axis] = high[axis].max(here[axis]);
                sum[axis] += here[axis];
            }
            for [dx, dy] in NEIGHBOURS {
                if cell_shift([x + dx, y + dy], [0.0, 0.0]) == here {
                    same += 1;
                }
            }
        }
    }
    assert_eq!(same, 0, "neighbours that shift the picture alike");
    for axis in 0..2 {
        assert!(low[axis] < 0.05 && high[axis] > 0.95, "{low:?} {high:?}");
        let mean = sum[axis] / 1600.0;
        assert!((mean - 0.5).abs() < 0.05, "shifts average {mean}");
    }
}

#[test]
fn the_four_numbers_of_a_look_are_not_alike() {
    // A turn, a mirror and a shift that move together would make fewer
    // different tiles than they look like they do.
    let mut pairs = 0;
    for x in -15..15 {
        for y in -15..15 {
            let look = cell_look([x, y]);
            if (look[0] - look[2]).abs() < 0.01 || (look[1] - look[3]).abs() < 0.01 {
                pairs += 1;
            }
        }
    }
    // 900 corners, each pair of numbers within 0.01 about 2% of the time.
    assert!(pairs < 90, "{pairs} corners of 900 have numbers that agree");
}

#[test]
fn snapping_keeps_every_shift_to_whole_multiples_of_the_pictures_grid() {
    // Bricks: 8 across and 12 up (a course is 1/24 of the picture, and the
    // running bond repeats every two).
    let mut across = BTreeSet::new();
    let mut up = BTreeSet::new();
    for x in -15..15 {
        for y in -15..15 {
            let [u, v] = cell_shift([x, y], [8.0, 12.0]);
            assert!((u * 8.0 - (u * 8.0).round()).abs() < 1e-4, "{u}");
            assert!((v * 12.0 - (v * 12.0).round()).abs() < 1e-4, "{v}");
            across.insert((u * 8.0).round() as i32);
            up.insert((v * 12.0).round() as i32);
        }
    }
    assert_eq!(across.len(), 8, "every shift across is used: {across:?}");
    assert_eq!(up.len(), 12, "every shift up is used: {up:?}");
    // A snap of 1 is no shift at all: a picture with its edge on a seam.
    assert_eq!(cell_shift([4, 9], [0.0, 1.0])[1], 0.0);
}

#[test]
fn every_pixel_blends_the_three_corners_of_its_triangle_and_the_weights_add_up_to_one() {
    for step in 0..2000 {
        let uv = [step as f32 * 0.00731 - 3.0, step as f32 * 0.00419 + 1.3];
        let (corners, weights) = corners_of(uv);
        let total: f32 = weights.iter().sum();
        assert!((total - 1.0).abs() < 1e-5, "{uv:?}: {weights:?}");
        assert!(weights.iter().all(|w| (-1e-6..=1.0 + 1e-6).contains(w)));
        assert!(corners[0] != corners[1] && corners[1] != corners[2] && corners[0] != corners[2]);
    }
}

#[test]
fn a_point_half_a_repeat_away_blends_other_corners() {
    // Moving by half of the picture changes which corners a point is among.
    let (a, _) = corners_of([0.3, 0.2]);
    let (b, _) = corners_of([0.8, 0.2]);
    let shared = a.iter().filter(|c| b.contains(c)).count();
    assert!(shared < 3, "{a:?} {b:?}");
}

#[test]
fn the_nearer_a_point_is_to_a_corner_the_more_it_takes_that_corners_picture() {
    // At a corner of the grid the whole weight is that corner's.
    // The corner (0, 0) is at the origin; the corner (1, 0) of the skewed
    // grid is 1/CELLS_PER_REPEAT across.
    let near = 1e-4;
    let (corners, weights) = corners_of([near, near]);
    let at = corners.iter().position(|c| *c == [0, 0]).unwrap();
    assert!(weights[at] > 0.99, "{weights:?}");
}

/// The committed large-scale picture's colour multiplier (1 where it is
/// 128 of 255), as the nearest texel, at `uv` in repeats.
struct Wide {
    texels: Vec<u8>,
    width: usize,
    height: usize,
}

impl Wide {
    fn read() -> Wide {
        let bytes = fs::read(repo().join("assets-src/generated/macro_variation/diffuse.jpg"))
            .expect("the macro picture is committed");
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("jpg"),
            CompressedImageFormats::NONE,
            false,
            ImageSampler::default(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .expect("it is a picture");
        Wide {
            width: image.width() as usize,
            height: image.height() as usize,
            texels: image.data.expect("it has its texels"),
        }
    }

    fn at(&self, uv: Vec2) -> [f32; 3] {
        let x = (uv.x.rem_euclid(1.0) * self.width as f32) as usize % self.width;
        let y = (uv.y.rem_euclid(1.0) * self.height as f32) as usize % self.height;
        let at = (y * self.width + x) * 4;
        [0, 1, 2].map(|c| f32::from(self.texels[at + c]) / 255.0 * 2.0)
    }

    /// What the shader multiplies the colour by where `world` is, on a
    /// surface facing `normal`: the two lookups multiplied.
    fn multiplier(&self, world: Vec3, normal: Vec3) -> [f32; 3] {
        let [first, second] = wide_lookups(world, normal, 1.0 / 80.0);
        let (a, b) = (self.at(first), self.at(second));
        [0, 1, 2].map(|c| a[c] * b[c])
    }
}

fn lightness(m: [f32; 3]) -> f32 {
    m[0] * 0.3 + m[1] * 0.55 + m[2] * 0.15
}

#[test]
fn the_large_scale_picture_differs_20_metres_apart() {
    let wide = Wide::read();
    let mut differ = 0;
    let mut total = 0;
    let (mut darkest, mut lightest) = (f32::MAX, f32::MIN);
    for x in 0..10 {
        for z in 0..10 {
            let here = Vec3::new(x as f32 * 20.0, 0.0, z as f32 * 20.0);
            let there = here + Vec3::new(20.0, 0.0, 0.0);
            let (a, b) = (
                lightness(wide.multiplier(here, Vec3::Y)),
                lightness(wide.multiplier(there, Vec3::Y)),
            );
            darkest = darkest.min(a);
            lightest = lightest.max(a);
            total += 1;
            if (a - b).abs() > 0.02 {
                differ += 1;
            }
        }
    }
    assert!(
        differ * 10 >= total * 8,
        "only {differ} of {total} places differ from the one 20 m east"
    );
    assert!(
        lightest - darkest > 0.3,
        "a 200 m floor runs from {darkest} to {lightest} in lightness"
    );
}

#[test]
fn the_large_scale_picture_changes_the_colour_by_a_few_tenths_and_averages_near_none() {
    let wide = Wide::read();
    let mut sum = 0.0;
    let mut count = 0;
    for x in 0..40 {
        for z in 0..40 {
            let m =
                lightness(wide.multiplier(Vec3::new(x as f32 * 7.0, 0.0, z as f32 * 7.0), Vec3::Y));
            assert!((0.2..=2.0).contains(&m), "{m}");
            sum += m;
            count += 1;
        }
    }
    let mean = sum / count as f32;
    assert!((0.7..=1.15).contains(&mean), "the picture averages {mean}");
}

#[test]
fn a_wall_looks_up_the_picture_by_along_the_wall_and_up_and_a_floor_by_east_and_north() {
    let per_metre = 1.0 / 80.0;
    // A floor: east (x) and north (-z in Bevy's axes), so moving up changes nothing.
    let [floor_a, _] = wide_lookups(Vec3::new(10.0, 0.0, 30.0), Vec3::Y, per_metre);
    let [floor_b, _] = wide_lookups(Vec3::new(10.0, 5.0, 30.0), Vec3::Y, per_metre);
    assert_eq!(floor_a, floor_b);
    // A wall facing east: along the wall is z, and going up changes the lookup.
    let [wall_a, _] = wide_lookups(Vec3::new(0.0, 1.0, 30.0), Vec3::X, per_metre);
    let [wall_b, _] = wide_lookups(Vec3::new(0.0, 6.0, 30.0), Vec3::X, per_metre);
    assert_ne!(wall_a, wall_b);
    // The same point of a wall facing east and one facing west looks the same
    // place up, so a stain carries on round a building.
    let [wall_c, _] = wide_lookups(Vec3::new(0.0, 1.0, 30.0), -Vec3::X, per_metre);
    assert_eq!(wall_a, wall_c);
}

// The library

fn real_library() -> MaterialLibrary {
    MaterialLibrary::read(&repo())
}

#[test]
fn the_eight_materials_made_from_scratch_ask_for_the_repeat_to_be_broken() {
    let library = real_library();
    for name in [
        "brick_red_weathered",
        "brick_painted_peeling",
        "concrete_floor_worn",
        "concrete_wall_rough",
        "steel_beam_rusted",
        "roof_corrugated_rusted",
        "ground_dirt_rubble",
        "grass_weeds",
    ] {
        assert!(library.asks_for_variation(name), "{name}");
    }
}

#[test]
fn the_materials_skate_park_and_bando_draw_with_ask_for_nothing_so_they_draw_as_before() {
    // Basis: Rule (#304: existing Maps and their render counts don't change).
    let library = real_library();
    let packs = common::packs();
    for map in ["opendrone/skate-park", "opendrone/bando"] {
        let map = packs.map(map).unwrap();
        assert!(!map.drawing.is_empty(), "{map:?} is drawn from its file");
        for part in &map.drawing {
            assert!(
                !library.asks_for_variation(&part.material),
                "{} uses {}, which asks to be drawn another way",
                part.name,
                part.material
            );
        }
    }
}

#[test]
fn a_material_that_asks_is_drawn_with_the_settings_the_library_gives_and_one_that_doesnt_is_not() {
    let library = real_library();
    let mut images = Assets::<Image>::default();
    let Surface::Varied(floor) =
        library.material("concrete_floor_worn", [0.17, 0.17, 0.15, 1.0], &mut images)
    else {
        panic!("the worn floor asks for its repeat to be broken");
    };
    let settings = floor.extension.settings;
    // Any shift, and turned (#324: its picture has no slab joints any more;
    // the Factory lays its slabs out as geometry).
    assert_eq!(settings.snap_turn.to_array(), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(settings.wide.x, 0.6);
    assert!(
        (settings.wide.y - 1.0 / 80.0).abs() < 1e-9,
        "80 m to the picture"
    );
    assert!(floor.extension.wide.is_some());
    let Surface::Varied(dirt) =
        library.material("ground_dirt_rubble", [0.1, 0.1, 0.1, 1.0], &mut images)
    else {
        panic!("the dirt asks too");
    };
    assert_eq!(
        dirt.extension.settings.snap_turn.to_array(),
        [0.0, 0.0, 1.0, 1.0]
    );
    // A material the library doesn't ask anything of is the plain one.
    assert!(matches!(
        library.material("foliage", [0.1, 0.24, 0.07, 1.0], &mut images),
        Surface::Plain(_)
    ));
}

#[test]
fn a_material_that_asks_for_the_large_picture_shows_none_of_it_when_the_picture_isnt_there() {
    // A scratch repo with the material pictures but no macro picture.
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("repeat-breaking-no-macro");
    let _ = fs::remove_dir_all(&folder);
    let pictures = folder.join("assets-src/generated/dirt");
    fs::create_dir_all(&pictures).unwrap();
    for picture in ["diffuse.jpg", "normal.jpg"] {
        fs::copy(
            repo()
                .join("assets-src/generated/ground_dirt_rubble")
                .join(picture),
            pictures.join(picture),
        )
        .unwrap();
    }
    fs::write(
        folder.join("assets-src/materials.toml"),
        r#"[materials.dirt]
colour         = [0.1, 0.1, 0.1]
roughness      = 0.9
texture        = "dirt"
break_repeat   = true
macro_strength = 0.7

[macro]
texture = "wide"
"#,
    )
    .unwrap();
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(dirt) = library.material("dirt", [0.1, 0.1, 0.1, 1.0], &mut images) else {
        panic!("it asks to have its repeat broken");
    };
    assert_eq!(dirt.extension.settings.wide.x, 0.0);
    assert!(dirt.extension.wide.is_none());
    assert_eq!(dirt.extension.settings.snap_turn.w, 1.0);
}
