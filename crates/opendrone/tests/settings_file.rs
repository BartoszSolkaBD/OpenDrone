//! Readable checks for the settings file: one pilot per install, one file,
//! every setting kept in it, Quads and Maps named by id. Basis: Rule, from
//! the spec (#37 "Settings file") and ADR-0003 (where settings live). Angles
//! in degrees, the Packet Rate in Hz.

use std::path::PathBuf;

use opendrone::camera::controls::QuadCamera;
use opendrone::camera::lens::Aspect;
use opendrone::camera::look::VideoLook;
use opendrone::settings::Settings;
use opendrone::settings_file::{
    BROKEN_FILE_NAME, FILE_NAME, FlyingDevice, PilotSettings, Preset, SettingsFile, TextSize,
};
use opendrone_input::profile::FlightMode;
use opendrone_sim::PacketRate;

/// A folder of its own for one test's settings file.
fn folder(test: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "opendrone-settings-file-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Settings with something changed in every section.
fn changed() -> PilotSettings {
    let mut settings = PilotSettings {
        first_launch_done: true,
        preset: Preset::Pro,
        quad: "opendrone/freestyle-5".to_string(),
        map: "test/flat-floor".to_string(),
        text_size: TextSize::Larger,
        reduce_motion: true,
        flight_mode: FlightMode::Angle,
        input_smoothing: true,
        endless_battery: true,
        auto_arm: true,
        packet_rate: PacketRate::from_hz(500).unwrap(),
        flying_device: FlyingDevice::Named("Radiomaster Pocket".to_string()),
        ..PilotSettings::default()
    };
    settings.camera.look = VideoLook::Digital;
    settings.camera.aspect = Aspect::SixteenNine;
    settings.camera.set_quad(
        "opendrone/freestyle-5",
        QuadCamera {
            tilt: 42.0,
            fov: 133.5,
        },
    );
    settings.camera.set_quad(
        "opendrone/whoop-65",
        QuadCamera {
            tilt: 12.0,
            fov: 150.0,
        },
    );
    settings
}

#[test]
fn a_settings_file_round_trips_every_setting() {
    let mut file = SettingsFile::in_folder(folder("round-trip"));
    let settings = changed();
    file.save(&settings).unwrap();
    let loaded = file.load();
    assert!(loaded.existed);
    assert_eq!(loaded.problems, Vec::<String>::new());
    assert_eq!(loaded.settings, settings);
}

#[test]
fn a_new_install_has_no_file_and_starts_the_whoop_65_on_the_skate_park() {
    // Basis: Rule (#68: a new install starts with the Whoop 65 on the Skate
    // Park, on the Beginner Preset, with every Assist off).
    let mut file = SettingsFile::in_folder(folder("new-install"));
    let loaded = file.load();
    assert!(!loaded.existed);
    let settings = loaded.settings;
    assert!(!settings.first_launch_done);
    assert_eq!(settings.quad, "opendrone/whoop-65");
    assert_eq!(settings.map, "opendrone/skate-park");
    assert_eq!(settings.preset, Preset::Beginner);
    assert!(!settings.auto_arm && !settings.input_smoothing && !settings.endless_battery);
    assert_eq!(settings.packet_rate.hz(), 250);
    assert_eq!(settings.flight_mode, FlightMode::Acro);
}

#[test]
fn quads_and_maps_are_named_by_id_in_the_file() {
    // Basis: Rule (#16 §2: ids in the settings file).
    let file = SettingsFile::in_folder(folder("ids"));
    file.save(&changed()).unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(text.contains("quad = \"opendrone/freestyle-5\""), "{text}");
    assert!(text.contains("map = \"test/flat-floor\""), "{text}");
    assert!(
        text.contains("[camera.quads.\"opendrone/whoop-65\"]"),
        "{text}"
    );
}

#[test]
fn a_setting_missing_from_an_older_file_keeps_its_default() {
    // Basis: Rule: a file from an older OpenDrone still reads.
    let folder = folder("older");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[pilot]\nfirst_launch_done = true\npreset = \"intermediate\"\n",
    )
    .unwrap();
    let loaded = SettingsFile::in_folder(folder).load();
    assert_eq!(loaded.problems, Vec::<String>::new());
    let expected = PilotSettings {
        first_launch_done: true,
        preset: Preset::Intermediate,
        ..PilotSettings::default()
    };
    assert_eq!(loaded.settings, expected);
}

#[test]
fn a_value_the_game_does_not_accept_is_ignored_and_said_so() {
    let folder = folder("bad-value");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[accessibility]\ntext_size = 130\nreduce_motion = true\n[flying]\npacket_rate = 251\nflight_mode = \"Angle\"\n[camera.quads.\"opendrone/whoop-65\"]\ntilt = 95.0\nfov = 160.0\n",
    )
    .unwrap();
    let loaded = SettingsFile::in_folder(folder).load();
    // What was fine is kept; what was not keeps its default.
    assert!(loaded.settings.reduce_motion);
    assert_eq!(loaded.settings.flight_mode, FlightMode::Angle);
    assert_eq!(loaded.settings.text_size, TextSize::Normal);
    assert_eq!(loaded.settings.packet_rate.hz(), 250);
    assert_eq!(loaded.settings.camera, PilotSettings::default().camera);
    assert_eq!(loaded.problems.len(), 3, "{:?}", loaded.problems);
    assert!(loaded.problems[0].starts_with("accessibility.text_size should be 100, 125 or 150"));
    assert!(loaded.problems[1].starts_with("flying.packet_rate should be 50, 100"));
    assert!(loaded.problems[2].contains("opendrone/whoop-65"));
}

#[test]
fn a_key_this_version_does_not_know_stays_in_the_file_when_it_saves() {
    // Basis: Rule: a newer OpenDrone's settings aren't lost.
    let folder = folder("unknown-keys");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[pilot]\npreset = \"pro\"\nfavourite_colour = \"orange\"\n[graphics]\nresolution_scale = 80\n",
    )
    .unwrap();
    let mut file = SettingsFile::in_folder(folder);
    let mut settings = file.load().settings;
    settings.reduce_motion = true;
    file.save(&settings).unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(text.contains("favourite_colour = \"orange\""), "{text}");
    assert!(text.contains("resolution_scale = 80"), "{text}");
    assert!(text.contains("reduce_motion = true"), "{text}");
    assert!(text.contains("preset = \"pro\""), "{text}");
}

#[test]
fn a_hand_edited_camera_entry_the_game_rejects_stays_in_the_file_when_it_saves() {
    // Basis: Rule (#259: a pilot's own edit is never silently dropped). Angles
    // in degrees.
    let folder = folder("rejected-quad-entry");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[camera.quads.\"opendrone/whoop-65\"]\ntilt = 95.0\nfov = 160.0\n[camera.quads.\"opendrone/freestyle-5\"]\ntilt = 42.0\nfov = 133.5\n[camera.quads.\"opendrone/typo\"]\ntilt = \"steep\"\nfov = 120.0\n",
    )
    .unwrap();
    let mut file = SettingsFile::in_folder(folder);
    let loaded = file.load();
    assert_eq!(loaded.problems.len(), 2, "{:?}", loaded.problems);
    let mut settings = loaded.settings;
    settings.reduce_motion = true;
    file.save(&settings).unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(text.contains("reduce_motion = true"), "{text}");

    // Read again: the two rejected entries are still there as they were, so
    // they are rejected and told again, and the good one is exactly as before.
    let again = file.load();
    assert_eq!(again.problems, loaded.problems);
    assert_eq!(again.settings, settings);
    let kept: toml::Table = text.parse().unwrap();
    let quads = kept["camera"]["quads"].as_table().unwrap();
    assert_eq!(quads["opendrone/whoop-65"]["tilt"].as_float(), Some(95.0));
    assert_eq!(quads["opendrone/whoop-65"]["fov"].as_float(), Some(160.0));
    assert_eq!(quads["opendrone/typo"]["tilt"].as_str(), Some("steep"));
    assert_eq!(quads["opendrone/typo"]["fov"].as_float(), Some(120.0));
    assert_eq!(
        quads["opendrone/freestyle-5"]["tilt"].as_float(),
        Some(42.0)
    );
    assert_eq!(
        quads["opendrone/freestyle-5"]["fov"].as_float(),
        Some(133.5)
    );
}

#[test]
fn a_rejected_camera_entry_is_replaced_once_the_pilot_sets_that_quads_camera() {
    // Basis: Rule: the pilot's new value wins over the entry the game ignored.
    let folder = folder("rejected-then-set");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[camera.quads.\"opendrone/whoop-65\"]\ntilt = 95.0\nfov = 160.0\n",
    )
    .unwrap();
    let mut file = SettingsFile::in_folder(folder);
    let mut settings = file.load().settings;
    settings.camera.set_quad(
        "opendrone/whoop-65",
        QuadCamera {
            tilt: 20.0,
            fov: 150.0,
        },
    );
    file.save(&settings).unwrap();
    let again = file.load();
    assert_eq!(again.problems, Vec::<String>::new());
    assert_eq!(again.settings, settings);
}

#[test]
fn the_pilot_is_told_which_camera_entry_was_ignored_and_why() {
    // Basis: Rule (#259). Angles in degrees.
    let folder = folder("rejected-quad-says-why");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[camera.quads.\"opendrone/whoop-65\"]\ntilt = 95.0\nfov = 160.0\n[camera.quads.\"opendrone/typo\"]\nfov = \"wide\"\n",
    )
    .unwrap();
    let problems = SettingsFile::in_folder(folder).load().problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].contains("opendrone/typo"), "{problems:?}");
    assert!(problems[0].contains("tilt is missing"), "{problems:?}");
    assert!(
        problems[0].contains("fov should be a number"),
        "{problems:?}"
    );
    assert!(problems[0].contains("stays in the file"), "{problems:?}");
    assert!(problems[1].contains("opendrone/whoop-65"), "{problems:?}");
    assert!(
        problems[1].contains("tilt is 95, which is outside 0 to 80 degrees"),
        "{problems:?}"
    );
    assert!(problems[1].contains("stays in the file"), "{problems:?}");
}

#[test]
fn a_file_that_is_not_toml_is_kept_aside_and_the_game_starts_fresh() {
    let folder = folder("broken");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(FILE_NAME), "this is [not toml").unwrap();
    let mut file = SettingsFile::in_folder(folder.clone());
    let loaded = file.load();
    assert!(loaded.existed);
    assert_eq!(loaded.settings, PilotSettings::default());
    assert_eq!(loaded.problems.len(), 1);
    assert!(loaded.problems[0].contains("isn't readable"));
    assert!(loaded.problems[0].contains(BROKEN_FILE_NAME));
    assert_eq!(
        std::fs::read_to_string(folder.join(BROKEN_FILE_NAME)).unwrap(),
        "this is [not toml"
    );
    assert!(!folder.join(FILE_NAME).exists());
}

#[test]
fn a_file_from_a_newer_format_reads_what_it_can_and_says_so() {
    let folder = folder("newer");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 7\n[pilot]\npreset = \"pro\"\n",
    )
    .unwrap();
    let loaded = SettingsFile::in_folder(folder).load();
    assert_eq!(loaded.settings.preset, Preset::Pro);
    assert_eq!(loaded.problems.len(), 1);
    assert!(loaded.problems[0].contains("format 7"));
}

#[test]
fn saving_writes_the_whole_file_in_one_step_and_leaves_no_half_written_file() {
    let mut file = SettingsFile::in_folder(folder("atomic"));
    file.save(&PilotSettings::default()).unwrap();
    file.save(&changed()).unwrap();
    let names: Vec<String> = std::fs::read_dir(file.folder())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec![FILE_NAME.to_string()]);
    assert_eq!(file.load().settings, changed());
}

#[test]
fn saved_flights_go_in_a_folder_beside_the_settings_file() {
    // Basis: Rule (ADR-0003: the game writes the settings file and, beside
    // it, saved flights).
    let file = SettingsFile::in_folder(PathBuf::from("/somewhere/OpenDrone"));
    assert_eq!(
        file.saved_flights_folder(),
        PathBuf::from("/somewhere/OpenDrone/saved-flights")
    );
}

#[test]
fn the_flight_is_set_up_from_the_settings_the_pilot_chose() {
    // Basis: Rule (ADR-0003: Flight Mode, the Assists and the Packet Rate go
    // to the Flight Controller and the Simulation at set-up).
    let settings = Settings::from_pilot(&changed());
    assert_eq!(settings.quad, "opendrone/freestyle-5");
    assert_eq!(settings.map, "test/flat-floor");
    assert_eq!(settings.packet_rate.hz(), 500);
    assert_eq!(settings.flight_mode, FlightMode::Angle);
    assert!(settings.auto_arm && settings.input_smoothing && settings.endless_battery);
    // The physics rate is the alpha's 8 kHz, never the pilot's to change.
    assert_eq!(settings.physics_rate.hz(), 8000);
}

#[test]
fn the_flying_device_is_auto_unless_the_file_names_a_device() {
    // Basis: Rule (#271: Auto by default; a device is saved by its profile's
    // name).
    let folder = folder("flying-device");
    std::fs::create_dir_all(&folder).unwrap();
    let mut file = SettingsFile::in_folder(folder.clone());
    assert_eq!(file.load().settings.flying_device, FlyingDevice::Auto);
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[flying]\nflying_device = \"DualSense\"\n",
    )
    .unwrap();
    let loaded = file.load();
    assert_eq!(loaded.problems, Vec::<String>::new());
    assert_eq!(
        loaded.settings.flying_device,
        FlyingDevice::Named("DualSense".to_string())
    );
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[flying]\nflying_device = \"auto\"\n",
    )
    .unwrap();
    assert_eq!(file.load().settings.flying_device, FlyingDevice::Auto);
}

#[test]
fn a_flying_device_that_is_not_usable_is_ignored_and_said_so() {
    let folder = folder("flying-device-bad");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[flying]\nflying_device = 3\n",
    )
    .unwrap();
    let loaded = SettingsFile::in_folder(folder.clone()).load();
    assert_eq!(loaded.settings.flying_device, FlyingDevice::Auto);
    assert!(loaded.problems[0].starts_with("flying.flying_device should be text"));
    std::fs::write(
        folder.join(FILE_NAME),
        "format = 1\n[flying]\nflying_device = \"  \"\n",
    )
    .unwrap();
    let loaded = SettingsFile::in_folder(folder).load();
    assert_eq!(loaded.settings.flying_device, FlyingDevice::Auto);
    assert!(
        loaded.problems[0].starts_with(
            "flying.flying_device should be auto, or the name of an Input Device profile"
        ),
        "{:?}",
        loaded.problems
    );
}
