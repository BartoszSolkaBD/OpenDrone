//! Readable checks for the shared unit list, which reads every number in
//! Scenario and Pack files (#11 §2, #16 §3).

use opendrone_pack::units::{
    Dimension, Expected, Range, parse_expected, parse_parts, parse_quantity, parse_range,
    significant_figures,
};

fn value(text: &str) -> f64 {
    parse_quantity(text)
        .unwrap_or_else(|p| panic!("{text:?} should read: {p}"))
        .value
}

fn refusal(text: &str) -> String {
    match parse_quantity(text) {
        Ok(q) => panic!("{text:?} should be refused, but read as {q:?}"),
        Err(problem) => problem.0,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

#[test]
fn symbols_and_plain_keyboard_spellings_read_the_same() {
    for spellings in [
        &["670 °/s", "670 deg/s", "670°/s"][..],
        &["140 g·cm²", "140 g cm^2", "140 g*cm^2", "140 g.cm^2"],
        &["29 mΩ", "29 mohm", "29 mohms"],
        &["50 µs", "50 us", "50 μs"],
        &["9.81 m/s²", "9.81 m/s^2"],
        &["0.3 s⁻¹", "0.3 s^-1", "0.3 1/s"],
        &["1.225 kg/m³", "1.225 kg/m^3"],
        &["-9.81 m/s²", "−9.81 m/s²"],
        &["5 m", "+5 m", "5m"],
    ] {
        let first = parse_quantity(spellings[0]).unwrap();
        for spelling in &spellings[1..] {
            let other = parse_quantity(spelling)
                .unwrap_or_else(|p| panic!("{spelling:?} should read: {p}"));
            assert_eq!(
                other.value, first.value,
                "{spelling:?} vs {:?}",
                spellings[0]
            );
            assert_eq!(other.dimension(), first.dimension());
        }
    }
}

#[test]
fn tools_write_symbols() {
    for (written, symbols) in [
        ("9.81 m/s^2", "9.81 m/s²"),
        ("140 g cm^2", "140 g·cm²"),
        ("0.3 1/s", "0.3 s⁻¹"),
        ("30 deg", "30°"),
        ("670 deg/s", "670 °/s"),
        ("29 mohm", "29 mΩ"),
        ("50 us", "50 µs"),
        ("−2 m", "-2 m"),
        ("50 %", "50%"),
        ("0.12", "0.12"),
    ] {
        assert_eq!(parse_quantity(written).unwrap().text(), symbols);
    }
}

#[test]
fn everyday_prefixes_keep_numbers_ordinary() {
    assert!(close(value("140 g·cm²"), 1.4e-5));
    assert!(close(value("23.0 g"), 0.023));
    assert!(close(value("8 kHz"), 8000.0));
    assert!(close(value("35 ms"), 0.035));
    assert!(close(value("320 mAh"), 0.32 * 3600.0));
    assert!(close(value("25 cm²"), 0.0025));
    assert!(close(value("3.6 km/h"), 1.0));
    assert!(close(value("2 min"), 120.0));
}

#[test]
fn a_prefix_on_a_unit_that_doesnt_take_one_is_refused() {
    assert!(refusal("5 kRPM").contains("isn't a unit on OpenDrone's unit list"));
    assert!(refusal("5 cs").contains("isn't a unit on OpenDrone's unit list"));
}

#[test]
fn angles_are_degrees_and_spin_speeds_rpm_and_both_read_as_turning_speed() {
    assert!(close(value("180°"), core::f64::consts::PI));
    assert!(close(value("1 RPM"), value("6 °/s")));
    assert!(close(
        value("19500 KV"),
        value("19500 RPM") / value("1 V") * 1.0
    ));
    assert_eq!(
        parse_quantity("1 RPM").unwrap().dimension(),
        Dimension::ROTATION_SPEED
    );
}

#[test]
fn radians_are_refused() {
    for text in ["3.14 rad", "100 rad/s", "2 radians"] {
        assert!(
            refusal(text).contains("radians, which OpenDrone files never use"),
            "{text}"
        );
    }
}

#[test]
fn a_decimal_comma_is_refused_with_the_fix() {
    assert_eq!(
        refusal("31,2 g"),
        "\"31,2 g\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"31.2 g\""
    );
}

#[test]
fn a_thousands_separator_is_refused_with_the_fix() {
    assert_eq!(
        refusal("2,000 °/s"),
        "\"2,000 °/s\" isn't a number OpenDrone can read: numbers have no thousands separators, so write \"2000 °/s\""
    );
}

#[test]
fn a_unit_not_on_the_list_is_refused() {
    assert_eq!(
        refusal("5 furlongs"),
        "\"5 furlongs\": \"furlongs\" isn't a unit on OpenDrone's unit list"
    );
}

#[test]
fn text_without_a_number_is_refused() {
    assert!(refusal("m/s").contains("doesn't start with a number"));
    assert_eq!(refusal("  "), "there's no number here");
}

#[test]
fn percent_is_its_own_kind_of_number() {
    let half = parse_quantity("50%").unwrap();
    assert_eq!(half.dimension(), Dimension::PERCENT);
    assert!(close(half.value, 0.5));
    assert!(
        parse_quantity("0.5")
            .unwrap()
            .as_a(Dimension::PERCENT)
            .is_err()
    );
}

#[test]
fn a_number_of_the_wrong_kind_is_refused_saying_what_it_needs() {
    assert_eq!(
        parse_quantity("5 m")
            .unwrap()
            .as_a(Dimension::SPEED)
            .unwrap_err()
            .0,
        "\"5 m\" is a length, but this needs a speed, such as \"5 m/s\""
    );
}

fn expected(text: &str) -> Expected {
    parse_expected(text).unwrap_or_else(|p| panic!("{text:?} should read: {p}"))
}

#[test]
fn plus_or_minus_an_amount_accepts_values_within_that_amount() {
    let g = expected("-9.81 m/s² ± 0.001 m/s²");
    assert!(g.accepts(-9.8105));
    assert!(g.accepts(-9.8091));
    assert!(!g.accepts(-9.812));
    assert_eq!(g.dimension(), Dimension::ACCELERATION);
    assert_eq!(g.text(), "-9.81 m/s² ± 0.001 m/s²");
    assert_eq!(expected("2 m +- 0.5 m"), expected("2 m ± 0.5 m"));
    assert_eq!(expected("2 m +/- 0.5 m"), expected("2 m ± 0.5 m"));
}

#[test]
fn plus_or_minus_a_percentage_accepts_that_share_of_the_value() {
    let max_rate = expected("670 °/s ± 3%");
    assert!(max_rate.accepts(value("690 °/s")));
    assert!(!max_rate.accepts(value("691 °/s")));
    assert_eq!(max_rate.text(), "670 °/s ± 3%");
}

#[test]
fn plus_or_minus_a_percentage_of_a_percentage_is_percentage_points() {
    let battery = expected("50% ± 5%");
    assert!(battery.accepts(0.54));
    assert!(!battery.accepts(0.56));
}

#[test]
fn between_two_values_accepts_both_ends_and_everything_inside() {
    let hover = expected("between 40% and 50%");
    assert!(hover.accepts(0.40));
    assert!(hover.accepts(0.45));
    assert!(hover.accepts(0.50));
    assert!(!hover.accepts(0.51));
    assert_eq!(expected("between 40 and 50%"), hover);
    assert_eq!(hover.text(), "between 40% and 50%");
}

#[test]
fn an_expected_value_without_a_tolerance_is_refused() {
    assert_eq!(
        parse_expected("670 °/s").unwrap_err().0,
        "\"670 °/s\" needs a tolerance: add \"± amount\" or \"± percent\", or write \"between X and Y\""
    );
}

#[test]
fn a_tolerance_of_another_kind_is_refused() {
    assert!(
        parse_expected("2 m ± 1 s")
            .unwrap_err()
            .0
            .contains("the tolerance must be in the value's own kind of unit")
    );
    assert!(parse_expected("between 1 m and 2 s").is_err());
    assert!(
        parse_expected("between 50% and 40%")
            .unwrap_err()
            .0
            .contains("the lower end comes first")
    );
}

#[test]
fn measured_values_are_written_to_three_significant_figures() {
    for (number, text) in [
        (-9.81, "-9.81"),
        (-4.905613, "-4.91"),
        (2000.0000001, "2000"),
        (89.99999999, "90.0"),
        (0.000125, "0.000125"),
        (1.0, "1.00"),
        (0.0, "0"),
        (-0.0, "0"),
        (123_456.0, "123000"),
        (0.9996, "1.00"),
    ] {
        assert_eq!(significant_figures(number, 3), text, "{number}");
    }
    let unit = parse_quantity("1 m/s²").unwrap().unit;
    assert_eq!(unit.write(-9.8100000001), "-9.81 m/s²");
}

#[test]
fn labelled_parts_read_with_the_label_before_or_after_and_the_unit_once() {
    let inertia = parse_parts(
        "roll 70, pitch 90, yaw 140 g·cm²",
        &["roll", "pitch", "yaw"],
    )
    .unwrap();
    assert_eq!(inertia.len(), 3);
    for (part, expected) in inertia.iter().zip([7e-6, 9e-6, 1.4e-5]) {
        assert_eq!(part.quantity.dimension(), Dimension::INERTIA);
        assert!(close(part.quantity.value, expected), "{part:?}");
    }
    let position = parse_parts("0 m east, -3 m north, 2 m up", &["east", "north", "up"]).unwrap();
    let labels: Vec<&str> = position.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["east", "north", "up"]);
    assert_eq!(position[1].quantity.value, -3.0);
}

#[test]
fn a_unit_written_once_at_the_end_is_every_parts_only_when_no_other_part_has_one() {
    let quick = parse_parts(
        "rc rate 1.00, max rate 670 °/s, expo 0.10",
        &["rc rate", "max rate", "expo"],
    )
    .unwrap();
    assert_eq!(quick[0].quantity.dimension(), Dimension::NONE);
    assert_eq!(quick[1].quantity.dimension(), Dimension::ROTATION_SPEED);
    assert_eq!(quick[2].quantity.dimension(), Dimension::NONE);
    let mixed = parse_parts("roll 70, pitch 90 °/s, yaw 140", &["roll", "pitch", "yaw"]).unwrap();
    assert_eq!(mixed[0].quantity.dimension(), Dimension::NONE);
}

#[test]
fn a_label_may_be_several_words_and_the_longest_that_fits_wins() {
    let actual = parse_parts(
        "center sensitivity 70 °/s, max rate 670 °/s, expo 0.54",
        &["center sensitivity", "max rate", "expo"],
    )
    .unwrap();
    let labels: Vec<&str> = actual.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["center sensitivity", "max rate", "expo"]);
    assert!(close(actual[1].quantity.value, value("670 °/s")));
    let betaflight = parse_parts("rc rate 1.00, rate 0.70", &["rate", "rc rate"]).unwrap();
    assert_eq!(betaflight[0].label, "rc rate");
    assert_eq!(betaflight[1].label, "rate");
    assert!(parse_parts("max rate 670 °/s", &["rate", "rc rate"]).is_err());
}

#[test]
fn a_part_without_a_known_label_or_given_twice_is_refused() {
    assert!(
        parse_parts("roll 70, wobble 3 °/s", &["roll", "pitch", "yaw"])
            .unwrap_err()
            .0
            .contains("needs one of these labels: roll, pitch, yaw")
    );
    assert!(
        parse_parts("roll 70, roll 80 °/s", &["roll", "pitch", "yaw"])
            .unwrap_err()
            .0
            .contains("gives roll twice")
    );
}

#[test]
fn an_estimates_range_reads_absolute_or_relative() {
    let lag = parse_range("20–50 ms").unwrap();
    assert_eq!(parse_range("20-50 ms").unwrap(), lag);
    assert!(lag.holds(0.035));
    assert!(!lag.holds(0.051));
    match parse_range("×0.5–×2").unwrap() {
        Range::Relative { low, high } => assert_eq!((low, high), (0.5, 2.0)),
        other => panic!("expected a relative range, got {other:?}"),
    }
    assert_eq!(
        parse_range("x0.5-x2").unwrap(),
        parse_range("×0.5–×2").unwrap()
    );
    match parse_range("0.1–0.6 s⁻¹").unwrap() {
        Range::Absolute { low, high } => {
            assert_eq!(low.dimension(), Dimension::PER_SECOND);
            assert!(close(low.value, 0.1) && close(high.value, 0.6));
        }
        other => panic!("expected an absolute range, got {other:?}"),
    }
    assert!(parse_range("about 30 ms").is_err());
}
