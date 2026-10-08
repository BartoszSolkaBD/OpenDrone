//! Readable checks for the Rates paste (#13, #53): a pilot's Rates read from
//! pasted Betaflight CLI output, from the active rate profile only. They read
//! the maintainer's real exports in `docs/research/quad-settings/`. Basis:
//! Rule (#13: only the active rate profile's rate settings; the last
//! `rateprofile` line names it; everything else is ignored; what's left out
//! keeps Betaflight's defaults) unless said.

use opendrone_flight_controller::cli::RatesPaste;
use opendrone_flight_controller::{AxisRates, Rates, RatesType};

const METEOR: &str = include_str!("../../../docs/research/quad-settings/meteor65-pro.diff-all.txt");
const CETUS: &str = include_str!("../../../docs/research/quad-settings/cetus-x.diff-all.txt");

fn paste(text: &str) -> RatesPaste {
    RatesPaste::read(text).unwrap_or_else(|refusal| panic!("should read:\n{refusal}"))
}

fn refused(text: &str) -> String {
    RatesPaste::read(text)
        .err()
        .unwrap_or_else(|| panic!("should be refused"))
        .to_string()
}

fn changed(text: &str, old: &str, new: &str) -> String {
    assert!(text.contains(old), "no {old:?} to change");
    text.replacen(old, new, 1)
}

/// Betaflight's default Rates with every axis's max rate at `srate` × 10
/// °/s.
fn actual_max(srate: u8) -> Rates {
    let axis = AxisRates {
        rc_rate: 7,
        srate,
        expo: 0,
    };
    Rates {
        roll: axis,
        pitch: axis,
        yaw: axis,
        ..Rates::BETAFLIGHT_DEFAULT
    }
}

#[test]
fn the_cetus_xs_paste_gives_actual_rates_with_650_degrees_a_second_on_every_axis() {
    // Basis: Source (the Cetus X's diff all: rate profile 0 sets
    // roll_srate, pitch_srate and yaw_srate to 65, the rest default).
    let cetus = paste(CETUS);
    assert_eq!(cetus.rate_profile, Some(0));
    assert_eq!(cetus.rates, actual_max(65));
    assert_eq!(
        cetus.read,
        [
            "set roll_srate = 65",
            "set pitch_srate = 65",
            "set yaw_srate = 65"
        ]
    );
}

#[test]
fn the_meteors_paste_sets_no_rate_so_every_one_keeps_betaflights_default() {
    // Basis: Source (the Meteor's rate profile 0 sets only tpa_rate, which in
    // 4.3 lived there but isn't a rate: it belongs to the Tune).
    let meteor = paste(METEOR);
    assert_eq!(meteor.rate_profile, Some(0));
    assert_eq!(meteor.rates, Rates::BETAFLIGHT_DEFAULT);
    assert!(meteor.read.is_empty());
    assert!(
        meteor
            .summary()
            .starts_with("Used rate profile 0 (no rate settings, so every one keeps Betaflight's default); ignored "),
        "{}",
        meteor.summary()
    );
}

#[test]
fn it_says_which_rate_profile_it_used_and_how_many_lines_it_ignored() {
    let cetus = paste(CETUS);
    // Every line but comments, blank lines, the rateprofile lines and the
    // three rate settings read.
    let counted = CETUS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("rateprofile "))
        .count();
    assert_eq!(cetus.ignored, counted - 3);
    assert_eq!(cetus.ignored, 104);
    assert_eq!(
        cetus.summary(),
        format!(
            "Used rate profile 0 (3 rate settings); ignored {} lines.",
            counted - 3
        )
    );
}

#[test]
fn the_last_rateprofile_line_wins() {
    let text = changed(
        CETUS,
        "rateprofile 2\n",
        "rateprofile 2\n\nset roll_srate = 80\nset rates_type = BETAFLIGHT\n",
    );
    // As exported, rate profile 0 is active: rate profile 2's lines are
    // ignored.
    assert_eq!(paste(&text).rates, actual_max(65));
    let text = changed(
        &text,
        "# restore original rateprofile selection\nrateprofile 0",
        "# restore original rateprofile selection\nrateprofile 2",
    );
    let second = paste(&text);
    assert_eq!(second.rate_profile, Some(2));
    assert_eq!(second.rates.rates_type, RatesType::Betaflight);
    assert_eq!(second.rates.roll.srate, 80);
    // Rate profile 0's 65s are ignored, so pitch and yaw keep the default.
    assert_eq!(second.rates.pitch.srate, 67);
}

#[test]
fn now_and_after_show_every_rate_setting_and_what_changes() {
    let cetus = paste(CETUS);
    let rows = cetus.now_after(&Rates::BETAFLIGHT_DEFAULT);
    assert_eq!(rows.len(), 19);
    assert_eq!(rows[0].setting, "rates_type");
    assert_eq!(
        (rows[0].now.as_str(), rows[0].after.as_str()),
        ("ACTUAL", "ACTUAL")
    );
    let changes: Vec<(&str, &str, &str)> = rows
        .iter()
        .filter(|r| r.changed())
        .map(|r| (r.setting, r.now.as_str(), r.after.as_str()))
        .collect();
    assert_eq!(
        changes,
        [
            ("roll_srate", "67", "65"),
            ("pitch_srate", "67", "65"),
            ("yaw_srate", "67", "65")
        ]
    );
}

#[test]
fn a_few_pasted_set_lines_read_as_one_rate_profile() {
    let pasted = paste("set rates_type = KISS\nset roll_rc_rate = 100\nset thr_expo = 20\n");
    assert_eq!(pasted.rate_profile, None);
    assert_eq!(pasted.rates.rates_type, RatesType::Kiss);
    assert_eq!(pasted.rates.roll.rc_rate, 100);
    assert_eq!(pasted.rates.thr_expo, 20);
    assert!(pasted.summary().starts_with("Found no `rateprofile` line"));
}

#[test]
fn a_rate_number_above_its_rates_types_limit_is_held_there_as_betaflight_does() {
    // Basis: Source (2026.6.2's ratesSettingLimits: Actual allows at most
    // 200 for the max rate, which Betaflight holds when it checks its
    // settings).
    let pasted = paste("rateprofile 0\nset roll_srate = 230\n");
    assert_eq!(pasted.rates.roll.srate, 200);
    assert_eq!(
        pasted.notes,
        ["roll_srate 230 is above 200, the most ACTUAL rates allow, so Betaflight keeps 200."]
    );
}

#[test]
fn a_value_betaflight_would_refuse_is_refused_with_its_line() {
    // Basis: Source (2026.6.2's settings.c: expo 0 to 100, rates_type one of
    // five words).
    assert_eq!(
        refused("rateprofile 0\nset roll_expo = 101\nset rates_type = FAST\n"),
        "Line 2: `roll_expo` is a whole number from 0 to 100, not \"101\".\nLine 3: `rates_type` is one of BETAFLIGHT, RACEFLIGHT, KISS, ACTUAL, QUICK, not \"FAST\"."
    );
}

#[test]
fn rates_from_betaflight_older_than_4_3_are_refused() {
    // Basis: Source (Actual rates became Betaflight's default in 4.3, so an
    // older diff that leaves rates_type out meant other Rates).
    let old = changed(CETUS, "(S411) 4.4.0 Oct", "(S411) 4.2.0 Oct");
    assert!(refused(&old).starts_with("This is Betaflight 4.2.0, which is older than 4.3"));
}

#[test]
fn a_paste_with_no_rate_profile_is_refused() {
    assert!(
        refused("set p_roll = 40\naux 0 0 0 1700 2100 0 0\n")
            .starts_with("There's no rate profile here")
    );
}
