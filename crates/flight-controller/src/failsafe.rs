//! Losing the Radio Link: Betaflight 2026.6.2's watch on the frames and its
//! hold of the Channels (`rxFrameCheck` and
//! `detectAndApplySignalLossBehaviour`, `src/main/rx/rx.c`), and its Failsafe
//! (`src/main/flight/failsafe.c`), reimplemented from their behaviour.
//!
//! # The timeline (Betaflight's defaults)
//!
//! Counted from the last good frame:
//!
//! - **Up to 150 ms** (`RXLOSS_TRIGGER_INTERVAL`) the Quad flies on its last
//!   values, as it does between any two frames.
//! - **At 150 ms** the receiver counts the signal as lost: arming is blocked
//!   (`RXLOSS`), and Betaflight looks again every 50 ms
//!   (`RX_FRAME_RECHECK_INTERVAL`). Each Channel still holds its last good
//!   value.
//! - **At 300 ms** (`MAX_INVALID_PULSE_TIME_MS`) each Channel takes its
//!   stage 1 value (`rxfail`, left at Betaflight's defaults: the sticks
//!   "auto", AUX "hold"): roll, pitch and yaw centre at `mid_rc`, the
//!   throttle drops to `rx_min_usec`, and AUX1–3 hold.
//! - **At `failsafe_delay`** (1.5 s) the link counts as down and stage 2
//!   starts: Failsafe's procedure, DROP, disarms the Quad (`FAILSAFE`). The
//!   check runs whenever more than 10 ms have passed since the last one, so
//!   every 11 ms.
//!
//! To fly again, frames must arrive for `failsafe_recovery_delay` (0.5 s)
//! before the link counts as up (`RXLOSS` clears), and then Failsafe waits as
//! long again before it ends (`FAILSAFE` clears). Until it ends, AUX1–3 hold
//! the values they had when the link went, so the Arm switch is read again
//! only then; arming also needs the switch to go off and on again
//! ([`crate::arming`]).
//!
//! DROP is the only procedure simulated: a Tune set to AUTO-LAND or
//! GPS-RESCUE flies DROP (#21).
//!
//! # A fresh Flight Controller
//!
//! Power-up skips Betaflight's own waits: Failsafe watches the link at once
//! (Betaflight waits 5 s, `FAILSAFE_POWER_ON_DELAY_US`), and the link counts
//! as already up and settled, as if a frame had just arrived, so the first
//! frame doesn't start the 0.5 s `RXLOSS` wait Betaflight has after it boots.

use opendrone_maths::Fingerprinter;

use crate::arming::{Arming, ArmingBlocks};
use crate::receiver::{CHANNEL_COUNT, RcData, STICK_COUNT, THROTTLE};
use crate::tune::Tune;
use crate::{Channels, constrain};

/// A frame must come within this, or the signal counts as lost, in µs
/// (`RXLOSS_TRIGGER_INTERVAL`).
const SIGNAL_LOST_AFTER: u64 = 150_000;
/// Once the signal is lost, how often Betaflight looks again, in µs
/// (`RX_FRAME_RECHECK_INTERVAL`).
const RECHECK: u64 = 50_000;
/// How long a Channel holds its last good value, in ms
/// (`MAX_INVALID_PULSE_TIME_MS`).
const HOLD: u64 = 300;
/// The shortest stage 1 and recovery periods, in ms
/// (`PERIOD_RXDATA_RECOVERY`).
const SHORTEST_PERIOD: u64 = 100;
/// Failsafe is checked whenever more than this has passed since the last
/// check, in ms (`PERIOD_RXDATA_FAILURE`).
const CHECK_AFTER: u64 = 10;
/// Every Channel is kept within this range, in µs (`PWM_PULSE_MIN`,
/// `PWM_PULSE_MAX`).
const PULSE_MIN: f64 = 750.0;
const PULSE_MAX: f64 = 2250.0;

/// The receiver's side: whether frames are arriving, and the Channels the
/// Flight Controller uses (`rcData`), held or set to their stage 1 and stage
/// 2 values while the link is lost.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Receiver {
    /// A frame must arrive before this, in µs (`needRxSignalBefore`).
    frame_due_by: u64,
    /// Frames are arriving (`rxSignalReceived`, `isRxReceivingSignal`).
    pub signal: bool,
    /// The newest frame's Channels in µs, in Betaflight's order (`rcRaw`);
    /// none before the first frame.
    newest: Option<[f64; CHANNEL_COUNT]>,
    /// Until when each Channel may hold its last good value, in ms
    /// (`validRxSignalTimeout`).
    good_until: [u64; CHANNEL_COUNT],
    /// The Channels the Flight Controller uses (`rcData`).
    pub data: RcData,
}

impl Receiver {
    /// At power-up: Betaflight's starting Channels, and the link settled, as
    /// if a frame had just arrived.
    pub fn at_power_up(tune: &Tune) -> Receiver {
        Receiver {
            frame_due_by: SIGNAL_LOST_AFTER,
            signal: true,
            newest: None,
            good_until: [HOLD; CHANNEL_COUNT],
            data: RcData::at_power_up(tune),
        }
    }

    /// Betaflight's look for a frame, once a loop (`rxFrameCheck`): true when
    /// the Channels must be worked out again, because a frame arrived or
    /// because none came in time.
    pub fn look(&mut self, now_us: u64, frame: Option<&Channels>) -> bool {
        if let Some(channels) = frame {
            self.frame_due_by = now_us + SIGNAL_LOST_AFTER;
            self.signal = true;
            self.newest = Some(RcData::from_channels(channels).values());
            true
        } else if now_us > self.frame_due_by {
            self.signal = false;
            self.frame_due_by += RECHECK;
            true
        } else {
            false
        }
    }

    /// Works out the Channels the Flight Controller uses
    /// (`detectAndApplySignalLossBehaviour`), and says whether the flight
    /// Channels are good (`rxFlightChannelsValid`).
    pub fn apply(&mut self, now_ms: u64, tune: &Tune, stage_2: bool) -> bool {
        let mut flight_channels_good = self.signal;
        let mut values = self.data.values();
        for channel in 0..CHANNEL_COUNT {
            // Before the first frame Betaflight's raw value is 0, which is
            // never a good pulse.
            let newest = self.newest.map_or(0.0, |newest| newest[channel]);
            let good = flight_channels_good && pulse_is_good(newest, tune);
            if good {
                self.good_until[channel] = now_ms + HOLD;
            }
            let held = values[channel];
            let sample = if stage_2 {
                if channel < STICK_COUNT {
                    match (good, channel) {
                        (true, _) => newest,
                        (false, THROTTLE) => f64::from(tune.failsafe_throttle),
                        (false, _) => f64::from(tune.mid_rc),
                    }
                } else {
                    stage_1_value(channel, held, tune)
                }
            } else if good {
                newest
            } else if now_ms < self.good_until[channel] {
                held
            } else {
                if channel < STICK_COUNT {
                    // A flight Channel bad for more than 300 ms loses the
                    // signal for the rest.
                    flight_channels_good = false;
                }
                stage_1_value(channel, held, tune)
            };
            values[channel] = constrain(sample, PULSE_MIN, PULSE_MAX);
        }
        self.data = RcData::from_values(values);
        flight_channels_good
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.frame_due_by);
        f.write_u64(u64::from(self.signal));
        match &self.newest {
            None => f.write_u64(0),
            Some(newest) => {
                f.write_u64(1);
                f.write_f64s(newest);
            }
        }
        for until in self.good_until {
            f.write_u64(until);
        }
        f.write_f64s(&self.data.values());
    }
}

/// True when a Channel is within `rx_min_usec` and `rx_max_usec`, read as a
/// whole number of µs, as Betaflight passes it (`isPulseValid`).
fn pulse_is_good(micros: f64, tune: &Tune) -> bool {
    let whole = micros.trunc();
    whole >= f64::from(tune.rx_min_usec) && whole <= f64::from(tune.rx_max_usec)
}

/// A Channel's stage 1 value with Betaflight's default `rxfail` settings
/// (`getRxfailValue`): the sticks "auto" (roll, pitch and yaw at `mid_rc`,
/// the throttle at `rx_min_usec`), AUX1–3 "hold" (their last good value).
fn stage_1_value(channel: usize, held: f64, tune: &Tune) -> f64 {
    match channel {
        THROTTLE => f64::from(tune.rx_min_usec),
        c if c < STICK_COUNT => f64::from(tune.mid_rc),
        _ => held,
    }
}

/// Betaflight's Failsafe phases (`failsafePhase_e`), those DROP passes
/// through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailsafePhase {
    /// Watching the link; nothing has happened, or stage 1 runs (`IDLE`).
    Idle,
    /// The link went down while armed (`RX_LOSS_DETECTED`).
    RxLossDetected,
    /// DROP: disarm now (`LANDED`).
    Landed,
    /// Disarmed, waiting for the link to come back (`RX_LOSS_MONITORING`).
    RxLossMonitoring,
    /// The link is back: Failsafe ends (`RX_LOSS_RECOVERED`).
    RxLossRecovered,
}

impl FailsafePhase {
    /// Betaflight's name for it.
    pub fn name(self) -> &'static str {
        match self {
            FailsafePhase::Idle => "IDLE",
            FailsafePhase::RxLossDetected => "RX_LOSS_DETECTED",
            FailsafePhase::Landed => "LANDED",
            FailsafePhase::RxLossMonitoring => "RX_LOSS_MONITORING",
            FailsafePhase::RxLossRecovered => "RX_LOSS_RECOVERED",
        }
    }

    fn code(self) -> u64 {
        match self {
            FailsafePhase::Idle => 0,
            FailsafePhase::RxLossDetected => 1,
            FailsafePhase::Landed => 2,
            FailsafePhase::RxLossMonitoring => 3,
            FailsafePhase::RxLossRecovered => 4,
        }
    }
}

/// Betaflight's Failsafe (`failsafeState_t`), with every time in ms since
/// power-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failsafe {
    pub phase: FailsafePhase,
    /// Stage 2 is on (`failsafeIsActive`): from the drop until Failsafe
    /// ends.
    pub active: bool,
    /// The link counts as up (`rxLinkState`, `failsafeIsReceivingRxData`).
    pub link_up: bool,
    /// Stage 2 starts this long after the last good frame
    /// (`rxDataFailurePeriod`).
    failure_period: u64,
    /// Frames must arrive this long before the link counts as up again
    /// (`rxDataRecoveryPeriod`).
    recovery_period: u64,
    /// When good Channels last arrived (`validRxDataReceivedAt`).
    good_at: u64,
    /// When the Channels were last bad (`validRxDataFailedAt`).
    bad_at: u64,
    /// After a drop, Failsafe ends once the link has been up past this
    /// (`receivingRxDataPeriod`).
    up_until_end: u64,
    /// When Failsafe was last checked (the scheduler's
    /// `lastFailsafeCheckMs`).
    last_check: u64,
}

impl Failsafe {
    /// At power-up: watching at once, the link up.
    pub fn at_power_up(tune: &Tune) -> Failsafe {
        let tenths = |value: u8| (u64::from(value) * 100).max(SHORTEST_PERIOD);
        Failsafe {
            phase: FailsafePhase::Idle,
            active: false,
            link_up: true,
            failure_period: tenths(tune.failsafe_delay),
            recovery_period: tenths(tune.failsafe_recovery_delay),
            good_at: 0,
            bad_at: 0,
            up_until_end: 0,
            last_check: 0,
        }
    }

    /// Good Channels arrived (`failsafeOnValidDataReceived`): once they have
    /// kept arriving for the recovery period, the link is up and `RXLOSS`
    /// clears.
    pub fn good_channels(&mut self, now_ms: u64, blocks: &mut ArmingBlocks) {
        self.good_at = now_ms;
        if self.good_at.saturating_sub(self.bad_at) > self.recovery_period {
            self.link_up = true;
            blocks.rx_loss = false;
        }
    }

    /// The Channels were bad (`failsafeOnValidDataFailed`): `RXLOSS` blocks
    /// arming, and past the stage 1 period the link is down.
    pub fn bad_channels(&mut self, now_ms: u64, blocks: &mut ArmingBlocks) {
        blocks.rx_loss = true;
        self.bad_at = now_ms;
        if self.bad_at.saturating_sub(self.good_at) > self.failure_period {
            self.link_up = false;
        }
    }

    /// Betaflight's scheduler checks Failsafe whenever more than 10 ms have
    /// passed since the last check: the link goes down once no good Channels
    /// have arrived for the stage 1 period
    /// (`failsafeCheckDataFailurePeriod`), then Failsafe moves on
    /// (`failsafeUpdateState`).
    pub fn check(&mut self, now_ms: u64, arming: &mut Arming) {
        if now_ms.saturating_sub(self.last_check) <= CHECK_AFTER {
            return;
        }
        if now_ms.saturating_sub(self.good_at) > self.failure_period {
            self.link_up = false;
            arming.blocks.rx_loss = true;
        }
        self.update(now_ms, arming);
        self.last_check = now_ms;
    }

    /// `failsafeUpdateState`, for DROP: every procedure flies it.
    fn update(&mut self, now_ms: u64, arming: &mut Arming) {
        let receiving = self.link_up;
        loop {
            match self.phase {
                FailsafePhase::Idle => {
                    if !(arming.armed && !receiving) {
                        return;
                    }
                    // Betaflight's "just disarm", for a throttle that was
                    // low for `failsafe_throttle_low_delay`, ends the same
                    // way as DROP.
                    self.phase = FailsafePhase::RxLossDetected;
                }
                FailsafePhase::RxLossDetected => {
                    if receiving {
                        self.phase = FailsafePhase::RxLossRecovered;
                    } else {
                        self.active = true;
                        self.phase = FailsafePhase::Landed;
                    }
                }
                FailsafePhase::Landed => {
                    arming.disarm();
                    arming.blocks.failsafe = true;
                    self.up_until_end = now_ms + self.recovery_period;
                    self.phase = FailsafePhase::RxLossMonitoring;
                }
                FailsafePhase::RxLossMonitoring => {
                    if !receiving {
                        self.up_until_end = now_ms + self.recovery_period;
                        return;
                    }
                    if now_ms <= self.up_until_end {
                        return;
                    }
                    self.phase = FailsafePhase::RxLossRecovered;
                }
                FailsafePhase::RxLossRecovered => {
                    self.phase = FailsafePhase::Idle;
                    self.active = false;
                    arming.blocks.failsafe = false;
                }
            }
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.phase.code());
        f.write_u64(u64::from(self.active));
        f.write_u64(u64::from(self.link_up));
        for ms in [
            self.failure_period,
            self.recovery_period,
            self.good_at,
            self.bad_at,
            self.up_until_end,
            self.last_check,
        ] {
            f.write_u64(ms);
        }
    }
}
