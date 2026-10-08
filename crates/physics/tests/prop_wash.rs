//! Readable checks for Prop Wash (#46, ADR-0005), beyond what the Physics
//! Scenarios show through the Simulation. Basis: Rule.
//!
//! The Quad is the drag-free whoop from `common`, given the Whoop 65's Prop
//! Wash numbers: 20% strength, flickering 15 times a second.

mod common;

use common::{STEP, WORLD, whoop};
use opendrone_maths::{Attitude, Fingerprinter, Vec3};
use opendrone_physics::{
    Flicker, MapCollision, MotorCommands, Mount, PropWash, QuadBody, QuadParameters, QuadStart,
    QuadState, SetUpProblem, StartingMotors,
};

const WHOOP_PROP_WASH: PropWash = PropWash {
    strength: 0.2,
    flicker: 15.0,
};

fn with_prop_wash(prop_wash: PropWash) -> QuadParameters {
    QuadParameters {
        prop_wash,
        ..whoop()
    }
}

/// Level and not turning, sinking straight down at `descent` m/s, its motors
/// settled.
fn sinking(parameters: QuadParameters, descent: f64) -> QuadBody {
    let start = QuadStart {
        state: QuadState {
            position: Vec3::new(0.0, 0.0, 100.0),
            velocity: Vec3::new(0.0, 0.0, -descent),
            attitude: Attitude::BODY_IS_WORLD,
            rotation: Vec3::ZERO,
        },
        motors: StartingMotors::Settled,
        battery: 1.0,
        mount: Mount::Free,
    };
    QuadBody::new(parameters, start, &WORLD).unwrap()
}

/// Steps the Quad on for `steps` steps at its settled drive.
fn run(quad: &mut QuadBody, steps: u32) {
    let empty_air = MapCollision::default();
    let commands = MotorCommands::all(quad.motors()[0].drive);
    for _ in 0..steps {
        quad.step(&WORLD, &empty_air, &commands, STEP);
    }
}

fn fingerprint(quad: &QuadBody) -> u64 {
    let mut f = Fingerprinter::new();
    quad.write_fingerprint(&mut f);
    f.finish().0
}

#[test]
fn settled_motors_in_the_band_make_up_prop_washs_average_loss() {
    // Drag-free and sinking at 5 m/s, each rotor carries a quarter of the
    // weight, so v_h is 5.6975 m/s and 5 m/s is 0.878 v_h, near the band's
    // middle. Settled with the flicker at its middle, the four thrusts still
    // carry the weight exactly; the rotors spin faster to give it.
    let weight = 0.0312 * 9.81;
    let washed = sinking(with_prop_wash(WHOOP_PROP_WASH), 5.0);
    let clean = sinking(with_prop_wash(PropWash::NONE), 5.0);
    let thrust: f64 = washed.motors().iter().map(|m| m.thrust).sum();
    assert!((thrust - weight).abs() < 1e-12, "{thrust} N");
    assert!(washed.motors()[0].speed > 1.1 * clean.motors()[0].speed);
}

#[test]
fn a_copied_quad_flickers_on_exactly_as_the_original() {
    // The generator, the levels and their timing are part of the Quad's
    // state, so a copy made mid-flicker goes on to the last bit as the
    // original does.
    let mut quad = sinking(with_prop_wash(WHOOP_PROP_WASH), 5.0).with_flicker(Flicker::new(3));
    run(&mut quad, 800);
    let mut copy = quad.clone();
    assert_eq!(fingerprint(&copy), fingerprint(&quad));
    run(&mut quad, 800);
    run(&mut copy, 800);
    assert_eq!(copy.state(), quad.state());
    assert_eq!(copy.motors(), quad.motors());
    assert_eq!(fingerprint(&copy), fingerprint(&quad));
}

#[test]
fn the_seed_decides_the_flicker_and_the_fingerprint_covers_it() {
    let flown = |seed| {
        let mut quad =
            sinking(with_prop_wash(WHOOP_PROP_WASH), 5.0).with_flicker(Flicker::new(seed));
        run(&mut quad, 800);
        quad
    };
    assert_eq!(flown(1).state(), flown(1).state());
    assert_ne!(flown(1).state(), flown(2).state());
    // Before any step the two Quads are the same but for their flicker, which
    // the fingerprint sees.
    let one = sinking(with_prop_wash(WHOOP_PROP_WASH), 5.0).with_flicker(Flicker::new(1));
    let two = sinking(with_prop_wash(WHOOP_PROP_WASH), 5.0).with_flicker(Flicker::new(2));
    assert_eq!(one.state(), two.state());
    assert_ne!(fingerprint(&one), fingerprint(&two));
}

#[test]
fn with_its_strength_at_0_a_quad_sinking_through_the_band_feels_nothing_of_it() {
    // The same flight with the flicker drawn from two seeds is the same to
    // the last bit, and dead level.
    let flown = |seed| {
        let mut quad =
            sinking(with_prop_wash(PropWash::NONE), 5.0).with_flicker(Flicker::new(seed));
        run(&mut quad, 800);
        *quad.state()
    };
    assert_eq!(flown(1), flown(2));
    assert_eq!(flown(1).rotation, Vec3::ZERO);
}

#[test]
fn a_strength_outside_0_to_100_percent_or_a_flicker_below_0_hz_is_refused() {
    for prop_wash in [
        PropWash {
            strength: 1.5,
            flicker: 15.0,
        },
        PropWash {
            strength: -0.1,
            flicker: 15.0,
        },
        PropWash {
            strength: f64::NAN,
            flicker: 15.0,
        },
        PropWash {
            strength: 0.2,
            flicker: -1.0,
        },
        PropWash {
            strength: 0.2,
            flicker: f64::INFINITY,
        },
    ] {
        let start = QuadStart {
            state: common::still_at(Attitude::BODY_IS_WORLD, Vec3::ZERO),
            motors: StartingMotors::Stopped,
            battery: 1.0,
            mount: Mount::Free,
        };
        assert_eq!(
            QuadBody::new(with_prop_wash(prop_wash), start, &WORLD).err(),
            Some(SetUpProblem::PropWashOutOfRange),
            "{prop_wash:?}"
        );
    }
}
