//! How a surface is wet, rough, shut in or metal (#320): what the material
//! library can ask for beyond colour and normal pictures, and the numbers the
//! game draws it with.
//!
//! - **A roughness picture, an ambient-occlusion picture and a metallic
//!   picture.** A material's folder has `roughness.jpg` (every generated
//!   texture has one), and may have `ao.jpg` and `metallic.jpg`. The game
//!   reads the ones a material asks for (`roughness_map`, `ao_map`,
//!   `metallic_map` in `assets-src/materials.toml`) and packs them into one
//!   picture the shader samples once: red is occlusion, green roughness, blue
//!   metal ([`pack_surface`]). What a picture leaves out is filled in with
//!   what it means when absent: no occlusion (1), the library's own roughness
//!   number and fully metal, which the material's `metallic` number then
//!   scales.
//! - **Wetness.** A material with `wetness` between 0 and 1 is drawn darker,
//!   glossier, a little more reflective and with its small pits filled in, as
//!   far as it is wet ([`wet`]). The shader (`repeat_breaker.wesl`) makes the
//!   same numbers: keep the two in step.
//!
//! A material that asks for none of it is drawn as before. Nothing here
//! touches the Simulation.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// The share of its colour that a fully wet surface keeps: water darkens what
/// it soaks into.
pub const WET_KEEPS: f32 = 0.6;

/// A fully wet surface's roughness is `WET_GLOSS_FLOOR + WET_GLOSS_SCALE *`
/// its dry roughness: a dry 0.8 becomes 0.29, a dry 0.3 becomes 0.14.
pub const WET_GLOSS_FLOOR: f32 = 0.05;
pub const WET_GLOSS_SCALE: f32 = 0.30;

/// A dry surface reflects `0.5` (about 4% of the light head-on); a fully wet
/// one this, a little more.
pub const DRY_REFLECTANCE: f32 = 0.5;
pub const WET_REFLECTANCE: f32 = 0.65;

/// How much of a normal map's slope a fully wet surface keeps: water fills the
/// small pits, so the surface reads smoother.
pub const WET_KEEPS_SLOPE: f32 = 0.6;

/// What being wet does to a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wet {
    /// What the colour is multiplied by (1 for dry).
    pub colour: f32,
    pub roughness: f32,
    pub reflectance: f32,
    /// What the normal map's slope is multiplied by (1 for dry).
    pub slope: f32,
}

/// A surface of dry roughness `roughness`, wet by `wetness` (0 dry, 1
/// soaked; outside that it is held to it).
pub fn wet(roughness: f32, wetness: f32) -> Wet {
    let w = wetness.clamp(0.0, 1.0);
    let mix = |dry: f32, soaked: f32| dry + (soaked - dry) * w;
    Wet {
        colour: mix(1.0, WET_KEEPS),
        roughness: mix(roughness, WET_GLOSS_FLOOR + WET_GLOSS_SCALE * roughness),
        reflectance: mix(DRY_REFLECTANCE, WET_REFLECTANCE),
        slope: mix(1.0, WET_KEEPS_SLOPE),
    }
}

/// One channel of a picture: its size and a byte a pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub size: UVec2,
    pub bytes: Vec<u8>,
}

/// A picture's first channel, which is all a grey picture has: the game reads
/// a JPG as four bytes a pixel, with grey copied into each. `None` if the
/// picture has no pixels or is in some other form.
pub fn plane_of(image: &Image) -> Option<Plane> {
    let data = image.data.as_ref()?;
    let size = UVec2::new(image.width(), image.height());
    let pixels = size.x as usize * size.y as usize;
    let step = match image.texture_descriptor.format {
        TextureFormat::R8Unorm => 1,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => 4,
        _ => return None,
    };
    if pixels == 0 || data.len() != pixels * step {
        return None;
    }
    Some(Plane {
        size,
        bytes: data.iter().step_by(step).copied().collect(),
    })
}

/// A roughness as the byte that stands for it in a picture.
pub fn roughness_byte(roughness: f32) -> u8 {
    (roughness.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The three pictures as one: red is occlusion, green roughness, blue metal.
/// What is missing is filled in with its default (no occlusion, `roughness`,
/// fully metal), and a picture whose size isn't the first one's is left out
/// the same way. `None` when no picture is given, as there is nothing to say.
/// The picture has no smaller sizes yet.
pub fn pack_surface(
    roughness_picture: Option<Plane>,
    ao_picture: Option<Plane>,
    metallic_picture: Option<Plane>,
    roughness: f32,
) -> Option<Image> {
    let size = [&roughness_picture, &ao_picture, &metallic_picture]
        .into_iter()
        .flatten()
        .map(|p| p.size)
        .next()?;
    let fits = |p: Option<Plane>| p.filter(|p| p.size == size);
    let (rough, ao, metal) = (
        fits(roughness_picture),
        fits(ao_picture),
        fits(metallic_picture),
    );
    let fill = roughness_byte(roughness);
    let pixels = size.x as usize * size.y as usize;
    let mut data = Vec::with_capacity(pixels * 4);
    for i in 0..pixels {
        data.push(ao.as_ref().map_or(255, |p| p.bytes[i]));
        data.push(rough.as_ref().map_or(fill, |p| p.bytes[i]));
        data.push(metal.as_ref().map_or(255, |p| p.bytes[i]));
        data.push(255);
    }
    let mut image = Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    Some(image)
}
