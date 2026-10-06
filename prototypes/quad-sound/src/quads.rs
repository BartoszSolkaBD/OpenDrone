//! PROTOTYPE (#34). The two alpha Quads: the rough motor model's numbers (physics, NOT tunable
//! here: "a sound is never fixed by moving a physics number") and the sound block set by ear
//! (the thing this prototype tunes), plus the listener settings, group volumes and clip choices.

use firewheel::diff::{Diff, Patch};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum QuadKind {
    #[default]
    Whoop65,
    Freestyle5,
}

impl QuadKind {
    pub fn label(self) -> &'static str {
        match self {
            QuadKind::Whoop65 => "Whoop 65 (Meteor65 Pro)",
            QuadKind::Freestyle5 => "Freestyle 5\"",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            QuadKind::Whoop65 => "whoop65",
            QuadKind::Freestyle5 => "five",
        }
    }
}

/// The rough motor model. Every number cites where it came from; none is tuned by ear.
#[derive(Clone, Debug)]
pub struct QuadDef {
    pub kind: QuadKind,
    pub mass: f32,
    /// Distance from the centre to each motor axis, m.
    pub arm: f32,
    pub inertia: [f32; 3],
    pub blades: u32,
    pub poles: u32,
    pub prop_d: f32,
    pub kv_rpm: f32,
    /// Lumped winding + ESC + lead resistance, ohm (Derived: fits the max-thrust current).
    pub r_motor: f32,
    pub i0: f32,
    pub k_f: f32,
    pub k_m: f32,
    pub rotor_j: f32,
    pub cells: u32,
    /// Pack open-circuit voltage at mid charge, V.
    pub v_oc: f32,
    pub r_batt: f32,
    /// Betaflight dshot_idle_value / 10000.
    pub idle: f32,
    pub has_buzzer: bool,
    /// DShot600 (else DShot300): changes when Bluejay hears the signal (start-up timing).
    pub dshot600: bool,
    pub ducted: bool,
    /// Linear drag (duct ram drag or rotor drag), 1/s mass-normalised.
    pub drag_lin: f32,
    /// Quadratic body drag, N/(m/s)^2.
    pub drag_quad: f32,
    pub max_rate_dps: f32,
    pub center_rate_dps: f32,
    /// Static per-motor duty spread from the frame and parts (model detail, not tuned by ear).
    pub motor_spread: [f32; 4],
    /// Thrust noise the FC fights in a steady hover (air, the props' own wake), fraction.
    pub turbulence: f32,
    /// Centre of gravity off the thrust centre (x forward, y left), m. The FC holds it with
    /// steady motor differences, as on a real quad with its pack a little off-centre.
    pub cg_offset: [f32; 2],
    /// Path speeds for the Where-you-stand flights, m/s.
    pub cruise_speed: f32,
}

impl QuadDef {
    pub fn get(kind: QuadKind) -> Self {
        match kind {
            // docs/research/flight-dynamics.md §8.1 and §9.1 (Meteor65 Pro), quad-settings/meteor65-pro.
            QuadKind::Whoop65 => QuadDef {
                kind,
                mass: 0.0313,             // Manufacturer: 23.0 g dry + 8.3 g LAVA 300
                arm: 0.033,               // Manufacturer: 66 mm diagonal
                inertia: [0.7e-5, 0.9e-5, 1.4e-5], // Estimate (§9.1)
                blades: 3,                // Gemfan 35 mm 3-blade (§8.1)
                poles: 12,                // motor_poles = 12 (diff all)
                prop_d: 0.035,
                kv_rpm: 19500.0,          // 0802SE 19500KV
                r_motor: 0.50,            // Derived: 30.6 g @ 3.4 A, 4 V with k_f below
                i0: 0.2,                  // Estimate
                // #34 round 2: the only published whoop RPM point is the 0802 (2026) on 40 mm
                // props, 55 g at 46,481 RPM (§8.1) -> C_T ≈ 0.29. The same C_T on the Pro's 35 mm
                // props gives k_f = C_T·ρ·D⁴/4π² ≈ 1.35e-8. Round 1 used §9.1's 2.15e-8, which is
                // the 40 mm figure unscaled (it would need C_T ≈ 0.46).
                k_f: 1.35e-8,             // Derived
                k_m: 7.05e-11,            // Derived: torque at 3.4 A over ω²; k_m/k_f ≈ 0.0052 m
                rotor_j: 2.45e-8,         // Derived: gives ~30 ms at hover (§9.1: 20-50 ms)
                cells: 1,
                v_oc: 4.05,               // LiHV mid-pack
                r_batt: 0.037,            // Derived 35-39 mOhm (§9.1)
                idle: 0.06,               // dshot_idle_value = 600
                has_buzzer: false,        // like the real Meteor65 Pro (#32)
                dshot600: false,          // motor_pwm_protocol = DSHOT300 (diff all)
                ducted: true,
                drag_lin: 1.2,            // Derived duct ram drag (§9.1)
                drag_quad: 0.0028,        // Estimate: ~15 m/s top speed
                max_rate_dps: 670.0,      // Betaflight 4.3 defaults (quad-settings)
                center_rate_dps: 70.0,
                // Round 2: real whoop hovers spread the four motors widely (the Tiny Hawk 2
                // recording's blade-pass line covers ±10 %), so the rough model spreads and
                // buffets them more.
                motor_spread: [0.03, -0.02, 0.015, -0.025],
                turbulence: 0.05,
                cg_offset: [0.0015, -0.001],      // Estimate: 1.5 mm and 1 mm
                cruise_speed: 9.0,
            },
            // docs/research/flight-dynamics.md §8.2 and §9.1 (generic 5").
            QuadKind::Freestyle5 => QuadDef {
                kind,
                mass: 0.65,               // Manufacturer 620-650 g
                arm: 0.1125,              // Manufacturer: 225 mm diagonal
                inertia: [1.4e-3, 1.5e-3, 2.5e-3], // Estimate (§9.1)
                blades: 3,                // T5147 3-blade
                poles: 14,                // Betaflight default; 2207 motors are 12N14P
                prop_d: 0.13,
                kv_rpm: 1750.0,           // T-Motor Velox V2207 V3 1750KV
                r_motor: 0.19,            // Derived: 34.6 A, 29,447 RPM at 23.5 V
                i0: 1.5,                  // Estimate
                k_f: 1.64e-6,             // Derived 1.5-1.65e-6 (§9.1)
                k_m: 1.9e-8,              // Derived: k_m/k_f ≈ 0.0116 m (§9.1 says ≈ 0.01)
                rotor_j: 6.4e-6,          // Derived: gives ~33 ms at hover (§8.2: 33 ms measured)
                cells: 6,
                v_oc: 23.6,
                r_batt: 0.03,             // Estimate (§9.1)
                idle: 0.055,              // Betaflight default dshot_idle_value = 550
                has_buzzer: true,         // #32
                dshot600: true,           // Betaflight default DSHOT600
                ducted: false,
                drag_lin: 0.4,            // Measured on a similar quad 0.24-0.54 (§9.1)
                drag_quad: 0.042,         // Estimate: ~35 m/s top speed
                max_rate_dps: 650.0,      // like the Cetus X tune's 650
                center_rate_dps: 70.0,
                motor_spread: [0.015, -0.01, 0.008, -0.012],
                turbulence: 0.03,
                cg_offset: [0.004, -0.002],       // Estimate: 4 mm and 2 mm
                cruise_speed: 22.0,
            },
        }
    }

    pub fn kt(&self) -> f32 {
        60.0 / (2.0 * std::f32::consts::PI * self.kv_rpm)
    }

    /// Quasi-static motor: speed (rad/s) and current (A) for a duty `u` at voltage `v`.
    pub fn steady(&self, u: f32, v: f32) -> (f32, f32) {
        let kt = self.kt();
        let a = self.k_m;
        let b = kt * kt / self.r_motor;
        let c = kt * (u * v / self.r_motor - self.i0);
        if c <= 0.0 {
            return (0.0, 0.0);
        }
        let w = (-b + (b * b + 4.0 * a * c).sqrt()) / (2.0 * a);
        let i = (u * v - kt * w) / self.r_motor;
        (w, i)
    }

    /// Duty needed to hold a speed `w` at voltage `v` (inverse of `steady`).
    pub fn duty_for_speed(&self, w: f32, v: f32) -> f32 {
        let kt = self.kt();
        let i = self.k_m * w * w / kt + self.i0;
        ((kt * w + i * self.r_motor) / v).clamp(0.0, 1.0)
    }

    pub fn thrust_for_duty(&self, u: f32, v: f32) -> f32 {
        let (w, _) = self.steady(u, v);
        self.k_f * w * w
    }

    pub fn duty_for_thrust(&self, t: f32, v: f32) -> f32 {
        let w = (t.max(0.0) / self.k_f).sqrt();
        self.duty_for_speed(w, v)
    }

    pub fn omega_max(&self) -> f32 {
        self.steady(1.0, self.v_oc).0
    }

    pub fn i_max(&self) -> f32 {
        self.steady(1.0, self.v_oc).1.max(0.1)
    }

    pub fn disk_area(&self) -> f32 {
        std::f32::consts::PI * (self.prop_d * 0.5).powi(2)
    }
}

/// A Quad's sound block, set by ear. Carries no Confidence; it isn't physics.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, Diff, Patch)]
#[serde(default)]
pub struct SoundBlock {
    // Motors and props
    /// Level of the blade-pass tone (blade count x rotation rate).
    pub bpf_level: f32,
    /// How many blade-pass harmonics are made.
    pub bpf_harmonics: f32,
    /// How fast the harmonics fall off: amplitude of harmonic h is h^-rolloff.
    pub harmonic_rolloff: f32,
    /// Tones between the blade-pass harmonics, from blades that aren't quite alike (growl).
    pub blade_mismatch: f32,
    /// The 1x rotation-rate tone (imbalance).
    pub shaft_level: f32,
    /// Loudness grows as (speed/max)^this.
    pub level_exponent: f32,
    /// Broadband prop noise ("whoosh").
    pub broadband_level: f32,
    /// Broadband centre, as a multiple of the blade-pass frequency.
    pub broadband_center: f32,
    pub broadband_q: f32,
    /// How much the broadband noise pulses at the blade-pass rate (swish).
    pub broadband_swish: f32,
    /// Motor's electrical whine (pole pairs x rotation rate), grows with current.
    pub motor_whine: f32,
    /// Load (current) adds brightness: more upper harmonics when the prop is working hard.
    pub load_brightness: f32,
    /// Load (current) adds loudness: a working prop is louder than a windmilling one.
    pub load_loudness: f32,
    /// Random speed jitter (turbulence, commutation), fraction of speed.
    pub jitter: f32,
    /// Slow random flutter in each tone's strength (unsteady air on the blades). Less sounds
    /// more synthetic.
    pub tone_roughness: f32,
    /// High hiss above `hiss_hz` (blade tips and trailing edges), grows steeply with speed.
    pub hiss_level: f32,
    pub hiss_hz: f32,
    /// Body/duct resonance (peaking EQ on the motors).
    pub body_peak_hz: f32,
    pub body_peak_db: f32,
    pub body_peak_q: f32,
    // Frame hum
    pub hum_level: f32,
    pub hum_mode1_hz: f32,
    pub hum_mode1_q: f32,
    pub hum_mode2_hz: f32,
    pub hum_mode2_q: f32,
    /// Direct rotation-rate hum (follows the motors) vs the fixed frame modes.
    pub hum_follow: f32,
    // Beeps
    pub esc_beep_level: f32,
    /// 0 = pure tone, 1 = buzzy square-ish tone (the windings as a speaker).
    pub esc_beep_brightness: f32,
    /// The motor bell's ring that the ESC's current pulses excite.
    pub esc_ring_hz: f32,
    pub buzzer_hz: f32,
    pub buzzer_level: f32,
    // Prop Strikes
    pub tick_level: f32,
    pub tick_hz: f32,
    pub tick_decay_ms: f32,
    // Overall
    /// Overall level of this Quad (so the 5" can be louder than the whoop Where you stand).
    pub quad_level_db: f32,
    /// Hit clip loudness at a 5 m/s impact.
    pub hit_level: f32,
}

impl Default for SoundBlock {
    fn default() -> Self {
        Self::for_quad(QuadKind::Whoop65)
    }
}

impl SoundBlock {
    pub fn for_quad(kind: QuadKind) -> Self {
        match kind {
            QuadKind::Whoop65 => SoundBlock {
                // Round 2 ("a bit synthetic"): darker overtones, more growl between them, more
                // and lower whoosh, a little flutter, a softer duct peak.
                bpf_level: 1.0,
                bpf_harmonics: 8.0,
                harmonic_rolloff: 1.5,
                blade_mismatch: 0.2,
                shaft_level: 0.02,
                level_exponent: 2.0,
                broadband_level: 0.9,
                broadband_center: 2.0,
                broadband_q: 0.5,
                broadband_swish: 0.5,
                motor_whine: 0.05,
                load_brightness: 0.5,
                load_loudness: 0.4,
                jitter: 0.015,
                tone_roughness: 0.6,
                hiss_level: 1.7,
                hiss_hz: 2000.0,
                body_peak_hz: 2500.0,
                body_peak_db: 1.5,
                body_peak_q: 1.0,
                hum_level: 0.03,
                hum_mode1_hz: 900.0,
                hum_mode1_q: 6.0,
                hum_mode2_hz: 1600.0,
                hum_mode2_q: 8.0,
                hum_follow: 0.5,
                esc_beep_level: 0.35,
                esc_beep_brightness: 0.6,
                esc_ring_hz: 4500.0,
                buzzer_hz: 2700.0,
                buzzer_level: 0.0,
                tick_level: 0.6,
                tick_hz: 4500.0,
                tick_decay_ms: 2.0,
                quad_level_db: -12.0,
                hit_level: 0.5,
            },
            QuadKind::Freestyle5 => SoundBlock {
                bpf_level: 1.0,
                bpf_harmonics: 12.0,
                harmonic_rolloff: 1.3,
                blade_mismatch: 0.25,
                shaft_level: 0.08,
                level_exponent: 2.0,
                broadband_level: 1.0,
                broadband_center: 4.0,
                broadband_q: 0.7,
                broadband_swish: 0.5,
                motor_whine: 0.06,
                load_brightness: 0.6,
                load_loudness: 0.5,
                jitter: 0.012,
                tone_roughness: 0.5,
                hiss_level: 1.2,
                hiss_hz: 1500.0,
                body_peak_hz: 2500.0,
                body_peak_db: 2.0,
                body_peak_q: 1.2,
                hum_level: 0.2,
                hum_mode1_hz: 180.0,
                hum_mode1_q: 4.0,
                hum_mode2_hz: 420.0,
                hum_mode2_q: 6.0,
                hum_follow: 0.3,
                esc_beep_level: 0.3,
                esc_beep_brightness: 0.6,
                esc_ring_hz: 2500.0,
                buzzer_hz: 2700.0,
                buzzer_level: 0.25,
                tick_level: 0.8,
                tick_hz: 3000.0,
                tick_decay_ms: 3.0,
                quad_level_db: 0.0,
                hit_level: 0.8,
            },
        }
    }
}

/// Listener settings. Proposed as the game's (not per Quad): the camera-mic character and the
/// open-air effects. Kept apart from the sound block on purpose; see the reaction questions.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, Diff, Patch)]
#[serde(default)]
pub struct ListenerParams {
    /// false = On the Quad (default), true = Where you stand.
    pub where_you_stand: bool,
    // On the Quad: the camera mic
    pub mic_lowcut_hz: f32,
    pub mic_highcut_hz: f32,
    pub wind_level: f32,
    /// Airspeed at which the wind reaches wind_level, m/s.
    pub wind_ref_speed: f32,
    /// Wind loudness grows as (speed/ref)^this.
    pub wind_exponent: f32,
    /// How much of the props' downwash blows over the mic (0..1).
    pub wind_downwash: f32,
    pub wind_cutoff_hz: f32,
    pub wind_gust: f32,
    pub comp_on: bool,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub comp_attack_ms: f32,
    pub comp_release_ms: f32,
    pub comp_makeup_db: f32,
    pub onquad_level_db: f32,
    // Where you stand
    /// Inside this distance the Quad is at full level, m.
    pub stand_ref_m: f32,
    pub stand_level_db: f32,
    /// Air absorption: low-pass cutoff the sound has at 100 m, Hz.
    pub air_cutoff_100m_hz: f32,
    pub delay_on: bool,
    pub wall_muffle_on: bool,
    /// Corner of the wall shelf: above it, sound is cut by wall_highs_db.
    pub wall_muffle_hz: f32,
    /// Level cut behind the first wall (all frequencies).
    pub wall_loss_db: f32,
    /// Extra cut above the corner behind the first wall.
    pub wall_highs_db: f32,
}

impl Default for ListenerParams {
    fn default() -> Self {
        ListenerParams {
            where_you_stand: false,
            mic_lowcut_hz: 90.0,
            mic_highcut_hz: 11000.0,
            wind_level: 0.5,
            wind_ref_speed: 20.0,
            wind_exponent: 2.0,
            wind_downwash: 0.5,
            wind_cutoff_hz: 350.0,
            wind_gust: 0.5,
            comp_on: true,
            comp_threshold_db: -26.0,
            comp_ratio: 4.0,
            comp_attack_ms: 20.0,
            comp_release_ms: 250.0,
            comp_makeup_db: 10.0,
            onquad_level_db: 0.0,
            stand_ref_m: 3.0,
            stand_level_db: 10.0,
            air_cutoff_100m_hz: 8000.0,
            delay_on: true,
            wall_muffle_on: true,
            wall_muffle_hz: 800.0,
            wall_loss_db: 6.0,
            wall_highs_db: 10.0,
        }
    }
}

/// The Sound tab's group volumes (0..1). These belong to the pilot.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Volumes {
    pub master: f32,
    pub quad: f32,
    pub crashes: f32,
    pub background: f32,
    pub menus: f32,
    /// Background level while the Pause Menu is open, as a fraction.
    pub pause_background: f32,
}

impl Default for Volumes {
    fn default() -> Self {
        // Slider values; Firewheel turns a slider value v into amplitude v² (0.8 = -3.9 dB).
        Volumes { master: 0.8, quad: 1.0, crashes: 0.8, background: 0.6, menus: 0.6, pause_background: 0.5 }
    }
}

/// Which CC0 clips are chosen (file paths relative to assets/, from clips.toml).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ClipChoice {
    pub whoop_hit: String,
    pub five_hit: String,
    pub menu_click: String,
    pub menu_back: String,
    pub skate_park_background: String,
    pub bando_background: String,
    /// Each Map's Background Sound level, dB (the Map's own value, before the Background volume).
    /// The files stay as recorded (#34 reaction 14).
    pub skate_park_level_db: f32,
    pub bando_level_db: f32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub whoop65: SoundBlock,
    pub five: SoundBlock,
    pub listener: ListenerParams,
    pub volumes: Volumes,
    pub clips: ClipChoice,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            whoop65: SoundBlock::for_quad(QuadKind::Whoop65),
            five: SoundBlock::for_quad(QuadKind::Freestyle5),
            listener: ListenerParams::default(),
            volumes: Volumes::default(),
            // First picks from assets/clips.toml (all CC0); the maintainer chooses by ear.
            clips: ClipChoice {
                whoop_hit: "clips/hits/kenney-generic-light.ogg".into(),
                five_hit: "clips/hits/fs-quad-wall-collision.ogg".into(),
                menu_click: "clips/menus/kenney-click.ogg".into(),
                menu_back: "clips/menus/kenney-back.ogg".into(),
                skate_park_background: "clips/background-skate-park/fs-city-birds-distant-vehicles.ogg".into(),
                bando_background: "clips/background-bando/fs-construction-site-wind-crane.ogg".into(),
                // Starting values that put both at about -30 dBFS RMS: #640600 is -20.3 dBFS as
                // recorded, #545035 is -39.2 dBFS.
                skate_park_level_db: -10.0,
                bando_level_db: 9.0,
            },
        }
    }
}

impl Tuning {
    pub fn block(&self, kind: QuadKind) -> &SoundBlock {
        match kind {
            QuadKind::Whoop65 => &self.whoop65,
            QuadKind::Freestyle5 => &self.five,
        }
    }
    pub fn block_mut(&mut self, kind: QuadKind) -> &mut SoundBlock {
        match kind {
            QuadKind::Whoop65 => &mut self.whoop65,
            QuadKind::Freestyle5 => &mut self.five,
        }
    }
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&s).map_err(|e| e.to_string())
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let s = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, s).map_err(|e| e.to_string())
    }
}

/// One slider in the tuning panel: (group, label, field, min, max, help).
pub type Knob<'a> = (&'static str, &'static str, &'a mut f32, f32, f32, &'static str);

impl SoundBlock {
    pub fn knobs(&mut self) -> Vec<Knob<'_>> {
        vec![
            ("Motors and props", "Blade-pass tone", &mut self.bpf_level, 0.0, 2.0, "blades x rotation rate: the main pitch"),
            ("Motors and props", "Harmonics (count)", &mut self.bpf_harmonics, 1.0, 16.0, "how many blade-pass overtones"),
            ("Motors and props", "Harmonic roll-off", &mut self.harmonic_rolloff, 0.3, 2.5, "higher = darker"),
            ("Motors and props", "Blade mismatch (growl)", &mut self.blade_mismatch, 0.0, 1.0, "tones between the harmonics"),
            ("Motors and props", "Shaft tone (imbalance)", &mut self.shaft_level, 0.0, 1.0, "1x rotation rate"),
            ("Motors and props", "Loudness vs speed (exponent)", &mut self.level_exponent, 0.5, 4.0, "how much louder at full speed"),
            ("Motors and props", "Broadband whoosh", &mut self.broadband_level, 0.0, 2.0, "prop air noise"),
            ("Motors and props", "Whoosh centre (x blade-pass)", &mut self.broadband_center, 0.5, 10.0, ""),
            ("Motors and props", "Whoosh Q", &mut self.broadband_q, 0.2, 4.0, ""),
            ("Motors and props", "Whoosh swish", &mut self.broadband_swish, 0.0, 1.0, "pulsing at the blade rate"),
            ("Motors and props", "Motor whine", &mut self.motor_whine, 0.0, 0.5, "pole pairs x rotation rate, with current"),
            ("Motors and props", "Load brightness", &mut self.load_brightness, 0.0, 1.5, "harder-working props sound brighter"),
            ("Motors and props", "Load loudness", &mut self.load_loudness, 0.0, 1.0, "windmilling props sound quieter"),
            ("Motors and props", "Speed jitter", &mut self.jitter, 0.0, 0.05, "fast speed wobble (fraction): widens each tone"),
            ("Motors and props", "Tone roughness", &mut self.tone_roughness, 0.0, 1.0, "flutter in each tone; less = more synthetic"),
            ("Motors and props", "High hiss", &mut self.hiss_level, 0.0, 3.0, "air noise of the blade tips"),
            ("Motors and props", "Hiss from Hz", &mut self.hiss_hz, 500.0, 8000.0, ""),
            ("Motors and props", "Body/duct peak Hz", &mut self.body_peak_hz, 200.0, 10000.0, ""),
            ("Motors and props", "Body/duct peak dB", &mut self.body_peak_db, -12.0, 12.0, ""),
            ("Motors and props", "Body/duct peak Q", &mut self.body_peak_q, 0.3, 6.0, ""),
            ("Frame hum", "Hum level", &mut self.hum_level, 0.0, 1.5, ""),
            ("Frame hum", "Follows motors (0..1)", &mut self.hum_follow, 0.0, 1.0, "direct rotation-rate hum vs frame ring"),
            ("Frame hum", "Frame mode 1 Hz", &mut self.hum_mode1_hz, 50.0, 3000.0, ""),
            ("Frame hum", "Frame mode 1 Q", &mut self.hum_mode1_q, 0.5, 30.0, ""),
            ("Frame hum", "Frame mode 2 Hz", &mut self.hum_mode2_hz, 50.0, 5000.0, ""),
            ("Frame hum", "Frame mode 2 Q", &mut self.hum_mode2_q, 0.5, 30.0, ""),
            ("Beeps", "ESC beep level", &mut self.esc_beep_level, 0.0, 1.0, "Bluejay melody and beeps"),
            ("Beeps", "ESC beep buzz", &mut self.esc_beep_brightness, 0.0, 1.0, "pure sine to Bluejay's pulse train"),
            ("Beeps", "ESC beep ring Hz", &mut self.esc_ring_hz, 800.0, 9000.0, "the motor bell the pulses ring"),
            ("Beeps", "Buzzer pitch Hz", &mut self.buzzer_hz, 1500.0, 5000.0, "5\" active buzzer"),
            ("Beeps", "Buzzer level", &mut self.buzzer_level, 0.0, 1.0, "0 on the whoop: it has no buzzer"),
            ("Crashes", "Prop Strike tick level", &mut self.tick_level, 0.0, 2.0, ""),
            ("Crashes", "Tick pitch Hz", &mut self.tick_hz, 500.0, 9000.0, ""),
            ("Crashes", "Tick decay ms", &mut self.tick_decay_ms, 0.3, 15.0, ""),
            ("Crashes", "Hit clip level", &mut self.hit_level, 0.0, 2.0, "at a 5 m/s impact"),
            ("Overall", "Level Where you stand dB", &mut self.quad_level_db, -30.0, 6.0, "how loud this Quad is to bystanders (whoop quieter than the 5\")"),
        ]
    }
}

impl ListenerParams {
    pub fn knobs_onquad(&mut self) -> Vec<Knob<'_>> {
        vec![
            ("On the Quad: mic", "Mic low cut Hz", &mut self.mic_lowcut_hz, 20.0, 400.0, ""),
            ("On the Quad: mic", "Mic high cut Hz", &mut self.mic_highcut_hz, 3000.0, 20000.0, "action-camera audio rolls off the top"),
            ("On the Quad: mic", "Level dB", &mut self.onquad_level_db, -20.0, 10.0, ""),
            ("On the Quad: wind", "Wind level", &mut self.wind_level, 0.0, 2.0, "at the reference speed"),
            ("On the Quad: wind", "Wind ref speed m/s", &mut self.wind_ref_speed, 5.0, 50.0, ""),
            ("On the Quad: wind", "Wind steepness (exponent)", &mut self.wind_exponent, 0.5, 4.0, ""),
            ("On the Quad: wind", "Downwash on the mic", &mut self.wind_downwash, 0.0, 1.0, ""),
            ("On the Quad: wind", "Wind cutoff Hz", &mut self.wind_cutoff_hz, 80.0, 3000.0, "rises with speed"),
            ("On the Quad: wind", "Wind gustiness", &mut self.wind_gust, 0.0, 1.0, ""),
            ("On the Quad: compressor", "Threshold dB", &mut self.comp_threshold_db, -50.0, 0.0, ""),
            ("On the Quad: compressor", "Ratio", &mut self.comp_ratio, 1.0, 20.0, ""),
            ("On the Quad: compressor", "Attack ms", &mut self.comp_attack_ms, 0.5, 80.0, "slower lets the swell's first instant through"),
            ("On the Quad: compressor", "Release ms", &mut self.comp_release_ms, 20.0, 1500.0, ""),
            ("On the Quad: compressor", "Makeup dB", &mut self.comp_makeup_db, 0.0, 24.0, ""),
        ]
    }
    pub fn knobs_stand(&mut self) -> Vec<Knob<'_>> {
        vec![
            ("Where you stand", "Full level within m", &mut self.stand_ref_m, 0.5, 10.0, "then fades as 1/distance"),
            ("Where you stand", "Level dB", &mut self.stand_level_db, -20.0, 20.0, ""),
            ("Where you stand", "Air cutoff at 100 m Hz", &mut self.air_cutoff_100m_hz, 1000.0, 20000.0, "air absorbs highs"),
            ("Where you stand", "Wall muffle corner Hz", &mut self.wall_muffle_hz, 150.0, 4000.0, "above this, walls cut the highs"),
            ("Where you stand", "Wall loss dB", &mut self.wall_loss_db, 0.0, 20.0, "all frequencies, first wall; more walls add a quarter each, at most 1.5x"),
            ("Where you stand", "Wall highs cut dB", &mut self.wall_highs_db, 0.0, 24.0, "extra cut above the corner, first wall"),
        ]
    }
}
