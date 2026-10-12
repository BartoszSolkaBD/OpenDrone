//! Starts the game. Run `opendrone --help` for what it can do.

use std::path::PathBuf;
use std::process::ExitCode;

use opendrone::drawing::View;
use opendrone::pilot::Pilot;
use opendrone::set_up::{FlightPlan, open_packs, repo_folder};
use opendrone::settings::{QUADS, Settings};
use opendrone::{game, headless, reference, screenshot_after};

const HELP: &str = "\
OpenDrone, the first flight: fly a Quad over the grid floor.

  opendrone                       fly the Whoop 65
  opendrone --quad freestyle-5    fly the Freestyle 5\u{2033} (or --quad whoop-65)
  opendrone --headless [--seconds N]
                                  run with no window for N seconds (3 if not
                                  given), then say how far the Simulation
                                  stepped; the sound plays if there's a sound
                                  device, and runs silent if not
  opendrone --reference SCENARIO...
                                  fly each Scenario through the game's own
                                  stepping and check its fingerprint matches
                                  the headless Scenario runner's
  opendrone --screenshot FILE.bmp [--seconds N]
                                  a developer command: fly as usual, save the
                                  window's picture after N seconds (5 if not
                                  given), then quit
  opendrone --chase               a developer command: watch the Quad from a
                                  camera that follows it, instead of flying
                                  from the camera on its frame

Move a stick on the Input Device you want to fly with: it becomes the Flying
Input Device. R resets, Esc pauses.";

enum Mode {
    Game,
    Screenshot { path: PathBuf, seconds: f64 },
    Headless { seconds: f64 },
    Reference(Vec<PathBuf>),
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut quad = QUADS[0].to_string();
    let mut mode = Mode::Game;
    let mut view = View::Fpv;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--quad" => match args.next() {
                Some(name) if QUADS.contains(&name.as_str()) => quad = name,
                _ => return usage(&format!("--quad takes one of {}", QUADS.join(", "))),
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
            "--seconds" => match (&mut mode, args.next().and_then(|s| s.parse::<f64>().ok())) {
                (Mode::Headless { seconds } | Mode::Screenshot { seconds, .. }, Some(n))
                    if n > 0.0 =>
                {
                    *seconds = n
                }
                _ => {
                    return usage(
                        "--seconds comes after --headless or --screenshot, with a number above 0",
                    );
                }
            },
            "--chase" => view = View::Chase,
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
    let packs = match open_packs(&repo) {
        Ok(packs) => packs,
        Err(problems) => return fail(&format!("the Packs can't be read:\n{problems}")),
    };
    let plan = match FlightPlan::from_packs(&packs, Settings::tracer(&quad)) {
        Ok(plan) => plan,
        Err(problem) => return fail(&problem),
    };
    let profiles = packs.input_devices().into_iter().cloned().collect();
    let pilot = Pilot::start(&plan, profiles);
    let app = match mode {
        Mode::Headless { seconds } => headless(plan, pilot, seconds),
        Mode::Screenshot { path, seconds } => game(plan, pilot).map(|mut app| {
            screenshot_after(&mut app, seconds, path);
            app
        }),
        _ => game(plan, pilot),
    };
    match app {
        Ok(mut app) => {
            app.insert_resource(view);
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

fn usage(problem: &str) -> ExitCode {
    eprintln!("{problem}\n\n{HELP}");
    ExitCode::from(2)
}

fn fail(problem: &str) -> ExitCode {
    eprintln!("OpenDrone can't start: {problem}");
    ExitCode::FAILURE
}
