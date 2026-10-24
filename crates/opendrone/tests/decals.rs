//! Readable checks for decals and two materials on one mesh (#321): what the
//! game makes of a blended part's weights and of a decal, which numbers the
//! shader draws them with, and that a decal is never drawn as a rectangle.
//! Nothing here opens a window: whether moss looks like moss is judged on the
//! pictures `--render-views` saves, which the PR shows.
//! Basis: Rule (the ticket: a vertex colour says how much of material B shows
//! through A, with an edge a noise and the pictures' height break up; a decal
//! lies over the surface, alpha-blended, lit by the same light, overlapping
//! ones in a fixed order, no rectangle showing; a decal can be wet) and
//! Observed (the committed pictures).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::Assets;
use bevy::image::Image;
use bevy::math::Vec4;
use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::AlphaMode;
use opendrone::decals::{
    BLEND_DEPTH, DECAL_LAYER_STEP, FILM_SHEEN, MASK_FLOOR, blend_weight, decal_depth_bias,
    decal_over,
};
use opendrone::map_drawing::{Blended, MaterialLibrary, Surface, part_mesh};
use opendrone::surface_look::WET_KEEPS;
use opendrone_pack::{Blend, DrawnPart};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A square of ground, two triangles, with a weight of the second material
/// at each corner if `weights` says.
fn ground(weights: Option<Vec<f32>>) -> DrawnPart {
    DrawnPart {
        name: "yard".to_string(),
        material: "grass_weeds".to_string(),
        colour: [0.09, 0.11, 0.02, 1.0],
        corners: vec![
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 4.0, 0.0],
            [0.0, 4.0, 0.0],
        ],
        uvs: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        light_uvs: Vec::new(),
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        decal: None,
        scatter: None,
        blend: weights.map(|weights| Blend {
            material: "ground_dirt_rubble".to_string(),
            weights,
        }),
    }
}

fn colours(mesh: &Mesh) -> Option<Vec<[f32; 4]>> {
    match mesh.attribute(Mesh::ATTRIBUTE_COLOR)? {
        VertexAttributeValues::Float32x4(v) => Some(v.clone()),
        other => panic!("{other:?}"),
    }
}

// The mesh

#[test]
fn a_blended_part_carries_each_corners_weight_as_the_alpha_of_a_white_vertex_colour() {
    let mesh = part_mesh(&ground(Some(vec![0.0, 0.25, 1.0, 0.5])));
    // Three corners a triangle, in the triangles' order: 0 1 2, 0 2 3.
    let expected: Vec<[f32; 4]> = [0.0, 0.25, 1.0, 0.0, 1.0, 0.5]
        .iter()
        .map(|w| [1.0, 1.0, 1.0, *w])
        .collect();
    assert_eq!(colours(&mesh), Some(expected));
}

#[test]
fn a_part_with_nothing_blended_in_has_no_vertex_colour_so_the_maps_that_exist_draw_as_before() {
    assert_eq!(colours(&part_mesh(&ground(None))), None);
}

#[test]
fn the_skate_park_and_bando_have_no_decals_and_nothing_blended_so_they_draw_as_before() {
    // Basis: Rule (#321: the existing Maps' pictures and render counts don't move).
    let packs = common::packs();
    for id in ["opendrone/skate-park", "opendrone/bando"] {
        let map = packs.map(id).unwrap();
        assert!(!map.drawing.is_empty(), "{id}");
        assert!(
            map.drawing
                .iter()
                .all(|p| p.decal.is_none() && p.blend.is_none()),
            "{id}"
        );
    }
}

// The materials

#[test]
fn grass_with_dirt_blended_in_is_drawn_by_the_shader_with_the_dirt_on_its_own_tiles_and_size() {
    let library = MaterialLibrary::read(&repo());
    let mut images = Assets::<Image>::default();
    let Blended::Drawn(Surface::Varied(m)) =
        library.blend("grass_weeds", "ground_dirt_rubble", &mut images)
    else {
        panic!("both have their pictures in the repo, so the shader draws them");
    };
    let s = &m.extension.settings;
    // Grass's own tiles and roughness picture are as before.
    assert_eq!(s.snap_turn, Vec4::new(0.0, 0.0, 1.0, 1.0));
    // Dirt: hex tiles that turn, a 2 m picture like the grass's (so one
    // repeat to one), roughness 0.92, dry.
    assert_eq!(s.blend_tiles, Vec4::new(0.0, 0.0, 1.0, 1.0));
    assert_eq!(s.blend, Vec4::new(1.0, 1.0, 0.92, 0.0));
    // Its roughness picture, as one channel.
    assert_eq!(s.blend_surface, Vec4::new(1.0, 1.0, 0.0, 0.0));
    assert!(m.extension.blend_diffuse.is_some() && m.extension.blend_normal.is_some());
    assert!(m.extension.blend_surface.is_some());
    assert_eq!(s.decal, Vec4::ZERO);
    assert_eq!(m.base.alpha_mode, AlphaMode::Opaque);
}

#[test]
fn a_second_material_whose_picture_is_a_different_size_is_scaled_to_its_own_size() {
    // Brick (1.8 m to the picture) with concrete (2 m) blended in: the
    // concrete's picture repeats 0.9 times for each of the brick's.
    let library = MaterialLibrary::read(&repo());
    let mut images = Assets::<Image>::default();
    let Blended::Drawn(Surface::Varied(m)) =
        library.blend("brick_red_weathered", "concrete_wall_rough", &mut images)
    else {
        panic!("drawn by the shader");
    };
    assert!((m.extension.settings.blend.y - 0.9).abs() < 1e-6);
    // The concrete keeps its own snapping, not the brick's.
    assert_eq!(
        m.extension.settings.blend_tiles,
        Vec4::new(2.0, 2.0, 0.0, 1.0)
    );
}

/// A scratch repo whose library has grass and dirt, with no pictures on disk.
fn bare_repo(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("decals-{name}"));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(folder.join("assets-src")).unwrap();
    fs::write(
        folder.join("assets-src/materials.toml"),
        "[materials.grass_weeds]\ncolour = [0.09, 0.11, 0.02]\nroughness = 0.84\ntexture = \"grass_weeds\"\n\n\
         [materials.ground_dirt_rubble]\ncolour = [0.12, 0.08, 0.05]\nroughness = 0.92\ntexture = \"ground_dirt_rubble\"\n\n\
         [materials.decal_damp]\ncolour = [0.05, 0.05, 0.04]\nroughness = 0.33\ntexture = \"decal_damp\"\ncover = 0.0\nwetness = 1.0\n",
    )
    .unwrap();
    folder
}

#[test]
fn without_the_pictures_a_blend_is_drawn_in_the_mix_of_the_two_flat_colours() {
    let library = MaterialLibrary::read(&bare_repo("flat"));
    let mut images = Assets::<Image>::default();
    let Blended::Flat(material) = library.blend("grass_weeds", "ground_dirt_rubble", &mut images)
    else {
        panic!("no pictures, so flat colours");
    };
    // White, so the vertex colours show as they are.
    assert_eq!(material.base_color, bevy::color::Color::WHITE);
    let grass = [0.09, 0.11, 0.02, 1.0];
    assert_eq!(library.flat_blend(grass, "ground_dirt_rubble", 0.0), grass);
    assert_eq!(
        library.flat_blend(grass, "ground_dirt_rubble", 1.0),
        [0.12, 0.08, 0.05, 1.0]
    );
    let half = library.flat_blend(grass, "ground_dirt_rubble", 0.5);
    assert!((half[0] - 0.105).abs() < 1e-6 && (half[1] - 0.095).abs() < 1e-6);
}

#[test]
fn a_damp_patch_is_a_decal_drawn_over_what_is_under_it_with_its_mask_and_no_colour_of_its_own() {
    let library = MaterialLibrary::read(&repo());
    let mut images = Assets::<Image>::default();
    let Some(Surface::Varied(m)) = library.decal("decal_damp", 3, &mut images) else {
        panic!("its pictures and mask are in the repo");
    };
    assert_eq!(m.base.alpha_mode, AlphaMode::Premultiplied);
    assert_eq!(m.base.depth_bias, decal_depth_bias(3));
    let s = &m.extension.settings;
    // A decal, showing none of its own colour (cover 0), soaking what is under it.
    assert_eq!(s.decal, Vec4::new(1.0, 0.0, 0.0, 0.0));
    assert_eq!(s.surface.y, 1.0);
    // One picture: no tiles, no large picture over it.
    assert_eq!(s.snap_turn, Vec4::ZERO);
    assert_eq!(s.wide.x, 0.0);
    assert!(m.extension.mask.is_some());
}

#[test]
fn moss_covers_what_is_under_it_wholly_where_its_mask_is_whole() {
    let library = MaterialLibrary::read(&repo());
    let mut images = Assets::<Image>::default();
    let Some(Surface::Varied(m)) = library.decal("decal_moss", 1, &mut images) else {
        panic!("its pictures and mask are in the repo");
    };
    // `cover` left out of the library is 1.
    assert_eq!(m.extension.settings.decal, Vec4::new(1.0, 1.0, 0.0, 0.0));
    // It reads its roughness picture, as the materials do.
    assert!(m.extension.surface.is_some());
}

#[test]
fn a_decal_whose_pictures_arent_on_disk_isnt_drawn_rather_than_drawn_as_a_rectangle() {
    let library = MaterialLibrary::read(&bare_repo("no-decal"));
    let mut images = Assets::<Image>::default();
    assert!(library.decal("decal_damp", 0, &mut images).is_none());
    assert!(library.decal("no_such_material", 0, &mut images).is_none());
}

#[test]
fn every_decal_material_in_the_library_has_its_mask_on_disk() {
    let text = fs::read_to_string(repo().join("assets-src/materials.toml")).unwrap();
    let doc: toml::Table = text.parse().unwrap();
    let materials = doc["materials"].as_table().unwrap();
    let decals: Vec<&String> = materials
        .keys()
        .filter(|n| n.starts_with("decal_"))
        .collect();
    assert!(decals.len() >= 3, "{decals:?}");
    let library = MaterialLibrary::read(&repo());
    for name in decals {
        let texture = materials[name]["texture"].as_str().unwrap();
        assert!(
            repo()
                .join("assets-src/generated")
                .join(texture)
                .join("mask.jpg")
                .is_file(),
            "{name}"
        );
        let mut images = Assets::<Image>::default();
        assert!(library.decal(name, 0, &mut images).is_some(), "{name}");
    }
}

#[test]
fn the_roughness_the_library_gives_each_decal_is_the_average_of_its_picture() {
    // Observed, as for the materials made from scratch.
    let library = MaterialLibrary::read(&repo());
    for name in ["decal_moss", "decal_stain"] {
        let mut images = Assets::<Image>::default();
        let Some(Surface::Varied(m)) = library.decal(name, 0, &mut images) else {
            panic!("{name}");
        };
        let data = images
            .get(m.extension.surface.as_ref().unwrap())
            .unwrap()
            .data
            .clone()
            .unwrap();
        let pixels = data.len() * 3 / 4;
        let mean =
            data[..pixels].iter().map(|b| f64::from(*b)).sum::<f64>() / (pixels as f64 * 255.0);
        let number = f64::from(m.base.perceptual_roughness);
        assert!(
            (mean - number).abs() < 0.08,
            "{name}: {mean:.2} against {number:.2}"
        );
    }
}

// Overlapping decals

#[test]
fn a_higher_layer_is_drawn_after_a_lower_one_from_wherever_the_camera_looks() {
    // Bevy draws see-through parts far to near by their distance plus their
    // depth bias. Two decals that overlap are never 50 m apart, so the higher
    // layer's key is always the larger, and it is drawn later, on top.
    for layer in 0..15u8 {
        let (low, high) = (decal_depth_bias(layer), decal_depth_bias(layer + 1));
        for apart in [-50.0f32, -10.0, 0.0, 10.0, 50.0] {
            assert!(high + apart > low, "layer {layer}, {apart} m apart");
        }
        assert_eq!(high - low, DECAL_LAYER_STEP);
    }
    // Even layer 0 is drawn after a part with no bias at the same place.
    assert!(decal_depth_bias(0) > 0.0);
}

// What the shader does

#[test]
fn where_a_corner_says_none_of_the_second_material_none_shows_and_where_it_says_all_all_shows() {
    for noise in [0.0, 0.3, 0.7, 1.0] {
        for (a, b) in [(0.1, -0.1), (-0.1, 0.1), (0.0, 0.0)] {
            assert_eq!(blend_weight(0.0, noise, a, b), 0.0);
            assert_eq!(blend_weight(1.0, noise, a, b), 1.0);
        }
    }
}

#[test]
fn in_the_middle_of_the_edge_the_higher_of_the_two_pictures_shows_first() {
    // A light stone in the dirt (height above its average) comes through the
    // grass before a dark gap does.
    let stone = blend_weight(0.5, 0.5, 0.0, 0.08);
    let gap = blend_weight(0.5, 0.5, 0.0, -0.08);
    assert!(stone > 0.9, "{stone}");
    assert!(gap < 0.1, "{gap}");
}

#[test]
fn the_noise_moves_the_edge_in_its_middle_and_the_fade_between_the_two_is_soft() {
    let left = blend_weight(0.5, 0.0, 0.0, 0.0);
    let right = blend_weight(0.5, 1.0, 0.0, 0.0);
    assert!(left < 0.1 && right > 0.9, "{left} {right}");
    // With no noise and flat pictures the weight passes through a half, over
    // a stretch of the weight about BLEND_DEPTH wide.
    assert!((blend_weight(0.5, 0.5, 0.0, 0.0) - 0.5).abs() < 1e-6);
    let soft: Vec<f32> = [0.45, 0.5, 0.55]
        .iter()
        .map(|w| blend_weight(*w, 0.5, 0.0, 0.0))
        .collect();
    assert!(
        soft[0] > 0.0 && soft[2] < 1.0,
        "{soft:?} for a depth of {BLEND_DEPTH}"
    );
}

#[test]
fn a_damp_patch_darkens_the_surface_under_it_as_much_as_a_wet_surface_and_adds_a_little_sheen() {
    let damp = decal_over(1.0, 0.0, 1.0);
    assert_eq!(damp.own, 0.0);
    assert!((damp.under - WET_KEEPS).abs() < 1e-6, "{damp:?}");
    assert!((damp.film - FILM_SHEEN).abs() < 1e-6, "{damp:?}");
}

#[test]
fn moss_where_its_mask_is_whole_hides_what_is_under_it_and_half_shows_half() {
    let whole = decal_over(1.0, 1.0, 0.0);
    assert_eq!((whole.own, whole.under, whole.film), (1.0, 0.0, 0.0));
    let half = decal_over(MASK_FLOOR + 0.5 * (1.0 - MASK_FLOOR), 1.0, 0.0);
    assert!(
        (half.own - 0.5).abs() < 1e-5 && (half.under - 0.5).abs() < 1e-5,
        "{half:?}"
    );
}

#[test]
fn where_the_mask_is_only_the_noise_of_its_picture_the_decal_does_nothing() {
    for mask in [0.0, 0.01, MASK_FLOOR] {
        assert_eq!(decal_over(mask, 1.0, 1.0).under, 1.0, "{mask}");
    }
}
