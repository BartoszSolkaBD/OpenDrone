//! The screens (#68): First Launch, the Hub, the loading screen, the Pause
//! Menu and a small Settings page, and how a pilot moves between them with a
//! Radio, a Gamepad, or the keyboard and mouse.
//!
//! - [`model`]: what each screen holds and the moves between them, as plain
//!   code the readable checks drive;
//! - [`menu_input`]: a Radio's and a Gamepad's sticks and buttons turned into
//!   menu moves;
//! - [`view`]: the screens drawn on Bevy 0.20's UI widgets, in a placeholder
//!   style;
//! - this module joins them to the game ([`ScreensPlugin`]): it feeds the
//!   model what the Input Devices, the keyboard and the mouse did, carries out
//!   its [`Effect`]s, and keeps the flight's world, the Simulation's clock,
//!   the Quad's sound and the settings file in step with the screen.
//!
//! **How a flight starts.** FLY (or a swap of the Map or the Quad from the
//! Pause Menu) opens the loading screen. Once it has been on screen for a few
//! frames, the game makes the flight from the Packs and the pilot's settings,
//! draws the world behind the loading screen, and lets the renderer compile
//! its shaders there: it waits until the renderer has no pipeline left to
//! build, so Metal's first-draw stutter happens before flight. Then Free
//! Flight starts and the Simulation's clock runs. While anything but Free
//! Flight is on screen the clock stands still, and the Quad is silent.
//!
//! **Which device flies.** The device that pressed FLY: a Radio by a flick of
//! yaw, a Gamepad by ✕. FLY pressed with the keyboard or the mouse uses the
//! Flying device setting (#271): Auto flies a Radio, else a Gamepad; a device
//! the pilot named flies whatever pressed FLY, and when it isn't plugged in
//! Auto picks and says so. Where two Radios or two Gamepads tie, or nothing
//! is plugged in, the Quad waits on the Launch Spot, and the first of the
//! tied devices whose sticks move flies, as before the screens (#62).

pub mod menu_input;
pub mod model;
pub mod view;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::text::{DEFAULT_REM_SIZE_PX, RemSize};
use opendrone_input::DeviceId;
use opendrone_pack::Packs;

use self::menu_input::MenuInput;
use self::model::{Catalogue, Effect, Facts, ItemId, MapChoice, Menus, QuadChoice};
use self::view::{CurrentPage, MouseAction, MouseMenu};

pub use self::view::ViewPlugin;
use crate::camera::QuadPoses;
use crate::camera::controls::CameraSettings;
use crate::camera::pose::PoseHistory;
use crate::drawing::WorldDrawing;
use crate::pilot::{FlightWorld, Pilot, read_keyboard};
use crate::set_up::FlightPlan;
use crate::settings::Settings;
use crate::settings_file::{Loaded, SettingsFile};
use crate::sound::QuadSoundPlaying;
use crate::stepping::Flight;

pub use self::model::Screen;

/// The loading screen stays up for at least this many frames with the world
/// drawn, so every pipeline the first frames need has been asked for.
const WARM_FRAMES: u32 = 20;

/// And then until the renderer has had no pipeline left to build for this
/// many frames in a row.
const QUIET_FRAMES: u32 = 5;

/// The loading screen goes on after this long whatever the renderer says, so
/// a pipeline that never finishes can't keep a pilot out of the air.
const LOADING_GIVES_UP: Duration = Duration::from_secs(30);

/// A notice about the settings file stays this long: it takes longer to read.
const LONG_NOTICE_FOR: Duration = Duration::from_secs(15);

/// The frames the loading screen is on screen before the game starts the
/// work that can hold a frame up, so the pilot sees it first.
const SHOWN_FIRST_FRAMES: u32 = 3;

/// The model of every screen, with what the game needs to carry it out.
#[derive(Resource)]
pub struct Screens {
    menus: Menus,
    /// The settings file, or `None` for a game that keeps nothing.
    store: Option<SettingsFile>,
    catalogue: Catalogue,
    packs: Packs,
    /// The flight being made behind the loading screen.
    loading: Option<Loading>,
    /// The game has said (or found nothing to say) about the sound device.
    said_about_sound: bool,
    /// Notices from before the game runs (a settings file that couldn't be
    /// read, a Quad that isn't there any more), said on the first frame.
    to_say: Vec<String>,
}

impl Screens {
    /// The screens of a pilot with the settings `loaded` from `store`, over
    /// the Maps and Quads in `packs` (read from the folder `repo`). A Map or
    /// Quad the settings name that the Packs don't have is replaced, with a
    /// notice. `start` opens on a screen other than the usual one: a
    /// developer command that shows one.
    pub fn new(
        loaded: Loaded,
        store: Option<SettingsFile>,
        packs: Packs,
        repo: &Path,
        start: Option<Screen>,
    ) -> Screens {
        let catalogue = catalogue_of(&packs, repo);
        let mut settings = loaded.settings;
        let mut notices = loaded.problems;
        let defaults = crate::settings_file::PilotSettings::default();
        if !catalogue.maps.iter().any(|m| m.id == settings.map)
            && let Some(map) = catalogue
                .maps
                .iter()
                .find(|m| m.id == defaults.map)
                .or(catalogue.maps.first())
        {
            notices.push(format!(
                "The Map {} isn't in any Pack, so {} is selected.",
                settings.map, map.name
            ));
            settings.map = map.id.clone();
        }
        if !catalogue.quads.iter().any(|q| q.id == settings.quad)
            && let Some(quad) = catalogue
                .quads
                .iter()
                .find(|q| q.id == defaults.quad)
                .or(catalogue.quads.first())
        {
            notices.push(format!(
                "The Quad {} isn't in any Pack, so {} is selected.",
                settings.quad, quad.name
            ));
            settings.quad = quad.id.clone();
        }
        let menus = match start {
            Some(screen) => Menus::starting_on(settings, screen),
            None => Menus::new(settings),
        };
        // Opening on the loading screen loads the flight, as FLY does.
        let loading =
            (menus.screen() == Screen::Loading).then(|| Loading::begin(None, Duration::ZERO));
        Screens {
            menus,
            store,
            catalogue,
            packs,
            loading,
            said_about_sound: false,
            to_say: notices,
        }
    }

    /// The flight the pilot's settings start from: their Map and Quad, set
    /// up as FLY sets them up. The game is made around it, and makes it
    /// afresh when the pilot presses FLY.
    pub fn first_plan(&self) -> Result<FlightPlan, String> {
        FlightPlan::from_packs(&self.packs, Settings::from_pilot(self.menus.settings()))
    }

    /// Where Save Flight writes: beside the settings file, if there is one
    /// (#73).
    pub fn saved_flights_folder(&self) -> Option<std::path::PathBuf> {
        self.store.as_ref().map(SettingsFile::saved_flights_folder)
    }

    /// The model.
    pub fn menus(&self) -> &Menus {
        &self.menus
    }

    /// What the settings file keeps, as the pilot has it now.
    pub fn settings(&self) -> &crate::settings_file::PilotSettings {
        self.menus.settings()
    }

    /// The Maps and Quads a pilot can pick.
    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }
}

/// The Maps and Quads the Packs give a pilot to pick from: every Map that
/// passed the checker (the Test Maps are the developers'), and every Quad
/// that our Flight Controller can fly. A Quad's picture is the file its
/// definition names, in the Quad's folder under `repo`.
pub fn catalogue_of(packs: &Packs, repo: &Path) -> Catalogue {
    let maps = packs
        .maps()
        .into_iter()
        .map(|map| MapChoice {
            id: map.id.clone(),
            name: map.name.clone(),
            description: map.description.clone(),
        })
        .collect();
    let quads = packs
        .quads()
        .into_iter()
        .filter(|quad| quad.flight_controller.is_ok())
        .map(|quad| {
            let picture = quad.id.split_once('/').map(|(pack, name)| {
                repo.join("packs")
                    .join(pack)
                    .join("quads")
                    .join(name)
                    .join(&quad.picture)
            });
            QuadChoice {
                id: quad.id.clone(),
                name: quad.name.clone(),
                description: quad.description.clone(),
                spec_line: quad.spec_line.clone(),
                picture: picture.filter(|p| p.is_file()),
            }
        })
        .collect();
    Catalogue { maps, quads }
}

/// How far the flight behind the loading screen has got.
struct Loading {
    stage: Stage,
    /// Who pressed FLY.
    by: Option<DeviceId>,
    /// Frames since the loading screen opened.
    frames: u32,
    /// Frames since the world was drawn.
    warmed: u32,
    /// Frames in a row with no pipeline left to build.
    quiet: u32,
    /// The most pipelines seen waiting at once.
    peak: usize,
    started: Duration,
}

impl Loading {
    /// The loading screen has just opened at `now`, after `by` pressed FLY.
    fn begin(by: Option<DeviceId>, now: Duration) -> Loading {
        Loading {
            stage: Stage::Showing,
            by,
            frames: 0,
            warmed: 0,
            quiet: 0,
            peak: 0,
            started: now,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The loading screen is shown first.
    Showing,
    /// The flight is made and the world asked for; it draws on the next frame.
    Drawing,
    /// The world is drawn and the shaders compile.
    Warming,
}

/// How many pipelines the renderer still has to build, counted in the render
/// world and read by the loading screen.
#[derive(Resource, Clone, Default)]
pub struct PipelinesWaiting(pub Arc<AtomicUsize>);

fn count_the_waiting_pipelines(cache: Res<PipelineCache>, waiting: Res<PipelinesWaiting>) {
    let count = cache
        .pipelines()
        .filter(|p| {
            matches!(
                p.state,
                CachedPipelineState::Queued | CachedPipelineState::Creating(_)
            )
        })
        .count();
    waiting.0.store(count, Ordering::Relaxed);
}

/// The screens, in the game's window.
pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        let waiting = PipelinesWaiting::default();
        app.insert_resource(waiting.clone())
            .insert_resource(WorldDrawing::wanting(false))
            .init_resource::<MouseMenu>()
            .init_resource::<CurrentPage>()
            .init_resource::<InputFocus>()
            .init_resource::<RemSize>()
            .add_systems(Startup, stand_still_until_flying)
            .add_systems(
                PreUpdate,
                drive_the_screens
                    .after(crate::pilot::read_input_devices)
                    .after(read_keyboard),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(waiting).add_systems(
                Render,
                count_the_waiting_pipelines.in_set(RenderSystems::Cleanup),
            );
        }
    }
}

/// Until Free Flight starts, the Simulation's clock stands still.
fn stand_still_until_flying(mut clock: ResMut<Time<Virtual>>) {
    clock.pause();
}

/// The parts of the game the screens carry their effects out on.
#[derive(SystemParam)]
struct Game<'w> {
    pilot: ResMut<'w, Pilot>,
    flight: NonSendMut<'w, Flight>,
    plan: ResMut<'w, FlightPlan>,
    clock: ResMut<'w, Time<Virtual>>,
    fixed: Res<'w, Time<Fixed>>,
    poses: ResMut<'w, QuadPoses>,
    drawing: ResMut<'w, WorldDrawing>,
}

/// What a pilot did to a menu this frame.
enum Press {
    Input(MenuInput),
    Point(ItemId),
}

/// Each frame: what the Input Devices, the keyboard and the mouse did goes to
/// the model, its effects are carried out, the flight behind the loading
/// screen is made, and the Simulation's clock follows the screen.
#[allow(clippy::too_many_arguments)]
fn drive_the_screens(
    mut screens: ResMut<Screens>,
    mut game: Game,
    real: Res<Time<Real>>,
    camera: Res<CameraSettings>,
    waiting: Res<PipelinesWaiting>,
    sound: Option<Res<QuadSoundPlaying>>,
    mut mouse: ResMut<MouseMenu>,
    mut current: ResMut<CurrentPage>,
    mut focus: ResMut<InputFocus>,
    mut rem: ResMut<RemSize>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    let now = real.elapsed();
    let input_now = game.pilot.input_clock(&real);
    // A click moves the widgets' own keyboard focus; the screens keep theirs.
    focus.clear();

    let Screens {
        menus,
        store,
        catalogue,
        packs,
        loading,
        said_about_sound,
        to_say,
    } = &mut *screens;
    let mut save = false;
    for text in to_say.drain(..) {
        menus.notice_for(text, now, LONG_NOTICE_FOR);
    }

    // Camera Tilt and FOV a flight changed go into the file with the rest.
    if camera.is_changed() && menus.set_camera(camera.clone()) {
        save = true;
    }
    for text in game.pilot.take_device_notices() {
        menus.notice(text, now);
    }
    if !*said_about_sound {
        match sound.as_deref().map(QuadSoundPlaying::device) {
            Some(opendrone_sound::Device::Opening) => {}
            Some(opendrone_sound::Device::Playing { .. }) => *said_about_sound = true,
            Some(opendrone_sound::Device::None(_)) | None => {
                menus.notice("No sound device: OpenDrone runs silent.", now);
                *said_about_sound = true;
            }
        }
    }

    // The Flying device setting reaches the pilot before it picks a device.
    game.pilot
        .set_flying_device(menus.settings().flying_device.clone());

    // What the screens show of the game.
    let plugged_in = game.pilot.devices();
    let devices: Vec<model::DeviceFacts> = plugged_in.iter().map(|(_, d)| d.clone()).collect();
    let flying_with = game
        .pilot
        .device_to_fly()
        .and_then(|id| game.pilot.device_name(id))
        .map(str::to_string);
    let in_the_air = quad_in_the_air(&game.flight, &game.plan);
    let facts = Facts {
        devices: &devices,
        catalogue,
        flying_with: flying_with.as_deref(),
        in_the_air,
    };

    // What the pilot did.
    let mut presses: Vec<(Option<DeviceId>, Press)> = game
        .pilot
        .take_menu_inputs()
        .into_iter()
        .map(|(from, input)| (from, Press::Input(input)))
        .collect();
    for action in mouse.0.drain(..) {
        match action {
            MouseAction::Point(id) => presses.push((None, Press::Point(id))),
            MouseAction::Click(id) => {
                presses.push((None, Press::Point(id)));
                presses.push((None, Press::Input(MenuInput::Select)));
            }
        }
    }
    let mut effects = Vec::new();
    for (from, press) in presses {
        match press {
            Press::Point(id) => menus.point_at(id, &facts),
            Press::Input(input) => effects.extend(menus.press(input, from, &facts, now)),
        }
    }
    effects.extend(menus.tick(now));

    for effect in effects {
        match effect {
            Effect::Load { by } => {
                // The loading screen shows first: making the flight can hold
                // a frame up.
                *loading = Some(Loading::begin(by, now));
                game.drawing.wanted = false;
            }
            Effect::Reset => {
                let mut world = FlightWorld {
                    flight: &mut game.flight,
                    plan: &game.plan,
                    fixed_clock: game.fixed.elapsed(),
                    game_clock: &mut game.clock,
                };
                game.pilot.reset(input_now, &mut world);
            }
            Effect::ToHub => {
                let mut world = FlightWorld {
                    flight: &mut game.flight,
                    plan: &game.plan,
                    fixed_clock: game.fixed.elapsed(),
                    game_clock: &mut game.clock,
                };
                game.pilot.stop_flying(input_now, &mut world);
                game.drawing.wanted = false;
                *loading = None;
            }
            Effect::Quit => {
                exit.write(AppExit::Success);
            }
            Effect::SettingsChanged => save = true,
            // The clock follows the screen, below.
            Effect::Freeze | Effect::GoOn => {}
        }
    }

    // The flight behind the loading screen.
    if menus.screen() == Screen::Loading
        && let Some(state) = loading
    {
        let mut done = false;
        state.frames += 1;
        match state.stage {
            Stage::Showing => {
                menus.set_progress(0.05);
                if state.frames > SHOWN_FIRST_FRAMES {
                    let settings = Settings::from_pilot(menus.settings());
                    match FlightPlan::from_packs(packs, settings) {
                        Ok(new_plan) => {
                            *game.plan = new_plan;
                            let device = game.pilot.device_for_flight(state.by);
                            let mut world = FlightWorld {
                                flight: &mut game.flight,
                                plan: &game.plan,
                                fixed_clock: game.fixed.elapsed(),
                                game_clock: &mut game.clock,
                            };
                            game.pilot.begin_flight(device, input_now, &mut world);
                            game.poses.0 = PoseHistory::default();
                            menus.set_arm_bound(device.and_then(|d| game.pilot.arm_bound(d)));
                            // The Quad's sound follows the Quad, in a game that
                            // has sound.
                            if let Some(playing) = sound.as_deref()
                                && playing.quad() != game.plan.quad.id
                            {
                                match QuadSoundPlaying::start(&game.plan) {
                                    Some(playing) => commands.insert_resource(playing),
                                    None => commands.remove_resource::<QuadSoundPlaying>(),
                                }
                            }
                            game.drawing.wanted = true;
                            state.stage = Stage::Drawing;
                        }
                        Err(problem) => {
                            warn!("the flight can't be made: {problem}");
                            menus.fail_loading(
                                "That Map and Quad can't be flown together. See the terminal for why.",
                                now,
                            );
                            done = true;
                        }
                    }
                }
            }
            Stage::Drawing => {
                menus.set_progress(0.25);
                if game.drawing.is_drawn() {
                    state.stage = Stage::Warming;
                }
            }
            Stage::Warming => {
                state.warmed += 1;
                let still = waiting.0.load(Ordering::Relaxed);
                state.peak = state.peak.max(still);
                if still == 0 {
                    state.quiet += 1;
                } else {
                    state.quiet = 0;
                }
                let by_frames = (state.warmed as f32 / WARM_FRAMES as f32).min(1.0);
                let by_pipelines = if state.peak == 0 {
                    by_frames
                } else {
                    1.0 - still as f32 / state.peak as f32
                };
                menus.set_progress(0.3 + 0.7 * by_frames.min(by_pipelines));
                let ready = state.warmed >= WARM_FRAMES && state.quiet >= QUIET_FRAMES;
                let gave_up = now.saturating_sub(state.started) > LOADING_GIVES_UP;
                if ready || gave_up {
                    if gave_up && !ready {
                        warn!("the shaders took too long to build, so the flight starts anyway");
                    }
                    menus.finish_loading();
                    done = true;
                }
            }
        }
        if done {
            *loading = None;
        }
    }

    // The world is drawn behind the screens that show it. The loading screen
    // asks for it itself, once the flight is made.
    if loading.is_none() {
        game.drawing.wanted = menus.world_shown();
    }

    // The Simulation's clock and the Input Devices follow the screen.
    let frozen = menus.frozen();
    if frozen != game.clock.is_paused() {
        if frozen {
            game.clock.pause();
        } else {
            game.clock.unpause();
        }
    }
    game.pilot.set_input_mode(menus.input_mode());
    let wanted_rem = DEFAULT_REM_SIZE_PX * menus.settings().text_size.percent() as f32 / 100.0;
    if rem.0 != wanted_rem {
        rem.0 = wanted_rem;
    }
    if save
        && let Some(store) = store
        && let Err(problem) = store.save(menus.settings())
    {
        warn!(
            "the settings can't be saved to {}: {problem}",
            store.path().display()
        );
    }

    current.0 = Some(menus.page(now, &facts));
}

/// Whether the Quad is in the air: for Resume's 3-2-1.
fn quad_in_the_air(flight: &Flight, plan: &FlightPlan) -> bool {
    let output = flight.simulation().quad_output(0);
    let armed = output.flight_controller.is_some_and(|record| record.armed);
    let thrust: f64 = output.motors.iter().map(|m| m.thrust).sum();
    let weight = plan.quad.parameters.mass * plan.map.world.gravity;
    model::quad_is_in_the_air(output.state.velocity.length(), armed, thrust, weight)
}
