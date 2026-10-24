//! The sky of a baked Map (#315): a gradient and a sun made from the Map's
//! `[light]` values, drawn behind everything the Map is, in place of the flat
//! clear colour the Skate Park and Bando still have.
//!
//! A [`Sky`] is a function from a direction to the light that comes from that
//! way, in the atlas's own units (a white surface under an open sky of
//! `sky_strength` 1 is 1). The game makes a cubemap from it once, when the
//! Map is drawn ([`Sky::cubemap`]), and Bevy's `Skybox` draws that on every
//! camera that draws the Map ([`attach_the_sky`]). It's plain Rust with `libm`
//! maths and a fixed seed, so it is the same picture on every computer.
//!
//! - **The gradient.** `sky_colour` at the zenith, towards a paler, whiter
//!   haze at the horizon, a touch brighter, as a clear sky is.
//! - **The sun.** A disc a little bigger than the real one (so it is a few
//!   texels wide, not one) in `sun_colour`, hundreds of times brighter than
//!   the ground it lights, so a camera that looks at it blows it out and
//!   Auto-exposure darkens the picture, as a real camera does. A soft glow
//!   round it.
//! - **Clouds.** A few soft ones, from layered value noise on a flat layer
//!   overhead. They thin towards the horizon, and leave the sun clear.
//! - **Below the horizon.** A dull ground, lit as a grey surface would be by
//!   this sun and sky. The Backdrop's ground hides it anyway.
//!
//! Bevy's skybox doesn't light anything: the Map's light is in its atlas.

use bevy::image::ImageSampler;
use bevy::light::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use opendrone_pack::MapLight;

use crate::map_drawing::{MapLighting, half_bits};

/// How many texels a side of the sky's cubemap is. 6 faces of 256 x 256 half
/// floats are about 3 MB in the graphics card.
pub const CUBEMAP_SIZE: u32 = 256;

/// How many times brighter than the sun's own strength the sun's disc is.
/// The real sun is tens of thousands of times brighter than the ground it
/// lights; this is as many as half floats and a camera's range need.
pub const SUN_DISC_GAIN: f64 = 60.0;

/// The sun's disc, as an angle from its centre to its edge, in radians: about
/// 1.3 degrees, two and a half times the real sun's, so it is a few texels wide.
const DISC_RADIUS: f64 = 0.022;

/// How much brighter the haze at the horizon is than the zenith.
const HAZE_GAIN: f64 = 1.25;

/// How much of the way from `sky_colour` to white the haze is.
const HAZE_WHITENESS: f64 = 0.55;

/// How reflective the dull ground below the horizon is.
const GROUND_ALBEDO: f64 = 0.18;

/// The ground's colour, as a share of white.
const GROUND_TINT: [f64; 3] = [1.0, 0.95, 0.85];

/// The sky of a baked Map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// The colour straight up, in linear light, in the atlas's units.
    pub zenith: [f64; 3],
    /// The colour at the horizon, in the same.
    pub haze: [f64; 3],
    /// The dull ground below the horizon, in the same.
    pub ground: [f64; 3],
    /// A unit vector towards the sun, in Bevy's axes (east +x, up +y, north -z).
    pub to_sun: [f64; 3],
    /// The sun's colour in linear light times its strength.
    pub sun: [f64; 3],
    /// How bright the clouds are, in linear light.
    cloud: [f64; 3],
}

impl Sky {
    /// The sky a Map's `[light]` makes.
    pub fn of(light: &MapLight) -> Sky {
        let to_sun = -MapLighting::of(light).light_travels;
        let sky = light.sky_colour.map(srgb_to_linear);
        let sun_colour = light.sun_colour.map(srgb_to_linear);
        let sky_strength = light.sky_strength;
        let sun_strength = light.sun_strength;
        let mix = |a: f64, b: f64, t: f64| a + (b - a) * t;
        // What a white surface facing up gets, which a dull ground sends back
        // a share of.
        let up = sun_strength * libm::sin(light.sun_elevation).max(0.0) + sky_strength;
        let mut zenith = [0.0; 3];
        let mut haze = [0.0; 3];
        let mut ground = [0.0; 3];
        let mut sun = [0.0; 3];
        let mut cloud = [0.0; 3];
        for c in 0..3 {
            zenith[c] = sky[c] * sky_strength;
            haze[c] = mix(sky[c], 1.0, HAZE_WHITENESS) * sky_strength * HAZE_GAIN;
            ground[c] = GROUND_TINT[c] * GROUND_ALBEDO * up;
            sun[c] = sun_colour[c] * sun_strength;
            // A cloud is white, lit by the sky's own light and a share of the
            // sun's, with some of the sun's colour in it.
            cloud[c] = mix(1.0, sun_colour[c], 0.35) * (sky_strength * 1.7 + sun_strength * 0.35);
        }
        Sky {
            zenith,
            haze,
            ground,
            to_sun: [
                f64::from(to_sun.x),
                f64::from(to_sun.y),
                f64::from(to_sun.z),
            ],
            sun,
            cloud,
        }
    }

    /// The light from a direction (a unit vector in Bevy's axes), in linear
    /// light in the atlas's units, with no clouds.
    pub fn clear_sky_toward(&self, direction: [f64; 3]) -> [f64; 3] {
        let up = direction[1].clamp(-1.0, 1.0);
        // Pale haze low down, the zenith's colour high up.
        let high = libm::pow(1.0 - up.max(0.0), 3.5);
        // The ground takes over over the few degrees below the horizon.
        let ground = smoothstep(0.0, -0.08, up);
        // The sun: a broad glow, a tight one and the disc.
        let angle = libm::acos(dot(direction, self.to_sun).clamp(-1.0, 1.0));
        let glow = 0.10 * libm::exp(-angle / 0.35) + 0.5 * libm::exp(-angle / 0.05);
        let disc = smoothstep(DISC_RADIUS * 1.25, DISC_RADIUS * 0.85, angle) * SUN_DISC_GAIN;
        std::array::from_fn(|c| {
            let above = self.zenith[c] + (self.haze[c] - self.zenith[c]) * high;
            let colour = above + (self.ground[c] - above) * ground;
            colour + self.sun[c] * (glow + disc) * (1.0 - ground)
        })
    }

    /// The light from a direction, clouds included.
    pub fn toward(&self, direction: [f64; 3]) -> [f64; 3] {
        let clear = self.clear_sky_toward(direction);
        let cover = self.cloud_cover(direction);
        if cover <= 0.0 {
            return clear;
        }
        let mut colour = [0.0; 3];
        for c in 0..3 {
            // A cloud's thick middle is a little darker than its edge.
            let lit = self.cloud[c] * (1.0 - 0.25 * cover * cover);
            colour[c] = clear[c] + (lit - clear[c]) * cover;
        }
        colour
    }

    /// How much cloud there is in a direction, from 0 (clear) to 1 (thick):
    /// layered value noise on a flat layer overhead, thinning to nothing
    /// towards the horizon and in a patch round the sun.
    pub fn cloud_cover(&self, direction: [f64; 3]) -> f64 {
        let up = direction[1];
        if up <= 0.02 {
            return 0.0;
        }
        let angle = libm::acos(dot(direction, self.to_sun).clamp(-1.0, 1.0));
        let near_the_sun = smoothstep(0.06, 0.4, angle);
        let towards_the_horizon = smoothstep(0.02, 0.3, up);
        // Where the ray meets the layer, a little below its own height so
        // that clouds crowd together towards the horizon.
        let x = direction[0] / (up + 0.12) * 1.1;
        let z = direction[2] / (up + 0.12) * 1.1;
        let density = fbm(x + 3.7, z - 1.9);
        smoothstep(0.5, 0.78, density) * 0.9 * near_the_sun * towards_the_horizon
    }

    /// The sky as a cubemap of half floats, `size` texels a side: the six
    /// faces one after another, +X, -X, +Y, -Y, +Z, -Z, as the graphics card
    /// reads them.
    pub fn cubemap(&self, size: u32) -> Image {
        let mut data = Vec::with_capacity((6 * size * size) as usize * 8);
        for face in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    let colour = self.toward(texel_world_direction(face, x, y, size));
                    for value in [colour[0], colour[1], colour[2], 1.0] {
                        // Half floats hold up to 65504, far above the sun.
                        data.extend_from_slice(&half_bits(value as f32).to_le_bytes());
                    }
                }
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba16Float,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
        image.sampler = ImageSampler::linear();
        image
    }
}

/// The unit direction a texel of a cubemap face looks towards in the cube's
/// own, left-handed axes, by the graphics card's own layout (WebGPU's cube map faces): faces are +X, -X, +Y, -Y, +Z,
/// -Z, and `x` and `y` are the texel's column and row, from the top left.
pub fn texel_direction(face: u32, x: u32, y: u32, size: u32) -> [f64; 3] {
    let u = 2.0 * (f64::from(x) + 0.5) / f64::from(size) - 1.0;
    let v = 2.0 * (f64::from(y) + 0.5) / f64::from(size) - 1.0;
    let direction = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    let length = libm::sqrt(dot(direction, direction));
    direction.map(|d| d / length)
}

/// The direction in the world a texel of a cubemap face is drawn for. Bevy's
/// skybox looks the cubemap up with the world's direction with its z negated,
/// because cubemaps are left-handed, so the texel the card reads for a
/// direction's opposite z is the one that is drawn for that direction.
pub fn texel_world_direction(face: u32, x: u32, y: u32, size: u32) -> [f64; 3] {
    let [x, y, z] = texel_direction(face, x, y, size);
    [x, y, -z]
}

/// What the cameras of a drawn baked Map draw behind it.
#[derive(Resource, Clone, Debug)]
pub struct MapSky {
    pub image: Handle<Image>,
    /// What turns the cubemap's atlas units into the cd/m2 Bevy wants: the
    /// same unit the atlas is drawn at.
    pub brightness: f32,
}

impl MapSky {
    /// Makes the sky of a Map's `[light]` and keeps its cubemap in `images`.
    pub fn make(light: &MapLight, images: &mut Assets<Image>) -> MapSky {
        let began = std::time::Instant::now();
        let cubemap = Sky::of(light).cubemap(CUBEMAP_SIZE);
        info!(
            "the sky: a cubemap of 6 x {CUBEMAP_SIZE} x {CUBEMAP_SIZE}, {:.1} MB in the graphics card, made in {:.0} ms",
            cubemap.data.as_ref().map_or(0, Vec::len) as f64 / 1e6,
            began.elapsed().as_secs_f64() * 1e3
        );
        MapSky {
            image: images.add(cubemap),
            brightness: MapLighting::of(light).lightmap_exposure,
        }
    }
}

/// Puts the sky on every camera that draws the world, and takes it off when
/// the Map has none. The FPV Camera, the chase camera and the plain camera of
/// `--render-views` are made in different places, so none of them has to know
/// about the Map's light.
pub fn attach_the_sky(
    sky: Option<Res<MapSky>>,
    bare: Query<Entity, (With<Camera3d>, Without<Skybox>)>,
    skied: Query<Entity, (With<Camera3d>, With<Skybox>)>,
    mut commands: Commands,
) {
    match sky {
        Some(sky) => {
            for camera in &bare {
                commands.entity(camera).insert(Skybox {
                    image: Some(sky.image.clone()),
                    brightness: sky.brightness,
                    ..default()
                });
            }
        }
        None => {
            for camera in &skied {
                commands.entity(camera).remove::<Skybox>();
            }
        }
    }
}

fn srgb_to_linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// 0 at `from`, 1 at `to`, smoothly between (either way round).
fn smoothstep(from: f64, to: f64, x: f64) -> f64 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A number from 0 to 1 for a point of the lattice, from a fixed hash.
fn lattice(ix: i64, iz: i64) -> f64 {
    let mut h = (ix as u32).wrapping_mul(0x9e37_79b1) ^ (iz as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    f64::from(h >> 8) / f64::from(1u32 << 24)
}

/// Smooth value noise, 0 to 1.
fn value_noise(x: f64, z: f64) -> f64 {
    let (fx, fz) = (libm::floor(x), libm::floor(z));
    let (tx, tz) = (x - fx, z - fz);
    let (ix, iz) = (fx as i64, fz as i64);
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (sx, sz) = (fade(tx), fade(tz));
    let top = lattice(ix, iz) + (lattice(ix + 1, iz) - lattice(ix, iz)) * sx;
    let bottom = lattice(ix, iz + 1) + (lattice(ix + 1, iz + 1) - lattice(ix, iz + 1)) * sx;
    top + (bottom - top) * sz
}

/// Four layers of value noise, each twice as fine and half as strong, 0 to 1.
fn fbm(x: f64, z: f64) -> f64 {
    let (mut sum, mut weight, mut scale) = (0.0, 0.5, 1.0);
    let mut total = 0.0;
    for octave in 0..4 {
        // Each layer is shifted so that their lattices don't line up.
        let shift = f64::from(octave) * 17.3;
        sum += weight * value_noise(x * scale + shift, z * scale - shift);
        total += weight;
        weight *= 0.5;
        scale *= 2.0;
    }
    sum / total
}
