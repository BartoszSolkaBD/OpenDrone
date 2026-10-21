//! Readable checks for stepping the Simulation in Bevy's fixed timestep, run
//! headless through the game's own stepping plugin. Basis: Rule (#37, "The
//! game": 1 kHz ticks of 8 steps in one system, catch-up stepping that never
//! skips or stretches a step, slowing down on overload, Pause stops
//! stepping, and the picture drawn between the last two states).

mod common;

use std::time::Duration;

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use opendrone::stepping::{Flight, SteppingPlugin};
use opendrone_sim::{Channel, Channels, FlightInput, SimulationTime};

/// The game's stepping, headless, flying the Whoop 65 from its Launch Spot.
fn app() -> App {
    let plan = common::plan("whoop-65");
    let flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin, SteppingPlugin))
        .insert_non_send(flight)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    // Bevy's first frame starts its clocks: no time passes in it.
    app.update();
    app
}

/// Runs one frame that lasts `ms` milliseconds.
fn frame(app: &mut App, ms: u64) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        ms,
    )));
    app.update();
}

fn steps(app: &App) -> u64 {
    app.world().non_send::<Flight>().simulation().time().ticks()
}

#[test]
fn each_1_khz_tick_runs_8_simulation_steps() {
    let mut app = app();
    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app.update();
    assert_eq!(steps(&app), 8);
    app.update();
    assert_eq!(steps(&app), 16);
}

#[test]
fn catch_up_stepping_runs_every_step_the_time_calls_for_whatever_the_frame_lengths() {
    let mut app = app();
    let lengths = [3, 17, 8, 41, 1, 0, 16, 2];
    let mut total_ms = 0;
    for ms in lengths {
        frame(&mut app, ms);
        total_ms += ms;
        // Every whole millisecond is a tick of 8 steps, none skipped; the
        // part of a millisecond left over waits for the next frame.
        assert_eq!(steps(&app), 8 * total_ms, "after {total_ms} ms");
    }
}

#[test]
fn a_step_is_never_stretched_each_lasts_one_8000th_of_a_second() {
    let mut app = app();
    frame(&mut app, 41);
    let sim = app.world().non_send::<Flight>().simulation();
    assert_eq!(sim.physics_rate().hz(), 8000);
    assert_eq!(sim.time().seconds(sim.physics_rate()), 0.041);
}

#[test]
fn on_overload_the_simulation_slows_down_instead_of_skipping_steps() {
    let mut app = app();
    // One frame took 400 ms: Bevy lets anything over 250 ms go.
    frame(&mut app, 400);
    assert_eq!(steps(&app), 8 * 250);
    // Back to normal: it goes on from there, step by step.
    frame(&mut app, 10);
    assert_eq!(steps(&app), 8 * 260);
}

#[test]
fn pause_stops_stepping_and_flying_on_goes_on_from_the_same_step() {
    let mut app = app();
    frame(&mut app, 10);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    frame(&mut app, 500);
    frame(&mut app, 500);
    assert_eq!(steps(&app), 80);
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    frame(&mut app, 10);
    assert_eq!(steps(&app), 160);
}

#[test]
fn the_picture_is_drawn_between_the_last_two_ticks_states() {
    let mut app = app();
    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app.update();
    let first = *app.world().non_send::<Flight>().simulation().quad_state(0);
    app.update();
    let flight = app.world().non_send::<Flight>();
    let (before, after) = flight.drawn_between();
    assert_eq!(*before, first);
    assert_eq!(after, flight.simulation().quad_state(0));
}

#[test]
fn a_flight_input_stamped_with_a_moment_already_stepped_still_reaches_the_flight_controller() {
    let mut app = app();
    frame(&mut app, 2);
    let mut sticks = Channels::RESTING;
    sticks.roll = Channel::HIGH;
    app.world_mut()
        .non_send_mut::<Flight>()
        .input(SimulationTime::from_ticks(3), FlightInput::Channels(sticks));
    // Steps 0 to 15 are done; the Channels enter at step 16, and the Radio
    // Link's next 250 Hz frame, at step 32 (4 ms), carries them.
    frame(&mut app, 3);
    let flight = app.world().non_send::<Flight>();
    assert_eq!(flight.simulation().time().ticks(), 40);
    assert_eq!(
        flight.simulation().quad_output(0).radio_link.last_frame,
        Some(sticks)
    );
}

/// A flight on the Whoop 65 with Auto-arm on, climbing and rolling for 4 s
/// (4,000 ticks), so the Quad is well away from its Launch Spot.
fn flight_away_from_the_launch_spot() -> (opendrone::set_up::FlightPlan, Flight) {
    let mut settings = opendrone::settings::Settings::tracer("whoop-65");
    settings.auto_arm = true;
    let plan = opendrone::set_up::FlightPlan::from_packs(&common::packs(), settings).unwrap();
    let device = opendrone_sim::InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    let mut flight = Flight::new(plan.simulation(Some(device)).unwrap()).unwrap();
    let mut sticks = Channels::RESTING;
    flight.input(SimulationTime::START, FlightInput::Channels(sticks));
    for tick in 1..=4000_u64 {
        if tick == 2200 {
            // The first raise after the ESCs' ready beep, which Auto-arm arms on.
            sticks.throttle = Channel::from_micros(1500.0);
            flight.input(
                SimulationTime::from_ticks(tick * 8),
                FlightInput::Channels(sticks),
            );
        }
        if tick == 3000 {
            sticks.roll = Channel::from_micros(1731.4);
            flight.input(
                SimulationTime::from_ticks(tick * 8),
                FlightInput::Channels(sticks),
            );
        }
        flight.tick();
    }
    let away = flight.simulation().quad_state(0).position - plan.launch_spot().position;
    assert!(
        away.length() > 0.05,
        "the Quad is {} m from the spot",
        away.length()
    );
    (plan, flight)
}

/// Runs `ticks` ticks, and says how far the picture moves the Quad across
/// each tick, in metres, the most of them.
fn the_most_the_picture_moves_the_quad_in_a_tick(flight: &mut Flight, ticks: u32) -> f64 {
    let mut most: f64 = 0.0;
    for _ in 0..ticks {
        flight.tick();
        let (before, after) = flight.drawn_between();
        most = most.max((after.position - before.position).length());
    }
    most
}

#[test]
fn the_first_picture_after_a_reset_shows_the_quad_on_the_launch_spot_with_no_streak_from_the_old_place()
 {
    // Basis: Rule (#184: after a Reset the picture shows the Quad at the
    // Launch Spot with no streak between frames). The Reset reaches the
    // Simulation in the next tick.
    let (plan, mut flight) = flight_away_from_the_launch_spot();
    let now = flight.simulation().time();
    flight.input(now, FlightInput::Reset);
    flight.tick();
    assert!(flight.jumped_in_last_tick());
    let (before, after) = flight.drawn_between();
    assert_eq!(before.position, after.position);
    assert_eq!(before.attitude, after.attitude);
    let off = after.position - plan.launch_spot().position;
    assert!(
        off.length() < 0.01,
        "{} m (a 0.01 m bound) off the spot",
        off.length()
    );
}

#[test]
fn a_reset_stamped_ahead_of_the_simulation_is_drawn_with_no_streak_when_it_arrives() {
    // Basis: Rule (#184). The input thread stamps a key press with the
    // computer's clock, which can be a frame (about 16 ms) ahead of the
    // Simulation; the Reset then arrives in a later tick, and that tick is
    // the one the picture must not draw motion across. The Quad moves less
    // than 0.1 m (a bound: 1 ms at under 100 m/s) across any tick of its
    // own.
    let (plan, mut flight) = flight_away_from_the_launch_spot();
    let ahead = SimulationTime::from_ticks(flight.simulation().time().ticks() + 16 * 8);
    flight.input(ahead, FlightInput::Reset);
    let most = the_most_the_picture_moves_the_quad_in_a_tick(&mut flight, 40);
    assert!(
        most < 0.1,
        "the picture moved the Quad {most} m across one tick"
    );
    let off = flight.simulation().quad_state(0).position - plan.launch_spot().position;
    assert!(
        off.length() < 0.01,
        "the Reset did happen: {} m off the spot",
        off.length()
    );
}
