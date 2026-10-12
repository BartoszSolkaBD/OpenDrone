//! Readable checks for the `diff all` importer (#53): a Betaflight 4.3 or
//! newer export becomes a 2026.6 Tune (ADR-0008, ADR-0015, #21's
//! resolution). They read the maintainer's real exports in
//! `docs/research/quad-settings/`: the Meteor65 Pro's (Betaflight 4.3.0) and
//! the Cetus X's (4.4.0). Basis: Source (those exports, and each version's
//! defaults from its source) unless said.

use opendrone_flight_controller::Tune;
use opendrone_flight_controller::cli::table::{self, Source};
use opendrone_flight_controller::cli::{Family, TuneImport, Version, Where, Why, import_tune};

const METEOR: &str = include_str!("../../../docs/research/quad-settings/meteor65-pro.diff-all.txt");
const CETUS: &str = include_str!("../../../docs/research/quad-settings/cetus-x.diff-all.txt");
const FREESTYLE_5_TUNE: &str = include_str!("../../../packs/opendrone/quads/freestyle-5/tune.txt");

fn import(text: &str) -> TuneImport {
    import_tune(text).unwrap_or_else(|refusal| panic!("should import:\n{refusal}"))
}

fn meteor() -> TuneImport {
    import(METEOR)
}

fn cetus() -> TuneImport {
    import(CETUS)
}

/// A setting's value and mark.
fn got(import: &TuneImport, name: &str) -> (String, String) {
    let setting = import
        .setting(name)
        .unwrap_or_else(|| panic!("no {name} in the import"));
    (setting.value.clone(), setting.mark.clone())
}

fn is(value: &str, mark: &str) -> (String, String) {
    (value.to_string(), mark.to_string())
}

/// The text with one piece changed, which must be there.
fn changed(text: &str, old: &str, new: &str) -> String {
    assert!(text.contains(old), "no {old:?} to change");
    text.replacen(old, new, 1)
}

fn refusal(text: &str) -> String {
    import_tune(text)
        .err()
        .unwrap_or_else(|| panic!("should be refused"))
        .to_string()
}

/// The `set` lines of a Tune's text, as name, value and mark.
fn set_lines(text: &str) -> Vec<(String, String, String)> {
    text.lines()
        .filter_map(|line| line.strip_prefix("set "))
        .map(|rest| {
            let (setting, mark) = rest.split_once('#').expect("every line has a mark");
            let (name, value) = setting.split_once('=').unwrap();
            (
                name.trim().to_string(),
                value.trim().to_string(),
                mark.trim().to_string(),
            )
        })
        .collect()
}

#[test]
fn a_diff_all_from_betaflight_4_3_imports_as_a_2026_6_tune() {
    let meteor = meteor();
    assert_eq!(
        meteor.version,
        Version {
            major: 4,
            minor: 3,
            patch: 0
        }
    );
    assert_eq!(meteor.family, Family::V4_3);
    assert_eq!(meteor.command.as_deref(), Some("diff all"));
    assert_eq!(meteor.craft_name.as_deref(), Some("Meteor65 pro"));
    assert_eq!(meteor.configured.as_deref(), Some("2023-01-16"));
    assert_eq!(meteor.profile, 0);
    assert_eq!(meteor.problems, Vec::<String>::new());
    // Every setting the Flight Controller reads is spelled out, so the Tune
    // reads in full.
    let text = meteor.tune_txt("Whoop 65", "meteor65-pro.diff-all.txt");
    let lines = set_lines(&text);
    let tune = Tune::read(lines.iter().map(|(n, v, _)| (n.as_str(), v.as_str())));
    assert!(tune.is_ok(), "{tune:?}");
    assert!(text.starts_with(
        "# Whoop 65 Tune: Betaflight 2026.6 names and units.\n# Imported from the diff all of the quad named \"Meteor65 pro\": Betaflight 4.3.0,\n# configured 2023-01-16 (meteor65-pro.diff-all.txt),\n"
    ));
}

#[test]
fn the_meteors_d_min_becomes_d_and_its_d_becomes_d_max() {
    // Basis: Source (the Betaflight research §3: in 4.3–4.5 `d_roll` was the
    // peak and `d_min_roll` the base; 2025.12 named the base `d_roll` and the
    // peak `d_max_roll`). The Meteor sets D 48/52, D-min 45/49.
    let meteor = meteor();
    assert_eq!(got(&meteor, "d_roll"), is("45", "diff (was d_min_roll)"));
    assert_eq!(got(&meteor, "d_max_roll"), is("48", "diff (was d_roll)"));
    assert_eq!(got(&meteor, "d_pitch"), is("49", "diff (was d_min_pitch)"));
    assert_eq!(got(&meteor, "d_max_pitch"), is("52", "diff (was d_pitch)"));
    // Yaw has no D in 4.3, so Dynamic D is off there and stays off.
    assert_eq!(got(&meteor, "d_yaw"), is("0", "4.3 default"));
    assert_eq!(
        got(&meteor, "d_max_yaw"),
        is("0", "4.3 default (was d_min_yaw)")
    );
}

#[test]
fn with_dynamic_d_off_in_4_3_d_stays_d_and_d_max_keeps_it_off() {
    // Basis: Source (4.3.0's pid_init.c runs Dynamic D only when d_min is
    // above 0 and below D; 2026.6.2's only when d_max is above D).
    let off = import(&changed(
        METEOR,
        "set d_min_roll = 45",
        "set d_min_roll = 0",
    ));
    assert_eq!(got(&off, "d_roll"), is("48", "diff"));
    assert_eq!(got(&off, "d_max_roll"), is("0", "diff (was d_min_roll)"));
    let above = import(&changed(
        METEOR,
        "set d_min_roll = 45",
        "set d_min_roll = 50",
    ));
    assert_eq!(got(&above, "d_roll"), is("48", "diff"));
    assert_eq!(
        got(&above, "d_max_roll"),
        is(
            "0",
            "ADR-0008; Dynamic D was off in 4.3: d_min_roll 50 isn't below d_roll 48"
        )
    );
}

#[test]
fn dshot_idle_value_becomes_motor_idle_and_level_limit_becomes_angle_limit() {
    let meteor = meteor();
    assert_eq!(
        got(&meteor, "motor_idle"),
        is("600", "diff (was dshot_idle_value)")
    );
    assert_eq!(
        got(&meteor, "angle_limit"),
        is("55", "4.3 default (was level_limit)")
    );
    let limited = import(&changed(
        METEOR,
        "set d_min_pitch = 49",
        "set d_min_pitch = 49\nset level_limit = 45",
    ));
    assert_eq!(
        got(&limited, "angle_limit"),
        is("45", "diff (was level_limit)")
    );
    // Nothing reads Angle yet (#51), so it stays under "Not simulated yet".
    assert_eq!(
        limited.setting("angle_limit").unwrap().written,
        Where::NotSimulatedYet
    );
}

#[test]
fn a_setting_the_diff_leaves_out_takes_its_own_versions_default() {
    // Basis: Source (4.3.0 and 4.4.0's resetPidProfile: pid_at_min_throttle
    // ON, motor_output_limit 100, feedforward_smooth_factor 25, where
    // 2026.6.2's is 65; 4.4.0's anti_gravity_gain 80).
    let meteor = meteor();
    assert_eq!(got(&meteor, "pid_at_min_throttle"), is("ON", "4.3 default"));
    assert_eq!(got(&meteor, "motor_output_limit"), is("100", "4.3 default"));
    assert_eq!(
        got(&meteor, "feedforward_smooth_factor"),
        is("25", "4.3 default")
    );
    assert_eq!(got(&meteor, "feedforward_averaging"), is("2_POINT", "diff"));
    assert_eq!(
        got(&meteor, "yaw_motors_reversed"),
        is("OFF", "4.3 default")
    );
    let cetus = cetus();
    assert_eq!(got(&cetus, "anti_gravity_gain"), is("80", "4.4 default"));
    assert_eq!(got(&cetus, "feedforward_smooth_factor"), is("30", "diff"));
}

#[test]
fn settings_4_3_lacked_take_adr_0008s_values() {
    // Basis: Rule (ADR-0008: yaw hold off, low-throttle TPA off and Angle's
    // earth reference off), and #26 (4.3's Crash Flip had no rate fade).
    let meteor = meteor();
    for (name, value) in [
        ("feedforward_yaw_hold_gain", "0"),
        ("tpa_low_rate", "0"),
        ("angle_earth_ref", "0"),
        ("crashflip_rate", "0"),
    ] {
        assert_eq!(got(&meteor, name), is(value, "ADR-0008"), "{name}");
    }
    // The Flight Controller already knows low-throttle TPA, so the Tune
    // spells it out, flown as off (#50), and reads Crash Flip's rate fade
    // (#54), so the Tune sets it under its tab. It doesn't know the others
    // yet; the Tune spells them out all the same, under "Not simulated yet".
    let tpa_low = meteor.setting("tpa_low_rate").unwrap();
    assert_eq!(tpa_low.written, Where::FlownAsOff);
    assert_eq!(tpa_low.note, "not simulated yet (#50)");
    assert_eq!(
        meteor.setting("crashflip_rate").unwrap().written,
        Where::UnderItsTab
    );
    for name in ["feedforward_yaw_hold_gain", "angle_earth_ref"] {
        assert_eq!(
            meteor.setting(name).unwrap().written,
            Where::NotSimulatedYet
        );
    }
}

#[test]
fn where_no_value_behaves_like_4_3_the_tune_takes_2026_6s_default() {
    // Basis: Rule (ADR-0008's consequences: 4.3's anti-gravity worked
    // differently, so 2026.6's punch-out boost stays weaker).
    let meteor = meteor();
    assert_eq!(
        got(&meteor, "anti_gravity_gain"),
        is("80", "2026.6 default")
    );
    assert_eq!(
        got(&meteor, "feedforward_yaw_hold_time"),
        is("100", "2026.6 default")
    );
    let boosted = import(&changed(
        METEOR,
        "set d_min_pitch = 49",
        "set d_min_pitch = 49\nset anti_gravity_gain = 5000",
    ));
    assert_eq!(
        got(&boosted, "anti_gravity_gain"),
        is("80", "2026.6 default")
    );
    assert!(
        boosted
            .left_out
            .iter()
            .any(|l| l.text == "set anti_gravity_gain = 5000"
                && matches!(l.why, Why::Retired(why) if why.contains("worked differently")))
    );
}

#[test]
fn the_i_limit_4_3_called_iterm_limit_becomes_2026_6s_iterm_windup() {
    // Basis: Source (the Betaflight research §3: 4.3's I limit was
    // iterm_limit, 400; 2026.6's is iterm_windup percent of pidsum_limit, 500).
    let meteor = meteor();
    assert_eq!(
        got(&meteor, "iterm_windup"),
        is(
            "80",
            "ADR-0008; 4.3's iterm_limit 400 is 80% of pidsum_limit 500 (yaw's I limit becomes 320, was 400)"
        )
    );
    let lower = import(&changed(
        METEOR,
        "set d_min_pitch = 49",
        "set d_min_pitch = 49\nset iterm_limit = 300\nset iterm_windup = 70",
    ));
    assert_eq!(got(&lower, "iterm_windup").0, "60");
    // 4.3's own iterm_windup meant something else, so it's left out.
    assert!(
        lower
            .left_out
            .iter()
            .any(|l| l.text == "set iterm_windup = 70" && matches!(l.why, Why::Retired(_)))
    );
}

#[test]
fn the_stick_boost_of_4_3s_dynamic_d_becomes_d_max_advance_7() {
    // Basis: Source (#21's resolution: Dynamic D 37/≈7; the Betaflight
    // research §6.3: before 2025.12 the stick boost was d_max_gain ×
    // d_max_advance ÷ 100).
    let meteor = meteor();
    assert_eq!(got(&meteor, "d_max_gain"), is("37", "4.3 default"));
    assert_eq!(
        got(&meteor, "d_max_advance"),
        is(
            "7",
            "ADR-0008; 4.3's stick boost: d_max_gain 37 × d_max_advance 20 ÷ 100"
        )
    );
}

#[test]
fn the_tpa_4_3_kept_in_its_rate_profile_comes_from_the_active_rate_profile() {
    // Basis: Source (4.3.0 kept tpa_rate in its rate profile; the Meteor sets
    // 70 in rate profile 0; 4.4 moved it to the PID profile, where the Cetus
    // sets 70).
    let meteor = meteor();
    let tpa = meteor.setting("tpa_rate").unwrap();
    assert_eq!((tpa.value.as_str(), tpa.mark.as_str()), ("70", "diff"));
    assert_eq!(tpa.line, Some(143));
    assert_eq!(got(&cetus(), "tpa_rate"), is("70", "diff"));
}

#[test]
fn only_the_active_pid_profile_is_read() {
    // Basis: Rule (#21: only the active PID profile; the last `profile`
    // line selects it).
    let two = changed(METEOR, "profile 1\n", "profile 1\n\nset p_roll = 77\n");
    let first = import(&two);
    assert_eq!(first.profile, 0);
    assert_eq!(got(&first, "p_roll"), is("40", "diff"));
    assert!(
        first
            .left_out
            .iter()
            .any(|l| l.text == "set p_roll = 77" && l.why == Why::OtherProfile)
    );
    let second = import(&changed(
        &two,
        "# restore original profile selection\nprofile 0",
        "# restore original profile selection\nprofile 1",
    ));
    assert_eq!(second.profile, 1);
    assert_eq!(got(&second, "p_roll"), is("77", "diff"));
    // Profile 1 sets nothing else, so the rest are 4.3's defaults.
    assert_eq!(got(&second, "p_pitch"), is("47", "4.3 default"));
    assert!(
        second
            .left_out
            .iter()
            .any(|l| l.text == "set p_roll = 40" && l.why == Why::OtherProfile)
    );
}

#[test]
fn the_simplified_slider_lines_are_ignored() {
    // Basis: Rule (#21: the final numbers are read; the firmware never
    // re-applies the sliders).
    let meteor = meteor();
    let sliders: Vec<&str> = meteor
        .left_out
        .iter()
        .filter(|l| l.why == Why::Slider)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(sliders.len(), 10);
    assert!(sliders.iter().all(|s| s.starts_with("set simplified_")));
    let text = meteor.tune_txt("Whoop 65", "meteor");
    assert!(!text.contains("simplified_"));
}

#[test]
fn hardware_only_settings_are_dropped() {
    // Basis: Rule (#21: OSD, VTX, LEDs, ports, beeper and expresslrs_* are
    // dropped).
    for import in [meteor(), cetus()] {
        let text = import.tune_txt("Quad", "export");
        for name in [
            "osd_vbat_pos",
            "vtx_band",
            "acc_calibration",
            "gyro_1_sensor_align",
            "name =",
        ] {
            assert!(!text.contains(name), "{name} in\n{text}");
        }
    }
    let cetus = cetus();
    for name in [
        "expresslrs_uid",
        "expresslrs_rate_index",
        "craft_name",
        "debug_mode",
    ] {
        assert!(
            cetus
                .left_out
                .iter()
                .any(|l| l.why == Why::HardwareOnly && l.text.starts_with(&format!("set {name} "))),
            "{name}"
        );
    }
    // The lines that aren't `set` lines don't reach a Tune either: the aux
    // lines and the rate profiles belong to the pilot (ADR-0015).
    let meteor = meteor();
    assert!(
        meteor
            .left_out
            .iter()
            .any(|l| l.text == "aux 0 0 0 1700 2100 0 0" && l.why == Why::NotASetting)
    );
    assert!(
        cetus
            .left_out
            .iter()
            .any(|l| l.text == "set roll_srate = 65" && l.why == Why::RateProfile)
    );
}

#[test]
fn settings_not_simulated_yet_stay_under_their_own_heading() {
    // Basis: Rule (ADR-0015, #21: RPM filter, notches and the rest stay in
    // the Tune, listed as not simulated yet).
    let meteor = meteor();
    let text = meteor.tune_txt("Whoop 65", "meteor");
    let (_, after) = text
        .split_once("\n# Not simulated yet. ")
        .expect("a Not simulated yet heading");
    let (known, after) = after
        .split_once("\n# Not simulated yet either: settings the Flight Controller doesn't know\n")
        .expect("a heading for the settings the Flight Controller doesn't know");
    let (rest, unknown) = after
        .split_once(
            "\n# Not simulated yet, and not known to OpenDrone: the other settings the diff sets.\n",
        )
        .expect("a heading for the settings the translator doesn't know");
    // Every setting the Flight Controller knows but flies as off, from the
    // diff, 4.3's defaults or ADR-0008, with its ticket, in its order.
    let later = Tune::not_simulated_yet();
    let lines = set_lines(known);
    assert_eq!(
        lines.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>(),
        later.iter().map(|(n, _)| *n).collect::<Vec<_>>()
    );
    for ((name, value, mark), (_, ticket)) in lines.iter().zip(&later) {
        assert!(
            mark.ends_with(&format!("; not simulated yet ({ticket})")),
            "{name} {value} {mark}"
        );
    }
    assert!(known.contains(
        "set dyn_notch_count = 2                # diff; not simulated yet (waits for gyro noise, #21)"
    ));
    assert!(
        known.contains(
            "set rc_smoothing = ON                  # 4.3 default; not simulated yet (#49)"
        )
    );
    // Then every other setting the translator knows, spelled out, ADR-0008's
    // values among them.
    for line in [
        "set feedforward_jitter_factor = 9      # diff",
        "set feedforward_smooth_factor = 25     # 4.3 default",
        "set feedforward_yaw_hold_gain = 0      # ADR-0008",
        "set angle_earth_ref = 0                # ADR-0008; percent",
        "set d_max_advance = 7                  # ADR-0008;",
        "set failsafe_switch_mode = STAGE1      # 4.3 default",
        "set blackbox_sample_rate = 1/2         # diff",
        "set dyn_notch_q = 350                  # diff",
    ] {
        assert!(rest.contains(line), "{line} not under Not simulated yet");
    }
    let known_rows = table::SETTINGS.len();
    assert_eq!(
        set_lines(&text).len(),
        known_rows + 2,
        "every row of the table, and the diff's two unknown settings"
    );
    // Last, the settings the diff sets that the translator doesn't know.
    assert_eq!(
        set_lines(unknown)
            .iter()
            .map(|(n, v, m)| format!("{n} = {v} # {m}"))
            .collect::<Vec<_>>(),
        [
            "dshot_bidir = ON # diff",
            "vbat_max_cell_voltage = 435 # diff"
        ]
    );
    // A setting 2026.6 has no counterpart for is left out, with the reason.
    assert!(
        meteor
            .left_out
            .iter()
            .any(|l| l.text == "set min_throttle = 1070"
                && matches!(l.why, Why::Retired(why) if why.contains("motor_idle")))
    );
}

#[test]
fn every_line_carries_its_adr_0015_mark() {
    // Basis: Rule (ADR-0015: `diff`, `4.3 default`, `ADR-0008`, `(was …)`,
    // `hand-set: <reason>`; a note may follow a `;`).
    let known = |mark: &str| {
        let head = mark.split(';').next().unwrap().trim();
        let head = head.split(" (was ").next().unwrap();
        head == "diff"
            || head == "ADR-0008"
            || ["4.3 default", "4.4 default", "2026.6 default"].contains(&head)
    };
    for import in [meteor(), cetus()] {
        let text = import.tune_txt("Quad", "export");
        let lines = set_lines(&text);
        assert!(lines.len() > 40, "{text}");
        for (name, _, mark) in lines {
            assert!(known(&mark), "{name}'s mark \"{mark}\"");
        }
        assert!(
            text.lines()
                .all(|l| l.is_empty() || l.starts_with('#') || l.starts_with("set "))
        );
    }
}

#[test]
fn diff_diff_all_and_dump_from_4_3_or_newer_are_accepted() {
    // A `diff` lists the active profiles only, without `defaults nosave`; a
    // `dump` lists every setting. Both name the same Flight Controller
    // settings here.
    let diff = changed(METEOR, "# diff all", "# diff");
    let diff = changed(&diff, "defaults nosave\n", "");
    let dump = changed(METEOR, "# diff all", "# dump all");
    let dump = changed(
        &dump,
        "set p_yaw = 35",
        "set p_yaw = 35\nset i_roll = 64\nset pidsum_limit = 500",
    );
    let reads = |import: &TuneImport| -> Vec<(String, String)> {
        import
            .settings
            .iter()
            .filter(|s| s.written == Where::UnderItsTab)
            .map(|s| (s.name.clone(), s.value.clone()))
            .collect()
    };
    let all = reads(&meteor());
    for text in [&diff, &dump] {
        let import = import(text);
        assert_eq!(reads(&import), all);
    }
    assert_eq!(import(&diff).command.as_deref(), Some("diff"));
    let dump = import(&dump);
    assert_eq!(dump.command.as_deref(), Some("dump all"));
    assert_eq!(got(&dump, "pidsum_limit"), is("500", "diff"));
    assert!(dump.tune_txt("Quad", "x").contains("not in the dump"));
}

#[test]
fn betaflight_older_than_4_3_is_refused_with_a_clear_message() {
    // Basis: Rule (ADR-0008: the importer accepts Betaflight 4.3 and newer
    // only).
    let old = changed(METEOR, "(S411) 4.3.0 Jun", "(S411) 4.2.11 Jun");
    assert_eq!(
        refusal(&old),
        "This is Betaflight 4.2.11, which is older than 4.3: OpenDrone imports Betaflight 4.3 or newer (ADR-0008)."
    );
}

#[test]
fn a_betaflight_newer_than_2026_6_or_one_that_was_never_released_is_refused_too() {
    // Newer than the Betaflight our Flight Controller copies.
    let newer = changed(METEOR, "(S411) 4.3.0 Jun", "(S411) 2026.12.0 Jun");
    assert!(
        refusal(&newer).starts_with("This is Betaflight 2026.12.0, which is newer than 2026.6")
    );
    let unreleased = changed(METEOR, "(S411) 4.3.0 Jun", "(S411) 4.6.0 Jun");
    assert_eq!(
        refusal(&unreleased),
        "This is Betaflight 4.6.0, which isn't a release OpenDrone knows: it imports 4.3, 4.4, 4.5, 2025.12 and 2026.6."
    );
}

#[test]
fn a_diff_all_from_betaflight_4_5_imports_with_4_5s_own_angle_and_low_throttle_tpa() {
    // Basis: Source (4.5.0's resetPidProfile: Angle and Horizon rebuilt, with
    // angle_limit 60 and low-throttle TPA at 20%; still d_min, iterm_limit
    // and no yaw hold). The Cetus X's settings all exist in 4.5 under the same
    // names, so its export stands in for one.
    let text = changed(CETUS, "(S411) 4.4.0 Oct", "(S411) 4.5.0 Oct");
    let text = changed(
        &text,
        "set tpa_rate = 70",
        "set tpa_rate = 70\nset dyn_idle_start_increase = 40",
    );
    let four_five = import(&text);
    assert_eq!(four_five.family, Family::V4_5);
    assert_eq!(four_five.problems, Vec::<String>::new());
    assert_eq!(got(&four_five, "d_roll"), is("48", "diff (was d_min_roll)"));
    assert_eq!(got(&four_five, "angle_limit"), is("60", "4.5 default"));
    assert_eq!(
        got(&four_five, "horizon_level_strength"),
        is("75", "4.5 default")
    );
    assert_eq!(got(&four_five, "tpa_low_rate"), is("20", "4.5 default"));
    assert_eq!(
        got(&four_five, "feedforward_yaw_hold_gain"),
        is("0", "ADR-0008")
    );
    assert_eq!(
        got(&four_five, "motor_pwm_protocol"),
        is("DSHOT300", "diff")
    );
    assert_eq!(
        got(&four_five, "failsafe_recovery_delay"),
        is("5", "4.5 default")
    );
    assert_eq!(got(&four_five, "iterm_windup").0, "80");
    assert!(
        four_five
            .left_out
            .iter()
            .any(|l| l.text == "set dyn_idle_start_increase = 40"
                && matches!(l.why, Why::Retired(_)))
    );
}

#[test]
fn a_diff_all_from_betaflight_2025_12_imports_under_its_own_names() {
    // Basis: Source (2025.12.1's resetPidProfile: d is the base and d_max the
    // peak, iterm_windup is the I limit, and Dynamic D's defaults are still
    // 37/20, where 2026.6.2's are 0/35).
    let text = "# diff all\n# version\n# Betaflight / STM32F405 (S405) 2025.12.1 Dec  1 2025 / 12:00:00 (abcdef0) MSP API: 1.47\nbatch start\ndefaults nosave\nset motor_idle = 450\nset transient_throttle_limit = 5\nprofile 0\nset d_roll = 35\nset d_max_roll = 45\nset iterm_windup = 70\nrateprofile 0\nsave\n";
    let latest = import(text);
    assert_eq!(latest.family, Family::V2025_12);
    assert_eq!(got(&latest, "d_roll"), is("35", "diff"));
    assert_eq!(got(&latest, "d_max_roll"), is("45", "diff"));
    assert_eq!(got(&latest, "iterm_windup"), is("70", "diff"));
    assert_eq!(got(&latest, "motor_idle"), is("450", "diff"));
    assert_eq!(got(&latest, "d_max_gain"), is("37", "2025.12 default"));
    assert_eq!(got(&latest, "d_max_advance"), is("20", "2025.12 default"));
    assert_eq!(
        got(&latest, "feedforward_yaw_hold_gain"),
        is("15", "2025.12 default")
    );
    assert_eq!(
        got(&latest, "motor_pwm_protocol"),
        is("DSHOT600", "2025.12 default")
    );
    assert!(
        latest
            .left_out
            .iter()
            .any(|l| l.text == "set transient_throttle_limit = 5"
                && matches!(l.why, Why::Retired(_)))
    );
}

#[test]
fn an_export_without_its_version_line_or_pid_profile_or_from_another_firmware_is_refused() {
    let unversioned = changed(
        METEOR,
        "# Betaflight / STM32F411 (S411) 4.3.0 Jun 14 2022 / 00:48:04 (229ac66) MSP API: 1.44\n",
        "",
    );
    assert!(refusal(&unversioned).starts_with("There's no `# version` line"));
    let inav = changed(METEOR, "# Betaflight / ", "# INAV / ");
    assert!(refusal(&inav).starts_with("This is INAV's CLI output, not Betaflight's"));
    let mut master_only = String::new();
    for line in METEOR.lines() {
        if line.starts_with("profile ") || line.starts_with("rateprofile ") {
            break;
        }
        master_only.push_str(line);
        master_only.push('\n');
    }
    assert!(refusal(&master_only).starts_with(
        "There's no `profile` line, so this export holds none of Betaflight's PID profiles"
    ));
}

#[test]
fn the_cetus_xs_diff_all_from_betaflight_4_4_imports_too() {
    // It doesn't ship as a Quad; this proves a 4.4 export imports.
    let cetus = cetus();
    assert_eq!(cetus.family, Family::V4_4);
    assert_eq!(cetus.craft_name.as_deref(), Some("Cetus X"));
    assert_eq!(cetus.configured, None);
    assert_eq!(cetus.built.as_deref(), Some("Oct 9 2023"));
    assert_eq!(cetus.problems, Vec::<String>::new());
    assert_eq!(got(&cetus, "p_roll"), is("58", "diff"));
    assert_eq!(got(&cetus, "d_roll"), is("48", "diff (was d_min_roll)"));
    assert_eq!(got(&cetus, "d_max_roll"), is("52", "diff (was d_roll)"));
    assert_eq!(
        got(&cetus, "motor_idle"),
        is("1200", "diff (was dshot_idle_value)")
    );
    assert_eq!(got(&cetus, "yaw_motors_reversed"), is("ON", "diff"));
    assert_eq!(got(&cetus, "iterm_windup").0, "80");
    let text = cetus.tune_txt("Cetus X", "cetus-x.diff-all.txt");
    assert!(text.contains("# firmware built Oct 9 2023 (cetus-x.diff-all.txt),\n"));
    let tune = Tune::read(
        set_lines(&text)
            .iter()
            .map(|(n, v, _)| (n.as_str(), v.as_str())),
    );
    assert!(tune.is_ok(), "{tune:?}");
}

/// The Meteor's `diff all` as `diff all bare` prints it: no batch, no
/// `defaults nosave`, and no lines selecting the active profiles again.
fn meteor_all_bare() -> String {
    let text = changed(METEOR, "# diff all\n", "# diff all bare\n");
    let text = changed(&text, "batch start\n", "");
    let text = changed(&text, "defaults nosave\n", "");
    let text = changed(
        &text,
        "# restore original profile selection\nprofile 0\n",
        "",
    );
    changed(
        &text,
        "# restore original rateprofile selection\nrateprofile 0\n",
        "",
    )
}

#[test]
fn a_4_3_or_4_4_diff_not_taken_bare_is_imported_with_a_note_about_the_boards_defaults() {
    // Basis: Source (4.3.0's cli.c:6245 and 4.4.0's cli.c:6222: without
    // `bare`, a `diff` first applies the board's own defaults).
    for import in [meteor(), cetus()] {
        assert_eq!(import.warnings.len(), 1);
        assert!(
            import.warnings[0].starts_with(
                "In Betaflight 4.3 and 4.4, a `diff` without `bare` lists what differs from the board's own defaults, not Betaflight's:"
            ),
            "{:?}",
            import.warnings
        );
    }
    assert!(
        meteor()
            .report()
            .contains("- Note: In Betaflight 4.3 and 4.4, a `diff` without `bare`")
    );
    let bare = changed(METEOR, "# diff all\n", "# diff bare\n");
    assert_eq!(import(&bare).warnings, Vec::<String>::new());
}

#[test]
fn a_dump_or_a_diff_from_4_5_or_newer_is_imported_without_that_note() {
    // Basis: Source (a `dump` lists every value; from 4.5.0, cli.c's
    // backupAndResetConfigs only resets to the firmware's defaults, with a
    // board's defaults built into its firmware).
    let dump = changed(METEOR, "# diff all\n", "# dump all\n");
    assert_eq!(import(&dump).warnings, Vec::<String>::new());
    let four_five = changed(CETUS, "(S411) 4.4.0 Oct", "(S411) 4.5.0 Oct");
    assert_eq!(import(&four_five).warnings, Vec::<String>::new());
    let latest = changed(METEOR, "(S411) 4.3.0 Jun", "(S411) 2026.6.2 Jun");
    let latest = changed(
        &latest,
        "set dshot_idle_value = 600",
        "set motor_idle = 600",
    );
    assert_eq!(import(&latest).warnings, Vec::<String>::new());
}

#[test]
fn an_export_that_lists_every_profile_without_selecting_one_is_refused() {
    // Basis: Source (4.3.0's cli.c:6371 and 2026.6.2's cli.c:8123: only
    // without `bare` does `diff all` select the active profile again).
    let expected =
        "This export lists each of Betaflight's PID profiles but doesn't say which one is active";
    assert!(refusal(&meteor_all_bare()).starts_with(expected));
    // Not from the echoed command alone: with the echo gone too.
    let unechoed = changed(&meteor_all_bare(), "# diff all bare\n", "");
    assert!(refusal(&unechoed).starts_with(expected));
}

#[test]
fn a_failsafe_procedure_the_flight_controller_doesnt_simulate_yet_is_noted() {
    // Basis: Rule (#21: a Tune set to LAND or GPS Rescue imports with a "not
    // simulated yet" note and flies DROP).
    let landing = import(&changed(
        METEOR,
        "set small_angle = 180",
        "set small_angle = 180\nset failsafe_procedure = AUTO-LAND",
    ));
    let procedure = landing.setting("failsafe_procedure").unwrap();
    assert_eq!(procedure.value, "AUTO-LAND");
    assert_eq!(procedure.note, "not simulated yet (#21)");
    assert!(
        landing
            .warnings
            .iter()
            .any(|w| w.starts_with("`failsafe_procedure` is AUTO-LAND, which isn't simulated yet")),
        "{:?}",
        landing.warnings
    );
}

#[test]
fn a_2026_6_export_reads_its_active_battery_profile_with_the_rest() {
    // Basis: Source (2026.6.2's cli.c lists `battery_profile` sections after
    // the rate profiles and selects the active one again at the end).
    let text = "# diff all\n# version\n# Betaflight / STM32H743 (SH74) 2026.6.2 Jun  1 2026 / 12:00:00 (abcdef0) MSP API: 1.48\nbatch start\ndefaults nosave\nprofile 0\nset p_roll = 50\nprofile 0\nrateprofile 0\nset roll_srate = 80\nbattery_profile 0\nset vbat_max_cell_voltage = 435\nbattery_profile 1\nset vbat_max_cell_voltage = 420\nrateprofile 0\nbattery_profile 0\nsave\n";
    let latest = import(text);
    assert_eq!(got(&latest, "p_roll"), is("50", "diff"));
    assert_eq!(got(&latest, "vbat_max_cell_voltage"), is("435", "diff"));
    assert!(
        latest
            .left_out
            .iter()
            .any(|l| l.text == "set vbat_max_cell_voltage = 420" && l.why == Why::OtherProfile)
    );
    assert!(
        latest
            .left_out
            .iter()
            .any(|l| l.text == "set roll_srate = 80" && l.why == Why::RateProfile)
    );
}

#[test]
fn a_4_3_failsafe_recovery_delay_below_2_imports_as_2_as_4_3_waited_at_least_200_ms() {
    // Basis: Source (4.3.0's failsafe.c:94-98 waits at least 200 ms and its
    // range is 0 to 200; 2026.6.2 waits at least 100 ms and takes 1 to 200).
    let quick = import(&changed(
        METEOR,
        "set small_angle = 180",
        "set small_angle = 180\nset failsafe_recovery_delay = 0",
    ));
    assert_eq!(
        got(&quick, "failsafe_recovery_delay"),
        is(
            "2",
            "ADR-0008; 4.3 waited at least 200 ms, so its failsafe_recovery_delay 0 acted as 2"
        )
    );
    assert_eq!(quick.problems, Vec::<String>::new());
    assert_eq!(
        got(&meteor(), "failsafe_recovery_delay"),
        is("10", "4.3 default")
    );
}

#[test]
fn a_quad_with_no_craft_name_has_none_in_its_tune() {
    // Basis: Rule. Betaflight writes "-" for a quad with no name.
    let text = changed(METEOR, "# name: Meteor65 pro", "# name: -");
    let text = changed(&text, "set name = Meteor65 pro", "set name = -");
    let nameless = import(&text);
    assert_eq!(nameless.craft_name, None);
    assert!(
        nameless
            .tune_txt("Quad", "x")
            .contains("# Imported from the diff all: Betaflight 4.3.0,\n")
    );
}

#[test]
fn a_value_the_flight_controller_cant_read_is_named() {
    // 4.3's own default protocol was DISABLED: each board chose one.
    let no_protocol = changed(METEOR, "set motor_pwm_protocol = DSHOT300\n", "");
    let import = import(&no_protocol);
    assert_eq!(
        got(&import, "motor_pwm_protocol"),
        is("DISABLED", "4.3 default")
    );
    assert_eq!(import.problems.len(), 1);
    assert!(
        import.problems[0]
            .starts_with("`motor_pwm_protocol` is DISABLED, but the simulated ESCs run Bluejay"),
        "{:?}",
        import.problems
    );
}

#[test]
fn the_translator_knows_every_setting_the_flight_controller_reads() {
    // Basis: Rule (ADR-0015: a Tune spells out every setting the Flight
    // Controller reads, and the ones it knows but flies as off for now, so
    // the importer must write each one).
    let later = Tune::not_simulated_yet();
    for name in Tune::settings()
        .into_iter()
        .chain(later.iter().map(|(name, _)| *name))
    {
        assert!(
            table::SETTINGS.iter().any(|s| s.name == name),
            "the table has no row for {name}"
        );
    }
    // And each row says where its value comes from in each version, with
    // 2026.6's own default.
    for setting in table::SETTINGS {
        assert!(
            matches!(setting.source(Family::V2026_6), Source::Same(d) if !d.is_empty()),
            "{}",
            setting.name
        );
    }
}

#[test]
fn a_bare_2026_6_diff_gives_betaflight_2026_6_2s_defaults_as_the_freestyle_5s_tune_holds_them() {
    // Basis: Source (the Freestyle 5″'s Tune is 2026.6.2's firmware
    // defaults, each checked against its source by the Pack checker's
    // built-in checks), so the table's 2026.6 column agrees with it.
    let bare = "# diff all\n\n# version\n# Betaflight / STM32H743 (SH74) 2026.6.2 Jun  1 2026 / 12:00:00 (abcdef0) MSP API: 1.48\n\nbatch start\ndefaults nosave\n\nprofile 0\n\nrateprofile 0\n\nsave\n";
    let import = import(bare);
    assert_eq!(import.family, Family::V2026_6);
    assert_eq!(import.problems, Vec::<String>::new());
    let five = set_lines(FREESTYLE_5_TUNE);
    let tune = import.tune_txt("Freestyle 5″", "bare");
    let imported = set_lines(&tune);
    assert!(
        imported
            .iter()
            .all(|(_, _, mark)| mark.starts_with("2026.6 default"))
    );
    // The 5″ spells out what the Flight Controller reads and flies as off;
    // the import writes those first, in the same order, then the rest.
    let values = |lines: &[(String, String, String)]| -> Vec<(String, String)> {
        lines
            .iter()
            .map(|(n, v, _)| (n.clone(), v.clone()))
            .collect()
    };
    assert_eq!(values(&imported[..five.len()]), values(&five));
}
