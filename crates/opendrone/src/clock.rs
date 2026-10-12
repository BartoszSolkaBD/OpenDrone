//! Turning the computer's clock into Simulation Time. Only the game does
//! this (ADR-0003): the input thread stamps what it reads with the computer's
//! clock, and each Flight Input enters the Simulation stamped with Simulation
//! Time.
//!
//! The Simulation follows the game's own clock, which stops while the game
//! is paused and runs slow when the computer can't keep up (Bevy's virtual
//! time). So the mapping counts every stretch of the computer's clock that
//! the game's clock didn't follow, and takes it off: a report read during a
//! pause, or while the game fell behind, enters at the moment flying goes on.

use std::time::Duration;

use opendrone_sim::{PhysicsRate, SimulationTime};

/// The mapping from the input thread's clock to Simulation Time.
#[derive(Clone, Copy, Debug)]
pub struct SimulationClock {
    rate: PhysicsRate,
    /// How far the input thread's clock is ahead of the game's clock, in
    /// nanoseconds: the time between the two clocks' starts, plus every
    /// pause and every stretch the game fell behind. It only grows. `None`
    /// before the first frame.
    ahead: Option<i128>,
    /// The game clock's time at Simulation Time 0: when the flight started.
    start: Duration,
}

impl SimulationClock {
    pub fn new(rate: PhysicsRate) -> SimulationClock {
        SimulationClock {
            rate,
            ahead: None,
            start: Duration::ZERO,
        }
    }

    /// Follows both clocks once a frame, read at the same moment: the input
    /// thread's clock and the game's clock. While the game's clock stands
    /// still (a pause) or runs slow (an overload), the gap grows.
    pub fn follow(&mut self, input_clock: Duration, game_clock: Duration) {
        let gap = nanos(input_clock) - nanos(game_clock);
        self.ahead = Some(self.ahead.map_or(gap, |ahead| ahead.max(gap)));
    }

    /// The flight starts: Simulation Time 0 is the game clock's `game_clock`.
    pub fn start_flight(&mut self, game_clock: Duration) {
        self.start = game_clock;
    }

    /// The Simulation Time of a moment on the input thread's clock: the first
    /// physics step at or after it. A moment before the flight started maps
    /// to its start.
    pub fn simulation_time(&self, input_clock: Duration) -> SimulationTime {
        let game = nanos(input_clock) - self.ahead.unwrap_or(0) - nanos(self.start);
        let Ok(game) = u128::try_from(game) else {
            return SimulationTime::START;
        };
        let ticks = (game * u128::from(self.rate.hz())).div_ceil(1_000_000_000);
        SimulationTime::from_ticks(u64::try_from(ticks).unwrap_or(u64::MAX))
    }
}

fn nanos(duration: Duration) -> i128 {
    i128::try_from(duration.as_nanos()).unwrap_or(i128::MAX)
}
