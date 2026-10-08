//! The air: what moving through it does to the rotors' thrust, and the drag
//! on the rotors, the ducts and the body (E10, E11, E16–E18, E25 and E26 in
//! #10; flight-dynamics research §4 and §5).
//!
//! Each effect is always on. How strong it is comes from the Quad definition:
//! a Test Quad with an effect's numbers set to zero is how a Scenario looks at
//! the others alone. Everything here reads only the Quad's motion through
//! still air; nothing reads the Map but its air density and gravity.
//!
//! # Inflow: thrust in a climb, a descent and at speed (E10, E11)
//!
//! A rotor's thrust on the thrust stand, in still air, is `T₀ = k_f·ω²`
//! ([`crate::motor`]). In flight the air moves through it, and momentum theory
//! says how its thrust changes at the same rotor speed. Hoffmann et al.
//! (research §4.3, its source S4, eqs. 12, 13 and 14) hold the power the
//! rotor turns into moving air fixed, as the motor does at a fixed speed
//! here (the prop's torque doesn't change with the air):
//!
//! - in still air the rotor pushes the air down through its disc at
//!   `v_h = √(T₀ / 2ρA)`, with `A` the disc's area, and the power that takes
//!   is `T₀·v_h`;
//! - moving, with the air arriving along its axis at `v_c` (climbing is
//!   positive) and across it at `μ`, the air goes through the disc at
//!   `u = v_c + v_i`, where the rotor adds `v_i = T / (2ρA·√(μ² + u²))`
//!   (Glauert's induced velocity, Hoffmann's eq. 12);
//! - the same power gives `T·u = T₀·v_h`.
//!
//! So in units of `v_h`, with `x = v_c / v_h` and `m = μ / v_h`, the flow
//! through the disc `ū = u / v_h` solves `(ū − x)·ū·√(m² + ū²) = 1`, and the
//! thrust is `T₀ / ū`. In still air `ū` is 1. Climbing, more air goes through
//! the disc and the thrust falls (at a climb of `v_h`, to 68%); descending,
//! less does and it rises (E10). Flying across the disc, the rotor meets new
//! air all the time and the thrust rises too: translational lift (E11).
//!
//! The equation has exactly one root above both `x` and 0, and Newton's
//! method from `max(x, 0) + 1` comes down to it without overshooting, since
//! the left side only grows, ever faster, from there. It's worked out a step
//! at a time until it no longer comes down.
//!
//! Two limits, each where momentum theory stops describing a real rotor:
//!
//! - **A guard.** Descending and flying across the disc at once, both far
//!   faster than `v_h` (a rotor at idle in a fast, tumbling dive), the
//!   equation lets the thrust grow with both speeds multiplied, as the flow
//!   through the disc nears nothing, where a real rotor's blades stall. So the
//!   thrust is never more than it would be in the same descent with no air
//!   across the disc (which grows only with the root of the descent), plus
//!   the most momentum theory lets any disc take from the air arriving at it,
//!   `½ρA·(v_c² + μ²)` (an ideal windmill's thrust coefficient of 1). Straight
//!   up or down, straight across or climbing, it never acts; it acts at all
//!   only once the descent passes about 0.8 `v_h` with at least 1.1 `v_h`
//!   across. It trims about 2% at a descent of `v_h` with 1.2 `v_h` across,
//!   19% at 2 `v_h` each way and 32% at 3 `v_h`: fast dives with the motors
//!   low reach that. **The guard has no source**: it is a modelling choice the
//!   research doesn't give.
//! - **Spinning backwards** (Crash Flip), the rotor's thrust is its still-air
//!   reverse share: the momentum theory above is for a rotor pushing air
//!   down through itself.
//!
//! Vortex ring state, where a rotor descends into its own air, is momentum
//! theory's blind spot. What it does there, Prop Wash, is its own effect
//! (ADR-0005).
//!
//! **To watch: nearly stopped props.** As a rotor's power goes to nothing
//! while air comes down through it and across it, momentum theory's thrust
//! stops depending on its speed: it tends to the smaller of `2ρA·|v_c|·μ` and
//! `½ρA·(v_c² + μ²)` (an ideal autorotation). Only a prop at exactly 0 RPM
//! gives none, and a coasting motor never quite reaches 0, so nearly stopped
//! props in a fast fall still lift a little, roughly as windmilling props
//! would. Prop Wash, Failsafe and disarm, Prop Strikes and Feel Tests should
//! keep an eye on it.
//!
//! # Rotor drag and the nose lifting at speed (E16, E18)
//!
//! Air crossing a spinning rotor tips its thrust back and drags the rotor
//! along with it: the H-force. It grows with the air's speed across the disc
//! and with the rotor's speed (research §4.2, its source S8): for each rotor
//! `F = −λ·ω·μ⃗`. The Quad definition gives the rotor drag as Faessler et al.
//! measured it (S5): per kilogram of the Quad, per m/s of airspeed, with every
//! rotor at the speed that hovers the Quad in this Map's air. So
//! `λ = m·d / (4·ω_hover)`. It acts where the rotor is, in the props' plane:
//! with the props above the centre of mass, it lifts the nose at speed (E18,
//! folded into rotor drag as the research suggests); with them below it, as
//! on the Freestyle 5″, it dips it a little.
//!
//! Each rotor sees the air at its own hub, the Quad's motion plus its turning
//! (`v + Ω × r`). So the rotor drag also slows the Quad's turning, and a roll
//! or pitch changes the air along the rotors' axes, which slows it too.
//!
//! # A whoop's ducts (E25, E26)
//!
//! A duct swallows air through the rotor and turns any air arriving across it
//! straight down: its ram drag is that air's momentum, the mass it swallows a
//! second times the speed across it (research §5.1, Pereira, S16). The mass a
//! second is `√(ρ·A·T)` for an ideal straight duct, so it grows with the root
//! of each rotor's thrust. The Quad definition gives the ram drag the same way
//! as rotor drag, per kilogram per m/s, in a hover: so each duct's is
//! `m·r / 4 · √(T / T_hover)` times the air's speed across it, with
//! `T_hover = m·g / 4`. A ducted rotor's centre of pressure sits higher than
//! an open rotor's, by the Quad definition's `nose_up_offset` (0.75 rotor
//! radius, Pereira), so the ram drag acts that far above the props' plane:
//! the duct's nose-up moment.
//!
//! # Body drag (E17)
//!
//! The frame, the pack and everything else on the Quad: `f = −½ρ·|A·v|·v` in
//! body axes, with `A` the Quad definition's drag areas (drag coefficient ×
//! area) facing forward, sideways and up (research §4.1, its source S7). So
//! the drag always points against the motion, and how big it is depends on
//! which way the Quad faces the air: `|A·v|` is the area it shows the air, as
//! an ellipsoid with those three areas would. It acts at the centre of mass.

use opendrone_maths::Vec3;
use opendrone_maths::functions::{max, min};

use crate::Drag;
use crate::motor::Model;

/// What the physics needs to know about the Quad to work out the air's push.
pub(crate) struct Airframe<'a> {
    pub mass: f64,
    pub drag: &'a Drag,
    /// Each rotor's hub from the centre of mass, in body axes.
    pub positions: &'a [Vec3; 4],
    /// The props' disc area, in m².
    pub disc_area: f64,
}

/// The rotors' and the air's push on the Quad over one step, in body axes.
pub(crate) struct Push {
    /// Through the centre of mass, in newtons.
    pub force: Vec3,
    /// About the centre of mass, in N·m.
    pub torque: Vec3,
    /// Each rotor's thrust, in newtons along the body's up axis.
    pub thrusts: [f64; 4],
}

/// The thrust and the air's push on a Quad moving at `velocity` (body axes,
/// m/s) and turning at `rotation` (body axes, rad/s), its rotors at `speeds`,
/// in air of `air_density` under `gravity`.
pub(crate) fn push(
    airframe: &Airframe,
    model: &Model,
    velocity: Vec3,
    rotation: Vec3,
    speeds: [f64; 4],
    air_density: f64,
    gravity: f64,
) -> Push {
    let drag = airframe.drag;
    let mass = airframe.mass;
    // A quarter of the weight: what each rotor carries in a hover.
    let hover_thrust = mass * gravity / 4.0;
    let hover_speed = model.settled_speed(hover_thrust);

    let mut force = Vec3::ZERO;
    let mut torque = Vec3::ZERO;
    let mut thrusts = [0.0; 4];
    for k in 0..4 {
        let at = airframe.positions[k];
        // The rotor's hub moves through still air at the Quad's speed plus
        // its turning about the centre of mass.
        let air = velocity + rotation.cross(at);
        let across = Vec3::new(air.x, air.y, 0.0);

        let thrust = thrust_in_moving_air(
            model.thrust(speeds[k]),
            air,
            airframe.disc_area,
            air_density,
        );
        thrusts[k] = thrust;
        let lift = Vec3::new(0.0, 0.0, thrust);
        force += lift;
        torque += at.cross(lift);

        // Rotor drag, in the props' plane (E16, E18).
        if drag.rotor > 0.0 && hover_speed > 0.0 {
            let pull = across * (-mass * drag.rotor / 4.0 * speeds[k].abs() / hover_speed);
            force += pull;
            torque += at.cross(pull);
        }
        // Ram drag, above the props' plane by the duct's offset (E25, E26).
        if drag.duct_ram > 0.0 && hover_thrust > 0.0 {
            let swallowed = (max(thrust, 0.0) / hover_thrust).sqrt();
            let pull = across * (-mass * drag.duct_ram / 4.0 * swallowed);
            force += pull;
            torque += (at + Vec3::new(0.0, 0.0, drag.duct_offset)).cross(pull);
        }
    }

    // Body drag, through the centre of mass (E17).
    let a = drag.body_area;
    let shown = Vec3::new(a.x * velocity.x, a.y * velocity.y, a.z * velocity.z).length();
    force += velocity * (-0.5 * air_density * shown);

    Push {
        force,
        torque,
        thrusts,
    }
}

/// The thrust of a rotor whose still-air thrust is `still` newtons, with its
/// hub moving through still air at `air` (body axes, m/s): momentum theory at
/// the same power, with its guard (see the module's "Inflow").
pub(crate) fn thrust_in_moving_air(still: f64, air: Vec3, disc_area: f64, air_density: f64) -> f64 {
    // "Not a number" passes through unchanged.
    if still.is_nan() || still <= 0.0 || disc_area <= 0.0 || air_density <= 0.0 {
        return still;
    }
    if air.x == 0.0 && air.y == 0.0 && air.z == 0.0 {
        return still;
    }
    let pushed = (still / (2.0 * air_density * disc_area)).sqrt();
    let climb = air.z / pushed;
    let across = (air.x * air.x + air.y * air.y).sqrt() / pushed;
    still * thrust_share(climb, across)
}

/// The thrust as a share of the still-air thrust, for air arriving along the
/// axis at `climb` and across the disc at `across`, both in units of the
/// still-air flow through the disc.
fn thrust_share(climb: f64, across: f64) -> f64 {
    let share = 1.0 / flow_through_the_disc(climb, across);
    if across <= 0.0 {
        return share;
    }
    // The guard: no more than straight down at the same descent, plus
    // ½ρA·(v_c² + μ²), which is (x² + m²)/4 of the still-air thrust. The
    // first part is at least 1 descending and 1/(x + 1) climbing, so only a
    // share above that can need it worked out.
    let windmill = (climb * climb + across * across) / 4.0;
    let at_least = if climb < 0.0 {
        1.0
    } else {
        1.0 / (climb + 1.0)
    };
    if share <= at_least + windmill {
        return share;
    }
    min(share, 1.0 / flow_through_the_disc(climb, 0.0) + windmill)
}

/// The flow through the disc `ū`, in units of the still-air flow, for air
/// arriving along the axis at `climb` and across it at `across` (in the same
/// units): the root of `(ū − climb)·ū·√(across² + ū²) = 1` above both `climb`
/// and 0.
fn flow_through_the_disc(climb: f64, across: f64) -> f64 {
    // Newton's method from above: there the left side is at least 1, and it
    // only grows, ever faster, so each step comes down towards the root
    // without passing it. Stop when a step no longer comes down.
    let mut flow = max(climb, 0.0) + 1.0;
    for _ in 0..MOST_STEPS {
        let root = (across * across + flow * flow).sqrt();
        let through = flow - climb;
        let left = through * flow * root - 1.0;
        if left.is_nan() || left <= 0.0 {
            break;
        }
        let slope = root * (2.0 * flow - climb) + through * flow * flow / root;
        let next = flow - left / slope;
        if next.is_nan() || next >= flow {
            break;
        }
        flow = next;
    }
    flow
}

/// Newton's method needs a handful of steps in a hover's air and a few dozen
/// at most in the fastest air; this only bounds the work.
const MOST_STEPS: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_still_air_the_thrust_is_the_thrust_stands() {
        assert_eq!(flow_through_the_disc(0.0, 0.0), 1.0);
        assert_eq!(thrust_in_moving_air(0.3, Vec3::ZERO, 1e-3, 1.225), 0.3);
    }

    #[test]
    fn climbing_at_the_still_air_flow_leaves_68_percent_of_the_thrust() {
        // (ū − 1)·ū² = 1 has its root at 1.465571…, so the thrust is
        // 1/1.465571 of the still-air thrust.
        assert!((thrust_share(1.0, 0.0) - 0.682_328).abs() < 1e-6);
    }

    #[test]
    fn descending_at_the_still_air_flow_gives_a_third_more_thrust() {
        // (ū + 1)·ū² = 1 has its root at 0.754878, so the thrust is 1.324718
        // times the still-air thrust.
        assert!((thrust_share(-1.0, 0.0) - 1.324_718).abs() < 1e-6);
    }

    #[test]
    fn the_flow_solves_the_momentum_equation_wherever_the_air_comes_from() {
        for climb in [-30.0, -5.0, -1.5, -0.4, 0.0, 0.3, 1.0, 4.0, 25.0] {
            for across in [0.0, 0.2, 1.0, 3.0, 12.0] {
                let flow = flow_through_the_disc(climb, across);
                let left = (flow - climb) * flow * (across * across + flow * flow).sqrt();
                assert!(
                    (left - 1.0).abs() < 1e-9 && flow > 0.0 && flow > climb,
                    "climb {climb}, across {across}: flow {flow} gives {left}"
                );
            }
        }
    }

    #[test]
    fn the_guard_never_acts_straight_up_down_or_across_and_caps_a_fast_tumbling_dive() {
        for climb in [-30.0, -1.0, -0.2, 0.0, 0.5, 3.0] {
            assert_eq!(
                thrust_share(climb, 0.0),
                1.0 / flow_through_the_disc(climb, 0.0)
            );
        }
        for across in [0.1, 1.0, 4.0, 30.0] {
            assert_eq!(
                thrust_share(0.0, across),
                1.0 / flow_through_the_disc(0.0, across)
            );
        }
        // A flare, the air arriving from below and ahead at about v_h:
        // untouched.
        assert_eq!(
            thrust_share(-0.34, 0.94),
            1.0 / flow_through_the_disc(-0.34, 0.94)
        );
        // Four times v_h down through the disc and across it: momentum theory
        // alone gives about 16 times the still-air thrust; the guard, the
        // straight-down 2.13 plus (16 + 16)/4.
        let guard = 1.0 / flow_through_the_disc(-4.0, 0.0) + 8.0;
        assert!(1.0 / flow_through_the_disc(-4.0, 4.0) > 15.0);
        assert_eq!(thrust_share(-4.0, 4.0), guard);
    }
}
