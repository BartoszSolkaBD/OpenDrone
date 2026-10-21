//! Makes and plays the Quad's sound on Firewheel, on the sound's own thread
//! ([ADR-0023]).
//!
//! An edge crate ([ADR-0003]): it uses no other OpenDrone crate. The game
//! hands it each frame's Quad state as a [`QuadFrame`], and nothing flows back
//! into the Simulation, so sound sits outside the bit-exact guarantee.
//!
//! What it makes so far (#74):
//!
//! - **Four motor voices,** one for each motor, from its speed and load and
//!   the Quad's blade count, pole count and sound block: the blade-pass tone
//!   and its overtones, the growl between them, the once-a-turn tone, the
//!   props' whoosh and hiss, and the motor's whine. A small sound-side
//!   wobble in each motor's speed widens its tone.
//! - **The frame hum,** following the motors.
//! - **On the Quad,** the Listening Position of a camera's microphone on
//!   board: wind on the mic, growing steeply with airspeed and with the
//!   props' downwash, and a camera-style compressor with a 20 ms attack. No
//!   clipping.
//!
//! [`Sound::start`] plays it on the sound device, asking for a 256-frame
//! block and taking the device's own if it can't do that. With no sound
//! device it runs silent; if the device goes away it moves to the new
//! default one. [`Offline`] makes the same sound with no device, for tests.
//!
//! Where you stand, the beeps, Prop Strike ticks, hits and Background Sounds
//! come with #75.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0023]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0023-quad-sound-made-live-on-firewheel.md

mod dsp;
mod graph;
mod live;
mod offline;
mod on_the_quad;
mod voice;

pub use live::{BLOCK_FRAMES_ASKED, Device, Sound};
pub use offline::Offline;

/// What the sound needs to know about a Quad, from its Quad definition.
/// Every number is in SI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadSound {
    /// Blades on each prop.
    pub blades: u32,
    /// Magnet poles in each motor: its whine sits at half this times the
    /// rotation rate.
    pub poles: u32,
    /// The props' diameter, in metres: with the thrust, it sets how fast
    /// their downwash blows over the camera's microphone.
    pub prop_diameter: f32,
    /// The Map's air, in kg/m³, for the downwash.
    pub air_density: f32,
    /// A motor's top speed, in rad/s, and the current it then draws, in
    /// amps. A motor's loudness and load are measured against these.
    pub top_speed: f32,
    pub top_current: f32,
    /// The values set by ear.
    pub block: SoundBlock,
}

/// A Quad's sound block: the motor and prop voice and the frame hum, set by
/// ear, with no Confidence because it isn't physics. Each field has the name
/// of its key in the Quad definition's `[sound_block]`. Frequencies are in
/// hertz, gains in decibels, shares as fractions; the rest are plain
/// numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundBlock {
    /// The main pitch's level: blade count × rotation rate.
    pub blade_pass_level: f32,
    /// How many blade-pass overtones are made, the main pitch included.
    pub harmonics: u32,
    /// How fast the overtones fall away: higher is darker.
    pub harmonic_rolloff: f32,
    /// The growl between the overtones, from blades that aren't quite alike.
    pub blade_mismatch: f32,
    /// The once-a-turn tone.
    pub shaft_level: f32,
    /// A motor's loudness grows as (speed ÷ top speed) to this power.
    pub loudness_exponent: f32,
    /// The props' broadband whoosh, its centre as a multiple of the main
    /// pitch, its width (Q), and how much it pulses with the blades.
    pub whoosh_level: f32,
    pub whoosh_centre: f32,
    pub whoosh_q: f32,
    pub whoosh_swish: f32,
    /// The motor's electrical whine, growing with its load.
    pub motor_whine: f32,
    /// A working prop sounds brighter, and louder, than a windmilling one.
    pub load_brightness: f32,
    pub load_loudness: f32,
    /// The sound-side wobble in each motor's speed, as a share of it. It
    /// stands in for turbulence faster than a Simulation tick, and widens
    /// each motor's tone; it changes nothing in a flight.
    pub speed_wobble: f32,
    /// A slow flutter in each tone's strength.
    pub tone_roughness: f32,
    /// The blade tips' hiss, above its corner.
    pub hiss_level: f32,
    pub hiss_corner: f32,
    /// The frame's or the ducts' resonance.
    pub body_peak: f32,
    pub body_peak_gain: f32,
    pub body_peak_q: f32,
    /// The frame hum: its level, its two ringing modes, and how much it
    /// follows the motors' rotation instead of ringing.
    pub hum_level: f32,
    pub hum_first_mode: f32,
    pub hum_first_mode_q: f32,
    pub hum_second_mode: f32,
    pub hum_second_mode_q: f32,
    pub hum_follow: f32,
}

/// One motor, as the Simulation left it on the last tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MotorSound {
    /// In rad/s; negative spinning backwards.
    pub speed: f32,
    /// Through the motor, in amps: its load.
    pub current: f32,
    /// Its prop's thrust, in newtons.
    pub thrust: f32,
}

/// The Quad's state the game hands the sound each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuadFrame {
    /// In Betaflight's motor order.
    pub motors: [MotorSound; 4],
    /// How fast the Quad moves through the air, in m/s.
    pub airspeed: f32,
    /// The Pause Menu is open, so the Simulation stands still: the Quad
    /// goes silent within about 0.1 s, and comes back on Resume.
    pub paused: bool,
}
