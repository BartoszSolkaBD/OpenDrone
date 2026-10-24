//! Readable checks for how the game draws a Map from its `.glb` (#63): the
//! meshes, their normals and tangents, the mipmaps and the material library.
//! Nothing here opens a window: it checks the numbers the picture is made of.
//! Basis: Rule (the Map's own sizes, and plain geometry).

mod common;

use std::fs;

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::image::Image;
use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use opendrone::map_drawing::{CREASE, MaterialLibrary, part_mesh, with_mipmaps};
use opendrone_pack::DrawnPart;

/// A part from corners and triangles, in the Map's axes (east, north, up).
fn part(corners: Vec<[f32; 3]>, triangles: Vec<[u32; 3]>) -> DrawnPart {
    DrawnPart {
        name: "part".to_string(),
        material: "concrete_plaza".to_string(),
        colour: [0.4, 0.4, 0.38, 1.0],
        uvs: corners.iter().map(|c| [c[0], c[1]]).collect(),
        corners,
        light_uvs: Vec::new(),
        triangles,
        decal: None,
        blend: None,
    }
}

fn floats(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<Vec<f32>> {
    match mesh.attribute(attribute).unwrap() {
        VertexAttributeValues::Float32x3(v) => v.iter().map(|a| a.to_vec()).collect(),
        VertexAttributeValues::Float32x4(v) => v.iter().map(|a| a.to_vec()).collect(),
        VertexAttributeValues::Float32x2(v) => v.iter().map(|a| a.to_vec()).collect(),
        other => panic!("{other:?}"),
    }
}

/// Two squares (four triangles' worth: a sheet) that meet at `angle` degrees,
/// as a hinge: one flat on the ground, one rising from its far edge.
fn hinge(angle: f32) -> DrawnPart {
    let (sin, cos) = angle.to_radians().sin_cos();
    part(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0 + cos, sin],
            [0.0, 1.0 + cos, sin],
        ],
        vec![[0, 1, 2], [0, 2, 3], [3, 2, 4], [3, 4, 5]],
    )
}

#[test]
fn a_part_becomes_three_corners_for_each_triangle_with_normals_uvs_and_tangents() {
    let mesh = part_mesh(&hinge(90.0));
    assert_eq!(mesh.count_vertices(), 12);
    for attribute in [
        Mesh::ATTRIBUTE_NORMAL,
        Mesh::ATTRIBUTE_UV_0,
        Mesh::ATTRIBUTE_TANGENT,
    ] {
        assert_eq!(floats(&mesh, attribute).len(), 12);
    }
}

#[test]
fn the_maps_axes_become_bevys_east_stays_east_up_is_up_and_north_is_minus_z() {
    // A corner 3 m east, 4 m north and 5 m up.
    let mesh = part_mesh(&part(
        vec![[3.0, 4.0, 5.0], [4.0, 4.0, 5.0], [3.0, 5.0, 5.0]],
        vec![[0, 1, 2]],
    ));
    assert_eq!(floats(&mesh, Mesh::ATTRIBUTE_POSITION)[0], [3.0, 5.0, -4.0]);
    // Its face looks up, which is +y in Bevy's.
    let normal = &floats(&mesh, Mesh::ATTRIBUTE_NORMAL)[0];
    assert!((normal[1] - 1.0).abs() < 1e-6, "{normal:?}");
}

#[test]
fn an_edge_between_faces_that_lean_far_apart_stays_sharp_and_a_gentle_bend_is_smooth() {
    // At 90° the floor and the wall are far past the crease: each corner keeps
    // its own face's normal.
    let sharp = part_mesh(&hinge(90.0));
    let normals = floats(&sharp, Mesh::ATTRIBUTE_NORMAL);
    for triangle in 0..2 {
        // The floor's triangles look up (+y in Bevy's axes).
        assert!(
            (normals[triangle * 3][1] - 1.0).abs() < 1e-6,
            "{:?}",
            normals[triangle * 3]
        );
    }
    // At 20° the two faces are within the crease angle: along the shared edge
    // each corner leans from its own face toward the other, by how much of
    // the corner's faces (by area) are the other's: here one triangle of
    // three.
    let smooth = part_mesh(&hinge(20.0));
    let normals = floats(&smooth, Mesh::ATTRIBUTE_NORMAL);
    // Triangle 0 is [0, 1, 2]: its corner 2 (index 2) is on the shared edge.
    let corner = &normals[2];
    assert!(
        corner[1] > 20f32.to_radians().cos() && corner[1] < 0.999,
        "{corner:?}"
    );
    assert!(
        corner[2].abs() > 0.05 && corner[2].abs() < 20f32.to_radians().sin(),
        "{corner:?}"
    );
    // And a corner that isn't on the edge keeps its own face's normal.
    assert!((normals[0][1] - 1.0).abs() < 1e-6);
    // The crease is 35°.
    assert!((CREASE - 35f32.to_radians().cos()).abs() < 1e-5);
}

#[test]
fn every_normal_has_length_one_and_every_tangent_is_square_to_it() {
    for angle in [20.0, 60.0, 90.0] {
        let mesh = part_mesh(&hinge(angle));
        let normals = floats(&mesh, Mesh::ATTRIBUTE_NORMAL);
        let tangents = floats(&mesh, Mesh::ATTRIBUTE_TANGENT);
        for (n, t) in normals.iter().zip(&tangents) {
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((length - 1.0).abs() < 1e-5, "{n:?}");
            let along = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
            assert!(along.abs() < 1e-4, "tangent {t:?} leans on normal {n:?}");
            assert!(t[3] == 1.0 || t[3] == -1.0);
        }
    }
}

#[test]
fn the_tangent_runs_the_way_the_pictures_u_grows() {
    // The floor's uv is (east, north): u grows east, which is +x in Bevy's.
    let mesh = part_mesh(&hinge(90.0));
    let tangent = &floats(&mesh, Mesh::ATTRIBUTE_TANGENT)[0];
    assert!((tangent[0] - 1.0).abs() < 1e-5, "{tangent:?}");
}

#[test]
fn mipmaps_run_from_the_picture_down_to_one_pixel_each_the_average_of_four() {
    // 4 × 2 pixels: the left half dark, the right half light.
    let mut data = Vec::new();
    for _row in 0..2 {
        for x in 0..4 {
            let v = if x < 2 { 10 } else { 200 };
            data.extend_from_slice(&[v, v, v, 255]);
        }
    }
    let image = Image::new(
        Extent3d {
            width: 4,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let image = with_mipmaps(image);
    // 4 × 2, then 2 × 1, then 1 × 1: three levels, 32 + 8 + 4 bytes.
    assert_eq!(image.texture_descriptor.mip_level_count, 3);
    let data = image.data.as_ref().unwrap();
    assert_eq!(data.len(), 32 + 8 + 4);
    assert_eq!(&data[32..40], &[10, 10, 10, 255, 200, 200, 200, 255]);
    assert_eq!(&data[40..44], &[105, 105, 105, 255]);
}

#[test]
fn a_material_with_no_pictures_on_disk_is_its_flat_colour_from_the_glb() {
    // The scratch repo has a library, but `cargo xtask textures` never ran.
    let repo = scratch_repo("no-pictures", false);
    let library = MaterialLibrary::read(&repo);
    assert_eq!(library.textured(), 0);
    let mut images = Assets::<Image>::default();
    let mut surface = library.material("concrete_plaza", [0.4, 0.4, 0.38, 1.0], &mut images);
    let material = surface.standard_mut();
    assert!(material.base_color_texture.is_none());
    assert_eq!(material.base_color.to_linear().red, 0.4);
    assert_eq!(material.perceptual_roughness, 0.9);
}

#[test]
fn the_log_counts_a_material_as_pictured_only_when_its_own_pictures_are_on_disk() {
    // #324: "N of its M materials with their pictures" counts the Map's own
    // materials by their own pictures, not the library's count.
    let with = MaterialLibrary::read(&scratch_repo("own-pictures", true));
    assert!(with.has_pictures("concrete_plaza"));
    assert!(!with.has_pictures("no_such_material"));
    let without = MaterialLibrary::read(&scratch_repo("no-own-pictures", false));
    assert!(!without.has_pictures("concrete_plaza"));
}

#[test]
fn a_picture_that_cant_be_read_falls_back_to_the_flat_colour_too() {
    // The files are there but aren't pictures: the Map is still flyable.
    let repo = scratch_repo("bad-pictures", true);
    let library = MaterialLibrary::read(&repo);
    assert_eq!(library.textured(), 1);
    let mut images = Assets::<Image>::default();
    let mut surface = library.material("concrete_plaza", [0.4, 0.4, 0.38, 1.0], &mut images);
    let material = surface.standard_mut();
    assert!(material.base_color_texture.is_none());
    assert_eq!(images.len(), 0);
}

#[test]
fn a_repo_with_no_library_draws_every_material_flat() {
    let repo = std::env::temp_dir().join("opendrone-no-library");
    let _ = fs::create_dir_all(&repo);
    let library = MaterialLibrary::read(&repo);
    let mut images = Assets::<Image>::default();
    let mut surface = library.material("anything", [0.1, 0.2, 0.3, 1.0], &mut images);
    let material = surface.standard_mut();
    assert_eq!(material.base_color.to_linear().green, 0.2);
}

#[test]
fn the_built_in_skate_park_has_a_mesh_for_every_part_it_draws() {
    let packs = common::packs();
    let map = packs.map("opendrone/skate-park").unwrap();
    let triangles: usize = map.drawing.iter().map(|p| p.triangles.len()).sum();
    let vertices: usize = map
        .drawing
        .iter()
        .map(|p| part_mesh(p).count_vertices())
        .sum();
    assert_eq!(vertices, triangles * 3);
    assert!(triangles > 40_000, "{triangles} triangles");
}

#[test]
fn the_built_in_bando_has_a_mesh_for_every_part_it_draws() {
    let packs = common::packs();
    let map = packs.map("opendrone/bando").unwrap();
    let triangles: usize = map.drawing.iter().map(|p| p.triangles.len()).sum();
    let vertices: usize = map
        .drawing
        .iter()
        .map(|p| part_mesh(p).count_vertices())
        .sum();
    assert_eq!(vertices, triangles * 3);
    assert!(triangles > 10_000, "{triangles} triangles");
}

/// A scratch repo with a material library of one material, and, if
/// `pictures`, files where its pictures would be.
fn scratch_repo(case: &str, pictures: bool) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("map-drawing")
        .join(case);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("assets-src")).unwrap();
    fs::write(
        root.join("assets-src/materials.toml"),
        "[materials.concrete_plaza]\ncolour = [0.4, 0.4, 0.38]\nroughness = 0.9\ntexture = \"worn\"\n\n[textures.worn]\n",
    )
    .unwrap();
    if pictures {
        let folder = root.join("assets-src/textures/worn");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("diffuse.jpg"), b"not a jpeg").unwrap();
        fs::write(folder.join("normal.jpg"), b"not a jpeg").unwrap();
    }
    root
}
