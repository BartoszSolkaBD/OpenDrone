//! The Flight Controller's beeper, reimplemented from the behaviour of
//! Betaflight 2026.6.2's `io/beeper.c`: what it beeps, when, and in which
//! pattern. The OSD's visual beeper flashes `  * * * *` whenever a beep
//! starts (#31 §1), and the sound (#75) plays the same beeps on a Quad with a
//! buzzer.
//!
//! Each beep is a pattern of on and off times, in hundredths of a second,
//! played 100 times a second (`TASK_BEEPER`). A new beep starts only if it
//! ranks above the one playing (Betaflight's priority list), so the same beep
//! asked for again doesn't restart. The ones the alpha can sound:
//!
//! - three "gyro calibrated" beeps at power-up: a fresh Flight Controller
//!   counts as calibrated at once (#32 §3);
//! - `RX_LOST` whenever Failsafe finds the link down, from the drop until
//!   the link is back;
//! - disarming (but not when runaway takeoff prevention disarmed it) and
//!   arming;
//! - the battery's low and critical beeps, five times a second while it is;
//! - the warning beeps when the Arm switch is on and arming is refused: five
//!   quick flashes, then the lowest refusal's number in Betaflight's list,
//!   counted out in long (fives) and short beeps, each time that refusal
//!   changes;
//! - `CRASHFLIP`, on every frame in Crash Flip.
//!
//! Every beep counts as on: the Tune's `beeper` settings, which can switch
//! some off, arrive with the sound (#75).

use opendrone_maths::Fingerprinter;

/// What the beeper is playing, by Betaflight's name for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeeperMode {
    /// `GYRO_CALIBRATED`: three beeps at power-up.
    GyroCalibrated,
    /// `RX_LOST`: the link is down.
    RxLost,
    /// `DISARMING`.
    Disarming,
    /// `ARMING`.
    Arming,
    /// `BAT_CRIT_LOW`: the battery is critical.
    BatteryCriticalLow,
    /// `BAT_LOW`: the battery is low.
    BatteryLow,
    /// `MULTI_BEEPS`: here, the warning beeps for a refused arm.
    MultiBeeps,
    /// `CRASHFLIP`: in Crash Flip.
    CrashFlip,
}

impl BeeperMode {
    /// Betaflight's name for it.
    pub fn name(self) -> &'static str {
        match self {
            BeeperMode::GyroCalibrated => "GYRO_CALIBRATED",
            BeeperMode::RxLost => "RX_LOST",
            BeeperMode::Disarming => "DISARMING",
            BeeperMode::Arming => "ARMING",
            BeeperMode::BatteryCriticalLow => "BAT_CRIT_LOW",
            BeeperMode::BatteryLow => "BAT_LOW",
            BeeperMode::MultiBeeps => "MULTI_BEEPS",
            BeeperMode::CrashFlip => "CRASHFLIP",
        }
    }

    /// Its place in Betaflight's priority list: lower ranks higher.
    fn priority(self) -> u8 {
        match self {
            BeeperMode::GyroCalibrated => 0,
            BeeperMode::RxLost => 1,
            BeeperMode::Disarming => 3,
            BeeperMode::Arming => 4,
            BeeperMode::BatteryCriticalLow => 7,
            BeeperMode::BatteryLow => 8,
            BeeperMode::MultiBeeps => 14,
            BeeperMode::CrashFlip => 20,
        }
    }

    /// Its fixed pattern: on, off, on, off… in hundredths of a second, a 0
    /// first for one that starts with a pause. The warning beeps are built
    /// for each refusal ([`warning_pattern`]).
    fn pattern(self) -> &'static [u8] {
        match self {
            BeeperMode::GyroCalibrated => &[20, 10, 20, 10, 20, 10],
            BeeperMode::RxLost => &[50, 50],
            BeeperMode::Disarming => &[15, 5, 15, 5],
            BeeperMode::Arming => &[30, 5, 5, 5],
            BeeperMode::BatteryCriticalLow => &[50, 2],
            BeeperMode::BatteryLow => &[25, 50],
            BeeperMode::MultiBeeps => &[],
            BeeperMode::CrashFlip => &[20, 15, 35, 5],
        }
    }

    fn code(self) -> u64 {
        u64::from(self.priority())
    }
}

/// `beeperWarningBeeps`: five quick flashes and a pause, then `count` told
/// in long beeps (each five) and short beeps.
pub(crate) fn warning_pattern(count: u8) -> Vec<u8> {
    let mut pattern = Vec::new();
    let mut add = |count: u8, on: u8, off: u8, off_last: u8| {
        for left in (1..=count).rev() {
            pattern.push(on);
            if left > 1 {
                pattern.push(off);
            } else if off_last > 0 {
                pattern.push(off_last);
            }
        }
    };
    // WARNING_FLASH_COUNT - 1 flashes of 50 ms, then a 500 ms pause.
    add(4, 5, 5, 50);
    // Long beeps of 250 ms; short beeps of 50 ms, 250 ms apart.
    add(count / 5, 25, 25, 50);
    add(count % 5, 5, 25, 0);
    pattern
}

/// The beeper as one loop left it, for the OSD and the sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeeperReadings {
    /// What it's playing, if anything.
    pub mode: Option<BeeperMode>,
    /// Sounding right now.
    pub on: bool,
    /// How many beeps have started since power-up: the visual beeper
    /// flashes once for each.
    pub beeps: u32,
}

/// The beeper's work every 10 ms (`TASK_BEEPER` at 100 Hz).
const UPDATE_EVERY_US: u64 = 10_000;

/// The beep playing and where it has got to.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Playing {
    mode: BeeperMode,
    pattern: Vec<u8>,
    /// The next step of the pattern.
    place: usize,
    /// When it next turns on or off, in µs; `None` before it starts.
    toggle_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Beeper {
    playing: Option<Playing>,
    on: bool,
    /// Whether the last step turned it on (`visualWasOn`).
    was_on: bool,
    beeps: u32,
    next_update: u64,
}

impl Beeper {
    pub fn new() -> Beeper {
        Beeper {
            playing: None,
            on: false,
            was_on: false,
            beeps: 0,
            next_update: 0,
        }
    }

    pub fn readings(&self) -> BeeperReadings {
        BeeperReadings {
            mode: self.playing.as_ref().map(|playing| playing.mode),
            on: self.on,
            beeps: self.beeps,
        }
    }

    /// `beeper`: starts a beep unless one that ranks as high or higher is
    /// playing.
    pub fn beep(&mut self, mode: BeeperMode) {
        self.start(mode, mode.pattern().to_vec());
    }

    /// The warning beeps for a refused arm, telling `count`.
    pub fn warn(&mut self, count: u8) {
        self.start(BeeperMode::MultiBeeps, warning_pattern(count));
    }

    fn start(&mut self, mode: BeeperMode, pattern: Vec<u8>) {
        if let Some(playing) = &self.playing
            && mode.priority() >= playing.mode.priority()
        {
            return;
        }
        self.playing = Some(Playing {
            mode,
            pattern,
            place: 0,
            toggle_at: None,
        });
    }

    /// `beeperUpdate`, when it is due by `now_us`.
    pub fn update(&mut self, now_us: u64) {
        if now_us < self.next_update {
            return;
        }
        self.next_update += UPDATE_EVERY_US;
        let Some(playing) = &mut self.playing else {
            return;
        };
        if playing.toggle_at.is_some_and(|at| at > now_us) {
            return;
        }
        // `beeperSequenceAdvance`: skip any 0, then take the next time.
        while playing.pattern.get(playing.place) == Some(&0) {
            playing.place += 1;
        }
        let turned_on = match playing.pattern.get(playing.place) {
            None => {
                self.playing = None;
                self.on = false;
                false
            }
            Some(&hundredths) => {
                playing.toggle_at = Some(now_us + 10_000 * u64::from(hundredths));
                playing.place += 1;
                // On and off go by place: odd places count turn it on.
                let on = playing.place % 2 == 1;
                self.on = on;
                on
            }
        };
        if turned_on && !self.was_on {
            self.beeps += 1;
        }
        self.was_on = turned_on;
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        match &self.playing {
            None => f.write_u64(0),
            Some(playing) => {
                f.write_u64(1);
                f.write_u64(playing.mode.code());
                f.write_u64(playing.pattern.len() as u64);
                for &step in &playing.pattern {
                    f.write_u64(u64::from(step));
                }
                f.write_u64(playing.place as u64);
                f.write_u64(playing.toggle_at.unwrap_or(u64::MAX));
            }
        }
        f.write_u64(u64::from(self.on));
        f.write_u64(u64::from(self.was_on));
        f.write_u64(u64::from(self.beeps));
        f.write_u64(self.next_update);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bootgrace_warning_flashes_five_times_then_counts_two_long_beeps() {
        // BOOTGRACE is Betaflight's 10th refusal (bit 9): two long beeps.
        assert_eq!(
            warning_pattern(10),
            [5, 5, 5, 5, 5, 5, 5, 50, 25, 25, 25, 50]
        );
    }

    #[test]
    fn the_rxloss_warning_flashes_then_counts_three_short_beeps() {
        assert_eq!(
            warning_pattern(3),
            [5, 5, 5, 5, 5, 5, 5, 50, 5, 25, 5, 25, 5]
        );
    }
}
