//! Readable checks for what the game hands the Quad's sound: each Quad's
//! sound block and motor facts from its Quad definition, and the Quad's
//! state each frame. Speeds in rad/s and RPM, currents in amps.

mod common;

use std::f64::consts::TAU;

use opendrone::sound::{quad_frame, quad_sound, top_speed_and_current};
use opendrone::stepping::Flight;

fn rpm(rad_per_s: f64) -> f64 {
    rad_per_s * 60.0 / TAU
}

#[test]
fn the_whoop_65s_sound_block_reaches_the_sound_as_set_by_ear() {
    // Basis: Rule. The values are the built-in Pack's (quad.toml), from
    // #34's round 2, in SI: a 1.5% wobble is 0.015, 900 Hz is 900.
    let plan = common::plan("whoop-65");
    let quad = quad_sound(&plan.quad, plan.map.world.air_density).expect("its sound block reads");
    assert_eq!((quad.blades, quad.poles), (3, 12));
    assert!((quad.prop_diameter - 0.035).abs() < 1e-6);
    assert_eq!(quad.block.harmonics, 8);
    assert!((quad.block.speed_wobble - 0.015).abs() < 1e-6);
    assert!((quad.block.hum_first_mode - 900.0).abs() < 1e-3);
    assert!((quad.block.body_peak_gain - 1.5).abs() < 1e-6);
}

#[test]
fn the_freestyle_5s_sound_block_reaches_the_sound_as_set_by_ear() {
    // Basis: Rule, as above.
    let plan = common::plan("freestyle-5");
    let quad = quad_sound(&plan.quad, plan.map.world.air_density).expect("its sound block reads");
    assert_eq!((quad.blades, quad.poles), (3, 14));
    assert_eq!(quad.block.harmonics, 12);
    assert!((quad.block.speed_wobble - 0.012).abs() < 1e-6);
    assert!((quad.block.hum_first_mode - 180.0).abs() < 1e-3);
}

#[test]
fn the_whoop_65s_motors_top_out_near_45000_rpm_and_3_4_a_on_a_full_pack() {
    // Basis: Source. docs/research/quad-sound.md §1: full thrust at about
    // 45,000 RPM, and BetaFPV's 30.6 g at 3.4 A. The full pack, 4.35 V,
    // gives a little more than the 4 V there.
    let plan = common::plan("whoop-65");
    let (speed, current) = top_speed_and_current(&plan.quad, plan.map.world.air_density);
    assert!(
        (42_000.0..52_000.0).contains(&rpm(speed)),
        "{:.0} RPM",
        rpm(speed)
    );
    assert!((3.0..4.5).contains(&current), "{current:.2} A");
}

#[test]
fn the_freestyle_5s_motors_top_out_near_29600_rpm_and_35_a_on_a_full_pack() {
    // Basis: Source. T-Motor's T5147 table: 29,447 RPM at 34.6 A on 23.5 V
    // (quad.toml's sources). The full pack, 25.2 V, gives a little more.
    let plan = common::plan("freestyle-5");
    let (speed, current) = top_speed_and_current(&plan.quad, plan.map.world.air_density);
    assert!(
        (28_000.0..34_000.0).contains(&rpm(speed)),
        "{:.0} RPM",
        rpm(speed)
    );
    assert!((30.0..45.0).contains(&current), "{current:.2} A");
}

#[test]
fn a_quad_landed_with_its_motors_stopped_hands_the_sound_stopped_motors_and_no_airspeed() {
    // Basis: Rule. Nothing is heard that the Simulation didn't do
    // (docs/context/sound.md).
    let plan = common::plan("whoop-65");
    let flight = Flight::new(plan.simulation(None).expect("it sets up")).expect("it steps");
    let frame = quad_frame(&flight, false);
    assert!(
        frame
            .motors
            .iter()
            .all(|m| m.speed == 0.0 && m.current == 0.0)
    );
    assert_eq!(frame.airspeed, 0.0);
    assert!(!frame.paused);
    assert!(quad_frame(&flight, true).paused);
}
