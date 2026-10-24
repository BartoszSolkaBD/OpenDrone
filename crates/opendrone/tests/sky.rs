//! Readable checks for the sky of a baked Map (#315): that it follows the
//! Map's `[light]` (the zenith's and the horizon's colour, where the sun is),
//! that it is the same on every computer, that it is laid out as the graphics
//! card reads a cubemap, and that a baked Map has one and a Map with no
//! `[light]` doesn't. Nothing here opens a window. Basis: Rule (#315, and the
//! sky's own definition in `sky.rs`), plain geometry for the sun's direction
//! (azimuth clockwise from north, as in `map.toml`) and WebGPU's cubemap faces.

mod common;

use std::fs;
use std::path::PathBuf;

use bevy::asset::Assets;
use bevy::ecs::system::RunSystemOnce;
use bevy::image::Image;
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureViewDimension};
use opendrone::drawing::draw_the_map;
use opendrone::map_drawing::MapLighting;
use opendrone::repeat_breaking::VariedMaterial;
use opendrone::set_up::FlightPlan;
use opendrone::settings::Settings;
use opendrone::sky::{
    CUBEMAP_SIZE, MapSky, SUN_DISC_GAIN, Sky, texel_direction, texel_world_direction,
};
use opendrone_pack::{DrawnPart, MapLight};

const DEGREE: f64 = std::f64::consts::PI / 180.0;

/// A sun 37 ° up in the south-west, a warm sun and a blue sky, as the example
/// in `docs/building-a-map.md` has them.
fn light() -> MapLight {
    MapLight {
        sun_elevation: 37.0 * DEGREE,
        sun_azimuth: 225.0 * DEGREE,
        sun_colour: [0xff, 0xf2, 0xd6],
        sun_strength: 3.0,
        sky_colour: [0xa5, 0xc4, 0xf0],
        sky_strength: 0.5,
    }
}

const UP: [f64; 3] = [0.0, 1.0, 0.0];
const DOWN: [f64; 3] = [0.0, -1.0, 0.0];

/// A direction `degrees` above the horizon, towards the compass heading
/// `azimuth` (degrees clockwise from north), in Bevy's axes.
fn above_the_horizon(degrees: f64, azimuth: f64) -> [f64; 3] {
    let (elevation, azimuth) = (degrees * DEGREE, azimuth * DEGREE);
    [
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
        -azimuth.cos() * elevation.cos(),
    ]
}

fn linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// What a texel of a cubemap holds, from its half floats.
fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f32::from(bits & 0x3ff);
    sign * match exponent {
        0 => fraction * 2.0_f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + fraction / 1024.0) * 2.0_f32.powi(exponent - 15),
    }
}

fn texel(image: &Image, face: u32, x: u32, y: u32) -> [f32; 3] {
    let size = image.width();
    let at = (((face * size + y) * size + x) * 8) as usize;
    let data = image.data.as_ref().unwrap();
    let channel =
        |c: usize| half_to_f32(u16::from_le_bytes([data[at + 2 * c], data[at + 2 * c + 1]]));
    [channel(0), channel(1), channel(2)]
}

/// Which face and texel of a cubemap the graphics card reads for a
/// direction, as WebGPU's specification says (its table of `sc`, `tc` and
/// `ma`), written out here on its own so that it checks the sky's layout.
fn texel_the_card_reads(direction: [f64; 3], size: u32) -> (u32, u32, u32) {
    let [x, y, z] = direction;
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if x > 0.0 {
            (0, -z, -y, ax)
        } else {
            (1, z, -y, ax)
        }
    } else if ay >= az {
        if y > 0.0 {
            (2, x, z, ay)
        } else {
            (3, x, -z, ay)
        }
    } else if z > 0.0 {
        (4, x, -y, az)
    } else {
        (5, -x, -y, az)
    };
    let to_texel = |c: f64| (((c / ma + 1.0) / 2.0 * f64::from(size)).floor() as u32).min(size - 1);
    (face, to_texel(sc), to_texel(tc))
}

fn brightness(colour: [f32; 3]) -> f32 {
    colour[0] + colour[1] + colour[2]
}

#[test]
fn the_sky_straight_up_is_the_maps_sky_colour_at_its_strength() {
    // With the sun off, so that its glow isn't in it.
    let dark_sun = MapLight {
        sun_strength: 0.0,
        ..light()
    };
    let up = Sky::of(&dark_sun).clear_sky_toward(UP);
    for (c, byte) in [0xa5, 0xc4, 0xf0].into_iter().enumerate() {
        let want = linear(byte) * 0.5;
        assert!((up[c] - want).abs() < 1e-9, "{c}: {} against {want}", up[c]);
    }
    let double = Sky::of(&MapLight {
        sky_strength: 1.0,
        ..dark_sun
    });
    assert!((double.clear_sky_toward(UP)[2] - 2.0 * up[2]).abs() < 1e-9);
    // With the sun on, only its faint glow is added.
    let with_sun = Sky::of(&light()).clear_sky_toward(UP);
    assert!(with_sun[2] > up[2] && with_sun[2] < up[2] * 1.2);
}

#[test]
fn the_sky_at_the_horizon_is_a_paler_whiter_haze_than_at_the_zenith() {
    let sky = Sky::of(&light());
    let (up, low) = (
        sky.clear_sky_toward(UP),
        // Away from the sun, so that its glow isn't in it.
        sky.clear_sky_toward(above_the_horizon(0.5, 45.0)),
    );
    // Whiter: red and green are closer to the blue than at the zenith.
    assert!(low[0] / low[2] > up[0] / up[2], "{low:?} against {up:?}");
    assert!(low[1] / low[2] > up[1] / up[2]);
    // A touch brighter, as a clear sky is.
    assert!(brightness([low[0] as f32, low[1] as f32, low[2] as f32]) > up[0] as f32 * 3.0);
    // And it goes smoothly: halfway up is between the two.
    let middle = sky.clear_sky_toward(above_the_horizon(30.0, 45.0));
    assert!(middle[0] < low[0] && middle[0] > up[0]);
}

#[test]
fn below_the_horizon_is_a_dull_ground_darker_than_the_sky() {
    let sky = Sky::of(&light());
    let ground = sky.clear_sky_toward(DOWN);
    let horizon = sky.clear_sky_toward(above_the_horizon(0.5, 45.0));
    for c in 0..3 {
        assert!(ground[c] > 0.0 && ground[c] < horizon[c], "{c}");
    }
    // Dull: a grey surface (about 0.18) lit by this sun and sky, not a colour.
    let lit_white = 3.0 * (37.0 * DEGREE).sin() + 0.5;
    assert!((ground[1] - 0.18 * 0.95 * lit_white).abs() < 1e-9);
}

#[test]
fn the_sun_is_where_the_maps_elevation_and_azimuth_put_it_in_the_colour_of_its_light() {
    // Basis: Rule (the sun's brightest point in the cubemap is its direction).
    for (elevation, azimuth) in [(37.0, 225.0), (60.0, 90.0), (15.0, 20.0)] {
        let sky = Sky::of(&MapLight {
            sun_elevation: elevation * DEGREE,
            sun_azimuth: azimuth * DEGREE,
            ..light()
        });
        let want = above_the_horizon(elevation, azimuth);
        for (c, wanted) in want.iter().enumerate() {
            assert!(
                (sky.to_sun[c] - wanted).abs() < 1e-6,
                "{c}: {:?}",
                sky.to_sun
            );
        }
        let image = sky.cubemap(128);
        let mut best = (0.0_f32, [0.0; 3], 0);
        for face in 0..6 {
            for y in 0..128 {
                for x in 0..128 {
                    let b = brightness(texel(&image, face, x, y));
                    if b > best.0 {
                        best = (b, texel_world_direction(face, x, y, 128), face);
                    }
                }
            }
        }
        let off = best
            .1
            .iter()
            .zip(want)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            .acos();
        assert!(
            off < 1.5 * DEGREE,
            "the brightest texel is {:.1} ° from the sun at {elevation} °, {azimuth} °",
            off / DEGREE
        );
        // The disc is far brighter than any sky or ground, and as warm as
        // the sun_colour says: it is its colour times the sun's strength.
        // Bevy looks the cube up with the world's z negated.
        let (face, x, y) = texel_the_card_reads([want[0], want[1], -want[2]], 128);
        let disc = texel(&image, face, x, y);
        assert!(f64::from(disc[0]) > 10.0 * sky.zenith[0].max(sky.haze[0]));
        assert!(
            f64::from(disc[0]) > 3.0 * SUN_DISC_GAIN * 0.9 * linear(0xff) * 0.9,
            "{disc:?}"
        );
        let ratio = f64::from(disc[2] / disc[0]);
        assert!(
            (ratio - linear(0xd6) / linear(0xff)).abs() < 0.05,
            "{ratio}"
        );
    }
}

#[test]
fn a_cubemap_is_laid_out_as_the_graphics_card_reads_one() {
    // Basis: Rule (WebGPU's cubemap faces: +X, -X, +Y, -Y, +Z, -Z).
    let size = 16;
    // The middle of a face looks along its axis.
    let axes = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    for (face, axis) in axes.into_iter().enumerate() {
        let d = texel_direction(face as u32, size / 2, size / 2, size);
        let dot: f64 = d.iter().zip(axis).map(|(a, b)| a * b).sum();
        assert!(dot > 0.99, "face {face}: {d:?}");
    }
    // Each texel's direction is read from that same texel again.
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let d = texel_direction(face, x, y, size);
                assert_eq!(
                    texel_the_card_reads(d, size),
                    (face, x, y),
                    "face {face} {x} {y}"
                );
            }
        }
    }
}

#[test]
fn the_sky_is_the_same_picture_every_time_and_has_clouds_that_leave_the_sun_clear() {
    // Basis: Rule (the same on every computer: fixed seed, no clock).
    let sky = Sky::of(&light());
    let a = sky.cubemap(32);
    let b = sky.cubemap(32);
    assert_eq!(a.data, b.data);
    // Some of the sky overhead is cloud, and some is clear.
    let mut covers = Vec::new();
    for azimuth in (0..360).step_by(15) {
        for degrees in [20.0, 40.0, 60.0, 80.0] {
            covers.push(sky.cloud_cover(above_the_horizon(degrees, f64::from(azimuth))));
        }
    }
    assert!(covers.iter().any(|c| *c > 0.5), "no cloud at all");
    assert!(covers.contains(&0.0), "no clear sky at all");
    assert!(covers.iter().all(|c| (0.0..=1.0).contains(c)));
    // Nothing covers the sun, and nothing is cloud on or under the horizon.
    assert_eq!(sky.cloud_cover(sky.to_sun), 0.0);
    assert_eq!(sky.cloud_cover(above_the_horizon(0.0, 100.0)), 0.0);
    assert_eq!(sky.cloud_cover(DOWN), 0.0);
}

#[test]
fn the_cubemap_is_six_faces_of_half_floats_about_3_mb_that_the_card_draws_as_a_cube() {
    let image = Sky::of(&light()).cubemap(CUBEMAP_SIZE);
    assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba16Float);
    assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 6);
    assert_eq!(
        image.texture_view_descriptor.as_ref().unwrap().dimension,
        Some(TextureViewDimension::Cube)
    );
    let bytes = image.data.as_ref().unwrap().len();
    assert_eq!(bytes, 6 * 256 * 256 * 8);
    assert!((3.0e6..3.3e6).contains(&(bytes as f64)), "{bytes}");
}

fn plan(light: Option<MapLight>, atlas: Option<PathBuf>) -> FlightPlan {
    let mut plan = FlightPlan::from_packs(&common::packs(), Settings::defaults("whoop-65"))
        .expect("the tracer's Quads fly");
    plan.map.drawing = vec![DrawnPart {
        name: "ground".to_string(),
        material: "concrete_plaza".to_string(),
        colour: [0.4, 0.4, 0.38, 1.0],
        corners: vec![
            [-5.0, -5.0, 0.0],
            [5.0, -5.0, 0.0],
            [5.0, 5.0, 0.0],
            [-5.0, 5.0, 0.0],
        ],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        light_uvs: if atlas.is_some() {
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        } else {
            Vec::new()
        },
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        decal: None,
        blend: None,
    }];
    plan.map.light = light;
    plan.map.light_atlas = atlas;
    plan
}

fn drawn(plan: FlightPlan) -> World {
    let mut world = World::new();
    world.insert_resource(Assets::<Mesh>::default());
    world.insert_resource(Assets::<StandardMaterial>::default());
    world.insert_resource(Assets::<VariedMaterial>::default());
    world.insert_resource(Assets::<Image>::default());
    world
        .run_system_once(
            move |mut commands: Commands,
                  mut meshes: ResMut<Assets<Mesh>>,
                  mut materials: ResMut<Assets<StandardMaterial>>,
                  mut varied: ResMut<Assets<VariedMaterial>>,
                  mut images: ResMut<Assets<Image>>| {
                draw_the_map(
                    &plan,
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &mut varied,
                    &mut images,
                );
            },
        )
        .unwrap();
    world
}

fn atlas() -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sky");
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join("test.light.exr");
    let texel = |_: usize, _: usize| [1.0; 3];
    fs::write(
        &path,
        common::exr::write(&common::exr::Picture {
            width: 2,
            height: 2,
            sample: common::exr::Sample::Half,
            packing: common::exr::Packing::Zip,
            texel: &texel,
        }),
    )
    .unwrap();
    path
}

#[test]
fn a_baked_map_is_drawn_with_a_sky_at_the_atlas_unit_and_a_map_with_no_light_keeps_its_clear_colour()
 {
    let baked = drawn(plan(Some(light()), Some(atlas())));
    let sky = baked.resource::<MapSky>();
    assert_eq!(sky.brightness, MapLighting::of(&light()).lightmap_exposure);
    let image = baked.resource::<Assets<Image>>().get(&sky.image).unwrap();
    assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 6);

    let plain = drawn(plan(None, None));
    assert!(plain.get_resource::<MapSky>().is_none());
}
