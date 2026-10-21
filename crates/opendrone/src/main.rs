//! Starts the game. Run `opendrone --help` for what it can do.

use std::path::PathBuf;
use std::process::ExitCode;

use bevy::math::UVec2;
use opendrone::camera::controls::{CameraSettings, FOV_RANGE, QuadCamera, TILT_RANGE};
use opendrone::camera::lens::Aspect;
use opendrone::camera::look::VideoLook;
use opendrone::drawing::View;
use opendrone::pilot::Pilot;
use opendrone::set_up::{FlightPlan, open_packs, repo_folder};
use opendrone::settings::{MAPS, QUADS, Settings};
use opendrone::watch::open_scenario;
use opendrone::{game, headless, measure_camera_cost, reference, screenshot_after, watching_game};

const HELP: &str = "\
OpenDrone, the first flight: fly a Quad through its FPV Camera, from the
Launch Spot of the Skate Park.

  opendrone                       fly the Whoop 65 on the Skate Park
  opendrone --quad freestyle-5    fly the Freestyle 5\u{2033} (or --quad whoop-65)
  opendrone --map bando           fly Bando, the unfinished tower, instead (or
                                  --map skate-park, the default)
  opendrone --map flat-floor      fly over a grid floor with nothing to hit
  opendrone --look digital        see the Digital Video Look (or analog, the
                                  default)
  opendrone --aspect 16:9         a 16:9 picture that crops the top and bottom
                                  (or 4:3, the default)
  opendrone --tilt 45 --fov 140   start at this Camera Tilt (0 to 80\u{b0}) and
                                  FOV (90 to 170\u{b0}) instead of the Quad's own
  opendrone --headless [--seconds N]
                                  run with no window for N seconds (3 if not
                                  given), then say how far the Simulation
                                  stepped; the sound plays if there's a sound
                                  device, and runs silent if not
  opendrone --reference SCENARIO...
                                  fly each Scenario through the game's own
                                  stepping and check its fingerprint matches
                                  the headless Scenario runner's
  opendrone --screenshot FILE.bmp [--seconds N] [--size WxH]
                                  a developer command: fly as usual, save the
                                  camera's picture after N seconds (5 if not
                                  given), then quit. The camera draws into a
                                  picture of its own, not the window's: it is
                                  1280x720 unless --size says otherwise
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

Move a stick on the Input Device you want to fly with: it becomes the Flying
Input Device. R resets, Esc pauses. The up and down arrows change the Camera
Tilt, and the right and left arrows change the FOV, 1\u{b0} a press, repeating
while held.

Save Flight is an Action with no default key: bind it to a button or a switch
in an Input Device profile (save_flight). It writes the flight so far as a
Scenario and its Input Track into scenarios/saved/.";

enum Mode {
    Game,
    Screenshot { path: PathBuf, seconds: f64 },
    CameraCost { seconds: f64 },
    Headless { seconds: f64 },
    Reference(Vec<PathBuf>),
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut quad = QUADS[0].to_string();
    let mut map = MAPS[0].1.to_string();
    let mut mode = Mode::Game;
    let mut view = View::Fpv;
    let mut camera = CameraSettings::default();
    let (mut tilt, mut fov) = (None, None);
    let mut size = None;
    let mut scenario: Option<PathBuf> = None;
    let mut quad_given = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--quad" => match args.next() {
                Some(name) if QUADS.contains(&name.as_str()) => {
                    quad = name;
                    quad_given = true;
                }
                _ => return usage(&format!("--quad takes one of {}", QUADS.join(", "))),
            },
            "--map" => match args
                .next()
                .and_then(|name| MAPS.iter().find(|(n, _)| *n == name))
            {
                Some((_, id)) => map = (*id).to_string(),
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
                Some("analog") => camera.look = VideoLook::Analog,
                Some("digital") => camera.look = VideoLook::Digital,
                _ => return usage("--look takes analog or digital"),
            },
            "--aspect" => match args.next().as_deref() {
                Some("4:3") => camera.aspect = Aspect::FourThree,
                Some("16:9") => camera.aspect = Aspect::SixteenNine,
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
    if scenario.is_some() && quad_given {
        return usage("--scenario names its own Quad, so it doesn't go with --quad");
    }
    let packs = match open_packs(&repo) {
        Ok(packs) => packs,
        Err(problems) => return fail(&format!("the Packs can't be read:\n{problems}")),
    };
    let watched = match &scenario {
        Some(path) => match open_scenario(&repo, path) {
            Ok(watched) => Some(watched),
            Err(problem) => return fail(&problem),
        },
        None => None,
    };
    let plan = match &watched {
        Some(watched) => watched.plan.clone(),
        None => match FlightPlan::from_packs(&packs, Settings::tracer(&quad).on_map(&map)) {
            Ok(plan) => plan,
            Err(problem) => return fail(&problem),
        },
    };
    let profiles = packs.input_devices().into_iter().cloned().collect();
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
