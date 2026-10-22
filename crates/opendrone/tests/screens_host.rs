//! Readable checks for how the game carries the screens out: the real
//! systems of the game (the pilot, the screens, the Simulation's stepping and
//! its clock) run in a Bevy app with no window, no renderer and no SDL, and
//! Input Devices' reports are handed over as the input thread would hand
//! them. Time moves 16 ms a frame, exactly. Basis: Rule, from the Screens and
//! navigation deep dive (docs/context/screens.md) and the resolution of #23
//! and #19 §2.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use opendrone::camera::QuadPoses;
use opendrone::camera::controls::CameraSettings;
use opendrone::drawing::WorldDrawing;
use opendrone::pilot::{Pilot, PilotPlugin};
use opendrone::screens::menu_input::MenuInput;
use opendrone::screens::model::Screen;
use opendrone::screens::view::CurrentPage;
use opendrone::screens::{Screens, ScreensPlugin};
use opendrone::set_up::repo_folder;
use opendrone::settings_file::{Loaded, PilotSettings, SettingsFile, TextSize};
use opendrone::stepping::{Flight, SteppingPlugin};
use opendrone_input::controls::PadAxis;
use opendrone_input::profile::Connection;
use opendrone_input::{Batch, DeviceInfo, DeviceState, Raw, SdlId};

const POCKET: SdlId = SdlId(1);
const DUALSENSE: SdlId = SdlId(2);

/// A Radiomaster Pocket over USB, RF off: eight axis channels and the rest
/// as buttons, the throttle stick at the bottom.
fn pocket() -> Vec<Raw> {
    vec![
        Raw::Added {
            device: POCKET,
            info: DeviceInfo {
                name: "EdgeTX Radiomaster Pocket Joystick".to_string(),
                usb_vendor: 0x1209,
                usb_product: 0x4F54,
                sdl_gamepad: true,
                connection: Some(Connection::Usb),
                heartbeat: false,
            },
            state: DeviceState::new(8, 24, false),
        },
        // The throttle rests at the bottom.
        Raw::Axis {
            device: POCKET,
            axis: 2,
            value: -32767,
        },
    ]
}

/// A DualSense over USB with its motion sensors on.
fn dualsense() -> Vec<Raw> {
    vec![Raw::Added {
        device: DUALSENSE,
        info: DeviceInfo {
            name: "DualSense Wireless Controller".to_string(),
            usb_vendor: 0x054C,
            usb_product: 0x0CE6,
            sdl_gamepad: true,
            connection: Some(Connection::Usb),
            heartbeat: true,
        },
        state: DeviceState::new(6, 0, true),
    }]
}

/// A folder of its own for one test's settings file.
fn folder(test: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "opendrone-screens-host-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The game with its screens, one frame at a time.
struct Host {
    app: App,
    settings_folder: PathBuf,
}

impl Host {
    /// A returning pilot, on the Hub, with the settings file in a folder of
    /// this test's own.
    fn returning(test: &str) -> Host {
        Host::with(
            test,
            PilotSettings {
                first_launch_done: true,
                ..PilotSettings::default()
            },
        )
    }

    fn with(test: &str, settings: PilotSettings) -> Host {
        let settings_folder = folder(test);
        let store = SettingsFile::in_folder(settings_folder.clone());
        let loaded = Loaded {
            settings,
            existed: true,
            problems: Vec::new(),
        };
        let packs = common::packs();
        let profiles = packs.input_devices().into_iter().cloned().collect();
        let screens = Screens::new(loaded, Some(store), packs, &repo_folder(), None);
        let plan = screens.first_plan().expect("the first plan flies");
        let pilot = Pilot::without_input_thread(&plan, profiles).with_screens();
        let flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SteppingPlugin, PilotPlugin, ScreensPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .insert_resource(plan)
            .insert_non_send(flight)
            .insert_resource(pilot)
            .insert_resource(screens)
            .init_resource::<QuadPoses>()
            .init_resource::<CameraSettings>()
            // No renderer: the world counts as drawn as soon as it's wanted.
            .add_systems(Update, |mut world: ResMut<WorldDrawing>| {
                let wanted = world.wanted;
                world.set_drawn(wanted);
            });
        let mut host = Host {
            app,
            settings_folder,
        };
        // The first frame runs the start-up systems.
        host.frames(1);
        host
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Frames until `done`, at most `most`.
    fn until(&mut self, most: u32, done: impl Fn(&Host) -> bool) {
        for _ in 0..most {
            if done(self) {
                return;
            }
            self.app.update();
        }
        assert!(done(self), "didn't happen within {most} frames");
    }

    fn screen(&self) -> Screen {
        self.app.world().resource::<Screens>().menus().screen()
    }

    fn page(&self) -> opendrone::screens::model::Page {
        self.app
            .world()
            .resource::<CurrentPage>()
            .0
            .clone()
            .expect("a page has been made")
    }

    fn cursor_label(&self) -> String {
        let page = self.page();
        page.items[page.cursor].label.clone()
    }

    fn pilot(&self) -> &Pilot {
        self.app.world().resource::<Pilot>()
    }

    /// The input thread's clock now.
    fn input_clock(&self) -> Duration {
        self.pilot()
            .input_clock(self.app.world().resource::<Time<Real>>())
    }

    /// Hands the pilot what the input thread would have read now.
    fn devices_report(&mut self, events: Vec<Raw>) {
        let at = self.input_clock();
        self.app
            .world_mut()
            .resource_mut::<Pilot>()
            .hand_over(Batch { at, events });
    }

    fn press(&mut self, input: MenuInput) {
        self.app
            .world_mut()
            .resource_mut::<Pilot>()
            .press_in_menu(input);
        self.frames(1);
    }

    fn simulation_steps(&self) -> u64 {
        self.app
            .world()
            .non_send::<Flight>()
            .simulation()
            .time()
            .ticks()
    }

    fn paused(&self) -> bool {
        self.app.world().resource::<Time<Virtual>>().is_paused()
    }

    fn world_wanted(&self) -> bool {
        self.app.world().resource::<WorldDrawing>().wanted
    }

    fn quad_id(&self) -> String {
        self.app
            .world()
            .resource::<opendrone::set_up::FlightPlan>()
            .quad
            .id
            .clone()
    }

    /// Presses FLY with the keyboard and waits for Free Flight.
    fn fly(&mut self) {
        self.press(MenuInput::Select);
        self.until(400, |h| h.screen() == Screen::Flying);
    }

    fn settings_on_disk(&self) -> PilotSettings {
        SettingsFile::in_folder(self.settings_folder.clone())
            .load()
            .settings
    }
}

/// A yaw flick to the right on the Pocket, then the stick let go.
fn flick_right_and_let_go(host: &mut Host) {
    host.devices_report(vec![Raw::Axis {
        device: POCKET,
        axis: 3,
        value: 30000,
    }]);
    host.frames(2);
    host.devices_report(vec![Raw::Axis {
        device: POCKET,
        axis: 3,
        value: 0,
    }]);
    host.frames(2);
}

#[test]
fn a_radio_alone_takes_a_returning_pilot_from_the_hub_into_the_air() {
    // Basis: Rule (#23: every menu works with a Radio alone; #19 §2: the
    // device that pressed FLY flies).
    let mut host = Host::returning("radio-alone");
    host.devices_report(pocket());
    host.frames(5);
    assert_eq!(host.screen(), Screen::Hub);
    assert_eq!(host.cursor_label(), "FLY");
    flick_right_and_let_go(&mut host);
    assert_eq!(host.screen(), Screen::Loading);
    host.until(400, |h| h.screen() == Screen::Flying);
    assert_eq!(
        host.pilot().flying_name(),
        Some("EdgeTX Radiomaster Pocket Joystick")
    );
    assert!(!host.paused());
}

#[test]
fn the_simulation_stands_still_on_the_hub_and_the_loading_screen_and_runs_in_free_flight() {
    // Basis: Rule (screens.md; ADR-0003: the game steps the Simulation only
    // while flying).
    let mut host = Host::returning("stands-still");
    host.frames(60);
    assert!(host.paused());
    assert_eq!(host.simulation_steps(), 0);
    host.press(MenuInput::Select);
    assert_eq!(host.screen(), Screen::Loading);
    host.frames(2);
    assert_eq!(host.simulation_steps(), 0, "nothing steps while it loads");
    host.until(400, |h| h.screen() == Screen::Flying);
    host.frames(60);
    assert!(!host.paused());
    // About a second of Simulation Time at 8 kHz.
    assert!(
        host.simulation_steps() > 4000,
        "{} steps",
        host.simulation_steps()
    );
}

#[test]
fn the_world_is_made_for_the_loading_screen_and_taken_down_with_the_hub() {
    // Basis: Rule (#23 §4: the loading screen shows the Map; shaders compile
    // there, before flight).
    let mut host = Host::returning("world");
    assert!(!host.world_wanted());
    host.press(MenuInput::Select);
    // The loading screen is shown first, then the world is asked for.
    assert!(!host.world_wanted());
    host.until(100, |h| h.world_wanted());
    assert_eq!(host.screen(), Screen::Loading);
    host.until(400, |h| h.screen() == Screen::Flying);
    assert!(host.world_wanted());
    // Pause, Back to the Hub.
    host.press(MenuInput::Pause);
    assert!(host.world_wanted(), "the Pause Menu is over the world");
    // Resume, Reset, Map, Quad, Settings, then Back to the Hub.
    for _ in 0..5 {
        host.press(MenuInput::Down);
    }
    assert_eq!(host.cursor_label(), "Back to the Hub");
    host.press(MenuInput::Select);
    assert_eq!(host.screen(), Screen::Hub);
    assert!(!host.world_wanted());
    assert_eq!(host.pilot().flying_name(), None);
    assert!(host.paused());
}

#[test]
fn pause_freezes_the_simulation_and_resume_on_the_ground_flies_on_at_once() {
    // Basis: Rule (screens.md: Pause freezes the simulation; #23 §6).
    let mut host = Host::returning("pause");
    host.fly();
    host.frames(30);
    host.press(MenuInput::Pause);
    assert_eq!(host.screen(), Screen::PauseMenu);
    host.frames(2);
    let frozen_at = host.simulation_steps();
    host.frames(60);
    assert_eq!(host.simulation_steps(), frozen_at, "frozen in the menu");
    assert!(host.paused());
    // Resume: the Quad is on the ground, so there's no count.
    host.press(MenuInput::Select);
    assert_eq!(host.screen(), Screen::Flying);
    host.frames(30);
    assert!(!host.paused());
    assert!(host.simulation_steps() > frozen_at);
}

#[test]
fn swapping_the_quad_in_the_pause_menu_loads_it_and_saves_it() {
    // Basis: Rule (#23 §6: picking a new Quad puts it on its Launch Spot
    // with a freshly powered Flight Controller; ids are what is saved).
    let mut host = Host::returning("swap-quad");
    host.fly();
    assert_eq!(host.quad_id(), "opendrone/whoop-65");
    host.frames(60);
    host.press(MenuInput::Pause);
    // Resume, Reset, Map, Quad: down three, select; the Freestyle 5″ is
    // first in the picker, and the cursor starts on the Whoop 65.
    for _ in 0..3 {
        host.press(MenuInput::Down);
    }
    host.press(MenuInput::Select);
    assert_eq!(
        host.screen(),
        Screen::Picker(opendrone::screens::model::Pick::Quad)
    );
    host.press(MenuInput::Up);
    host.press(MenuInput::Select);
    assert_eq!(host.screen(), Screen::Loading);
    // Saved at once, before the flight is even made.
    assert_eq!(host.settings_on_disk().quad, "opendrone/freestyle-5");
    host.until(400, |h| h.screen() == Screen::Flying);
    assert_eq!(host.quad_id(), "opendrone/freestyle-5");
    // A fresh Simulation: it has only just started.
    assert!(
        host.simulation_steps() < 2000,
        "{}",
        host.simulation_steps()
    );
}

#[test]
fn quit_ends_the_game() {
    let mut host = Host::returning("quit");
    host.press(MenuInput::Down);
    host.press(MenuInput::Down);
    host.press(MenuInput::Down);
    host.press(MenuInput::Down);
    host.press(MenuInput::Down);
    assert_eq!(host.cursor_label(), "Quit");
    assert!(host.app.should_exit().is_none());
    host.press(MenuInput::Select);
    assert_eq!(host.app.should_exit(), Some(AppExit::Success));
}

#[test]
fn a_setting_is_saved_the_moment_it_changes() {
    // Basis: Rule (#68: saved the moment a setting changes).
    let mut host = Host::returning("saved-at-once");
    assert!(!host.settings_folder.join("settings.toml").exists());
    // Settings, then Text size to 125 %.
    for _ in 0..4 {
        host.press(MenuInput::Down);
    }
    host.press(MenuInput::Select);
    assert_eq!(host.screen(), Screen::Settings);
    host.press(MenuInput::Right);
    assert_eq!(host.settings_on_disk().text_size, TextSize::Large);
    // The screens grow with it: 125 % of the usual 20 px.
    let rem = host.app.world().resource::<bevy::text::RemSize>().0;
    assert_eq!(rem, 25.0);
}

#[test]
fn camera_tilt_and_fov_changes_in_flight_are_saved_too() {
    // Basis: Rule (#66's note: persist the camera settings).
    let mut host = Host::returning("camera-saved");
    host.frames(3);
    host.app
        .world_mut()
        .resource_mut::<CameraSettings>()
        .set_quad(
            "opendrone/whoop-65",
            opendrone::camera::controls::QuadCamera {
                tilt: 41.0,
                fov: 150.0,
            },
        );
    host.frames(2);
    let saved = host.settings_on_disk();
    let whoop = saved
        .camera
        .quads()
        .find(|(id, _)| id.as_str() == "opendrone/whoop-65")
        .map(|(_, camera)| *camera)
        .expect("the Whoop 65's camera is in the file");
    assert_eq!((whoop.tilt, whoop.fov), (41.0, 150.0));
}

#[test]
fn fly_pressed_on_the_keyboard_flies_with_the_only_device_plugged_in() {
    // Basis: Rule (#19 §2, and the module's rule for the keyboard).
    let mut host = Host::returning("keyboard-fly");
    host.devices_report(dualsense());
    host.frames(5);
    assert_eq!(
        host.page().flying_with.as_deref(),
        Some("Flying with: DualSense Wireless Controller")
    );
    host.fly();
    assert_eq!(
        host.pilot().flying_name(),
        Some("DualSense Wireless Controller")
    );
}

#[test]
fn with_two_devices_and_the_keyboard_the_first_stick_move_picks_the_flyer() {
    // Basis: Rule (the Pilot module: the first device whose sticks move
    // flies, as before the screens (#62)).
    let mut host = Host::returning("two-devices");
    host.devices_report(pocket());
    host.devices_report(dualsense());
    host.frames(5);
    assert_eq!(
        host.page().flying_with.as_deref(),
        Some("Flying with: the Input Device you press FLY with")
    );
    host.fly();
    assert_eq!(host.pilot().flying_name(), None);
    // A flick of the DualSense's right stick starts the flight with it.
    host.devices_report(vec![Raw::PadAxis {
        device: DUALSENSE,
        axis: PadAxis::RightX,
        value: 30000,
    }]);
    host.frames(3);
    assert_eq!(
        host.pilot().flying_name(),
        Some("DualSense Wireless Controller")
    );
}

#[test]
fn a_device_plugged_in_after_the_game_started_brings_a_short_notice() {
    // Basis: Rule (#23 §3: a short notice when an Input Device connects or
    // disconnects). Devices found in the first 1.5 s were there before.
    let mut host = Host::returning("notice");
    host.frames(120);
    host.devices_report(dualsense());
    host.frames(3);
    assert!(
        host.page()
            .notices
            .contains(&"DualSense Wireless Controller connected".to_string())
    );
    host.devices_report(vec![Raw::Removed { device: DUALSENSE }]);
    host.frames(3);
    assert!(
        host.page()
            .notices
            .contains(&"DualSense Wireless Controller disconnected".to_string())
    );
}

#[test]
fn a_device_found_as_the_game_starts_brings_no_notice() {
    let mut host = Host::returning("no-notice-at-start");
    host.devices_report(dualsense());
    host.frames(3);
    assert!(host.page().notices.iter().all(|n| !n.contains("connected")));
}

#[test]
fn the_throttle_and_the_switches_never_move_a_menu() {
    // Basis: Rule (#23, screens.md: menus never read the throttle or the
    // switches). The throttle is full up and Arm (CH5) is high.
    let mut host = Host::returning("never-reads-throttle");
    host.devices_report(pocket());
    host.frames(5);
    host.devices_report(vec![
        Raw::Axis {
            device: POCKET,
            axis: 2,
            value: 32767,
        },
        Raw::Axis {
            device: POCKET,
            axis: 4,
            value: 32767,
        },
        Raw::Axis {
            device: POCKET,
            axis: 5,
            value: 32767,
        },
    ]);
    host.frames(120);
    assert_eq!(host.screen(), Screen::Hub);
    assert_eq!(host.cursor_label(), "FLY");
}

#[test]
fn a_radio_stick_held_when_a_menu_opens_does_not_select() {
    // Basis: Rule: opening the Pause Menu in a turn selects nothing. Yaw is
    // held right when Pause is pressed.
    let mut host = Host::returning("held-stick");
    host.devices_report(pocket());
    host.fly();
    host.devices_report(vec![Raw::Axis {
        device: POCKET,
        axis: 3,
        value: 30000,
    }]);
    host.frames(5);
    host.press(MenuInput::Pause);
    host.frames(30);
    assert_eq!(host.screen(), Screen::PauseMenu);
}

#[test]
fn a_notice_about_a_settings_file_that_could_not_be_read_shows_on_the_first_frame() {
    let folder = folder("broken-settings");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("settings.toml"), "not [toml").unwrap();
    let mut file = SettingsFile::in_folder(folder);
    let loaded = file.load();
    let packs = common::packs();
    let screens = Screens::new(loaded, Some(file), packs, &repo_folder(), None);
    let plan = screens.first_plan().unwrap();
    let pilot = Pilot::without_input_thread(&plan, Vec::new()).with_screens();
    let flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SteppingPlugin, PilotPlugin, ScreensPlugin))
        .insert_resource(plan)
        .insert_non_send(flight)
        .insert_resource(pilot)
        .insert_resource(screens)
        .init_resource::<QuadPoses>()
        .init_resource::<CameraSettings>();
    app.update();
    app.update();
    let page = app.world().resource::<CurrentPage>().0.clone().unwrap();
    assert!(
        page.notices.iter().any(|n| n.contains("isn't readable")),
        "{:?}",
        page.notices
    );
    // A new install: First Launch.
    assert_eq!(page.screen, Screen::FirstLaunch);
}

#[test]
fn with_no_sound_device_the_game_says_so_once_and_runs_silent() {
    // Basis: Rule (#32 §4, and the maintainer's note on #74: a short notice
    // once, not a Pre-flight Warning). This game has no sound at all, as one
    // with no sound device.
    let mut host = Host::returning("no-sound");
    host.frames(2);
    let says = |host: &Host| {
        host.page()
            .notices
            .contains(&"No sound device: OpenDrone runs silent.".to_string())
    };
    assert!(says(&host));
    assert!(
        host.page().warnings.iter().all(|w| !w.contains("sound")),
        "not a Pre-flight Warning"
    );
    // Gone after its few seconds, and not said again.
    host.frames(500);
    assert!(!says(&host));
}
