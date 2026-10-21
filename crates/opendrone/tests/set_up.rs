//! Readable checks for how the tracer sets a flight up: the Quad on the flat
//! floor's origin, landed, and the pilot's Channels as a receiver hands them
//! over. Lengths in metres, speeds in m/s, Channels in µs and CRSF steps.

mod common;

use opendrone::pilot::flight_channels;
use opendrone::set_up::{FlightPlan, landed_height};
use opendrone::settings::Settings;
use opendrone::stepping::Flight;
use opendrone_input::profile::FlightMode;
use opendrone_maths::{DEGREE, Vec3};
use opendrone_sim::{Channel, EscState};

#[test]
fn the_whoop_65_rests_on_its_body_box_with_its_centre_10_mm_above_the_floor() {
    // Basis: Rule. Its body box is 20 mm tall round its centre of mass, and
    // its pack, prop discs and duct rings all sit higher (quad.toml), as
    // scenarios/physics/landed-whoop-stays-still.toml starts it.
    let plan = common::plan("whoop-65");
    assert!((landed_height(&plan.quad.parameters.shape) - 0.010).abs() < 1e-12);
}

#[test]
fn the_freestyle_5_rests_on_its_body_box_with_its_centre_17_5_mm_above_the_floor() {
    // Basis: Rule. Its body box is 35 mm tall, as
    // scenarios/flight-controller/arming-waits-for-the-escs-after-reset.toml
    // starts it.
    let plan = common::plan("freestyle-5");
    assert!((landed_height(&plan.quad.parameters.shape) - 0.0175).abs() < 1e-12);
}

#[test]
fn the_tracer_flies_the_flat_floor_with_the_quad_landed_at_its_origin() {
    for quad in ["whoop-65", "freestyle-5"] {
        let plan = common::plan(quad);
        assert_eq!(plan.map.id, "test/flat-floor");
        let spot = plan.launch_spot();
        assert_eq!((spot.position.x, spot.position.y), (0.0, 0.0), "{quad}");
    }
}

#[test]
fn on_the_skate_park_the_quad_starts_landed_on_its_launch_spot_facing_its_heading() {
    // Basis: Rule (#63): a Quad starts, and Reset puts it, on the Map's
    // Launch Spot, landed on the ground there, facing the spot's heading.
    for quad in ["whoop-65", "freestyle-5"] {
        let plan = FlightPlan::from_packs(
            &common::packs(),
            Settings::tracer(quad).on_map("opendrone/skate-park"),
        )
        .expect("the Skate Park is in the built-in Pack");
        let spot = plan
            .map
            .launch_spot
            .expect("the Skate Park has a Launch Spot");
        let start = plan.launch_spot();
        let landed = landed_height(&plan.quad.parameters.shape);
        assert_eq!(
            start.position,
            spot.position + Vec3::new(0.0, 0.0, landed),
            "{quad}"
        );
        let heading = start.attitude.pilot_angles().heading / DEGREE;
        assert!((heading - 270.0).abs() < 1e-9, "{quad} faces {heading}°");
        assert_eq!(start.velocity, Vec3::ZERO);
    }
}

#[test]
fn a_quad_set_up_on_the_skate_park_stays_landed_on_the_launch_spot() {
    // Basis: Rule. The Map's colliders reach the Simulation: landed with a
    // fresh Flight Controller, nothing moves the Quad (as the Scenarios
    // landed-*-stays-still-on-the-skate-parks-launch-spot.toml prove headless).
    let plan = FlightPlan::from_packs(
        &common::packs(),
        Settings::tracer("whoop-65").on_map("opendrone/skate-park"),
    )
    .unwrap();
    let mut flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
    let start = plan.launch_spot();
    for _ in 0..1000 {
        flight.tick();
    }
    let output = flight.simulation().quad_output(0);
    assert!((output.state.position - start.position).length() < 1e-6);
}

#[test]
fn a_quad_set_up_by_the_game_stays_landed_while_its_escs_start_up_then_they_are_ready() {
    // Basis: Rule. Landed with a fresh Flight Controller is exactly Reset:
    // the ESCs play their start-up and are ready about 1.7 s later (#32 §4),
    // and nothing moves the Quad meanwhile.
    for quad in ["whoop-65", "freestyle-5"] {
        let plan = common::plan(quad);
        let mut flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
        let start = plan.launch_spot();
        assert!(matches!(
            flight.simulation().quad_output(0).motors[0].esc,
            EscState::StartingUp(_)
        ));
        for _ in 0..2000 {
            flight.tick();
        }
        let output = flight.simulation().quad_output(0);
        assert!(
            (output.state.position.z - start.position.z).abs() < 1e-6,
            "{quad}: {} m up",
            output.state.position.z
        );
        assert!(output.state.velocity.length() < 1e-6, "{quad}");
        assert_eq!(output.motors[0].esc, EscState::Ready, "{quad}");
    }
}

#[test]
fn input_device_channels_become_the_whole_number_steps_a_receiver_hands_over() {
    // Basis: Rule (docs/context/input.md): 988, 1500 and 2012 µs are ELRS's
    // steps 172, 992 and 1811.
    let mut channels = opendrone_input::Channels::at_rest();
    channels.roll = 2012.0;
    channels.pitch = 988.0;
    channels.arm = Some(2012.0);
    let fc = flight_channels(&channels, FlightMode::Acro);
    assert_eq!(fc.roll, Channel::HIGH);
    assert_eq!(fc.pitch, Channel::LOW);
    assert_eq!(fc.yaw, Channel::CENTRE);
    assert_eq!(fc.throttle, Channel::LOW);
    assert_eq!(fc.arm, Channel::HIGH);
}

#[test]
fn a_device_with_no_flight_mode_switch_flies_the_flight_mode_setting() {
    // Basis: Rule (ADR-0017): AUX2 low is Acro, middle Horizon, high Angle.
    let channels = opendrone_input::Channels::at_rest();
    assert_eq!(
        flight_channels(&channels, FlightMode::Acro).flight_mode,
        Channel::LOW
    );
    assert_eq!(
        flight_channels(&channels, FlightMode::Horizon).flight_mode,
        Channel::CENTRE
    );
    assert_eq!(
        flight_channels(&channels, FlightMode::Angle).flight_mode,
        Channel::HIGH
    );
}

#[test]
fn a_switch_with_no_source_is_low() {
    // Basis: Rule: with no source, Arm and Crash Flip stay off.
    let fc = flight_channels(&opendrone_input::Channels::at_rest(), FlightMode::Acro);
    assert_eq!(fc.arm, Channel::LOW);
    assert_eq!(fc.crash_flip, Channel::LOW);
}
