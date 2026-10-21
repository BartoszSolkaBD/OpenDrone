//! The Quad's sound (#74): `opendrone-sound` plays it on its own thread, and
//! the game hands it the Quad's state each frame.
//!
//! The sound reads, from the Quad definition, the props' blade count and
//! size, the motors' pole count and the sound block, and works out each
//! motor's top speed and current from the motor model; from the Simulation,
//! after each frame's ticks, each motor's speed, current and thrust and the
//! Quad's airspeed; and from the game, whether the Pause Menu is open.
//! Nothing flows back into the Simulation.

use std::f64::consts::TAU;

use bevy::prelude::*;
use opendrone_pack::{QuadDefinition, Sound as SoundData};
use opendrone_sound::{Device, MotorSound, QuadFrame, QuadSound, Sound, SoundBlock};

use crate::set_up::FlightPlan;
use crate::stepping::Flight;

/// Starts the Quad's sound and keeps it fed.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_the_sound)
            .add_systems(Update, (hand_the_quad_over, say_where_the_sound_plays));
    }
}

/// The Quad's sound, playing on its own thread.
#[derive(Resource)]
pub struct QuadSoundPlaying {
    sound: Sound,
    /// Whether the game has said where the sound plays, or that there's no
    /// sound device: it says so once.
    said: bool,
}

impl QuadSoundPlaying {
    /// Where the sound plays, or why it's silent.
    pub fn device(&self) -> Device {
        self.sound.device()
    }
}

fn start_the_sound(plan: Res<FlightPlan>, mut commands: Commands) {
    match quad_sound(&plan.quad, plan.map.world.air_density) {
        Ok(quad) => commands.insert_resource(QuadSoundPlaying {
            sound: Sound::start(quad),
            said: false,
        }),
        Err(problem) => warn!("the Quad's sound can't play, so the game runs silent: {problem}"),
    }
}

fn hand_the_quad_over(
    sound: Option<Res<QuadSoundPlaying>>,
    flight: NonSend<Flight>,
    game_clock: Res<Time<Virtual>>,
) {
    if let Some(sound) = sound {
        sound
            .sound
            .hand_over(quad_frame(&flight, game_clock.is_paused()));
    }
}

fn say_where_the_sound_plays(sound: Option<ResMut<QuadSoundPlaying>>) {
    let Some(mut sound) = sound else {
        return;
    };
    if sound.said {
        return;
    }
    match sound.device() {
        Device::Opening => return,
        Device::Playing {
            name,
            block_frames,
            sample_rate,
        } => info!(
            "the Quad's sound plays on {} in blocks of {block_frames} frames at {sample_rate} Hz",
            name.as_deref().unwrap_or("the default sound device")
        ),
        Device::None(why) => warn!("no sound device, so the game runs silent: {why}"),
    }
    sound.said = true;
}

/// The Quad's state after the last tick, as the sound reads it.
pub fn quad_frame(flight: &Flight, paused: bool) -> QuadFrame {
    let output = flight.simulation().quad_output(0);
    QuadFrame {
        motors: output.motors.map(|m| MotorSound {
            speed: m.speed as f32,
            current: m.current as f32,
            thrust: m.thrust as f32,
        }),
        airspeed: output.state.velocity.length() as f32,
        paused,
    }
}

/// What the sound needs from a Quad definition, flying in air of
/// `air_density` (kg/m³).
pub fn quad_sound(quad: &QuadDefinition, air_density: f64) -> Result<QuadSound, String> {
    let (top_speed, top_current) = top_speed_and_current(quad, air_density);
    Ok(QuadSound {
        blades: quad.props.blades,
        poles: quad.motors.poles,
        prop_diameter: quad.props.diameter as f32,
        air_density: air_density as f32,
        top_speed: top_speed as f32,
        top_current: top_current as f32,
        block: sound_block(&quad.sound)?,
    })
}

/// A motor's steady speed (rad/s) and current (A) at full throttle on a full
/// battery, from the Quad definition's motor model, in air of `air_density`.
///
/// The battery's voltage V drives the motor against its back-voltage ω ÷ kv
/// and its winding resistance R: V = ω ÷ kv + R·i. The current turns the
/// prop against its drag, τ = C_P·ρ·D⁵·ω² ÷ (2π)³, plus the no-load current:
/// i = kv·τ + i₀. Solved for ω, a quadratic. The sound measures each motor's
/// loudness and load against these; they never reach the Simulation.
pub fn top_speed_and_current(quad: &QuadDefinition, air_density: f64) -> (f64, f64) {
    let volts = quad.battery.full * f64::from(quad.battery.cells);
    let kv = quad.motors.kv;
    let r = quad.motors.winding_resistance;
    let i0 = quad.motors.no_load_current;
    let drag =
        quad.props.power_coefficient * air_density * quad.props.diameter.powi(5) / TAU.powi(3);
    // r·kv·drag·ω² + ω ÷ kv + (r·i₀ − V) = 0
    let a = r * kv * drag;
    let b = 1.0 / kv;
    let c = r * i0 - volts;
    let speed = if a > 0.0 {
        (-b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a)
    } else {
        volts * kv
    };
    (speed, kv * drag * speed * speed + i0)
}

/// The sound block's values, from the Quad definition's `[sound_block]`.
pub fn sound_block(sound: &SoundData) -> Result<SoundBlock, String> {
    let value = |key: &str| -> Result<f32, String> {
        sound
            .block
            .get(key)
            .map(|v| *v as f32)
            .ok_or_else(|| format!("the sound block has no {key}"))
    };
    Ok(SoundBlock {
        blade_pass_level: value("blade_pass_level")?,
        harmonics: value("harmonics")? as u32,
        harmonic_rolloff: value("harmonic_rolloff")?,
        blade_mismatch: value("blade_mismatch")?,
        shaft_level: value("shaft_level")?,
        loudness_exponent: value("loudness_exponent")?,
        whoosh_level: value("whoosh_level")?,
        whoosh_centre: value("whoosh_centre")?,
        whoosh_q: value("whoosh_q")?,
        whoosh_swish: value("whoosh_swish")?,
        motor_whine: value("motor_whine")?,
        load_brightness: value("load_brightness")?,
        load_loudness: value("load_loudness")?,
        speed_wobble: value("speed_wobble")?,
        tone_roughness: value("tone_roughness")?,
        hiss_level: value("hiss_level")?,
        hiss_corner: value("hiss_corner")?,
        body_peak: value("body_peak")?,
        body_peak_gain: value("body_peak_gain")?,
        body_peak_q: value("body_peak_q")?,
        hum_level: value("hum_level")?,
        hum_first_mode: value("hum_first_mode")?,
        hum_first_mode_q: value("hum_first_mode_q")?,
        hum_second_mode: value("hum_second_mode")?,
        hum_second_mode_q: value("hum_second_mode_q")?,
        hum_follow: value("hum_follow")?,
    })
}
