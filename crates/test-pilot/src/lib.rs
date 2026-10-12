//! The Test Pilot: reads the state and moves only the sticks, from outside the
//! Simulation, through Flight Inputs.
//!
//! Its stick values feed the Simulation, so it follows the same house rules.
//!
//! Its jobs are hold height, hold level and hold heading ([`Jobs`]). Each
//! reads the Quad's state, as a pilot sees it, and moves only the sticks it
//! needs: hold height the throttle, hold level roll and pitch, hold heading
//! yaw. It never touches the Arm switch, the Flight Mode or the physics, and
//! it gives its sticks back as [`Channels`], the same Flight Input a person's
//! hands make. It knows nothing of the Quad's definition: it finds the
//! throttle that hovers the way a pilot does, by watching the Quad climb or
//! sink.
//!
//! Only Scenarios use it. Pilots never get it.
//!
//! # House rules
//!
//! This is a core crate: part of the Simulation, which gives bit-identical
//! results on every computer ([ADR-0001], [ADR-0003]). So it follows the house
//! rules:
//!
//! - Every maths function comes from `libm`, never from std's float methods
//!   such as `f64::sin` or `f64::powf`, which give different results on
//!   different platforms.
//! - No `HashMap` or `HashSet`: their order changes from run to run. Use
//!   `BTreeMap`, `BTreeSet` or a `Vec`.
//! - No clock, no files, no Bevy and nothing else that touches the operating
//!   system.
//!
//! Clippy enforces the first two (and catches the usual clock and file calls)
//! with this crate's `clippy.toml`, which only the five core crates have.
//! `cargo xtask walls` checks the dependencies.
//!
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use opendrone_maths::functions;
pub use opendrone_sim::{Channel, Channels, QuadState};

/// What the Test Pilot is asked to do. With every job off it moves nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Jobs {
    /// Hold this height above the Map's origin, in metres, with the throttle.
    pub hold_height: Option<f64>,
    /// Hold the Quad level, with roll and pitch.
    pub hold_level: bool,
    /// Hold this compass heading, in radians (0 north, turning right), with
    /// yaw.
    pub hold_heading: Option<f64>,
}

impl Jobs {
    /// No job: the sticks stay as they are held.
    pub const NONE: Jobs = Jobs {
        hold_height: None,
        hold_level: false,
        hold_heading: None,
    };
}

/// The climb the Test Pilot wants for each metre it is off its height, in
/// 1/s, and the fastest it asks for, in m/s.
const CLIMB_PER_METRE: f64 = 1.0;
const FASTEST_CLIMB: f64 = 1.5;
/// Throttle, as a share of the stick's travel, for each m/s of climb it is
/// short of what it wants.
const THROTTLE_PER_CLIMB: f64 = 0.04;
/// How fast it learns the throttle that hovers: the share of the travel it
/// adds each second for each m/s of climb it is short of.
const HOVER_LEARNED_PER_CLIMB: f64 = 0.08;
/// Stick, as a share of full, for each radian the Quad is off level or off
/// its heading.
const STICK_PER_RADIAN: f64 = 0.6;

/// The Test Pilot: reads the state, moves only the sticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestPilot {
    jobs: Jobs,
    /// One physics step, in seconds.
    step: f64,
    /// The throttle it has learned holds the height, as a share of the
    /// stick's travel.
    hover: f64,
}

impl TestPilot {
    /// A Test Pilot for a Simulation that steps `physics_hz` times a second,
    /// with no job.
    pub fn new(physics_hz: u32) -> TestPilot {
        TestPilot {
            jobs: Jobs::NONE,
            step: 1.0 / f64::from(physics_hz),
            hover: 0.5,
        }
    }

    /// Gives it its jobs from now on. Starting to hold height begins from the
    /// throttle stick as it is held now, as a pilot begins from where their
    /// thumb is.
    pub fn set_jobs(&mut self, jobs: Jobs, held: &Channels) {
        if jobs.hold_height.is_some() && self.jobs.hold_height.is_none() {
            self.hover = throttle_share(held.throttle);
        }
        self.jobs = jobs;
    }

    pub fn jobs(&self) -> Jobs {
        self.jobs
    }

    /// The Channels it sends for one physics step: the held ones, with the
    /// sticks its jobs move replaced. Nothing else changes, the Arm switch
    /// included.
    pub fn fly(&mut self, state: &QuadState, held: Channels) -> Channels {
        let mut channels = held;
        let angles = state.attitude.pilot_angles();
        if self.jobs.hold_level {
            // Rolling right and pitching nose up are positive. A stick right
            // rolls right, and a stick forward pitches the nose down.
            channels.roll = Channel::from_stick(within_full(-STICK_PER_RADIAN * angles.roll));
            channels.pitch = Channel::from_stick(within_full(STICK_PER_RADIAN * angles.pitch));
        }
        if let Some(heading) = self.jobs.hold_heading {
            let off = turn_between(angles.heading, heading);
            channels.yaw = Channel::from_stick(within_full(STICK_PER_RADIAN * off));
        }
        if let Some(height) = self.jobs.hold_height {
            let wanted = (CLIMB_PER_METRE * (height - state.position.z))
                .clamp(-FASTEST_CLIMB, FASTEST_CLIMB);
            let short = wanted - state.velocity.z;
            self.hover = (self.hover + HOVER_LEARNED_PER_CLIMB * short * self.step).clamp(0.0, 1.0);
            let throttle = (self.hover + THROTTLE_PER_CLIMB * short).clamp(0.0, 1.0);
            channels.throttle = Channel::from_throttle(throttle);
        }
        channels
    }
}

/// A throttle Channel as a share of the stick's travel, 0 to 1.
fn throttle_share(channel: Channel) -> f64 {
    let low = f64::from(Channel::LOW.step());
    (f64::from(channel.step()) - low) / (f64::from(Channel::HIGH.step()) - low)
}

fn within_full(stick: f64) -> f64 {
    stick.clamp(-1.0, 1.0)
}

/// How far to turn from `from` to `to`, the short way round, in radians from
/// -π to π (turning right is positive).
fn turn_between(from: f64, to: f64) -> f64 {
    let turn = 2.0 * core::f64::consts::PI;
    let mut difference = functions::fmod(to - from, turn);
    if difference > turn / 2.0 {
        difference -= turn;
    } else if difference < -turn / 2.0 {
        difference += turn;
    }
    difference
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrone_maths::{Attitude, PilotAngles, Vec3};

    const DEGREE: f64 = core::f64::consts::PI / 180.0;

    fn state(height: f64, climb: f64, roll: f64, pitch: f64, heading: f64) -> QuadState {
        QuadState {
            position: Vec3::new(0.0, 0.0, height),
            velocity: Vec3::new(0.0, 0.0, climb),
            attitude: Attitude::from_pilot_angles(PilotAngles {
                roll,
                pitch,
                heading,
            }),
            rotation: Vec3::ZERO,
        }
    }

    fn held() -> Channels {
        Channels {
            roll: Channel::CENTRE,
            pitch: Channel::CENTRE,
            yaw: Channel::CENTRE,
            throttle: Channel::from_throttle(0.4),
            arm: Channel::HIGH,
            flight_mode: Channel::LOW,
            crash_flip: Channel::LOW,
        }
    }

    fn pilot(jobs: Jobs) -> TestPilot {
        let mut pilot = TestPilot::new(8000);
        pilot.set_jobs(jobs, &held());
        pilot
    }

    #[test]
    fn with_no_job_the_sticks_stay_as_they_are_held() {
        let mut pilot = pilot(Jobs::NONE);
        let sent = pilot.fly(&state(5.0, -3.0, 0.3, 0.2, 1.0), held());
        assert_eq!(sent, held());
    }

    #[test]
    fn a_quad_below_its_height_gets_more_throttle_and_one_above_it_gets_less() {
        let jobs = Jobs {
            hold_height: Some(2.0),
            ..Jobs::NONE
        };
        let held_throttle = held().throttle.step();
        let below = pilot(jobs).fly(&state(1.0, 0.0, 0.0, 0.0, 0.0), held());
        let above = pilot(jobs).fly(&state(3.0, 0.0, 0.0, 0.0, 0.0), held());
        assert!(below.throttle.step() > held_throttle);
        assert!(above.throttle.step() < held_throttle);
    }

    #[test]
    fn a_quad_that_sinks_at_its_height_gets_more_throttle_every_second_it_goes_on_sinking() {
        let jobs = Jobs {
            hold_height: Some(2.0),
            ..Jobs::NONE
        };
        let mut pilot = pilot(jobs);
        let sinking = state(2.0, -0.5, 0.0, 0.0, 0.0);
        let first = pilot.fly(&sinking, held()).throttle.step();
        for _ in 0..8000 {
            pilot.fly(&sinking, held());
        }
        let later = pilot.fly(&sinking, held()).throttle.step();
        assert!(later > first);
    }

    #[test]
    fn holding_level_pushes_the_sticks_against_a_roll_and_a_pitch() {
        let jobs = Jobs {
            hold_level: true,
            ..Jobs::NONE
        };
        // Right side down and nose up: roll left, and push forward.
        let sent = pilot(jobs).fly(&state(2.0, 0.0, 10.0 * DEGREE, 10.0 * DEGREE, 0.0), held());
        assert!(sent.roll.step() < Channel::CENTRE.step());
        assert!(sent.pitch.step() > Channel::CENTRE.step());
        // The other way.
        let sent = pilot(jobs).fly(
            &state(2.0, 0.0, -10.0 * DEGREE, -10.0 * DEGREE, 0.0),
            held(),
        );
        assert!(sent.roll.step() > Channel::CENTRE.step());
        assert!(sent.pitch.step() < Channel::CENTRE.step());
    }

    #[test]
    fn holding_heading_turns_the_short_way_round() {
        let jobs = Jobs {
            hold_heading: Some(10.0 * DEGREE),
            ..Jobs::NONE
        };
        // At 350° the heading to hold is 20° to the right, across north.
        let sent = pilot(jobs).fly(&state(2.0, 0.0, 0.0, 0.0, 350.0 * DEGREE), held());
        assert!(sent.yaw.step() > Channel::CENTRE.step());
        let sent = pilot(jobs).fly(&state(2.0, 0.0, 0.0, 0.0, 30.0 * DEGREE), held());
        assert!(sent.yaw.step() < Channel::CENTRE.step());
    }

    #[test]
    fn it_never_touches_the_arm_switch_the_flight_mode_or_a_stick_it_has_no_job_for() {
        let jobs = Jobs {
            hold_height: Some(2.0),
            hold_level: true,
            hold_heading: None,
        };
        let sent = pilot(jobs).fly(&state(1.0, 0.0, 0.1, 0.1, 0.5), held());
        assert_eq!(sent.arm, held().arm);
        assert_eq!(sent.flight_mode, held().flight_mode);
        assert_eq!(sent.crash_flip, held().crash_flip);
        assert_eq!(sent.yaw, held().yaw);
    }
}
