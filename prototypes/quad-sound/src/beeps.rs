//! PROTOTYPE (#34). The Quad's beeps, from the firmware sources (every number is traced, with
//! file and line, in results/beeps-facts.md):
//! - Bluejay v0.21.0 (bird-sanctuary/bluejay): the start-up melody, "signal found" and "ready"
//!   beeps, the three falling stall tones and the 10-minute beacon.
//! - Betaflight 2026.6.2 `src/main/io/beeper.c`: the beeper patterns and priorities, played on
//!   an active buzzer (it makes its own fixed pitch; the FC only switches it on and off).

#[derive(Clone, Copy, Debug)]
pub struct Note {
    /// 0 = rest.
    pub hz: f32,
    pub ms: f32,
    /// Loudness 0..1: Bluejay's beep strength 40 (default) = 0.5, beacon strength 80 = 1.0.
    pub amp: f32,
}

const fn n(hz: f32, ms: f32) -> Note {
    Note { hz, ms, amp: 0.5 }
}
const fn rest(ms: f32) -> Note {
    Note { hz: 0.0, ms, amp: 0.0 }
}

// Bluejay's plain beeps at strength 40 (Hz, ms): facts §1.3.
const F1: Note = n(492.4, 107.6);
const F2: Note = n(661.5, 116.4);
const F3: Note = n(747.0, 123.2);
const F1_SHORT: Note = n(492.4, 60.9);
const F2_SHORT: Note = n(661.5, 66.5);

/// Bluejay's default melody ("Bluejay Default", 570 bpm), decoded from its EEPROM bytes
/// `2,58,4,32,52,66,13,0,69,45,13,0,52,66,13,0,78,39,211,0,69,45,208,25,52,25,0` (facts §1.4).
pub const MELODY: [Note; 11] = [
    n(492.4, 105.6),
    rest(13.0),
    n(661.5, 104.3),
    rest(13.0),
    n(492.4, 105.6),
    rest(13.0),
    n(733.5, 106.3),
    rest(211.7),
    n(661.5, 104.3),
    n(983.0, 211.6),
    n(983.0, 52.9),
];

/// The first non-zero throttle waits 100 ms ("glitch" check) before the motor starts (§1.5).
pub const BLUEJAY_START_WAIT_S: f32 = 0.1;
/// 200 steps of 94 Timer2 ticks at zero throttle with a signal: ≈603.5 s (§1.8).
pub const BLUEJAY_BEACON_DELAY_S: f32 = 603.5;
/// The beacon repeats every ≈3.1 s (§1.8).
pub const BLUEJAY_BEACON_INTERVAL_S: f32 = 3.1;
/// 100 ms between start attempts; 3 failed starts and it gives up (§1.7).
pub const BLUEJAY_RESTART_GAP_S: f32 = 0.1;
pub const BLUEJAY_RESTART_ATTEMPTS: u32 = 3;
/// Zero throttle this long before the "ready" beep (10 Timer2 periods of 32.1 ms, §1.5).
pub const BLUEJAY_ZERO_THROTTLE_S: f32 = 0.321;
/// The two current pulses of each beep period are ≈199 µs apart (§1.1).
pub const BLUEJAY_PULSE_GAP_S: f32 = 199e-6;

/// Power-up: wait 100 ms, melody, wait 100 ms, DShot detection, "signal found" (f1_short),
/// "ready" (f2_short) at ≈1.595 s; throttle accepted from ≈1.66 s (§1.5). DShot300 (the
/// Meteor65 Pro's `motor_pwm_protocol`) finds the signal at ≈1.343 s, DShot600 at ≈1.444 s.
pub fn bluejay_power_up(dshot600: bool) -> Vec<Note> {
    let mut v = vec![rest(100.0)];
    v.extend_from_slice(&MELODY);
    v.push(rest(1242.0 - 100.0 - 1041.4));
    let found = if dshot600 { 1444.0 } else { 1343.0 };
    v.push(rest(found - 1242.0));
    v.push(F1_SHORT);
    v.push(rest(1595.0 - found - F1_SHORT.ms));
    v.push(F2_SHORT);
    v
}

/// ≈1.66 s: the end of the "ready" beep (§1.5).
pub fn bluejay_ready_s() -> f32 {
    1.6615
}

/// A start that failed 3 times: f3, f2, f1 falling, then f1_short at once (§1.7). The f2_short
/// "ready" follows once the throttle has been zero for ≈0.32 s (`bluejay_ready_again`).
pub fn bluejay_stall() -> Vec<Note> {
    vec![F3, F2, F1, F1_SHORT]
}

pub fn bluejay_ready_again() -> Vec<Note> {
    vec![F2_SHORT]
}

/// The beacon: f4 at beacon strength 80, ≈971 Hz, ≈144 ms (§1.8).
pub fn bluejay_beacon() -> Vec<Note> {
    vec![Note { hz: 971.0, ms: 144.0, amp: 1.0 }]
}

/// Plays a list of notes (one ESC).
#[derive(Default, Clone)]
pub struct EscPlayer {
    notes: Vec<Note>,
    idx: usize,
    t: f32,
    delay: f32,
}

impl EscPlayer {
    pub fn play(&mut self, notes: Vec<Note>, delay_s: f32) {
        self.notes = notes;
        self.idx = 0;
        self.t = 0.0;
        self.delay = delay_s;
    }
    pub fn stop(&mut self) {
        self.notes.clear();
        self.idx = 0;
    }
    pub fn step(&mut self, dt: f32) {
        if self.delay > 0.0 {
            self.delay -= dt;
            return;
        }
        if self.idx >= self.notes.len() {
            return;
        }
        self.t += dt * 1000.0;
        while self.idx < self.notes.len() && self.t >= self.notes[self.idx].ms {
            self.t -= self.notes[self.idx].ms;
            self.idx += 1;
        }
    }
    pub fn output(&self) -> (f32, f32) {
        if self.delay > 0.0 {
            return (0.0, 0.0);
        }
        match self.notes.get(self.idx) {
            Some(n) if n.hz > 0.0 => (n.hz, n.amp),
            _ => (0.0, 0.0),
        }
    }
    pub fn playing(&self) -> bool {
        self.delay > 0.0 || self.idx < self.notes.len()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BfBeep {
    SystemInit,
    GyroCalibrated,
    Arming,
    Disarming,
    ModeChange,
    BatLow,
    BatCritLow,
    RxLost,
    CrashFlip,
}

impl BfBeep {
    pub fn label(self) -> &'static str {
        match self {
            BfBeep::SystemInit => "power-up chirp",
            BfBeep::GyroCalibrated => "GYRO_CALIBRATED",
            BfBeep::Arming => "ARMING",
            BfBeep::Disarming => "DISARMING",
            BfBeep::ModeChange => "flight mode change",
            BfBeep::BatLow => "BAT_LOW",
            BfBeep::BatCritLow => "BAT_CRIT_LOW",
            BfBeep::RxLost => "RX_LOST",
            BfBeep::CrashFlip => "CRASHFLIP",
        }
    }
    /// Lower number wins: Betaflight's beeperTable order (facts §2.3).
    fn priority(self) -> u8 {
        match self {
            BfBeep::GyroCalibrated => 0,
            BfBeep::RxLost => 1,
            BfBeep::Disarming => 3,
            BfBeep::Arming => 4,
            BfBeep::BatCritLow => 7,
            BfBeep::BatLow => 8,
            BfBeep::ModeChange => 14, // beeperConfirmationBeeps(1) uses MULTI_BEEPS
            BfBeep::SystemInit => 17, // played directly by init.c, before gyro calibration
            BfBeep::CrashFlip => 20,
        }
    }
    /// On/off durations in ms, starting with "on" (beeper.c arrays x 10 ms; facts §2.2, §2.4).
    /// None repeats by itself: callers ask again while the condition lasts.
    fn pattern(self) -> &'static [f32] {
        match self {
            // init.c: 10 x (off 25 ms, on 25 ms)
            BfBeep::SystemInit => &[
                0.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0, 25.0,
                25.0, 25.0, 25.0,
            ],
            BfBeep::GyroCalibrated => &[200.0, 100.0, 200.0, 100.0, 200.0, 100.0],
            BfBeep::Arming => &[300.0, 50.0, 50.0, 50.0],
            BfBeep::Disarming => &[150.0, 50.0, 150.0, 50.0],
            BfBeep::ModeChange => &[20.0, 200.0],
            BfBeep::BatLow => &[250.0, 500.0],
            BfBeep::BatCritLow => &[500.0, 20.0],
            BfBeep::RxLost => &[500.0, 500.0],
            BfBeep::CrashFlip => &[200.0, 150.0, 350.0, 50.0],
        }
    }
}

/// Betaflight's beeper: one pattern at a time; `beeper(mode)` is ignored while a pattern of the
/// same or higher priority plays (facts §2.1).
#[derive(Default, Clone)]
pub struct Beeper {
    cur: Option<BfBeep>,
    idx: usize,
    t: f32,
}

impl Beeper {
    pub fn start(&mut self, b: BfBeep) {
        if let Some(c) = self.cur {
            if b.priority() >= c.priority() {
                return;
            }
        }
        self.cur = Some(b);
        self.idx = 0;
        self.t = 0.0;
    }
    pub fn stop(&mut self, b: BfBeep) {
        if self.cur == Some(b) {
            self.cur = None;
        }
    }
    pub fn step(&mut self, dt: f32) {
        let Some(b) = self.cur else { return };
        let pat = b.pattern();
        self.t += dt * 1000.0;
        while self.t >= pat[self.idx] {
            self.t -= pat[self.idx];
            self.idx += 1;
            if self.idx >= pat.len() {
                self.cur = None;
                return;
            }
        }
    }
    /// Even steps are "on", odd steps "off".
    pub fn on(&self) -> bool {
        self.cur.is_some() && self.idx % 2 == 0
    }
    pub fn label(&self) -> &'static str {
        self.cur.map(|b| b.label()).unwrap_or("")
    }
}
