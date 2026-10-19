//! What the merged pass's shader is told each frame, worked out from the
//! picture, the Lens, the Video Look and the camera's limits.

use bevy::math::{UVec2, Vec2, Vec4};
use bevy::render::render_resource::ShaderType;

use super::lens::{Lens, Picture};
use super::look::{ANALOG_GRADE, CameraLimits, DIGITAL_GRADE, VideoLook};

/// The stops of light an sRGB picture holds between black and white with the
/// punchy curve. A camera with a different Dynamic Range squeezes or
/// stretches the light around the middle grey to that many stops (#28 §4).
pub const REFERENCE_STOPS: f32 = 8.6;

/// Everything the merged pass's shader needs, packed in `vec4`s as
/// `fpv_look.wesl` reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct FpvParams {
    /// The picture's left, top, width and height in the window's pixels.
    pub picture: Vec4,
    /// The 4:3 frame's half-diagonal in pixels, the Lens's half FOV in
    /// radians, its curve and what the curve makes of the half FOV.
    pub lens: Vec4,
    /// The tangents the flat render reaches to the right and to the top, and
    /// its width and height in pixels.
    pub source: Vec4,
    /// The Video Look (0 Analog, 1 Digital), the time in seconds, the frame's
    /// number and nothing yet.
    pub mode: Vec4,
    /// Analog: window pixels per analog pixel across, softness, grain and
    /// colour bleed.
    pub analog: Vec4,
    /// The grade: contrast, saturation and Digital's sharpening.
    pub grade: Vec4,
    /// The camera: the dynamic range's factor, window pixels per Analog line
    /// and per Digital line.
    pub camera: Vec4,
}

/// What one frame's picture is made from.
#[derive(Clone, Copy, Debug)]
pub struct FrameInputs {
    pub picture: Picture,
    pub lens: Lens,
    /// How far the flat render reaches, in tangents ([`Lens::flat_reach`]).
    pub reach: Vec2,
    /// The flat render's size in pixels.
    pub source: UVec2,
    pub look: VideoLook,
    /// The camera the Look shows, a Quad's own for Analog.
    pub limits: CameraLimits,
    /// The computer's time, in seconds, wrapped to keep it precise.
    pub time: f32,
    pub frame: u32,
}

impl FrameInputs {
    pub fn params(&self) -> FpvParams {
        let picture = &self.picture;
        let lens = &self.lens;
        // Lines are counted over the camera's own 4:3 frame: a 16:9 picture
        // is a crop of it, so it shows fewer lines.
        let frame_height = picture.frame_height();
        let lines = self.limits.lines.max(60.0);
        let sharpness = self.limits.sharpness.max(60.0);
        let (contrast, saturation, sharpen) = match self.look {
            VideoLook::Analog => (ANALOG_GRADE.contrast, ANALOG_GRADE.saturation, 0.0),
            VideoLook::Digital => (
                DIGITAL_GRADE.contrast,
                DIGITAL_GRADE.saturation,
                DIGITAL_GRADE.sharpen,
            ),
        };
        FpvParams {
            picture: Vec4::new(picture.x, picture.y, picture.width, picture.height),
            lens: Vec4::new(
                picture.frame_radius,
                lens.theta_max,
                lens.curve,
                lens.at_corner,
            ),
            source: Vec4::new(
                self.reach.x,
                self.reach.y,
                self.source.x as f32,
                self.source.y as f32,
            ),
            mode: Vec4::new(
                match self.look {
                    VideoLook::Analog => 0.0,
                    VideoLook::Digital => 1.0,
                },
                self.time,
                self.frame as f32,
                0.0,
            ),
            analog: Vec4::new(
                frame_height / sharpness,
                ANALOG_GRADE.softness,
                ANALOG_GRADE.grain,
                ANALOG_GRADE.colour_bleed,
            ),
            grade: Vec4::new(contrast, saturation, sharpen, 0.0),
            camera: Vec4::new(
                REFERENCE_STOPS / self.limits.dynamic_range.clamp(3.0, 20.0),
                frame_height / lines,
                frame_height / lines,
                0.0,
            ),
        }
    }
}
