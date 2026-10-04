//! PROTOTYPE (#28). Every knob the maintainer can tune, with #14's starting values, and saving
//! a tuned set to a TOML file.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MapKind {
    #[default]
    SkatePark,
    Bando,
}

impl MapKind {
    pub fn label(self) -> &'static str {
        match self {
            MapKind::SkatePark => "Skate Park",
            MapKind::Bando => "Bando",
        }
    }
    pub fn glb(self) -> &'static str {
        match self {
            MapKind::SkatePark => "skate_park_ba.glb",
            MapKind::Bando => "bando_a_rooms.glb",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QuadKind {
    #[default]
    Whoop65,
    Freestyle5,
}

impl QuadKind {
    pub fn label(self) -> &'static str {
        match self {
            QuadKind::Whoop65 => "Whoop 65",
            QuadKind::Freestyle5 => "Freestyle 5\"",
        }
    }
    pub fn default_fov(self) -> f32 {
        match self {
            QuadKind::Whoop65 => 160.0,
            QuadKind::Freestyle5 => 155.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Look {
    #[default]
    Analog,
    Digital,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Aspect {
    #[default]
    FourThree,
    SixteenNine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BreakupLevel {
    Off,
    #[default]
    Light,
    Medium,
    Realistic,
}

impl BreakupLevel {
    pub const ALL: [BreakupLevel; 4] =
        [BreakupLevel::Off, BreakupLevel::Light, BreakupLevel::Medium, BreakupLevel::Realistic];
    pub fn label(self) -> &'static str {
        match self {
            BreakupLevel::Off => "Off",
            BreakupLevel::Light => "Light",
            BreakupLevel::Medium => "Medium",
            BreakupLevel::Realistic => "Realistic",
        }
    }
    pub fn next(self) -> Self {
        match self {
            BreakupLevel::Off => BreakupLevel::Light,
            BreakupLevel::Light => BreakupLevel::Medium,
            BreakupLevel::Medium => BreakupLevel::Realistic,
            BreakupLevel::Realistic => BreakupLevel::Off,
        }
    }
}

/// Auto-exposure, per Video Look. Speeds are in stops (EV) per second; the range is how far the
/// camera may brighten or darken from the sunlit baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExposureTuning {
    pub enabled: bool,
    pub speed_brighten: f32,
    pub speed_darken: f32,
    pub max_brighten_ev: f32,
    pub max_darken_ev: f32,
}

impl Default for ExposureTuning {
    fn default() -> Self {
        Self { enabled: true, speed_brighten: 3.0, speed_darken: 4.0, max_brighten_ev: 4.0, max_darken_ev: 3.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalogTuning {
    /// How big the Map is drawn underneath the fisheye, relative to the picture's pixels on screen.
    pub source_scale: f32,
    /// The analog picture's height in lines (720 = "about 960x720").
    pub lines: f32,
    /// Blur, in analog pixels.
    pub softness: f32,
    pub grain: f32,
    /// Sideways colour smear, in analog pixels.
    pub colour_bleed: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub brightness: f32,
    /// 0 = the stock fisheye curve (equidistant); higher = gentler, closer to a flat lens.
    pub lens_curve: f32,
    pub exposure: ExposureTuning,
}

impl Default for AnalogTuning {
    fn default() -> Self {
        Self {
            source_scale: 1.0,
            lines: 720.0,
            softness: 1.0,
            grain: 0.035,
            colour_bleed: 3.0,
            contrast: 1.12,
            saturation: 0.9,
            brightness: 1.0,
            lens_curve: 0.0,
            exposure: ExposureTuning {
                enabled: true,
                speed_brighten: 1.5,
                speed_darken: 2.5,
                max_brighten_ev: 3.0,
                max_darken_ev: 2.0,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigitalTuning {
    pub source_scale: f32,
    pub lens_curve: f32,
    pub sharpen: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub brightness: f32,
    /// Digital always shows the picture this much later than Analog.
    pub delay_ms: f32,
    /// Extra delay added near the edge of range.
    pub edge_extra_delay_ms: f32,
    /// After a freeze, the picture comes back this long after the signal does.
    pub relock_s: f32,
    pub exposure: ExposureTuning,
}

impl Default for DigitalTuning {
    fn default() -> Self {
        Self {
            source_scale: 1.0,
            lens_curve: 0.0,
            sharpen: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            brightness: 1.0,
            delay_ms: 15.0,
            edge_extra_delay_ms: 20.0,
            relock_s: 1.0,
            exposure: ExposureTuning {
                enabled: true,
                speed_brighten: 4.0,
                speed_darken: 6.0,
                max_brighten_ev: 5.0,
                max_darken_ev: 3.0,
            },
        }
    }
}

/// The Video Signal: a link budget at 5.8 GHz. Received power (dBm) = VTX power x Breakup-level
/// multiplier - free-space loss - wall loss (+ a little flutter). Thresholds map it to Breakup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SignalTuning {
    pub vtx_mw_whoop: f32,
    pub vtx_mw_five: f32,
    pub light_power_mult: f32,
    pub medium_power_mult: f32,
    pub realistic_power_mult: f32,
    /// Loss per metre of concrete the line passes through (brick counts as concrete).
    pub wall_db_per_m: f32,
    /// A slanted line through a slab can cross a lot of it; count at most this much per wall.
    pub wall_max_m: f32,
    /// One-sided surfaces (the Skate Park's bowls, ground sheets) count as this thick.
    pub open_surface_m: f32,
    pub analog_clean_dbm: f32,
    pub analog_lost_dbm: f32,
    pub digital_perfect_dbm: f32,
    pub digital_lost_dbm: f32,
    /// Random slow wobble of the signal, in dB (makes Breakup come and go).
    pub flutter_db: f32,
    /// On Light, Analog tops out here (1 = full static).
    pub light_cap_analog: f32,
    /// On Light, Digital tops out here (stutter starts at 0.5, freeze at 1).
    pub light_cap_digital: f32,
    pub receiver_height_m: f32,
}

impl Default for SignalTuning {
    fn default() -> Self {
        Self {
            vtx_mw_whoop: 25.0,
            vtx_mw_five: 400.0,
            light_power_mult: 16.0,
            medium_power_mult: 4.0,
            realistic_power_mult: 1.0,
            wall_db_per_m: 60.0,
            wall_max_m: 0.6,
            open_surface_m: 0.3,
            analog_clean_dbm: -78.0,
            analog_lost_dbm: -92.0,
            digital_perfect_dbm: -82.0,
            digital_lost_dbm: -93.0,
            flutter_db: 2.0,
            light_cap_analog: 0.75,
            light_cap_digital: 0.8,
            receiver_height_m: 1.7,
        }
    }
}

/// Stand-in lighting until the Maps have baked light. Not part of the look being judged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneTuning {
    pub sun_lux: f32,
    pub ambient_nits: f32,
    pub shadows: bool,
    pub msaa: bool,
    pub detail_texture: bool,
}

impl Default for SceneTuning {
    fn default() -> Self {
        Self { sun_lux: 100_000.0, ambient_nits: 1500.0, shadows: true, msaa: true, detail_texture: true }
    }
}

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub quad: QuadKind,
    /// Corner to corner of the 4:3 picture.
    pub fov_deg: f32,
    pub tilt_deg: f32,
    pub aspect: Aspect,
    pub look: Look,
    pub breakup: BreakupLevel,
    pub reduce_motion: bool,
    pub show_own_quad: bool,
    pub analog: AnalogTuning,
    pub digital: DigitalTuning,
    pub signal: SignalTuning,
    pub scene: SceneTuning,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            quad: QuadKind::Whoop65,
            fov_deg: 160.0,
            tilt_deg: 30.0,
            aspect: Aspect::FourThree,
            look: Look::Analog,
            breakup: BreakupLevel::Realistic,
            reduce_motion: false,
            show_own_quad: true,
            analog: AnalogTuning::default(),
            digital: DigitalTuning::default(),
            signal: SignalTuning::default(),
            scene: SceneTuning::default(),
        }
    }
}

impl Tuning {
    pub fn exposure(&self) -> &ExposureTuning {
        match self.look {
            Look::Analog => &self.analog.exposure,
            Look::Digital => &self.digital.exposure,
        }
    }
    pub fn source_scale(&self) -> f32 {
        match self.look {
            Look::Analog => self.analog.source_scale,
            Look::Digital => self.digital.source_scale,
        }
    }
    pub fn lens_curve(&self) -> f32 {
        match self.look {
            Look::Analog => self.analog.lens_curve,
            Look::Digital => self.digital.lens_curve,
        }
    }
    pub fn vtx_mw(&self) -> f32 {
        match self.quad {
            QuadKind::Whoop65 => self.signal.vtx_mw_whoop,
            QuadKind::Freestyle5 => self.signal.vtx_mw_five,
        }
    }
    pub fn power_mult(&self) -> Option<f32> {
        match self.breakup {
            BreakupLevel::Off => None,
            BreakupLevel::Light => Some(self.signal.light_power_mult),
            BreakupLevel::Medium => Some(self.signal.medium_power_mult),
            BreakupLevel::Realistic => Some(self.signal.realistic_power_mult),
        }
    }
}

pub fn proto_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn saved_dir() -> PathBuf {
    proto_dir().join("saved")
}

pub fn save_tuning(t: &Tuning, name: &str) -> Result<PathBuf, String> {
    let dir = saved_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let clean = if clean.is_empty() { "tuning".to_string() } else { clean };
    let stamp = crate::now_stamp();
    let path = dir.join(format!("{clean}-{stamp}.toml"));
    let body = toml::to_string_pretty(t).map_err(|e| e.to_string())?;
    let header = format!(
        "# PROTOTYPE tuning for issue #28, saved {stamp}.\n# Load it with: ./run.sh --load {}\n\n",
        path.file_name().unwrap().to_string_lossy()
    );
    std::fs::write(&path, header + &body).map_err(|e| e.to_string())?;
    let _ = std::fs::write(dir.join("latest.toml"), toml::to_string_pretty(t).unwrap_or_default());
    Ok(path)
}

pub fn load_tuning(path: &Path) -> Result<Tuning, String> {
    let p = if path.is_absolute() || path.exists() { path.to_path_buf() } else { saved_dir().join(path) };
    let s = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    toml::from_str(&s).map_err(|e| e.to_string())
}

pub fn list_saved() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(saved_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".toml"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}
