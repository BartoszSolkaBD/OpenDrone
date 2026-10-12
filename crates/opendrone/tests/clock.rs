//! Readable checks for turning the computer's clock into Simulation Time:
//! each report enters at the first step at or after it, and the mapping
//! stands still while the game's clock does (Pause) or runs slow (overload).
//! Basis: Rule (ADR-0003: the game converts the clock; Pause stops stepping).
//! Times in seconds; Simulation Time in steps at 8 kHz.

use std::time::Duration;

use opendrone::clock::SimulationClock;
use opendrone_sim::{PhysicsRate, SimulationTime};

fn clock() -> SimulationClock {
    SimulationClock::new(PhysicsRate::from_hz(8000).unwrap())
}

fn s(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

fn steps(n: u64) -> SimulationTime {
    SimulationTime::from_ticks(n)
}

#[test]
fn a_report_enters_at_the_first_simulation_step_at_or_after_it() {
    let mut clock = clock();
    clock.follow(s(0.0), s(0.0));
    assert_eq!(clock.simulation_time(s(0.001)), steps(8));
    assert_eq!(
        clock.simulation_time(Duration::from_nanos(1_000_001)),
        steps(9)
    );
    assert_eq!(clock.simulation_time(s(0.0)), steps(0));
}

#[test]
fn the_two_clocks_may_start_at_different_moments() {
    let mut clock = clock();
    // The input thread started 0.25 s before the game's clock.
    clock.follow(s(0.25), s(0.0));
    clock.follow(s(1.25), s(1.0));
    assert_eq!(clock.simulation_time(s(1.25)), steps(8000));
}

#[test]
fn simulation_time_stands_still_while_paused() {
    let mut clock = clock();
    clock.follow(s(1.0), s(1.0));
    // Paused for 2 s: the game's clock stood at 1 s.
    clock.follow(s(2.0), s(1.0));
    clock.follow(s(3.0), s(1.0));
    // Flying on, half a second later.
    clock.follow(s(3.5), s(1.5));
    assert_eq!(clock.simulation_time(s(3.5)), steps(12_000));
}

#[test]
fn a_report_read_during_a_pause_enters_when_flying_goes_on() {
    let mut clock = clock();
    clock.follow(s(1.0), s(1.0));
    clock.follow(s(3.0), s(1.0));
    // Read 2 s into the pause, at the paused moment's Simulation Time; the
    // Simulation stepped up to 1 s, so it enters at the next step (the
    // flight takes nothing before that).
    assert_eq!(clock.simulation_time(s(3.0)), steps(8000));
}

#[test]
fn when_the_game_falls_behind_the_simulation_slows_down_and_reports_keep_their_order() {
    let mut clock = clock();
    clock.follow(s(1.0), s(1.0));
    // A frame took 0.5 s, but the game's clock took only 0.25 s of it (Bevy
    // lets the rest go).
    clock.follow(s(1.5), s(1.25));
    assert_eq!(clock.simulation_time(s(1.5)), steps(10_000));
    let earlier = clock.simulation_time(s(1.4));
    let later = clock.simulation_time(s(1.45));
    assert!(earlier < later);
}

#[test]
fn simulation_time_starts_when_the_flight_starts() {
    let mut clock = clock();
    clock.follow(s(2.0), s(2.0));
    clock.start_flight(s(2.0));
    assert_eq!(clock.simulation_time(s(2.25)), steps(2000));
    // A report from before the flight started enters at its start.
    assert_eq!(clock.simulation_time(s(1.0)), steps(0));
}
