//! PROTOTYPE (#28). Your own Quad's ducts and props, in frame wherever Tilt and FOV show them.
//! Rough shapes only: a Meteor65-style whoop (65 mm, 35 mm props in ducts) and a generic 5".

use crate::rig::CameraRig;
use crate::settings::{QuadKind, Tuning};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

#[derive(Component)]
pub struct QuadPart(pub QuadKind);

#[derive(Component)]
pub struct Prop;

pub fn spawn_quads(
    mut commands: Commands,
    rig: Query<Entity, With<CameraRig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(rig) = rig.single() else { return };
    let plastic = mats.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.06), perceptual_roughness: 0.6, ..default() });
    let carbon = mats.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.4, ..default() });
    let motor = mats.add(StandardMaterial { base_color: Color::srgb(0.35, 0.35, 0.38), metallic: 0.8, perceptual_roughness: 0.35, ..default() });
    let blur = mats.add(StandardMaterial {
        base_color: Color::srgba(0.08, 0.08, 0.1, 0.35),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.5,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mut parts: Vec<(QuadKind, Handle<Mesh>, Handle<StandardMaterial>, Transform, bool)> = Vec::new();

    // Whoop 65: four ducts at (+-23, +-23) mm, 35 mm props.
    let duct = meshes.add(Extrusion::new(Annulus::new(0.0185, 0.0197), 0.012));
    let disc_w = meshes.add(Cylinder::new(0.0175, 0.0006));
    let bell_w = meshes.add(Cylinder::new(0.005, 0.006));
    let strut = meshes.add(Cuboid::new(0.004, 0.003, 0.03));
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let c = Vec3::new(0.023 * sx, 0.0, 0.023 * sz);
        parts.push((QuadKind::Whoop65, duct.clone(), plastic.clone(), Transform::from_translation(c).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), false));
        parts.push((QuadKind::Whoop65, disc_w.clone(), blur.clone(), Transform::from_translation(c + Vec3::Y * 0.004), true));
        parts.push((QuadKind::Whoop65, bell_w.clone(), motor.clone(), Transform::from_translation(c + Vec3::Y * 0.0005), false));
        let mid = c * 0.5 - Vec3::Y * 0.004;
        parts.push((QuadKind::Whoop65, strut.clone(), plastic.clone(), Transform::from_translation(mid).looking_to(c.normalize(), Vec3::Y), false));
    }
    parts.push((QuadKind::Whoop65, meshes.add(Cuboid::new(0.026, 0.006, 0.03)), plastic.clone(), Transform::from_xyz(0.0, -0.002, 0.004), false));

    // Freestyle 5": X frame, motors at (+-80, +-80) mm, 5" props.
    let arm = meshes.add(Cuboid::new(0.012, 0.005, 0.105));
    let disc_5 = meshes.add(Cylinder::new(0.0635, 0.001));
    let bell_5 = meshes.add(Cylinder::new(0.0125, 0.016));
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let c = Vec3::new(0.08 * sx, 0.0, 0.08 * sz);
        parts.push((QuadKind::Freestyle5, arm.clone(), carbon.clone(), Transform::from_translation(c * 0.55).looking_to(c.normalize(), Vec3::Y), false));
        parts.push((QuadKind::Freestyle5, bell_5.clone(), motor.clone(), Transform::from_translation(c + Vec3::Y * 0.011), false));
        parts.push((QuadKind::Freestyle5, disc_5.clone(), blur.clone(), Transform::from_translation(c + Vec3::Y * 0.022), true));
    }
    parts.push((QuadKind::Freestyle5, meshes.add(Cuboid::new(0.045, 0.004, 0.11)), carbon.clone(), Transform::from_xyz(0.0, 0.0, 0.0), false));
    parts.push((QuadKind::Freestyle5, meshes.add(Cuboid::new(0.045, 0.004, 0.09)), carbon.clone(), Transform::from_xyz(0.0, 0.032, 0.012), false));

    commands.entity(rig).with_children(|c| {
        for (q, m, mat, tf, prop) in parts {
            let mut e = c.spawn((Mesh3d(m), MeshMaterial3d(mat), tf, QuadPart(q), NotShadowCaster, Visibility::Hidden));
            if prop {
                e.insert(Prop);
            }
        }
    });
}

pub fn show_quad(tuning: Res<Tuning>, mut parts: Query<(&QuadPart, &mut Visibility)>) {
    for (p, mut v) in &mut parts {
        let want = if tuning.show_own_quad && p.0 == tuning.quad { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}
