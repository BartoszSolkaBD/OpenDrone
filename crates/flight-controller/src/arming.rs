//! The built-in Modes table and the arming checks, reimplemented from
//! Betaflight 2026.6.2's `rc_modes.c`, `rc_controls.c`
//! (`processRcStickPositions`) and `core.c` (`tryArm`, `disarm`,
//! `updateArmingStatus`).
//!
//! Switches reach the Flight Controller with fixed meanings (ADR-0017), so
//! the Modes table is built in: Arm on AUX1, high is armed. Flight Mode on
//! AUX2 and Crash Flip on AUX3 join it with their tickets (#51, #54).
//!
//! Arming follows Betaflight's checks, each named as Betaflight names it
//! ([`ArmingBlocks`]): the throttle below `min_check` (`THROTTLE`), the tilt
//! within the Tune's `small_angle` (`ANGLE`), the ESCs' ready beep
//! (`BOOTGRACE`), the Radio Link (`RXLOSS`, `NOT_DISARMED`) and Failsafe
//! (`FAILSAFE`), and the Arm switch going off before it can arm again once
//! any of them refused it (`ARM_SWITCH`).
//!
//! # Power-up
//!
//! Betaflight blocks arming for 5 s after it boots (`BOOTGRACE`,
//! `pwr_on_arm_grace`). OpenDrone skips that wait, and holds `BOOTGRACE`
//! instead until the ESCs have played their ready beep, about 1.7 s after
//! power-up (#32 §4): the sensor readings say when ([`crate::SensorReadings`]).
//! So an Arm switch already on at power-up is refused, and must go off and
//! on again (`ARM_SWITCH`).
//!
//! # Not yet
//!
//! - On arming and on disarming, Betaflight sends each ESC the DShot
//!   command "spin the normal way" (or reversed, for Crash Flip): ten times,
//!   1 ms apart, after a 10 ms wait, so the throttle reaches the ESCs about
//!   21 ms after the frame that armed or disarmed (`setMotorSpinDirection`,
//!   `dshot_command.c`). It comes with Crash Flip (#54); until then the
//!   motor commands change at once.
//! - Runaway takeoff prevention (`runaway_takeoff_prevention`, on by
//!   default) disarms a Quad whose PID sum on any axis stays at 600 or more
//!   with the gyro moving for 75 ms, until half a second of normal flight
//!   switches it off. It guards against wiring and orientation mistakes the
//!   sim can't have, so it comes later (#21); the Tune lists it as not
//!   simulated yet.

use opendrone_maths::{Attitude, Fingerprinter, Vec3, functions};

use crate::receiver::RcData;
use crate::tune::Tune;

/// One row of the Modes table: an AUX channel and the µs range in which the
/// mode is on, from `start` up to but not including `end`
/// (`isRangeActive`: Betaflight's ranges go in 25 µs steps from 900 µs).
struct ModeRange {
    aux: usize,
    start: f64,
    end: f64,
}

impl ModeRange {
    fn is_on(&self, rc: &RcData) -> bool {
        // Betaflight reads the channel as a whole number within 900–2099 µs.
        let value = rc.aux[self.aux].trunc().clamp(900.0, 2099.0);
        value >= self.start && value < self.end
    }
}

/// Arm: AUX1 from 1700 to 2100 µs, so the switch's high position (2012 µs)
/// arms and its low (988 µs) and middle (1500 µs) don't.
const ARM: ModeRange = ModeRange {
    aux: 0,
    start: 1700.0,
    end: 2100.0,
};

/// How many frames in a row the Arm switch must read off before an armed
/// Quad disarms: more than three (`rcDisarmTicks`).
const DISARM_FRAMES: u8 = 3;

/// Why arming is refused right now: Betaflight's arming-disabled flags
/// (`armingDisableFlags`) that OpenDrone's Flight Controller can raise, each
/// named as Betaflight names it. `RXLOSS` and `FAILSAFE` are raised and
/// cleared by the Radio Link and Failsafe whether armed or not; the others
/// are checked only while disarmed, so an armed Quad's stay as they were when
/// it armed (all clear).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmingBlocks {
    /// `FAILSAFE`: Failsafe dropped the Quad and hasn't ended.
    pub failsafe: bool,
    /// `RXLOSS`: no frames for 150 ms, until frames have arrived again for
    /// `failsafe_recovery_delay`.
    pub rx_loss: bool,
    /// `NOT_DISARMED`: the frames came back with the Arm switch on.
    pub not_disarmed: bool,
    /// `THROTTLE`: the throttle isn't below `min_check`.
    pub throttle: bool,
    /// `ANGLE`: the Quad is tilted further than `small_angle`.
    pub angle: bool,
    /// `BOOTGRACE`: just powered up, and the ESCs haven't played their ready
    /// beep yet.
    pub boot_grace: bool,
    /// `ARM_SWITCH`: the Arm switch went on while arming was refused, and
    /// must go off before it can arm.
    pub arm_switch: bool,
}

impl ArmingBlocks {
    /// Betaflight's names for the flags, in its order (its OSD shows the
    /// first one raised).
    pub const NAMES: [&'static str; 7] = [
        "FAILSAFE",
        "RXLOSS",
        "NOT_DISARMED",
        "THROTTLE",
        "ANGLE",
        "BOOTGRACE",
        "ARM_SWITCH",
    ];

    /// Each flag, in [`ArmingBlocks::NAMES`] order.
    pub fn flags(self) -> [bool; 7] {
        [
            self.failsafe,
            self.rx_loss,
            self.not_disarmed,
            self.throttle,
            self.angle,
            self.boot_grace,
            self.arm_switch,
        ]
    }

    /// Whether the flag Betaflight names `name` is raised, or `None` for a
    /// name it doesn't have here.
    pub fn named(self, name: &str) -> Option<bool> {
        let place = ArmingBlocks::NAMES.iter().position(|n| *n == name)?;
        Some(self.flags()[place])
    }

    /// The names of the flags raised, in Betaflight's order.
    pub fn names(self) -> Vec<&'static str> {
        ArmingBlocks::NAMES
            .into_iter()
            .zip(self.flags())
            .filter(|(_, raised)| *raised)
            .map(|(name, _)| name)
            .collect()
    }

    pub fn any(self) -> bool {
        self.flags().into_iter().any(|raised| raised)
    }
}

/// What the arming checks read of the world besides the Channels and the
/// Tune.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Readings {
    pub attitude: Attitude,
    /// The ESCs have played their ready beep.
    pub escs_ready: bool,
    /// Frames are arriving (`isRxReceivingSignal`).
    pub signal: bool,
    /// Failsafe counts the link as up (`failsafeIsReceivingRxData`).
    pub link_up: bool,
}

/// Whether the Quad is armed, and what the arming logic remembers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Arming {
    pub armed: bool,
    pub blocks: ArmingBlocks,
    /// Whether Arm was on as of the last frame's mode update.
    arm_mode: bool,
    /// Frames in a row with Arm off while armed.
    disarm_frames: u8,
    /// Whether frames were arriving when the arming status was last updated
    /// (`hadRx`). A fresh Flight Controller's link counts as settled.
    had_signal: bool,
}

impl Arming {
    /// At power-up: armed (a mid-air start) or not. Disarmed, `BOOTGRACE`
    /// holds until the ESCs are ready.
    pub fn new(armed: bool) -> Arming {
        Arming {
            armed,
            blocks: ArmingBlocks {
                boot_grace: !armed,
                ..ArmingBlocks::default()
            },
            arm_mode: armed,
            disarm_frames: 0,
            had_signal: true,
        }
    }

    /// Disarms (`disarm`), as Failsafe's DROP does.
    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// One frame's arming work, in Betaflight's order: the stick and switch
    /// checks read the modes as the last frame left them (arming, or counting
    /// towards a disarm), then the modes take this frame's switches, then the
    /// arming checks run again.
    ///
    /// A switch disarm counts only while Failsafe counts the link as up, so
    /// a held Arm Channel never disarms on its own during a loss.
    pub fn frame(&mut self, rc: &RcData, tune: &Tune, readings: Readings) {
        if self.arm_mode {
            self.disarm_frames = 0;
            self.update_status(rc, tune, readings);
            if !self.blocks.any() && !self.armed {
                self.armed = true;
            }
        } else if self.armed && readings.link_up {
            self.disarm_frames += 1;
            if self.disarm_frames > DISARM_FRAMES {
                self.armed = false;
                self.disarm_frames = 0;
            }
        }
        self.arm_mode = ARM.is_on(rc);
        self.update_status(rc, tune, readings);
    }

    /// Betaflight's `updateArmingStatus` while disarmed: each check sets or
    /// clears its flag; the Arm switch's flag is set whenever any flag stands
    /// with the switch on, and cleared only with the switch off.
    fn update_status(&mut self, rc: &RcData, tune: &Tune, readings: Readings) {
        if self.armed {
            return;
        }
        if self.blocks.boot_grace && readings.escs_ready {
            self.blocks.boot_grace = false;
        }
        let got_signal_back = !self.had_signal && readings.signal;
        if got_signal_back && self.arm_mode {
            self.blocks.not_disarmed = true;
        } else if readings.signal && !self.arm_mode {
            self.blocks.not_disarmed = false;
        }
        self.had_signal = readings.signal;
        self.blocks.throttle = !rc.throttle_low(tune);
        self.blocks.angle = !is_upright(readings.attitude, tune);
        if self.blocks.any() && self.arm_mode {
            self.blocks.arm_switch = true;
        } else if !self.arm_mode {
            self.blocks.arm_switch = false;
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.armed));
        for flag in self.blocks.flags() {
            f.write_u64(u64::from(flag));
        }
        f.write_u64(u64::from(self.arm_mode));
        f.write_u64(u64::from(self.disarm_frames));
        f.write_u64(u64::from(self.had_signal));
    }
}

/// True when the Quad is tilted less than `small_angle`: the cosine of its
/// tilt (its up axis against the world's) is above the cosine of the limit
/// (`isUpright`).
fn is_upright(attitude: Attitude, tune: &Tune) -> bool {
    let cos_tilt = attitude.body_to_world(Vec3::new(0.0, 0.0, 1.0)).z;
    let limit = f64::from(tune.small_angle) * opendrone_maths::DEGREE;
    cos_tilt > functions::cos(limit)
}
