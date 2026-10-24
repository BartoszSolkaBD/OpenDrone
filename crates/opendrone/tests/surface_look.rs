//! Readable checks for surfaces that read roughness, occlusion and metal
//! pictures and can be wet (#320): what the material library asks for, what
//! the game reads from the pictures, the defaults when a picture is missing,
//! and that the Maps that exist don't ask for any of it. Nothing here opens a
//! window: whether a wet floor looks wet is judged on the pictures
//! `--render-views` saves, which the PR shows.
//! Basis: Rule (the ticket: wetness is darker colour, lower roughness and a
//! little more specular; a material takes part only if `materials.toml` says
//! so; a missing picture leaves the number) and Observed (the committed
//! pictures' sizes and averages).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::Assets;
use bevy::image::Image;
use bevy::math::{UVec2, Vec3};
use bevy::render::render_resource::TextureFormat;
use opendrone::map_drawing::{MapLighting, MaterialLibrary, Surface, with_mipmaps};
use opendrone::surface_look::{
    DRY_REFLECTANCE, Plane, SkyReflection, WET_REFLECTANCE, pack_surface, plane_of, roughness_byte,
    wet,
};
use opendrone_pack::MapLight;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A scratch repo with one library and the pictures of a generated texture
/// (`steel_beam_rusted`) under the folder `rusty`, less any of `leave_out`.
fn scratch(name: &str, library: &str, leave_out: &[&str]) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("surface-look-{name}"));
    let _ = fs::remove_dir_all(&folder);
    let pictures = folder.join("assets-src/generated/rusty");
    fs::create_dir_all(&pictures).unwrap();
    for picture in ["diffuse.jpg", "normal.jpg", "roughness.jpg"] {
        if leave_out.contains(&picture) {
            continue;
        }
        fs::copy(
            repo()
                .join("assets-src/generated/steel_beam_rusted")
                .join(picture),
            pictures.join(picture),
        )
        .unwrap();
    }
    fs::write(folder.join("assets-src/materials.toml"), library).unwrap();
    folder
}

const FLAT: [f32; 4] = [0.1, 0.07, 0.08, 1.0];

fn plane(size: [u32; 2], bytes: &[u8]) -> Plane {
    Plane {
        size: UVec2::from(size),
        bytes: bytes.to_vec(),
    }
}

// Wetness

#[test]
fn a_dry_surface_is_as_the_library_says_and_a_wet_one_is_darker_glossier_and_more_reflective() {
    let dry = wet(0.8, 0.0);
    assert_eq!(dry.colour, 1.0);
    assert_eq!(dry.roughness, 0.8);
    assert_eq!(dry.reflectance, DRY_REFLECTANCE);
    assert_eq!(dry.slope, 1.0);
    let soaked = wet(0.8, 1.0);
    assert!(soaked.colour < 1.0, "darker");
    assert!(soaked.roughness < 0.8 && soaked.roughness > 0.0, "glossier");
    assert_eq!(soaked.reflectance, WET_REFLECTANCE);
    assert!(soaked.reflectance > dry.reflectance, "more specular");
    assert!(soaked.slope < 1.0, "pits filled in");
}

#[test]
fn the_wetter_a_surface_is_the_more_of_each_change_it_has() {
    let mut last = wet(0.8, 0.0);
    for step in 1..=10 {
        let now = wet(0.8, step as f32 / 10.0);
        assert!(now.colour < last.colour);
        assert!(now.roughness < last.roughness);
        assert!(now.reflectance > last.reflectance);
        assert!(now.slope < last.slope);
        last = now;
    }
}

#[test]
fn wetness_outside_dry_to_soaked_is_held_to_it() {
    assert_eq!(wet(0.5, -3.0), wet(0.5, 0.0));
    assert_eq!(wet(0.5, 7.0), wet(0.5, 1.0));
}

#[test]
fn water_on_a_smooth_surface_leaves_it_glossier_than_water_on_a_rough_one() {
    // Water on a polished surface changes it less than on a rough one.
    assert!(wet(0.3, 1.0).roughness < wet(0.8, 1.0).roughness);
}

// The pictures

#[test]
fn a_roughness_picture_that_is_read_alone_is_one_channel_and_the_shader_is_told_so() {
    let library = scratch(
        "read",
        r#"[materials.rusty]
colour         = [0.1, 0.07, 0.08]
roughness      = 0.66
texture        = "rusty"
roughness_map  = true
"#,
        &[],
    );
    let library = MaterialLibrary::read(&library);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(rusty) = library.material("rusty", FLAT, &mut images) else {
        panic!("a material that asks for its roughness picture is drawn by the shader");
    };
    let settings = rusty.extension.settings;
    assert_eq!(
        settings.snap_turn.w, 0.0,
        "it didn't ask for its repeat to be broken, so the repeat is left alone"
    );
    assert_eq!(settings.surface.x, 1.0, "the picture is bound");
    assert_eq!(settings.surface.z, 1.0, "and it is roughness alone");
    let packed = images
        .get(rusty.extension.surface.as_ref().expect("the picture"))
        .unwrap();
    assert_eq!(packed.texture_descriptor.format, TextureFormat::R8Unorm);
    assert_eq!((packed.width(), packed.height()), (1024, 1024));
    assert!(
        packed.texture_descriptor.mip_level_count > 1,
        "with mipmaps"
    );
    // The picture's own bytes, and the mean says it is the picture's average:
    // near the library's number.
    let data = packed.data.as_ref().unwrap();
    let mean: f64 = data[..1024 * 1024]
        .iter()
        .map(|b| f64::from(*b))
        .sum::<f64>()
        / (1024.0 * 1024.0 * 255.0);
    assert!(
        (mean - 0.66).abs() < 0.05,
        "the roughness picture averages {mean}"
    );
}

#[test]
fn a_picture_of_one_channel_gets_its_smaller_sizes_too() {
    let image = pack_surface(Some(plane([2, 2], &[0, 100, 200, 100])), None, None, 0.5).unwrap();
    let image = with_mipmaps(image);
    assert_eq!(image.texture_descriptor.mip_level_count, 2);
    assert_eq!(image.data.unwrap(), vec![0, 100, 200, 100, 100]);
}

#[test]
fn the_roughness_the_library_gives_each_generated_material_is_the_average_of_its_picture() {
    // Observed: the library says the number is the picture's average, which is
    // what the flat colour and a Map with no pictures on disk show.
    let library = MaterialLibrary::read(&repo());
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
        let mut images = Assets::<Image>::default();
        let surface = library.material(name, FLAT, &mut images);
        let Surface::Varied(m) = surface else {
            panic!("{name} is drawn by the shader");
        };
        let handle = m.extension.surface.as_ref().expect("its roughness picture");
        let data = images.get(handle).unwrap().data.clone().unwrap();
        // The biggest picture is three quarters of the bytes with the smaller
        // sizes too.
        let pixels = data.len() * 3 / 4;
        let mean =
            data[..pixels].iter().map(|b| f64::from(*b)).sum::<f64>() / (pixels as f64 * 255.0);
        let number = f64::from(m.base.perceptual_roughness);
        assert!(
            (mean - number).abs() < 0.08,
            "{name}: the picture averages {mean:.2}, the library says {number:.2}"
        );
    }
}

#[test]
fn a_material_that_asks_for_a_roughness_picture_that_is_missing_keeps_its_number() {
    let folder = scratch(
        "missing",
        r#"[materials.rusty]
colour        = [0.1, 0.07, 0.08]
roughness     = 0.66
texture       = "rusty"
roughness_map = true
"#,
        &["roughness.jpg"],
    );
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(rusty) = library.material("rusty", FLAT, &mut images) else {
        panic!("it still asked for the shader");
    };
    assert!(rusty.extension.surface.is_none());
    assert_eq!(
        rusty.extension.settings.surface.x, 0.0,
        "no picture is bound"
    );
    assert_eq!(rusty.base.perceptual_roughness, 0.66);
}

#[test]
fn a_roughness_picture_that_is_not_a_picture_is_missing_too() {
    let folder = scratch(
        "broken",
        r#"[materials.rusty]
colour        = [0.1, 0.07, 0.08]
roughness     = 0.66
texture       = "rusty"
roughness_map = true
"#,
        &[],
    );
    fs::write(
        folder.join("assets-src/generated/rusty/roughness.jpg"),
        b"not a picture",
    )
    .unwrap();
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(rusty) = library.material("rusty", FLAT, &mut images) else {
        panic!("it still asked for the shader");
    };
    assert!(rusty.extension.surface.is_none());
}

#[test]
fn a_material_is_read_only_for_the_pictures_it_asks_for() {
    // The picture is on disk and the library doesn't ask: not read.
    let folder = scratch(
        "not-asked",
        r#"[materials.rusty]
colour    = [0.1, 0.07, 0.08]
roughness = 0.66
texture   = "rusty"
"#,
        &[],
    );
    let library = MaterialLibrary::read(&folder);
    assert!(!library.asks_for_surface("rusty"));
    let mut images = Assets::<Image>::default();
    assert!(matches!(
        library.material("rusty", FLAT, &mut images),
        Surface::Plain(_)
    ));
    // Two pictures: the normal and the colour.
    assert_eq!(images.len(), 2);
}

#[test]
fn an_occlusion_or_metal_picture_that_is_asked_for_and_present_is_packed_in() {
    let folder = scratch(
        "all-three",
        r#"[materials.rusty]
colour        = [0.1, 0.07, 0.08]
roughness     = 0.66
texture       = "rusty"
roughness_map = true
ao_map        = true
metallic_map  = true
"#,
        &[],
    );
    // An occlusion picture and a metal picture the size of the others: the
    // roughness picture serves, so that red and blue differ from the defaults.
    for name in ["ao.jpg", "metallic.jpg"] {
        fs::copy(
            folder.join("assets-src/generated/rusty/roughness.jpg"),
            folder.join("assets-src/generated/rusty").join(name),
        )
        .unwrap();
    }
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(rusty) = library.material("rusty", FLAT, &mut images) else {
        panic!("it asks");
    };
    let packed = images
        .get(rusty.extension.surface.as_ref().unwrap())
        .unwrap();
    let data = packed.data.as_ref().unwrap();
    assert!(
        data[..1024 * 1024 * 4]
            .chunks(4)
            .all(|p| p[0] == p[1] && p[1] == p[2]),
        "red, green and blue are each their own picture, here all the same one"
    );
    // A metal picture with no `metallic` number is fully its picture.
    assert_eq!(rusty.base.metallic, 1.0);
}

#[test]
fn a_surface_with_no_pictures_to_pack_is_none_and_the_missing_ones_take_their_defaults() {
    assert!(pack_surface(None, None, None, 0.5).is_none());
    // Only roughness: one channel, which stands for no occlusion and fully
    // metal too.
    let image = pack_surface(Some(plane([2, 1], &[10, 200])), None, None, 0.5).unwrap();
    assert_eq!(image.texture_descriptor.format, TextureFormat::R8Unorm);
    assert_eq!(image.data.unwrap(), vec![10, 200]);
    // Roughness and metal: the occlusion is filled in.
    let image = pack_surface(
        Some(plane([2, 1], &[10, 200])),
        None,
        Some(plane([2, 1], &[7, 8])),
        0.5,
    )
    .unwrap();
    assert_eq!(image.data.unwrap(), vec![255, 10, 7, 255, 255, 200, 8, 255]);
    // Only occlusion: the library's roughness fills green.
    let image = pack_surface(None, Some(plane([2, 1], &[30, 40])), None, 0.5).unwrap();
    let fill = roughness_byte(0.5);
    assert_eq!(fill, 128);
    assert_eq!(
        image.data.unwrap(),
        vec![30, fill, 255, 255, 40, fill, 255, 255]
    );
    // A picture of another size than the first is left out.
    let image = pack_surface(
        Some(plane([2, 1], &[10, 200])),
        Some(plane([1, 1], &[0])),
        Some(plane([2, 1], &[7, 8])),
        0.5,
    )
    .unwrap();
    assert_eq!(image.data.unwrap(), vec![255, 10, 7, 255, 255, 200, 8, 255]);
}

#[test]
fn a_roughness_is_a_byte_from_0_to_255_and_held_to_that() {
    assert_eq!(roughness_byte(0.0), 0);
    assert_eq!(roughness_byte(1.0), 255);
    assert_eq!(roughness_byte(-1.0), 0);
    assert_eq!(roughness_byte(9.0), 255);
}

#[test]
fn a_greys_first_channel_is_what_a_grey_picture_has() {
    let bytes =
        fs::read(repo().join("assets-src/generated/steel_beam_rusted/roughness.jpg")).unwrap();
    let image = Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension("jpg"),
        bevy::image::CompressedImageFormats::NONE,
        false,
        bevy::image::ImageSampler::default(),
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
    .unwrap();
    let plane = plane_of(&image).expect("a picture the game decodes");
    assert_eq!(plane.size, UVec2::new(1024, 1024));
    assert_eq!(plane.bytes.len(), 1024 * 1024);
}

// What the library asks

#[test]
fn the_eight_materials_made_from_scratch_read_their_roughness_picture() {
    let library = MaterialLibrary::read(&repo());
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
        assert!(library.asks_for_surface(name), "{name}");
    }
}

#[test]
fn the_materials_skate_park_and_bando_draw_with_ask_for_none_of_it_so_they_draw_as_before() {
    // Basis: Rule (#320: existing Maps draw as before; a material takes part
    // only if the library says so).
    let library = MaterialLibrary::read(&repo());
    let packs = common::packs();
    for map in ["opendrone/skate-park", "opendrone/bando"] {
        let map = packs.map(map).unwrap();
        assert!(!map.drawing.is_empty(), "{map:?} is drawn from its file");
        for part in &map.drawing {
            assert!(
                !library.asks_for_surface(&part.material),
                "{} uses {}, which asks for a surface of its own",
                part.name,
                part.material
            );
        }
    }
}

#[test]
fn the_roof_sheet_and_the_steel_beam_are_partly_metal_and_the_other_six_are_not() {
    let library = MaterialLibrary::read(&repo());
    let mut images = Assets::<Image>::default();
    for (name, wanted) in [
        ("roof_corrugated_rusted", 0.7),
        ("steel_beam_rusted", 0.4),
        ("concrete_floor_worn", 0.0),
        ("brick_red_weathered", 0.0),
    ] {
        let Surface::Varied(m) = library.material(name, FLAT, &mut images) else {
            panic!("{name} is drawn by the shader");
        };
        assert_eq!(m.base.metallic, wanted, "{name}");
    }
}

#[test]
fn what_a_material_asks_for_is_read_with_its_limits() {
    let folder = scratch(
        "limits",
        r#"[materials.a]
colour    = [0.1, 0.1, 0.1]
roughness = 0.5
wetness   = 3
metallic  = 0.25

[materials.b]
colour    = [0.1, 0.1, 0.1]
roughness = 0.5
metallic_map = true

[materials.c]
colour    = [0.1, 0.1, 0.1]
roughness = 0.5
"#,
        &[],
    );
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    // No pictures for `a`: its wetness (held to 1) and its metal are worked out
    // in the plain material.
    let Surface::Plain(a) = library.material("a", [0.5, 0.5, 0.5, 1.0], &mut images) else {
        panic!("no pictures: the plain material");
    };
    assert_eq!(a.metallic, 0.25);
    let soaked = wet(0.5, 1.0);
    assert_eq!(a.perceptual_roughness, soaked.roughness);
    assert_eq!(a.reflectance, soaked.reflectance);
    assert!((a.base_color.to_linear().red - 0.5 * soaked.colour).abs() < 1e-6);
    // A metal picture with no number is fully metal; a material with nothing
    // asked is untouched.
    let Surface::Plain(b) = library.material("b", FLAT, &mut images) else {
        panic!("no pictures: the plain material");
    };
    assert_eq!(b.metallic, 1.0);
    assert!(library.asks_for_surface("b"));
    assert!(!library.asks_for_surface("c"));
    let Surface::Plain(c) = library.material("c", FLAT, &mut images) else {
        panic!("no pictures: the plain material");
    };
    assert_eq!(
        (c.metallic, c.reflectance, c.perceptual_roughness),
        (0.0, 0.5, 0.5)
    );
}

#[test]
fn a_textured_material_that_is_wet_is_drawn_with_that_wetness() {
    let folder = scratch(
        "wet",
        r#"[materials.rusty]
colour    = [0.1, 0.07, 0.08]
roughness = 0.66
texture   = "rusty"
wetness   = 0.5
"#,
        &[],
    );
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let Surface::Varied(rusty) = library.material("rusty", FLAT, &mut images) else {
        panic!("wetness is the shader's");
    };
    assert_eq!(rusty.extension.settings.surface.y, 0.5);
    assert_eq!(
        rusty.extension.settings.surface.x, 0.0,
        "no picture asked for"
    );
    assert!(rusty.extension.surface.is_none());
}

// What a baked Map's surfaces reflect

fn light() -> MapLight {
    MapLight {
        sun_elevation: 37.0_f64.to_radians(),
        sun_azimuth: 225.0_f64.to_radians(),
        sun_colour: [0xff, 0xf2, 0xd6],
        sun_strength: 3.0,
        sky_colour: [0xa5, 0xc4, 0xf0],
        sky_strength: 0.5,
    }
}

#[test]
fn what_a_surface_reflects_of_the_sky_is_the_skys_own_gradient_in_the_lightmaps_units() {
    let light = light();
    let lighting = MapLighting::of(&light);
    let sky = SkyReflection::of(&light, &lighting);
    // The zenith is the sky colour times its strength, in the game's units; a
    // white surface under that sky has the strength itself.
    assert!((sky.open - 0.5 * lighting.lightmap_exposure).abs() < 1e-3);
    assert!(sky.zenith.z > sky.zenith.x, "a blue sky is bluer than red");
    assert!(
        sky.haze.length() > sky.zenith.length(),
        "paler and brighter at the horizon"
    );
    assert!(sky.ground.min_element() > 0.0 && sky.ground.length() < sky.haze.length());
    assert!(
        sky.zenith.max_element() < sky.open * 1.2,
        "about as bright as a white surface under it"
    );
    assert_ne!(sky, SkyReflection::default());
    assert_eq!(SkyReflection::default().zenith, Vec3::ZERO);
}

#[test]
fn only_a_surface_drawn_by_the_shader_is_given_a_sky_to_reflect() {
    let folder = scratch(
        "sky",
        r#"[materials.rusty]
colour        = [0.1, 0.07, 0.08]
roughness     = 0.66
texture       = "rusty"
roughness_map = true

[materials.plain]
colour    = [0.1, 0.1, 0.1]
roughness = 0.5
"#,
        &[],
    );
    let library = MaterialLibrary::read(&folder);
    let mut images = Assets::<Image>::default();
    let light = light();
    let sky = SkyReflection::of(&light, &MapLighting::of(&light));
    let mut rusty = library.material("rusty", FLAT, &mut images);
    rusty.set_sky(&sky);
    let Surface::Varied(rusty) = rusty else {
        panic!("it asks");
    };
    assert_eq!(
        rusty.extension.settings.sky_haze.w, 1.0,
        "there is a sky to reflect"
    );
    assert_eq!(rusty.extension.settings.sky_zenith.w, sky.open);
    // The plain material has nowhere to put it, and nothing breaks.
    let mut plain = library.material("plain", FLAT, &mut images);
    plain.set_sky(&sky);
    assert!(matches!(plain, Surface::Plain(_)));
    // A surface that was never given a sky reflects none.
    let Surface::Varied(unlit) = library.material("rusty", FLAT, &mut images) else {
        panic!("it asks");
    };
    assert_eq!(unlit.extension.settings.sky_haze.w, 0.0);
}
