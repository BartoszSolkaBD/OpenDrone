//! Shared set-up for the game's readable checks.

#![allow(dead_code)]

use opendrone::set_up::{FlightPlan, open_packs, repo_folder};
use opendrone::settings::Settings;
use opendrone_pack::Packs;

pub fn packs() -> Packs {
    open_packs(&repo_folder()).expect("the built-in Pack reads")
}

/// The tracer's flight plan for a built-in Quad, such as `whoop-65`.
pub fn plan(quad: &str) -> FlightPlan {
    FlightPlan::from_packs(&packs(), Settings::tracer(quad)).expect("the tracer's Quads fly")
}
