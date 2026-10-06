//! PROTOTYPE (#34). Scripted flights, so a sound can be heard again on exactly the same flight:
//! timelines of stick moves and events fed to the rough simulation.

use crate::paths::PathKind;
use crate::sim::{Cmd, Sim, Throttle};

#[derive(Clone, Debug)]
pub enum Act {
    Cmd(Cmd),
    Arm(bool),
    Throttle(Throttle),
    Stick(f32),
    Sticks { roll: f32, pitch: f32, yaw: f32 },
    Angle(bool),
    CrashFlip(bool),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScriptKind {
    PowerUp,
    Hover,
    PunchOut,
    Flip,
    PropWash,
    PropStrike,
    Jam,
    Crash,
    Failsafe,
    LowBattery,
    Beacon,
    PauseMenu,
    Path(PathKind),
}

pub const SCRIPTS: [ScriptKind; 12] = [
    ScriptKind::PowerUp,
    ScriptKind::Hover,
    ScriptKind::PunchOut,
    ScriptKind::Flip,
    ScriptKind::PropWash,
    ScriptKind::PropStrike,
    ScriptKind::Jam,
    ScriptKind::Crash,
    ScriptKind::Failsafe,
    ScriptKind::LowBattery,
    ScriptKind::Beacon,
    ScriptKind::PauseMenu,
];

impl ScriptKind {
    pub fn label(self) -> &'static str {
        match self {
            ScriptKind::PowerUp => "Reset: power-up, arm, idle, disarm",
            ScriptKind::Hover => "Take off and hover",
            ScriptKind::PunchOut => "Punch-out from a hover",
            ScriptKind::Flip => "Flip (acro roll) and catch",
            ScriptKind::PropWash => "Drop into your own prop wash",
            ScriptKind::PropStrike => "Prop Strikes (light, then hard)",
            ScriptKind::Jam => "Jammed prop: restarts fail, motor gives up",
            ScriptKind::Crash => "Crash into the ground (hit clip)",
            ScriptKind::Failsafe => "Failsafe: link lost, drop, link back",
            ScriptKind::LowBattery => "Sagging pack: LOW BATTERY, LAND NOW",
            ScriptKind::Beacon => "Sit idle 10 minutes: Bluejay's beacon",
            ScriptKind::PauseMenu => "Pause Menu mid-hover, then Resume",
            ScriptKind::Path(p) => p.label(),
        }
    }

    /// (time in seconds, action). Most start mid-air or from a fresh power-up as noted.
    pub fn steps(self) -> Vec<(f32, Act)> {
        use Act::*;
        let hover = Throttle(crate::sim::Throttle::Alt(1.5));
        match self {
            ScriptKind::PowerUp => vec![
                (0.0, Cmd(crate::sim::Cmd::PowerUp)),
                (0.0, Throttle(crate::sim::Throttle::Stick)),
                (0.0, Stick(0.0)),
                (2.6, Arm(true)),
                (5.0, Arm(false)),
                (6.5, Stick(0.0)),
            ],
            ScriptKind::Hover => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(0.0))),
                (0.0, hover.clone()),
                (0.0, Angle(true)),
                (3.0, Sticks { roll: 0.15, pitch: 0.0, yaw: 0.0 }),
                (3.6, Sticks { roll: -0.15, pitch: 0.1, yaw: 0.0 }),
                (4.2, Sticks { roll: 0.0, pitch: 0.0, yaw: 0.3 }),
                (5.0, Sticks { roll: 0.0, pitch: 0.0, yaw: 0.0 }),
                (8.0, Throttle(crate::sim::Throttle::Alt(-0.5))),
                (10.0, Arm(false)),
            ],
            ScriptKind::PunchOut => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(1.5))),
                (0.0, hover.clone()),
                (0.0, Angle(true)),
                (2.0, Throttle(crate::sim::Throttle::Stick)),
                (2.0, Stick(1.0)),
                (3.6, Stick(0.0)),
                (5.0, Stick(0.45)),
                (5.6, Throttle(crate::sim::Throttle::Hold)),
                (9.0, Arm(false)),
            ],
            ScriptKind::Flip => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(4.0))),
                (0.0, Throttle(crate::sim::Throttle::Alt(4.0))),
                (0.0, Angle(true)),
                (1.5, Throttle(crate::sim::Throttle::Stick)),
                (1.5, Stick(0.9)),
                (1.8, Angle(false)),
                (1.8, Stick(0.05)),
                (1.8, Sticks { roll: 1.0, pitch: 0.0, yaw: 0.0 }),
                (2.32, Sticks { roll: 0.0, pitch: 0.0, yaw: 0.0 }),
                (2.32, Angle(true)),
                (2.4, Stick(0.85)),
                (2.7, Throttle(crate::sim::Throttle::Hold)),
                (5.0, Arm(false)),
            ],
            ScriptKind::PropWash => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(10.0))),
                (0.0, Throttle(crate::sim::Throttle::Alt(10.0))),
                (0.0, Angle(true)),
                (1.5, Throttle(crate::sim::Throttle::Stick)),
                (1.5, Stick(0.0)),
                (2.6, Stick(0.75)),
                (3.4, Throttle(crate::sim::Throttle::Hold)),
                (6.0, Arm(false)),
            ],
            ScriptKind::PropStrike => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(1.5))),
                (0.0, hover.clone()),
                (0.0, Angle(true)),
                (1.5, Cmd(crate::sim::Cmd::Strike { motor: 1, rub: 0.25, secs: 0.12 })),
                (3.5, Cmd(crate::sim::Cmd::Strike { motor: 3, rub: 0.8, secs: 0.35 })),
                (6.0, Arm(false)),
            ],
            ScriptKind::Jam => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(0.0))),
                (0.0, Throttle(crate::sim::Throttle::Stick)),
                (0.0, Stick(0.0)),
                (0.0, Angle(true)),
                (0.5, Cmd(crate::sim::Cmd::Jam { motor: 2 })),
                (0.6, Stick(0.2)),
                (6.0, Stick(0.0)),
                (6.5, Arm(false)),
            ],
            ScriptKind::Crash => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(3.0))),
                (0.0, Throttle(crate::sim::Throttle::Alt(3.0))),
                (0.0, Angle(true)),
                (1.0, Throttle(crate::sim::Throttle::Stick)),
                (1.0, Stick(0.05)),
                (1.0, Sticks { roll: 0.0, pitch: 0.6, yaw: 0.0 }),
                (2.4, Sticks { roll: 0.0, pitch: 0.0, yaw: 0.0 }),
                (5.0, Arm(false)),
            ],
            ScriptKind::Failsafe => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(3.0))),
                (0.0, Throttle(crate::sim::Throttle::Alt(3.0))),
                (0.0, Angle(true)),
                (0.0, Arm(true)),
                (1.0, Cmd(crate::sim::Cmd::Link(false))),
                (6.0, Cmd(crate::sim::Cmd::Link(true))),
                (6.2, Arm(false)),
            ],
            ScriptKind::LowBattery => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(2.0))),
                (0.0, Throttle(crate::sim::Throttle::Alt(2.0))),
                (0.0, Angle(true)),
                (0.0, Cmd(crate::sim::Cmd::Charge(0.35))),
                (1.0, Cmd(crate::sim::Cmd::Charge(0.12))),
                (4.0, Cmd(crate::sim::Cmd::Charge(0.03))),
                (6.5, Throttle(crate::sim::Throttle::Alt(-0.5))),
                (8.0, Arm(false)),
                (8.5, Cmd(crate::sim::Cmd::Charge(0.8))),
            ],
            ScriptKind::Beacon => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(0.0))),
                (0.0, Arm(false)),
                (0.0, Throttle(crate::sim::Throttle::Stick)),
                (0.0, Stick(0.0)),
                (0.5, Cmd(crate::sim::Cmd::IdleTenMinutes)),
            ],
            ScriptKind::PauseMenu => vec![
                (0.0, Cmd(crate::sim::Cmd::SettleInAir(1.5))),
                (0.0, hover.clone()),
                (0.0, Angle(true)),
                (2.5, Cmd(crate::sim::Cmd::Pause(true))),
                (5.0, Cmd(crate::sim::Cmd::Pause(false))),
                (7.0, Arm(true)),
            ],
            ScriptKind::Path(p) => vec![(0.0, Angle(true)), (0.0, Cmd(crate::sim::Cmd::Path(Some(p))))],
        }
    }
}

/// A script being played on a Sim.
pub struct ScriptRun {
    pub kind: ScriptKind,
    steps: Vec<(f32, Act)>,
    next: usize,
    pub t: f32,
}

impl ScriptRun {
    pub fn new(kind: ScriptKind) -> Self {
        ScriptRun { kind, steps: kind.steps(), next: 0, t: 0.0 }
    }

    pub fn length(&self) -> f32 {
        self.steps.last().map(|s| s.0).unwrap_or(0.0)
    }

    /// Apply every step due by now, then advance the clock by dt. Returns false when done.
    pub fn step(&mut self, sim: &mut Sim, dt: f32) -> bool {
        while self.next < self.steps.len() && self.steps[self.next].0 <= self.t {
            let act = self.steps[self.next].1.clone();
            self.next += 1;
            match act {
                Act::Cmd(c) => sim.apply(c),
                Act::Arm(a) => {
                    sim.controls.arm = a;
                    if a {
                        sim.controls.throttle = sim.controls.throttle.min(0.0);
                    }
                }
                Act::Throttle(crate::sim::Throttle::Hold) => {
                    sim.throttle_mode = crate::sim::Throttle::Alt(sim.pos.z - sim.map.launch.z)
                }
                Act::Throttle(t) => sim.throttle_mode = t,
                Act::Stick(s) => sim.controls.throttle = s,
                Act::Sticks { roll, pitch, yaw } => {
                    sim.controls.roll = roll;
                    sim.controls.pitch = pitch;
                    sim.controls.yaw = yaw;
                }
                Act::Angle(a) => sim.controls.angle_mode = a,
                Act::CrashFlip(c) => sim.controls.crash_flip = c,
            }
        }
        self.t += dt;
        self.next < self.steps.len() || sim.path_kind().is_some()
    }
}
