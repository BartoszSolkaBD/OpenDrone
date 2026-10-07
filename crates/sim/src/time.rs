/// How many physics steps make one second of Simulation Time. Physics and the
/// Flight Controller each step once per tick. It is part of every set-up and
/// every Scenario's starting state, never a constant in the code; the alpha
/// runs at 8 kHz.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicsRate {
    hz: u32,
}

impl PhysicsRate {
    /// A rate of `hz` steps a second, or `None` for zero.
    pub fn from_hz(hz: u32) -> Option<PhysicsRate> {
        (hz > 0).then_some(PhysicsRate { hz })
    }

    pub fn hz(self) -> u32 {
        self.hz
    }

    /// The length of one step, in seconds.
    pub fn step_length(self) -> f64 {
        1.0 / f64::from(self.hz)
    }
}

/// Time as the Simulation counts it: whole steps since the start, never the
/// computer's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimulationTime {
    ticks: u64,
}

impl SimulationTime {
    /// The start, before the first step.
    pub const START: SimulationTime = SimulationTime { ticks: 0 };

    pub const fn from_ticks(ticks: u64) -> SimulationTime {
        SimulationTime { ticks }
    }

    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    /// One step later.
    pub const fn next(self) -> SimulationTime {
        SimulationTime {
            ticks: self.ticks + 1,
        }
    }

    /// The same moment in seconds, at this physics rate.
    pub fn seconds(self, rate: PhysicsRate) -> f64 {
        self.ticks as f64 / f64::from(rate.hz())
    }
}
