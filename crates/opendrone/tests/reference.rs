//! The reference check (ADR-0003, "Checks in CI"): Scenarios flown through
//! the game's own stepping, in the game's build, give the same fingerprint
//! as the headless Scenario runner, whose Results file holds it. Basis: Rule
//! (ADR-0001: the same Flight Inputs give the same flight, bit for bit).
//! CI also runs it from the game itself: `opendrone --reference`.

use opendrone::reference;
use opendrone::set_up::repo_folder;

#[test]
fn a_quad_flown_into_a_wall_through_the_games_stepping_matches_the_headless_runner() {
    // Our Flight Controller, an armed flight, the Radio Link and collisions
    // with a box: 24,000 steps.
    let repo = repo_folder();
    let flight = reference::fly(
        &repo,
        &repo.join("scenarios/flight-controller/a-quad-that-hits-a-wall-stays-armed.toml"),
    )
    .unwrap();
    assert_eq!(flight.steps, 24_000);
    assert_eq!(flight.steps_per_tick, 8);
    assert_eq!(flight.game, flight.runner);
}

#[test]
fn a_whoop_flown_into_a_rail_through_the_games_stepping_matches_the_headless_runner() {
    // A convex Map shape, which goes to parry3d finished (#141): the maths
    // Bevy's glam shares with the core never runs in the Simulation.
    let repo = repo_folder();
    let flight = reference::fly(
        &repo,
        &repo.join("scenarios/physics/whoop-into-a-6-cm-rail-at-30-m-s.toml"),
    )
    .unwrap();
    assert_eq!(flight.game, flight.runner);
}
