//! Readable checks for how the game lights a Map from its baked light atlas
//! (#294): what the atlas is read into, how the Map's `[light]` becomes the
//! game's light, and what a baked Map is drawn with against one that has no
//! atlas. Nothing here opens a window: it checks the lights, materials and
//! meshes the picture is made of. Basis: Rule (#294 and ADR-0024), and plain
//! geometry for the sun's direction.

mod common;

use std::fs;
use std::path::PathBuf;

use bevy::asset::Assets;
use bevy::ecs::system::RunSystemOnce;
use bevy::image::Image;
use bevy::light::{DirectionalLight, GlobalAmbientLight};
use bevy::mesh::Mesh;
use bevy::pbr::{Lightmap, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use opendrone::drawing::{QUAD_SUN_SHARE, draw_the_map};
use opendrone::map_drawing::{
    MapLighting, ONE_STRENGTH_LUX, half_bits, part_mesh, read_light_atlas,
};
use opendrone::repeat_breaking::VariedMaterial;
use opendrone::set_up::FlightPlan;
use opendrone::settings::Settings;
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

fn scratch(case: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("baked-light")
        .join(case);
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes a 2 x 2 atlas of one white light value in every texel, as Cycles'
/// bake does: OpenEXR, red, green and blue, half floats, ZIP.
fn write_atlas(folder: &std::path::Path, value: f32) -> PathBuf {
    let path = folder.join("test.light.exr");
    let texel = |_: usize, _: usize| [value; 3];
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

/// A square of ground 10 m across, with coordinates into the atlas.
fn ground(light_uvs: Vec<[f32; 2]>) -> DrawnPart {
    DrawnPart {
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
        light_uvs,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

fn atlas_uvs() -> Vec<[f32; 2]> {
    vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
}

/// A flight plan on a small test Map: the ground square, and the light
/// values and atlas given, if any.
fn plan(light_uvs: Vec<[f32; 2]>, light: Option<MapLight>, atlas: Option<PathBuf>) -> FlightPlan {
    let mut plan = FlightPlan::from_packs(&common::packs(), Settings::defaults("whoop-65"))
        .expect("the tracer's Quads fly");
    plan.map.drawing = vec![ground(light_uvs)];
    plan.map.light = light;
    plan.map.light_atlas = atlas;
    plan
}

/// The world the Map is drawn into, as the game would, minus the window.
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

#[test]
fn a_16_bit_decimal_number_is_the_nearest_one() {
    // Basis: Rule (IEEE 754 binary16).
    assert_eq!(half_bits(0.0), 0x0000);
    assert_eq!(half_bits(1.0), 0x3c00);
    assert_eq!(half_bits(0.5), 0x3800);
    assert_eq!(half_bits(-2.0), 0xc000);
    assert_eq!(half_bits(65504.0), 0x7bff);
    assert_eq!(half_bits(1.0e6), 0x7c00, "too big is infinity");
    assert_eq!(half_bits(5.960_464_5e-8), 0x0001, "the smallest denormal");
    assert_eq!(half_bits(1.0e-12), 0x0000, "too small is zero");
    // 1 + 2^-11 is halfway between 1 and the next number up: ties go to even.
    assert_eq!(half_bits(1.0 + 2.0_f32.powi(-11)), 0x3c00);
    assert_eq!(half_bits(1.0 + 3.0 * 2.0_f32.powi(-11)), 0x3c02);
}

#[test]
fn an_openexr_atlas_is_read_into_a_picture_of_half_floats() {
    let path = write_atlas(&scratch("read"), 3.0);
    let mut images = Assets::<Image>::default();
    let handle = read_light_atlas(&path, &mut images).expect("the atlas reads");
    let atlas = images.get(&handle).unwrap();
    assert_eq!(atlas.texture_descriptor.format, TextureFormat::Rgba16Float);
    assert_eq!(
        (atlas.width(), atlas.height()),
        (2, 2),
        "the atlas's size is the picture's"
    );
    let data = atlas.data.as_ref().unwrap();
    assert_eq!(data.len(), 2 * 2 * 8, "four half floats of 2 bytes a texel");
    let numbers: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|n| u16::from_le_bytes(*n))
        .collect();
    // 3.0 in the red, green and blue of every texel.
    for texel in numbers.as_chunks::<4>().0 {
        assert_eq!(&texel[..3], &[half_bits(3.0); 3]);
    }
}

#[test]
fn a_file_that_is_not_an_openexr_picture_says_so() {
    let folder = scratch("garbage");
    let path = folder.join("test.light.exr");
    fs::write(&path, b"not a picture").unwrap();
    let mut images = Assets::<Image>::default();
    let why = read_light_atlas(&path, &mut images).unwrap_err();
    assert!(why.contains("isn't an OpenEXR picture"), "{why}");
    let why = read_light_atlas(&folder.join("missing.exr"), &mut images).unwrap_err();
    assert!(why.contains("can't be read"), "{why}");
}

#[test]
fn the_sun_shines_from_its_compass_direction_and_height() {
    // Basis: Rule (azimuth is clockwise from north, as in map.toml); Bevy's
    // axes are east +x, up +y, north -z.
    let at = |elevation: f64, azimuth: f64| {
        MapLighting::of(&MapLight {
            sun_elevation: elevation * DEGREE,
            sun_azimuth: azimuth * DEGREE,
            ..light()
        })
        .light_travels
    };
    let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-6;
    // A sun in the east on the horizon: its light travels west.
    assert!(near(at(0.0, 90.0), Vec3::new(-1.0, 0.0, 0.0)));
    // In the north: its light travels south, which is +z.
    assert!(near(at(0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)));
    // Overhead: straight down.
    assert!(near(at(90.0, 123.0), Vec3::new(0.0, -1.0, 0.0)));
    // South-west, 37 ° up: its light travels north-east and down.
    let down = at(37.0, 225.0);
    assert!(down.x > 0.0 && down.z < 0.0 && down.y < 0.0, "{down:?}");
    assert!((down.y + (37.0 * DEGREE).sin() as f32).abs() < 1e-6);
}

#[test]
fn a_strength_of_one_is_as_bright_as_the_real_time_sun_the_other_maps_have() {
    // Basis: Rule (Bevy's diffuse light is albedo * lux / pi, and the other
    // Maps' sun is 100,000 lux).
    let lighting = MapLighting::of(&MapLight {
        sun_strength: 1.0,
        sky_strength: 1.0,
        ..light()
    });
    assert_eq!(lighting.sun_illuminance, ONE_STRENGTH_LUX);
    assert_eq!(ONE_STRENGTH_LUX, 100_000.0);
    assert!((lighting.lightmap_exposure * std::f32::consts::PI - ONE_STRENGTH_LUX).abs() < 1.0);
    // An open sky of 1 lights a surface facing up as the atlas does: its
    // ambient brightness is the atlas's unit.
    assert_eq!(lighting.sky_brightness, lighting.lightmap_exposure);
    let double = MapLighting::of(&MapLight {
        sun_strength: 2.0,
        sky_strength: 0.5,
        ..light()
    });
    assert_eq!(double.sun_illuminance, 2.0 * ONE_STRENGTH_LUX);
    assert_eq!(double.sky_brightness, 0.5 * lighting.sky_brightness);
}

#[test]
fn a_part_with_light_coordinates_has_a_second_set_for_every_corner_and_one_without_has_none() {
    let with = part_mesh(&ground(vec![
        [0.1, 0.2],
        [0.3, 0.4],
        [0.5, 0.6],
        [0.7, 0.8],
    ]));
    let Some(bevy::mesh::VertexAttributeValues::Float32x2(second)) =
        with.attribute(Mesh::ATTRIBUTE_UV_1)
    else {
        panic!("no second set of coordinates")
    };
    // Three corners for each of two triangles, each with its own lookup.
    assert_eq!(second.len(), 6);
    assert_eq!(second[0], [0.1, 0.2]);
    assert_eq!(
        second[3],
        [0.1, 0.2],
        "the second triangle starts at corner 0"
    );
    assert_eq!(second[5], [0.7, 0.8]);
    assert!(
        part_mesh(&ground(Vec::new()))
            .attribute(Mesh::ATTRIBUTE_UV_1)
            .is_none()
    );
}

#[test]
fn a_map_with_an_atlas_draws_its_parts_lit_by_it_and_lights_the_quad_from_its_sky() {
    let atlas = write_atlas(&scratch("baked"), 1.0);
    let mut world = drawn(plan(atlas_uvs(), Some(light()), Some(atlas)));
    let lighting = MapLighting::of(&light());

    // Every drawn part has the lightmap and the atlas is one picture.
    let lightmaps: Vec<&Lightmap> = world.query::<&Lightmap>().iter(&world).collect();
    assert_eq!(lightmaps.len(), 1);
    assert_eq!(lightmaps[0].uv_rect, Rect::new(0.0, 0.0, 1.0, 1.0));
    // Its material shows the atlas at the game's unit of light.
    let materials = world.resource::<Assets<StandardMaterial>>();
    assert!(materials.iter().count() > 0);
    for (_, material) in materials.iter() {
        assert_eq!(material.lightmap_exposure, lighting.lightmap_exposure);
    }
    // The sky lights what the atlas doesn't, and not the Map's parts.
    let ambient = world.resource::<GlobalAmbientLight>();
    assert_eq!(ambient.brightness, lighting.sky_brightness);
    assert_eq!(ambient.color, lighting.sky_colour);
    assert!(!ambient.affects_lightmapped_meshes);
    // One sun, a share of the Map's, with no shadows and no diffuse light
    // on the Map's parts, which the atlas already holds.
    let suns: Vec<&DirectionalLight> = world.query::<&DirectionalLight>().iter(&world).collect();
    assert_eq!(suns.len(), 1);
    assert!(!suns[0].shadow_maps_enabled, "no sun shadows on the Map");
    assert!(!suns[0].affects_lightmapped_mesh_diffuse);
    assert_eq!(
        suns[0].illuminance,
        lighting.sun_illuminance * QUAD_SUN_SHARE
    );
    assert_eq!(suns[0].color, lighting.sun_colour);
}

#[test]
fn a_map_with_no_atlas_draws_as_before_with_the_sun_and_its_shadows() {
    let mut world = drawn(plan(Vec::new(), None, None));
    assert_eq!(world.query::<&Lightmap>().iter(&world).count(), 0);
    let suns: Vec<&DirectionalLight> = world.query::<&DirectionalLight>().iter(&world).collect();
    assert_eq!(suns.len(), 1);
    assert!(suns[0].shadow_maps_enabled);
    assert!(suns[0].affects_lightmapped_mesh_diffuse);
    assert_eq!(suns[0].illuminance, 100_000.0);
    let ambient = world.resource::<GlobalAmbientLight>();
    assert_eq!(ambient.brightness, 3000.0);
    assert!(ambient.affects_lightmapped_meshes);
    for (_, material) in world.resource::<Assets<StandardMaterial>>().iter() {
        assert_eq!(material.lightmap_exposure, 1.0);
    }
}

#[test]
fn a_map_whose_atlas_wont_read_is_drawn_with_the_real_time_sun_instead() {
    let folder = scratch("unreadable");
    let atlas = folder.join("test.light.exr");
    fs::write(&atlas, b"not a picture").unwrap();
    let mut world = drawn(plan(atlas_uvs(), Some(light()), Some(atlas)));
    assert_eq!(world.query::<&Lightmap>().iter(&world).count(), 0);
    let suns: Vec<&DirectionalLight> = world.query::<&DirectionalLight>().iter(&world).collect();
    assert!(suns[0].shadow_maps_enabled);
}
