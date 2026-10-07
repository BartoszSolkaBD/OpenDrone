//! The built-in Modes table and the basic arming checks, reimplemented from
//! Betaflight 2026.6.2's `rc_modes.c`, `rc_controls.c`
//! (`processRcStickPositions`) and `core.c` (`tryArm`,
//! `updateArmingStatus`).
//!
//! Switches reach the Flight Controller with fixed meanings (ADR-0017), so
//! the Modes table is built in: Arm on AUX1, high is armed. Flight Mode on
//! AUX2 and Crash Flip on AUX3 join it with their tickets (#51, #54).
//!
//! Arming follows three of Betaflight's checks: the throttle below
//! `min_check`, the tilt within the Tune's `small_angle`, and the Arm switch
//! going off before it can arm again once arming was refused. The rest
//! (the ESCs' ready beep, a switch already on at power-up, Failsafe) come
//! with the arming and power-up ticket (#52).

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

/// Why arming is refused right now, each as Betaflight names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmingBlocks {
    /// `THROTTLE`: the throttle isn't below `min_check`.
    pub throttle: bool,
    /// `ANGLE`: the Quad is tilted further than `small_angle`.
    pub angle: bool,
    /// `ARMSWITCH`: the Arm switch went on while arming was refused, and
    /// must go off before it can arm.
    pub arm_switch: bool,
}

impl ArmingBlocks {
    pub fn any(self) -> bool {
        self.throttle || self.angle || self.arm_switch
    }
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
}

impl Arming {
    pub fn new(armed: bool) -> Arming {
        Arming {
            armed,
            blocks: ArmingBlocks::default(),
            arm_mode: armed,
            disarm_frames: 0,
        }
    }

    /// One frame's arming work, in Betaflight's order: the stick and switch
    /// checks read the modes as the last frame left them (arming, or counting
    /// towards a disarm), then the modes take this frame's switches, then the
    /// arming checks run again.
    pub fn frame(&mut self, rc: &RcData, tune: &Tune, attitude: Attitude) {
        if self.arm_mode {
            self.disarm_frames = 0;
            self.update_blocks(rc, tune, attitude);
            if !self.blocks.any() && !self.armed {
                self.armed = true;
            }
        } else if self.armed {
            self.disarm_frames += 1;
            if self.disarm_frames > DISARM_FRAMES {
                self.armed = false;
                self.disarm_frames = 0;
            }
        }
        self.arm_mode = ARM.is_on(rc);
        self.update_blocks(rc, tune, attitude);
    }

    /// Betaflight's `updateArmingStatus` while disarmed: each check sets or
    /// clears its block; the Arm switch's block is set whenever any block
    /// stands with the switch on, and cleared only with the switch off.
    fn update_blocks(&mut self, rc: &RcData, tune: &Tune, attitude: Attitude) {
        if self.armed {
            return;
        }
        self.blocks.throttle = !rc.throttle_low(tune);
        self.blocks.angle = !is_upright(attitude, tune);
        if self.blocks.any() && self.arm_mode {
            self.blocks.arm_switch = true;
        } else if !self.arm_mode {
            self.blocks.arm_switch = false;
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for flag in [
            self.armed,
            self.blocks.throttle,
            self.blocks.angle,
            self.blocks.arm_switch,
            self.arm_mode,
        ] {
            f.write_u64(u64::from(flag));
        }
        f.write_u64(u64::from(self.disarm_frames));
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
