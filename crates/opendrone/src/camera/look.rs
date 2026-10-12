//! The two Video Looks and what each one is made of: the camera's limits, the
//! Lens curve, the colour grade and how fast and how far Auto-exposure
//! adapts. The values are #28's, tuned by eye with the maintainer; the camera
//! limits of an Analog picture come from each Quad's definition.

use opendrone_pack::Camera;

use super::lens::{GENTLE_CURVE, STOCK_CURVE};

/// How the pilot's picture is made. It belongs to the pilot, not to the Quad.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoLook {
    #[default]
    Analog,
    Digital,
}

impl VideoLook {
    /// The Lens curve ([`super::lens`]): the stock fisheye for Analog and a
    /// gentler one for Digital, so its centre holds more detail.
    pub fn lens_curve(self) -> f32 {
        match self {
            VideoLook::Analog => STOCK_CURVE,
            VideoLook::Digital => GENTLE_CURVE,
        }
    }

    /// How far Auto-exposure moves and how fast.
    pub fn auto_exposure(self) -> AutoExposureLimits {
        match self {
            VideoLook::Analog => ANALOG_AUTO_EXPOSURE,
            VideoLook::Digital => DIGITAL_AUTO_EXPOSURE,
        }
    }
}

/// What an FPV Camera's live picture holds: the limits Auto-exposure and the
/// softening work inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraLimits {
    /// Stops of light between black and white.
    pub dynamic_range: f32,
    /// Lines from the top of the picture to the bottom.
    pub lines: f32,
    /// How sharp the picture is across, in TV lines per picture height.
    pub sharpness: f32,
}

/// The generic Digital camera, one for every Quad (#28 §2): 10 stops, 1080
/// lines, and the transmitted 1080p picture across.
pub const DIGITAL_CAMERA: CameraLimits = CameraLimits {
    dynamic_range: 10.0,
    lines: 1080.0,
    // 1080p across a 4:3 picture is 1440 pixels, in lines per picture height.
    sharpness: 1080.0,
};

/// The camera a Look shows: a Quad's own for Analog, the generic one for
/// Digital.
pub fn camera_limits(look: VideoLook, camera: &Camera) -> CameraLimits {
    match look {
        VideoLook::Analog => CameraLimits {
            dynamic_range: camera.analog_dynamic_range as f32,
            lines: camera.analog_lines as f32,
            sharpness: camera.analog_sharpness as f32,
        },
        VideoLook::Digital => DIGITAL_CAMERA,
    }
}

/// The Analog Look's own character (#28 §3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalogGrade {
    /// Grain, as a fraction of the picture's brightness.
    pub grain: f32,
    /// How far colour smears sideways, in analog pixels.
    pub colour_bleed: f32,
    pub contrast: f32,
    pub saturation: f32,
    /// How much softer than the camera's own sharpness the picture is, in
    /// analog pixels: 1 is the camera and the link alone.
    pub softness: f32,
}

pub const ANALOG_GRADE: AnalogGrade = AnalogGrade {
    grain: 0.1,
    colour_bleed: 3.0,
    contrast: 1.1,
    saturation: 0.9,
    softness: 1.0,
};

/// The Digital Look's grade: clean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DigitalGrade {
    pub sharpen: f32,
    pub contrast: f32,
    pub saturation: f32,
}

pub const DIGITAL_GRADE: DigitalGrade = DigitalGrade {
    sharpen: 0.0,
    contrast: 1.0,
    saturation: 1.0,
};

/// How far a camera's Auto-exposure may brighten or darken from the sunlit
/// baseline, how fast it does, and where it aims the view's average light (#28 §4).
/// Speeds are in stops a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoExposureLimits {
    pub max_brighten: f32,
    pub max_darken: f32,
    pub speed_brighten: f32,
    pub speed_darken: f32,
    /// Where the view's average light aims, in stops from Bevy's default: 2 below,
    /// which suits the punchy curve.
    pub target: f32,
}

/// Analog's is slower and harsher: up to 5 stops brighter (3 a second) and
/// 1.5 darker (4 a second).
pub const ANALOG_AUTO_EXPOSURE: AutoExposureLimits = AutoExposureLimits {
    max_brighten: 5.0,
    max_darken: 1.5,
    speed_brighten: 3.0,
    speed_darken: 4.0,
    target: -2.0,
};

/// Digital's: up to 6 stops brighter (4 a second) and 2 darker (6 a second).
pub const DIGITAL_AUTO_EXPOSURE: AutoExposureLimits = AutoExposureLimits {
    max_brighten: 6.0,
    max_darken: 2.0,
    speed_brighten: 4.0,
    speed_darken: 6.0,
    target: -2.0,
};
