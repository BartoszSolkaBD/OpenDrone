//! Readable checks for the Rates in a Scenario's starting state: written as
//! the Betaflight App shows them, kept as Betaflight 2026.6's CLI stores them.
//! Basis: Source (Betaflight 2026.6.2's `controlrate_profile.c` and
//! `cli/settings.c`, and the Betaflight App's Rates tab).

use std::fs;
use std::path::Path;

use opendrone_scenario::{AxisRates, Rates, RatesType, Repo, ThrottleLimitType, read_scenario};

fn free_fall() -> String {
    let repo = Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    fs::read_to_string(
        repo.root
            .join("scenarios/physics/free-fall-is-exactly-g.toml"),
    )
    .unwrap()
}

const ACTUAL_RATES: &str = r#"type               = "Actual"
roll               = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
pitch              = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
yaw                = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
rate_limit         = "1998 °/s"
throttle           = "mid 0.50, hover 0.50, expo 0.00, limit off"
quickrates_rc_expo = "off""#;

/// The free-fall Scenario with its `[start.rates]` lines replaced.
fn with_rates(rates: &str) -> Result<Rates, Vec<String>> {
    let text = free_fall();
    assert!(
        text.contains(ACTUAL_RATES),
        "the free-fall Scenario's Rates changed"
    );
    read_scenario("rates.toml", &text.replacen(ACTUAL_RATES, rates, 1))
        .map(|scenario| scenario.start.rates)
        .map_err(|problems| problems.0.iter().map(|p| p.sentence.clone()).collect())
}

fn axis(rates_type: &str, axis: &str) -> Result<AxisRates, Vec<String>> {
    let rates = ACTUAL_RATES
        .replacen("\"Actual\"", &format!("\"{rates_type}\""), 1)
        .replacen(
            "roll               = \"center sensitivity 70 °/s, max rate 670 °/s, expo 0.00\"",
            &format!("roll               = \"{axis}\""),
            1,
        )
        .replace(
            "\"center sensitivity 70 °/s, max rate 670 °/s, expo 0.00\"",
            &format!("\"{axis}\""),
        );
    with_rates(&rates).map(|rates| rates.roll)
}

fn stored(rc_rate: u8, srate: u8, expo: u8) -> AxisRates {
    AxisRates {
        rc_rate,
        srate,
        expo,
    }
}

#[test]
fn betaflights_default_rate_profile_reads_as_its_cli_stores_it() {
    // Betaflight 2026.6's defaults: Actual, rc_rate 7, srate 67, expo 0,
    // rate limit 1998, thr_mid 50, thr_hover 50, thr_expo 0, limit off (100).
    let rates = with_rates(ACTUAL_RATES).unwrap();
    assert_eq!(
        rates,
        Rates {
            rates_type: RatesType::Actual,
            roll: stored(7, 67, 0),
            pitch: stored(7, 67, 0),
            yaw: stored(7, 67, 0),
            rate_limit: [1998; 3],
            thr_mid: 50,
            thr_hover: 50,
            thr_expo: 0,
            throttle_limit_type: ThrottleLimitType::Off,
            throttle_limit_percent: 100,
            quickrates_rc_expo: false,
        }
    );
}

#[test]
fn each_rates_type_reads_the_betaflight_apps_own_labels_and_scales() {
    for (rates_type, written, expected) in [
        (
            "Betaflight",
            "rc rate 1.00, rate 0.70, rc expo 0.15",
            stored(100, 70, 15),
        ),
        (
            "Raceflight",
            "rate 370 °/s, acro+ 80%, expo 50%",
            stored(37, 80, 50),
        ),
        (
            "KISS",
            "rc rate 1.20, rate 0.73, rc curve 0.30",
            stored(120, 73, 30),
        ),
        (
            "Actual",
            "center sensitivity 200 °/s, max rate 1000 °/s, expo 0.54",
            stored(20, 100, 54),
        ),
        (
            "Quick",
            "rc rate 1.00, max rate 670 °/s, expo 0.10",
            stored(100, 67, 10),
        ),
    ] {
        assert_eq!(
            axis(rates_type, written),
            Ok(expected),
            "{rates_type}: {written}"
        );
    }
}

/// The problem with the roll axis. `axis` writes the same text on all three
/// axes, so each has the same problem.
fn roll_problem(rates_type: &str, written: &str) -> String {
    let found = axis(rates_type, written).unwrap_err();
    assert_eq!(found.len(), 3, "{found:?}");
    found[0].clone()
}

#[test]
fn a_rates_number_between_two_stored_steps_is_refused_with_the_step() {
    assert_eq!(
        roll_problem(
            "Actual",
            "center sensitivity 75 °/s, max rate 670 °/s, expo 0.00"
        ),
        "`roll`: Actual rates' center sensitivity goes in steps of 10 °/s, from 10 °/s to 2000 °/s"
    );
    assert_eq!(
        roll_problem("Betaflight", "rc rate 1.00, rate 0.705, rc expo 0.00"),
        "`roll`: Betaflight rates' rate goes in steps of 0.01, from 0.00 to 1.00"
    );
}

#[test]
fn another_rates_types_labels_are_refused_with_an_example() {
    let found = roll_problem(
        "Betaflight",
        "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00",
    );
    assert!(
        found.ends_with(
            "Betaflight rates write each axis as the Betaflight App shows it, such as \"rc rate 1.00, rate 0.70, rc expo 0.00\""
        ),
        "{found:?}"
    );
}

#[test]
fn a_rate_limit_and_a_throttle_limit_read_per_axis_and_as_scale_or_clip() {
    let rates = with_rates(
        &ACTUAL_RATES
            .replacen(
                "rate_limit         = \"1998 °/s\"",
                "rate_limit         = \"roll 1998, pitch 1500, yaw 900 °/s\"",
                1,
            )
            .replacen(
                "\"mid 0.50, hover 0.50, expo 0.00, limit off\"",
                "\"mid 0.40, hover 0.35, expo 0.25, limit scale 80%\"",
                1,
            ),
    )
    .unwrap();
    assert_eq!(rates.rate_limit, [1998, 1500, 900]);
    assert_eq!(
        (rates.thr_mid, rates.thr_hover, rates.thr_expo),
        (40, 35, 25)
    );
    assert_eq!(
        (rates.throttle_limit_type, rates.throttle_limit_percent),
        (ThrottleLimitType::Scale, 80)
    );
    let refused = with_rates(&ACTUAL_RATES.replacen("\"1998 °/s\"", "\"2500 °/s\"", 1));
    assert!(refused.unwrap_err()[0].ends_with("each is a whole number of °/s from 200 to 1998"),);
}
