//! Starts the game. Run `opendrone --help` for what it can do.

use std::path::PathBuf;
use std::process::ExitCode;

use bevy::math::UVec2;
use opendrone::camera::controls::{CameraSettings, FOV_RANGE, QuadCamera, TILT_RANGE};
use opendrone::camera::lens::Aspect;
use opendrone::camera::look::VideoLook;
use opendrone::drawing::View;
use opendrone::pilot::Pilot;
use opendrone::screens::{Screen, Screens};
use opendrone::set_up::{FlightPlan, open_packs, repo_folder};
use opendrone::settings::Settings;
use opendrone::settings_file::{Loaded, PilotSettings, SettingsFile};
use opendrone::watch::open_scenario;
use opendrone::{
    game, headless, measure_camera_cost, reference, screens_game, screenshot_after, watching_game,
};

/// The Quads `--quad` takes, by the id after `opendrone/`.
const QUADS: [&str; 2] = ["whoop-65", "freestyle-5"];

/// The Maps `--map` takes, with the id each means. The Test Map `flat-floor`
/// is a developer's: it flies at once, with no screens and nothing saved.
const MAPS: [(&str, &str); 3] = [
    ("skate-park", "opendrone/skate-park"),
    ("bando", "opendrone/bando"),
    ("flat-floor", "test/flat-floor"),
];

/// The screens `--screen` can open on, for a developer to look at one.
const SCREENS: [(&str, Screen); 6] = [
    ("first-launch", Screen::FirstLaunch),
    ("input-device", Screen::InputDeviceStep),
    ("hub", Screen::Hub),
    ("loading", Screen::Loading),
    ("pause", Screen::PauseMenu),
    ("settings", Screen::Settings),
];

const HELP: &str = "\
OpenDrone: fly a Quad through its FPV Camera. A new install opens on First
Launch; after that you land on the Hub, and FLY takes you to the Launch Spot.

  opendrone                       open the screens: First Launch the first
                                  time, then the Hub
  opendrone --quad freestyle-5    pick this Quad (or --quad whoop-65), and
                                  keep the choice in your settings
  opendrone --map bando           pick Bando, the unfinished tower (or --map
                                  skate-park), and keep the choice
  opendrone --look digital        see the Digital Video Look (or analog, the
                                  default), and keep the choice
  opendrone --aspect 16:9         a 16:9 picture that crops the top and bottom
                                  (or 4:3, the default), and keep the choice
  opendrone --tilt 45 --fov 140   set this Camera Tilt (0 to 80\u{b0}) and FOV
                                  (90 to 170\u{b0}) for the Quad, and keep them
  opendrone --settings-folder DIR use the settings file (settings.toml) in
                                  this folder, and keep saved flights there;
                                  without it the game uses the usual folder
                                  for your computer
  opendrone --headless [--seconds N]
                                  run with no window for N seconds (3 if not
                                  given), then say how far the Simulation
                                  stepped; the sound plays if there's a sound
                                  device, and runs silent if not
  opendrone --reference SCENARIO...
                                  fly each Scenario through the game's own
                                  stepping and check its fingerprint matches
                                  the headless Scenario runner's
  opendrone --no-screens          a developer command: fly at once with the
                                  starting settings, as the first test flight
                                  did, and read and write no settings file
  opendrone --map flat-floor      a developer command: fly at once over a grid
                                  floor with nothing to hit (a Test Map), with
                                  no screens and no settings file
  opendrone --screenshot FILE.bmp [--seconds N] [--size WxH]
                                  a developer command: fly as usual, save the
                                  camera's picture after N seconds (5 if not
                                  given), then quit. The camera draws into a
                                  picture of its own, not the window's: it is
                                  1280x720 unless --size says otherwise
  opendrone --screen NAME         a developer command: open on one screen
                                  (first-launch, input-device, hub, loading,
                                  pause or settings), alone or with
                                  --screenshot. Loading makes the flight as
                                  FLY does, and flies on from there. It uses
                                  the starting settings and writes none,
                                  unless --settings-folder names where they
                                  are
  opendrone --camera-cost [--seconds N] [--size WxH]
                                  a developer command: say what the FPV
                                  Camera's own steps cost a frame on the GPU,
                                  N seconds of each Video Look (5 if not
                                  given), in a 2560x1440 picture unless
                                  --size says otherwise
  opendrone --chase               a developer command: watch the Quad from a
                                  camera that follows it, instead of flying
                                  from the camera on its frame
  opendrone --scenario FILE       a developer command: watch a Flight or
                                  Thrust Stand Scenario (a file in
                                  scenarios/) in the FPV view, nobody flying
                                  it. Esc pauses. It goes with --screenshot,
                                  --look, --aspect, --tilt, --fov and --chase

On every screen a Radio moves with its right stick, a flick of yaw to the
right selects and a flick to the left goes back. A Gamepad moves with its
D-pad or left stick; \u{2715} selects, \u{25cb} goes back, and L1 and R1 switch tabs. The
keyboard moves with the arrows, selects with Enter and goes back with
Backspace. FLY with a device and that device flies; with the keyboard or the
mouse, the device you used last, or the only one plugged in. In flight, R
resets and Esc opens the Pause Menu. The up and down arrows change the Camera
Tilt, and the right and left arrows change the FOV, 1\u{b0} a press, repeating
while held.

Save Flight is an Action with no default key: bind it to a button or a switch
in an Input Device profile (save_flight). It writes the flight so far as a
Scenario and its Input Track into the saved-flights folder beside your
settings file.";

enum Mode {
    Game,
    Screenshot { path: PathBuf, seconds: f64 },
    CameraCost { seconds: f64 },
    Headless { seconds: f64 },
    Reference(Vec<PathBuf>),
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut quad: Option<String> = None;
    let mut map: Option<(String, String)> = None;
    let mut mode = Mode::Game;
    let mut view = View::Fpv;
    let mut look: Option<VideoLook> = None;
    let mut aspect: Option<Aspect> = None;
    let (mut tilt, mut fov) = (None, None);
    let mut size = None;
    let mut scenario: Option<PathBuf> = None;
    let mut no_screens = false;
    let mut screen: Option<Screen> = None;
    let mut settings_folder: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--quad" => match args.next() {
                Some(name) if QUADS.contains(&name.as_str()) => quad = Some(name),
                _ => return usage(&format!("--quad takes one of {}", QUADS.join(", "))),
            },
            "--map" => match args
                .next()
                .and_then(|name| MAPS.iter().find(|(n, _)| *n == name))
            {
                Some((name, id)) => map = Some(((*name).to_string(), (*id).to_string())),
                None => {
                    let names: Vec<&str> = MAPS.iter().map(|(n, _)| *n).collect();
                    return usage(&format!("--map takes one of {}", names.join(", ")));
                }
            },
            "--headless" => mode = Mode::Headless { seconds: 3.0 },
            "--screenshot" => match args.next() {
                Some(path) => {
                    mode = Mode::Screenshot {
                        path: PathBuf::from(path),
                        seconds: 5.0,
                    }
                }
                None => return usage("--screenshot takes the file to save the picture to"),
            },
            "--camera-cost" => mode = Mode::CameraCost { seconds: 5.0 },
            "--seconds" => match (&mut mode, args.next().and_then(|s| s.parse::<f64>().ok())) {
                (
                    Mode::Headless { seconds }
                    | Mode::Screenshot { seconds, .. }
                    | Mode::CameraCost { seconds },
                    Some(n),
                ) if n > 0.0 => *seconds = n,
                _ => {
                    return usage(
                        "--seconds comes after --headless, --screenshot or --camera-cost, with a number above 0",
                    );
                }
            },
            "--look" => match args.next().as_deref() {
                Some("analog") => look = Some(VideoLook::Analog),
                Some("digital") => look = Some(VideoLook::Digital),
                _ => return usage("--look takes analog or digital"),
            },
            "--aspect" => match args.next().as_deref() {
                Some("4:3") => aspect = Some(Aspect::FourThree),
                Some("16:9") => aspect = Some(Aspect::SixteenNine),
                _ => return usage("--aspect takes 4:3 or 16:9"),
            },
            "--tilt" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                Some(n) if (TILT_RANGE.0..=TILT_RANGE.1).contains(&n) => tilt = Some(n),
                _ => return usage("--tilt takes a number of degrees from 0 to 80"),
            },
            "--fov" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                Some(n) if (FOV_RANGE.0..=FOV_RANGE.1).contains(&n) => fov = Some(n),
                _ => return usage("--fov takes a number of degrees from 90 to 170"),
            },
            "--size" => match args.next().as_deref().and_then(picture_size) {
                Some(s) => size = Some(s),
                None => return usage("--size takes the picture's size, such as 1920x1080"),
            },
            "--chase" => view = View::Chase,
            "--scenario" => match args.next() {
                Some(path) => scenario = Some(PathBuf::from(path)),
                None => return usage("--scenario takes the Scenario file to watch"),
            },
            "--no-screens" => no_screens = true,
            "--screen" => match args
                .next()
                .and_then(|name| SCREENS.iter().find(|(n, _)| *n == name))
            {
                Some((_, which)) => screen = Some(*which),
                None => {
                    let names: Vec<&str> = SCREENS.iter().map(|(n, _)| *n).collect();
                    return usage(&format!("--screen takes one of {}", names.join(", ")));
                }
            },
            "--settings-folder" => match args.next() {
                Some(folder) => settings_folder = Some(PathBuf::from(folder)),
                None => return usage("--settings-folder takes a folder"),
            },
            "--reference" => mode = Mode::Reference(Vec::new()),
            path => match &mut mode {
                Mode::Reference(paths) => paths.push(PathBuf::from(path)),
                _ => return usage(&format!("unknown argument {path:?}")),
            },
        }
    }

    let repo = repo_folder();
    if let Mode::Reference(paths) = mode {
        return check_references(&repo, &paths);
    }
    if scenario.is_some() && !matches!(mode, Mode::Game | Mode::Screenshot { .. }) {
        return usage(
            "--scenario opens a window: it goes with --screenshot, but not with --headless, --camera-cost or --reference",
        );
    }
    if scenario.is_some() && quad.is_some() {
        return usage("--scenario names its own Quad, so it doesn't go with --quad");
    }
    // A Test Map, a chase view and a watched Scenario are the developers':
    // they fly at once, with no screens.
    let test_map = map.as_ref().is_some_and(|(_, id)| id.starts_with("test/"));
    let direct = no_screens || test_map || scenario.is_some() || view == View::Chase;
    if screen.is_some() && (direct || !matches!(mode, Mode::Game | Mode::Screenshot { .. })) {
        return usage(
            "--screen goes with the screens: not with --no-screens, a Test Map, --scenario, --chase, --headless or --camera-cost",
        );
    }
    // The screens open for the pilot's game, and for a screenshot of one.
    let with_screens = !direct
        && (matches!(mode, Mode::Game)
            || (screen.is_some() && matches!(mode, Mode::Screenshot { .. })));
    let packs = match open_packs(&repo) {
        Ok(packs) => packs,
        Err(problems) => return fail(&format!("the Packs can't be read:\n{problems}")),
    };
    let profiles = packs.input_devices().into_iter().cloned().collect();

    if with_screens {
        return run_with_screens(ScreensRun {
            repo,
            packs,
            profiles,
            mode,
            quad,
            map: map.map(|(_, id)| id),
            look,
            aspect,
            tilt,
            fov,
            size,
            screen,
            settings_folder,
        });
    }

    // Flying at once: the starting settings, and no settings file.
    let quad = quad.unwrap_or_else(|| QUADS[0].to_string());
    let map = map.map_or_else(|| MAPS[0].1.to_string(), |(_, id)| id);
    let mut camera = CameraSettings::default();
    if let Some(look) = look {
        camera.look = look;
    }
    if let Some(aspect) = aspect {
        camera.aspect = aspect;
    }
    let watched = match &scenario {
        Some(path) => match open_scenario(&repo, path) {
            Ok(watched) => Some(watched),
            Err(problem) => return fail(&problem),
        },
        None => None,
    };
    let plan = match &watched {
        Some(watched) => watched.plan.clone(),
        None => match FlightPlan::from_packs(&packs, Settings::defaults(&quad).on_map(&map)) {
            Ok(plan) => plan,
            Err(problem) => return fail(&problem),
        },
    };
    let pilot = Pilot::start(&plan, profiles);
    // A window flying the pilot's own flight, or watching the Scenario.
    let window = |plan, pilot| match watched {
        Some(watched) => Ok(watching_game(watched, pilot)),
        None => game(plan, pilot),
    };
    if tilt.is_some() || fov.is_some() {
        let own = camera.quad(&plan.quad.id, &plan.quad.camera);
        camera.set_quad(
            &plan.quad.id,
            QuadCamera {
                tilt: tilt.unwrap_or(own.tilt),
                fov: fov.unwrap_or(own.fov),
            },
        );
    }
    let app = match mode {
        Mode::Headless { seconds } => headless(plan, pilot, seconds),
        Mode::Screenshot { path, seconds } => window(plan, pilot).map(|mut app| {
            let size = size.unwrap_or(UVec2::new(1280, 720));
            screenshot_after(&mut app, seconds, path, size);
            app
        }),
        Mode::CameraCost { seconds } => window(plan, pilot).map(|mut app| {
            let size = size.unwrap_or(UVec2::new(2560, 1440));
            measure_camera_cost(&mut app, seconds, size);
            app
        }),
        _ => window(plan, pilot),
    };
    run(app, view, camera)
}

/// What the game with its screens starts from.
struct ScreensRun {
    repo: PathBuf,
    packs: opendrone_pack::Packs,
    profiles: Vec<opendrone_input::InputDeviceProfile>,
    mode: Mode,
    quad: Option<String>,
    map: Option<String>,
    look: Option<VideoLook>,
    aspect: Option<Aspect>,
    tilt: Option<f64>,
    fov: Option<f64>,
    size: Option<UVec2>,
    screen: Option<Screen>,
    settings_folder: Option<PathBuf>,
}

/// The game with its screens: the pilot's settings file, read first and
/// written whenever a setting changes.
fn run_with_screens(start: ScreensRun) -> ExitCode {
    let ScreensRun {
        repo,
        packs,
        profiles,
        mode,
        quad,
        map,
        look,
        aspect,
        tilt,
        fov,
        size,
        screen,
        settings_folder,
    } = start;
    // The pilot's own game reads the usual folder. Opening on one screen with
    // `--screen`, or taking a screenshot, reads and writes no file unless it
    // was told where.
    let folder = match (&mode, screen, settings_folder) {
        (_, _, Some(folder)) => Some(folder),
        (Mode::Game, None, None) => {
            let usual = SettingsFile::usual_folder();
            if usual.is_none() {
                eprintln!(
                    "OpenDrone can't tell where your computer keeps settings, so it keeps none this time."
                );
            }
            usual
        }
        _ => None,
    };
    let (mut loaded, store) = match folder {
        Some(folder) => {
            let mut file = SettingsFile::in_folder(folder);
            (file.load(), Some(file))
        }
        None => (
            Loaded {
                settings: PilotSettings::default(),
                existed: false,
                problems: Vec::new(),
            },
            None,
        ),
    };
    for problem in &loaded.problems {
        eprintln!("settings: {problem}");
    }
    // The flags set the pilot's settings, which are kept.
    let settings = &mut loaded.settings;
    let mut flagged = false;
    if let Some(quad) = quad {
        settings.quad = format!("opendrone/{quad}");
        flagged = true;
    }
    if let Some(map) = map {
        settings.map = map;
        flagged = true;
    }
    if let Some(look) = look {
        settings.camera.look = look;
        flagged = true;
    }
    if let Some(aspect) = aspect {
        settings.camera.aspect = aspect;
        flagged = true;
    }
    if tilt.is_some() || fov.is_some() {
        match packs.quad(&settings.quad) {
            Ok(definition) => {
                let own = settings.camera.quad(&definition.id, &definition.camera);
                settings.camera.set_quad(
                    &definition.id,
                    QuadCamera {
                        tilt: tilt.unwrap_or(own.tilt),
                        fov: fov.unwrap_or(own.fov),
                    },
                );
                flagged = true;
            }
            Err(problem) => {
                return fail(&format!(
                    "can't use the Quad \"{}\":\n{problem}",
                    settings.quad
                ));
            }
        }
    }
    if flagged
        && let Some(store) = &store
        && let Err(problem) = store.save(settings)
    {
        eprintln!("settings: can't save {}: {problem}", store.path().display());
    }

    let screens = Screens::new(loaded, store, packs, &repo, screen);
    let plan = match screens.first_plan() {
        Ok(plan) => plan,
        Err(problem) => return fail(&problem),
    };
    let mut pilot = Pilot::start(&plan, profiles);
    if let Some(folder) = screens.saved_flights_folder() {
        pilot = pilot.saving_to(folder);
    }
    let camera = screens.settings().camera.clone();
    let app = screens_game(plan, pilot, screens).map(|mut app| {
        if let Mode::Screenshot { path, seconds } = mode {
            let size = size.unwrap_or(UVec2::new(1280, 720));
            screenshot_after(&mut app, seconds, path, size);
        }
        app
    });
    run(app, View::Fpv, camera)
}

/// Runs the app, and turns how it ended into an exit code.
fn run(app: Result<bevy::app::App, String>, view: View, camera: CameraSettings) -> ExitCode {
    match app {
        Ok(mut app) => {
            app.insert_resource(view).insert_resource(camera);
            if app.run().is_success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(problem) => fail(&problem),
    }
}

/// The reference check, for each Scenario: its fingerprint through the
/// game's stepping against the headless runner's.
fn check_references(repo: &std::path::Path, paths: &[PathBuf]) -> ExitCode {
    if paths.is_empty() {
        return usage("--reference takes one Scenario file or more");
    }
    let mut all_agree = true;
    for path in paths {
        match reference::fly(repo, path) {
            Ok(flight) => {
                println!(
                    "{}: {} steps, {} a tick, over {} frames of uneven length; the game's run fingerprint {}, the headless runner's {}: {}",
                    flight.label,
                    flight.steps,
                    flight.steps_per_tick,
                    flight.frames,
                    flight.game,
                    flight.runner,
                    if flight.agrees() {
                        "the same"
                    } else {
                        "DIFFERENT"
                    }
                );
                all_agree &= flight.agrees();
            }
            Err(problem) => {
                eprintln!("{}: {problem}", path.display());
                all_agree = false;
            }
        }
    }
    if all_agree {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A picture's size as `1920x1080`.
fn picture_size(text: &str) -> Option<UVec2> {
    let (w, h) = text.split_once('x')?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    ((64..=8192).contains(&w) && (64..=8192).contains(&h)).then(|| UVec2::new(w, h))
}

fn usage(problem: &str) -> ExitCode {
    eprintln!("{problem}\n\n{HELP}");
    ExitCode::from(2)
}

fn fail(problem: &str) -> ExitCode {
    eprintln!("OpenDrone can't start: {problem}");
    ExitCode::FAILURE
}
