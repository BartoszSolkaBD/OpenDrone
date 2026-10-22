//! The pilot's Input Devices: their Channels become Flight Inputs, stamped
//! with Simulation Time, and their buttons and switches fire Actions.
//!
//! - **The Flying Input Device** is the device that starts the flight: until
//!   one is picked, the Quad waits on the floor, and the first device whose
//!   sticks the pilot moves becomes it. The flight then starts afresh, set up
//!   with that device's facts (its Report Rate, and whether it reports at
//!   rest), as a new Quad does. Only its Channels fly.
//! - **Every report counts** from a device that reports at rest, such as a
//!   DualSense whose motion sensors run: each one enters the Simulation as
//!   Channels, changed or not, so the Radio Link hears its beat and its
//!   silence (ADR-0020). Other devices send Channels when they change.
//! - **While paused,** the Flight Inputs wait, and a run of Channels with
//!   nothing between them shrinks to the latest. A resting DualSense reports
//!   250 times a second, and without this all of them would land on the
//!   first step after flying goes on. Lost and back, and Reset, keep their
//!   place and still arrive.
//! - **Lost and back:** an unplug and a plug-in enter as the Flying Input
//!   Device lost and back. Silence doesn't need to: the Radio Link counts it
//!   itself, from the reports.
//! - **Actions** fire once, when their button or switch moves into its bound
//!   position, from every connected device (the DualSense's Options pauses
//!   and Create resets; the Pocket's SE resets). On the keyboard, R resets and
//!   Esc pauses, and keys drive the Virtual Switches the Flying Input
//!   Device's profile binds to them.
//! - **When the Quad disarms,** for any reason, every Arm Virtual Switch
//!   turns itself off.
//! - **Save Flight** is an Action with no default key on any device: a
//!   profile may bind it. It writes the flight so far as a Scenario and an
//!   Input Track ([`crate::save`]), and says where in the window's title.
//! - **Watching a Scenario** ([`Pilot::watching`]): no Input Device flies,
//!   Reset and Save Flight do nothing, and Pause works as always.
//! - **With the screens** ([`Pilot::with_screens`], #68): the Pause Action
//!   asks the screens for the Pause Menu instead of freezing the game
//!   itself, and the pilot has two modes ([`InputMode`]). In flight the
//!   Input Devices fly. In a menu they move the cursor
//!   ([`crate::screens::menu_input`]): a Radio by its right stick and a flick
//!   of yaw, a Gamepad by its D-pad, left stick and buttons, and the keyboard
//!   by its arrows, Enter and Backspace. The throttle and the switches are
//!   never read, and Reset and the camera's keys do nothing in a menu. The
//!   device that selects FLY becomes the Flying Input Device
//!   ([`Pilot::begin_flight`]).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use opendrone_input::channels::flight_mode_us;
use opendrone_input::controls::Key;
use opendrone_input::profile::{Action, FlightMode, Kind};
use opendrone_input::{Batch, DeviceId, InputEvent, InputThread, Inputs, Lost};
use opendrone_sim::{Channel, Channels, FlightInput, InputDeviceFacts, ReportRate};

use crate::clock::SimulationClock;
use crate::save::save_flight;
use crate::screens::menu_input::{MenuInput, PadMenu, RadioMenu};
use crate::screens::model::{DeviceFacts, InputMode};
use crate::set_up::{FlightPlan, repo_folder};
use crate::stepping::Flight;

/// Devices found in the first moments after the input thread starts were
/// plugged in before the game, so they bring no "connected" notice.
const PLUGGED_IN_BEFORE: Duration = Duration::from_millis(1500);

/// How long a note about Save Flight stays in the window's title.
const NOTICE_FOR: Duration = Duration::from_secs(6);

/// How far a stick must move from where it rested when its device was found
/// to start the flight: a quarter of the way from centre to full, so a
/// resting pad's flicker never does.
const STARTS_THE_FLIGHT_US: f64 = 128.0;

/// The pilot's Input Devices and what they've done so far.
#[derive(Resource)]
pub struct Pilot {
    /// SDL's input thread, or `None` if it couldn't start (then only the
    /// keyboard works).
    thread: Option<Mutex<InputThread>>,
    /// Why the input thread didn't start.
    pub thread_problem: Option<String>,
    inputs: Inputs,
    clock: SimulationClock,
    flying: Option<DeviceId>,
    /// The Flying Input Device went quiet for too long while it reports at
    /// rest. The Radio Link counts that silence itself, so its "back" isn't
    /// handed over either.
    silent: bool,
    /// Each device's Channels when it was found.
    found_with: BTreeMap<DeviceId, opendrone_input::Channels>,
    /// The Actions each device held last frame.
    actions_held: BTreeMap<DeviceId, Vec<Action>>,
    /// Whether the Quad was armed after the last frame's steps.
    armed: bool,
    flight_mode: FlightMode,
    /// Where Save Flight writes: `scenarios/saved/` in the repo.
    saves_to: PathBuf,
    /// What the last Save Flight said, and when, on the input thread's clock.
    notice: Option<(String, Duration)>,
    /// A Scenario is being watched: nothing flies.
    watching: bool,
    /// The flight keeps the run's fingerprint too: for the readable checks.
    checking_every_step: bool,
    /// The Flight Inputs that came while the game was paused, each with its
    /// moment on the input thread's clock, waiting to go on to the flight.
    /// Of a run of Channels with nothing between them, only the latest is
    /// kept.
    while_paused: Vec<(Duration, FlightInput)>,
    /// The screens drive the game (#68): Pause asks them for the Pause Menu.
    screens: bool,
    /// Whether the Input Devices fly the Quad or move the cursor.
    mode: InputMode,
    /// Each device's menu mapping, while a menu is open.
    device_menus: BTreeMap<DeviceId, DeviceMenu>,
    /// What the menus were asked since the screens last took them, with the
    /// device that asked (`None`: the keyboard).
    menu_queue: Vec<(Option<DeviceId>, MenuInput)>,
    /// The last device that moved a menu.
    last_menu_device: Option<DeviceId>,
    /// Short notices about devices coming and going, since the screens last
    /// took them.
    device_notices: Vec<String>,
    /// Batches handed over by the caller instead of the input thread, read
    /// with the next frame's ([`Pilot::hand_over`]).
    handed: Vec<Batch>,
}

/// How one device moves around a menu.
enum DeviceMenu {
    Radio(RadioMenu),
    Pad(PadMenu),
}

impl Pilot {
    /// Starts the input thread, with every Pack's Input Device profiles.
    pub fn start(plan: &FlightPlan, profiles: Vec<opendrone_input::InputDeviceProfile>) -> Pilot {
        let (thread, thread_problem) = match InputThread::start() {
            Ok(thread) => {
                println!("Input Devices: SDL {} started", thread.sdl_version());
                (Some(Mutex::new(thread)), None)
            }
            Err(problem) => {
                eprintln!("no Input Devices: SDL didn't start: {problem}");
                (None, Some(problem))
            }
        };
        Pilot {
            thread,
            thread_problem,
            ..Pilot::without_input_thread(plan, profiles)
        }
    }

    /// A pilot whose device reports the caller hands over itself
    /// ([`Pilot::read`]), as the readable checks do, with no SDL.
    pub fn without_input_thread(
        plan: &FlightPlan,
        profiles: Vec<opendrone_input::InputDeviceProfile>,
    ) -> Pilot {
        Pilot {
            thread: None,
            thread_problem: None,
            inputs: Inputs::new(profiles),
            clock: SimulationClock::new(plan.settings.physics_rate),
            flying: None,
            silent: false,
            found_with: BTreeMap::new(),
            actions_held: BTreeMap::new(),
            armed: false,
            flight_mode: plan.settings.flight_mode,
            saves_to: repo_folder().join("scenarios").join("saved"),
            notice: None,
            watching: false,
            checking_every_step: false,
            while_paused: Vec::new(),
            screens: false,
            mode: InputMode::Flight,
            device_menus: BTreeMap::new(),
            menu_queue: Vec::new(),
            last_menu_device: None,
            device_notices: Vec::new(),
            handed: Vec::new(),
        }
    }

    /// Hands over a batch of device reports as the input thread would, to be
    /// read with the next frame's: for the readable checks, which run the
    /// game's systems with no SDL.
    pub fn hand_over(&mut self, batch: Batch) {
        self.handed.push(batch);
    }

    /// A press on the keyboard or the mouse, as the menus take it: for the
    /// readable checks, which have no window to press keys in.
    pub fn press_in_menu(&mut self, input: MenuInput) {
        self.menu_queue.push((None, input));
    }

    /// A pilot whose game has screens ([`crate::screens`]): Pause asks them
    /// for the Pause Menu, and until a flight starts the Input Devices move
    /// the cursor.
    pub fn with_screens(mut self) -> Pilot {
        self.screens = true;
        self.mode = InputMode::Menu;
        self
    }

    /// Whether the Input Devices fly the Quad or move the cursor.
    pub fn input_mode(&self) -> InputMode {
        self.mode
    }

    /// Sets what the Input Devices do. Going into a menu starts every
    /// device's mapping afresh, so a stick held at that moment does nothing
    /// until it is let go; going into flight drops what the menus were asked.
    pub fn set_input_mode(&mut self, mode: InputMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.device_menus.clear();
        if mode == InputMode::Flight {
            self.menu_queue.clear();
        }
    }

    /// What the menus were asked since the last call, in order, with the
    /// device that asked (`None`: the keyboard).
    pub fn take_menu_inputs(&mut self) -> Vec<(Option<DeviceId>, MenuInput)> {
        std::mem::take(&mut self.menu_queue)
    }

    /// Short notices about Input Devices connecting and disconnecting, since
    /// the last call.
    pub fn take_device_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.device_notices)
    }

    /// The Input Devices that are plugged in, in the order they were found.
    pub fn devices(&self) -> Vec<(DeviceId, DeviceFacts)> {
        self.inputs
            .devices()
            .iter()
            .filter(|d| d.lost() != Some(Lost::Unplugged))
            .map(|d| {
                let arm_bound = d
                    .profile()
                    .is_some_and(|p| p.setup.switches().arm.is_some());
                (
                    d.id(),
                    DeviceFacts {
                        name: d.info().name.clone(),
                        kind: d.kind(),
                        arm_bound,
                    },
                )
            })
            .collect()
    }

    /// The Input Device that flies, or will fly if the pilot presses FLY
    /// with the keyboard or the mouse: the Flying Input Device, else the
    /// last device that moved a menu, else the only one plugged in.
    pub fn device_to_fly(&self) -> Option<DeviceId> {
        let plugged_in = self.devices();
        let is_plugged_in = |id: DeviceId| plugged_in.iter().any(|(d, _)| *d == id);
        self.flying
            .filter(|id| is_plugged_in(*id))
            .or(self.last_menu_device.filter(|id| is_plugged_in(*id)))
            .or(match plugged_in.as_slice() {
                [(only, _)] => Some(*only),
                _ => None,
            })
    }

    /// A device's name.
    pub fn device_name(&self, id: DeviceId) -> Option<&str> {
        Some(&self.inputs.device(id)?.info().name)
    }

    /// Whether the device's profile binds an Arm switch.
    pub fn arm_bound(&self, id: DeviceId) -> Option<bool> {
        let device = self.inputs.device(id)?;
        Some(
            device
                .profile()
                .is_some_and(|p| p.setup.switches().arm.is_some()),
        )
    }

    /// The flights this pilot starts also keep the fingerprint of the whole
    /// run ([`Flight::checking_every_step`]): for the readable checks, never
    /// the game.
    pub fn checking_every_step(mut self) -> Pilot {
        self.checking_every_step = true;
        self
    }

    /// Where Save Flight writes, instead of `scenarios/saved/` in the repo.
    pub fn saving_to(mut self, folder: PathBuf) -> Pilot {
        self.saves_to = folder;
        self
    }

    /// A pilot who only watches a Scenario ([`crate::watch`]): no Input
    /// Device becomes the Flying Input Device, Reset and Save Flight do
    /// nothing, and Pause and the camera's keys work as always.
    pub fn watching(mut self) -> Pilot {
        self.watching = true;
        self
    }

    /// What Save Flight said last, for a few seconds after it did.
    pub fn notice(&self, real: &Time<Real>) -> Option<&str> {
        let (text, at) = self.notice.as_ref()?;
        (self.input_clock(real) < *at + NOTICE_FOR).then_some(text.as_str())
    }

    /// One frame's reading, before its ticks: the input thread's batches
    /// since the last frame, read at `now` on its clock. Their Channels,
    /// lost and back enter the flight as Flight Inputs; the first device
    /// whose sticks move starts the flight; Actions fire; and if the Quad
    /// disarmed during the last frame's steps, every Arm Virtual Switch turns
    /// itself off.
    pub fn read(&mut self, batches: Vec<Batch>, now: Duration, world: &mut FlightWorld<'_>) {
        self.clock.follow(now, world.game_clock.elapsed());
        if !world.game_clock.is_paused() {
            self.go_on(world);
        }

        let armed = world
            .flight
            .simulation()
            .quad_output(0)
            .flight_controller
            .is_some_and(|record| record.armed);
        if std::mem::replace(&mut self.armed, armed) && !armed {
            let events = self.inputs.disarmed(now);
            self.handle(events, world);
        }

        for batch in batches {
            let at = batch.at;
            let events = self.inputs.take(batch);
            let changed = |device| {
                events
                    .iter()
                    .any(|e| matches!(e, InputEvent::Channels { device: d, .. } if *d == device))
            };
            // A report that changed nothing, from a Flying Input Device that
            // reports at rest: its Channels still enter, so the Radio Link
            // hears the beat.
            let heartbeat = self.flying.and_then(|id| {
                let device = self.inputs.device(id)?;
                (device.lost_when_silent() && device.last_heard() == at && !changed(id))
                    .then(|| device.channels())
            });
            self.handle(events, world);
            if let Some(channels) = heartbeat
                && self.flying.is_some()
            {
                let channels = self.channels(&channels);
                self.forward(world, at, FlightInput::Channels(channels));
            }
        }
        let silence = self.inputs.check_silence(now);
        self.handle(silence, world);

        // Actions, from every connected device.
        let mut fired = Vec::new();
        for device in self.inputs.devices() {
            if device.lost().is_some() {
                continue;
            }
            let held = device.actions_held();
            match self.actions_held.insert(device.id(), held.clone()) {
                // Held when it was found: not a press.
                None => {}
                Some(before) => fired.extend(held.into_iter().filter(|a| !before.contains(a))),
            }
        }
        for action in fired {
            self.fire(action, now, world);
        }

        if self.mode == InputMode::Menu {
            self.read_menus(now);
        }
    }

    /// What each connected device does to a menu this frame.
    fn read_menus(&mut self, now: Duration) {
        let connected: Vec<DeviceId> = self
            .inputs
            .devices()
            .iter()
            .filter(|d| d.lost().is_none())
            .map(|d| d.id())
            .collect();
        self.device_menus.retain(|id, _| connected.contains(id));
        for device in self.inputs.devices() {
            if !connected.contains(&device.id()) {
                continue;
            }
            let menu =
                self.device_menus
                    .entry(device.id())
                    .or_insert_with(|| match device.kind() {
                        Kind::Radio => DeviceMenu::Radio(RadioMenu::new()),
                        Kind::Gamepad => DeviceMenu::Pad(PadMenu::new()),
                    });
            let moves = match menu {
                DeviceMenu::Radio(radio) => {
                    let channels = device.channels();
                    radio.read(now, channels.roll, channels.pitch, channels.yaw)
                }
                DeviceMenu::Pad(pad) => match &device.state().pad {
                    Some(state) => pad.read(now, state),
                    None => Vec::new(),
                },
            };
            if !moves.is_empty() {
                self.last_menu_device = Some(device.id());
            }
            self.menu_queue
                .extend(moves.into_iter().map(|m| (Some(device.id()), m)));
        }
    }

    /// Starts a flight, set up from the plan, as the pilot pressed FLY or
    /// swapped the Map or the Quad: `device` flies it, set up with its
    /// facts, as a Quad flown from the first stick movement is
    /// ([`Pilot::read`]). With `None` the Quad waits on the Launch Spot until
    /// a device's sticks move, and the flight starts afresh then. Anything
    /// that waited belonged to the old flight.
    pub fn begin_flight(
        &mut self,
        device: Option<DeviceId>,
        at: Duration,
        world: &mut FlightWorld<'_>,
    ) {
        self.while_paused.clear();
        self.flying = None;
        self.silent = false;
        if let Some(device) = device {
            let events = self.start_flight(device, at, world);
            self.handle(events, world);
            if self.flying == Some(device) {
                return;
            }
        }
        // No device, or one that can't fly: a flight nobody flies yet.
        match world.plan.simulation(None).and_then(Flight::new) {
            Ok(flight) => {
                *world.flight = flight;
                self.clock.start_flight(world.fixed_clock);
                let events = self.inputs.set_flying(None, at);
                self.handle(events, world);
            }
            Err(problem) => error!("the flight can't start: {problem}"),
        }
    }

    /// Resets the Quad to its Launch Spot, powered up afresh: the Pause
    /// Menu's Reset.
    pub fn reset(&mut self, at: Duration, world: &mut FlightWorld<'_>) {
        info!("Reset");
        self.forward(world, at, FlightInput::Reset);
    }

    /// The flight is left (Back to the Hub): no Input Device flies.
    pub fn stop_flying(&mut self, at: Duration, world: &mut FlightWorld<'_>) {
        self.while_paused.clear();
        self.flying = None;
        self.silent = false;
        let events = self.inputs.set_flying(None, at);
        self.handle(events, world);
    }

    /// The Flying Input Device's name, once the pilot has picked one.
    pub fn flying_name(&self) -> Option<&str> {
        let device = self.inputs.device(self.flying?)?;
        Some(&device.info().name)
    }

    /// Whether the Flying Input Device is lost, and why.
    pub fn flying_lost(&self) -> Option<Lost> {
        self.inputs.device(self.flying?)?.lost()
    }

    /// The Actions held on every connected Input Device, each once, in a
    /// fixed order. The camera's Actions repeat while held, so it reads what
    /// is held rather than the presses [`Pilot::read`] fires.
    pub fn actions_held(&self) -> Vec<Action> {
        let mut held: Vec<Action> = self
            .inputs
            .devices()
            .iter()
            .filter(|device| device.lost().is_none())
            .flat_map(|device| device.actions_held())
            .collect();
        held.sort();
        held.dedup();
        held
    }

    /// How many Input Devices are plugged in.
    pub fn devices_plugged_in(&self) -> usize {
        self.inputs
            .devices()
            .iter()
            .filter(|d| d.lost() != Some(Lost::Unplugged))
            .count()
    }

    /// The input thread's clock at this frame's moment: the time since it
    /// started, as Bevy's real clock read it this frame.
    pub fn input_clock(&self, real: &Time<Real>) -> Duration {
        let started = match &self.thread {
            Some(thread) => thread.lock().map_or(real.startup(), |t| t.started()),
            None => real.startup(),
        };
        real.last_update()
            .unwrap_or(started)
            .saturating_duration_since(started)
    }

    /// Hands what `Inputs` gave over to the flight: Channels, lost and back
    /// as Flight Inputs, and picks the Flying Input Device.
    fn handle(&mut self, events: Vec<InputEvent>, world: &mut FlightWorld<'_>) {
        let mut queue = std::collections::VecDeque::from(events);
        while let Some(event) = queue.pop_front() {
            match event {
                InputEvent::Found {
                    device,
                    name,
                    profile,
                    at,
                    ..
                } => {
                    info!(
                        "Input Device found: {name} ({})",
                        profile.as_deref().unwrap_or("no profile fits it")
                    );
                    if let Some(found) = self.inputs.device(device) {
                        self.found_with.insert(device, found.channels());
                    }
                    self.notice_about(at, format!("{name} connected"));
                }
                InputEvent::Channels {
                    device,
                    at,
                    channels,
                } => {
                    if self.flying == Some(device) {
                        self.forward(world, at, FlightInput::Channels(self.channels(&channels)));
                    } else if !self.watching
                        && self.flying.is_none()
                        // With the screens, sticks move the cursor in a menu:
                        // only in flight does a stick move pick the device.
                        && self.mode == InputMode::Flight
                        && self.sticks_moved(device, &channels)
                    {
                        queue.extend(self.start_flight(device, at, world));
                    }
                }
                InputEvent::Lost { device, at, why } => {
                    let name = self
                        .device_name(device)
                        .unwrap_or("An Input Device")
                        .to_string();
                    let text = match why {
                        Lost::Unplugged => format!("{name} disconnected"),
                        Lost::Silent => format!("{name} stopped responding"),
                    };
                    self.notice_about(at, text);
                    if self.flying == Some(device) {
                        match why {
                            Lost::Unplugged => {
                                info!("the Flying Input Device was unplugged");
                                self.silent = false;
                                self.forward(world, at, FlightInput::InputDeviceLost);
                            }
                            Lost::Silent => {
                                info!("the Flying Input Device went silent");
                                self.silent = true;
                            }
                        }
                    }
                }
                InputEvent::Back { device, at } => {
                    let name = self
                        .device_name(device)
                        .unwrap_or("An Input Device")
                        .to_string();
                    self.notice_about(at, format!("{name} connected"));
                    if self.flying == Some(device) {
                        info!("the Flying Input Device is back");
                        if !std::mem::take(&mut self.silent) {
                            self.forward(world, at, FlightInput::InputDeviceBack);
                        }
                    }
                }
            }
        }
    }

    /// Keeps a short notice about a device for the screens, unless the
    /// device was plugged in before the game started (`at` on the input
    /// thread's clock).
    fn notice_about(&mut self, at: Duration, text: String) {
        if self.screens && at >= PLUGGED_IN_BEFORE {
            self.device_notices.push(text);
        }
    }

    /// The device that moved its sticks starts the flight: a fresh flight,
    /// set up with its facts, with its Channels from the start.
    fn start_flight(
        &mut self,
        device: DeviceId,
        at: Duration,
        world: &mut FlightWorld<'_>,
    ) -> Vec<InputEvent> {
        let Some(found) = self.inputs.device(device) else {
            return Vec::new();
        };
        let connection = found.info().connection;
        let facts = InputDeviceFacts {
            report_rate: found
                .profile()
                .and_then(|p| p.report_rate(connection))
                .and_then(|hz| ReportRate::from_hz(hz.round() as u32)),
            reports_at_rest: found.lost_when_silent(),
        };
        if found.heartbeat_missing() {
            warn!(
                "{}: its profile says it reports at rest, but its heartbeat didn't start, so it counts as lost only when unplugged",
                found.info().name
            );
        }
        let name = found.info().name.clone();
        let flight = world
            .plan
            .simulation(Some(facts))
            .and_then(|simulation| Flight::keeping(simulation, facts));
        let flight = match flight {
            Ok(flight) if self.checking_every_step => flight.checking_every_step(),
            Ok(flight) => flight,
            Err(problem) => {
                error!("the flight can't start: {problem}");
                return Vec::new();
            }
        };
        *world.flight = flight;
        // What waited belonged to the old flight.
        self.while_paused.clear();
        self.clock.start_flight(world.fixed_clock);
        self.flying = Some(device);
        self.silent = false;
        info!("{name} is the Flying Input Device: the flight starts");
        let events = self.inputs.set_flying(Some(device), at);
        if let Some(found) = self.inputs.device(device) {
            let channels = self.channels(&found.channels());
            world.flight.input(
                opendrone_sim::SimulationTime::START,
                FlightInput::Channels(channels),
            );
        }
        events
    }

    /// Whether a device's sticks moved far from where they rested when it
    /// was found.
    fn sticks_moved(&self, device: DeviceId, channels: &opendrone_input::Channels) -> bool {
        let Some(rest) = self.found_with.get(&device) else {
            return false;
        };
        [
            (channels.roll, rest.roll),
            (channels.pitch, rest.pitch),
            (channels.throttle, rest.throttle),
            (channels.yaw, rest.yaw),
        ]
        .into_iter()
        .any(|(now, then)| (now - then).abs() >= STARTS_THE_FLIGHT_US)
    }

    /// An Input Device's Channels as a receiver hands them over. A switch
    /// with no source is low (Arm off, Crash Flip off), and a device with no
    /// Flight Mode switch flies the Flight Mode setting.
    fn channels(&self, channels: &opendrone_input::Channels) -> Channels {
        flight_channels(channels, self.flight_mode)
    }

    /// Hands the flight a Flight Input, unless the game is paused: then it
    /// waits, and a Channels input replaces a Channels input right before it
    /// ([`Pilot::go_on`]).
    fn forward(&mut self, world: &mut FlightWorld<'_>, at: Duration, input: FlightInput) {
        if world.game_clock.is_paused() {
            let replaces = matches!(input, FlightInput::Channels(_))
                && matches!(
                    self.while_paused.last(),
                    Some((_, FlightInput::Channels(_)))
                );
            if replaces {
                self.while_paused.pop();
            }
            self.while_paused.push((at, input));
        } else {
            self.go_on(world);
            world.flight.input(self.clock.simulation_time(at), input);
        }
    }

    /// Flying goes on: hands the flight what waited during the pause, in
    /// order.
    fn go_on(&mut self, world: &mut FlightWorld<'_>) {
        for (at, input) in std::mem::take(&mut self.while_paused) {
            world.flight.input(self.clock.simulation_time(at), input);
        }
    }

    /// Fires an Action.
    fn fire(&mut self, action: Action, at: Duration, world: &mut FlightWorld<'_>) {
        match action {
            Action::Reset if self.watching => {}
            // In a menu Reset is the Pause Menu's own item.
            Action::Reset if self.mode == InputMode::Menu => {}
            Action::Reset => {
                info!("Reset");
                self.forward(world, at, FlightInput::Reset);
            }
            // With the screens, Pause is theirs to answer: it opens the Pause
            // Menu, and in the Pause Menu it goes on.
            Action::Pause if self.screens => {
                self.menu_queue.push((None, MenuInput::Pause));
            }
            Action::Pause => {
                toggle_pause(world.game_clock);
                if !world.game_clock.is_paused() {
                    self.go_on(world);
                }
            }
            Action::SaveFlight => {
                let said = if self.watching {
                    "a watched Scenario has no flight to save".to_string()
                } else {
                    match save_flight(world.plan, world.flight, &self.saves_to) {
                        Ok(saved) => {
                            info!(
                                "saved the flight: {} and {}, {:.3} s, the state's fingerprint after the last step {}",
                                saved.scenario.display(),
                                saved.track.display(),
                                saved.seconds,
                                saved.state_fingerprint
                            );
                            let name = saved
                                .scenario
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            format!("flight saved as {name}")
                        }
                        Err(problem) => {
                            error!("the flight can't be saved: {problem}");
                            format!("the flight can't be saved: {problem}")
                        }
                    }
                };
                self.notice = Some((said, at));
            }
            // Camera Tilt and FOV never enter the Simulation: the camera's
            // plugin reads them held, to repeat them.
            Action::CameraTiltUp
            | Action::CameraTiltDown
            | Action::FovWider
            | Action::FovNarrower => {}
        }
    }
}

/// What handling the pilot's inputs reaches: the flight, how it is set up,
/// and the game's clocks.
pub struct FlightWorld<'w> {
    pub flight: &'w mut Flight,
    pub plan: &'w FlightPlan,
    /// Bevy's fixed clock: how much time Bevy's ticks have covered since
    /// the game started. A new flight's Simulation Time starts here.
    pub fixed_clock: Duration,
    /// Bevy's virtual clock, which Pause stops.
    pub game_clock: &'w mut Time<Virtual>,
}

/// An Input Device's Channels, in µs, as the whole-number steps a receiver
/// hands the Flight Controller. A switch with no source is low, and a device
/// with no Flight Mode switch flies `flight_mode`.
pub fn flight_channels(channels: &opendrone_input::Channels, flight_mode: FlightMode) -> Channels {
    Channels {
        roll: Channel::from_micros(channels.roll),
        pitch: Channel::from_micros(channels.pitch),
        throttle: Channel::from_micros(channels.throttle),
        yaw: Channel::from_micros(channels.yaw),
        arm: channels.arm.map_or(Channel::LOW, Channel::from_micros),
        flight_mode: Channel::from_micros(
            channels
                .flight_mode
                .unwrap_or_else(|| flight_mode_us(flight_mode)),
        ),
        crash_flip: channels
            .crash_flip
            .map_or(Channel::LOW, Channel::from_micros),
    }
}

/// Pause stops the game's clock, so no tick runs and the Simulation stands
/// still; pressing it again goes on.
pub fn toggle_pause(game_clock: &mut Time<Virtual>) {
    if game_clock.is_paused() {
        info!("flying on");
        game_clock.unpause();
    } else {
        info!("paused");
        game_clock.pause();
    }
}

/// Reads the Input Devices before each frame's ticks.
pub struct PilotPlugin;

impl Plugin for PilotPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, read_input_devices);
    }
}

/// Reads the keyboard too: the game's window has one.
pub struct KeyboardPlugin;

impl Plugin for KeyboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            read_keyboard
                .after(read_input_devices)
                .after(bevy::input::InputSystems),
        );
    }
}

/// What the pilot's systems reach: the pilot, the flight and the game's
/// clocks.
#[derive(SystemParam)]
pub(crate) struct PilotAndFlight<'w> {
    pilot: ResMut<'w, Pilot>,
    flight: NonSendMut<'w, Flight>,
    plan: Res<'w, FlightPlan>,
    real: Res<'w, Time<Real>>,
    game_clock: ResMut<'w, Time<Virtual>>,
    fixed: Res<'w, Time<Fixed>>,
}

impl PilotAndFlight<'_> {
    /// The pilot, the input thread's clock at this frame's moment, and what
    /// handling the pilot's inputs reaches.
    fn split(&mut self) -> (&mut Pilot, Duration, FlightWorld<'_>) {
        let now = self.pilot.input_clock(&self.real);
        (
            &mut self.pilot,
            now,
            FlightWorld {
                flight: &mut self.flight,
                plan: &self.plan,
                fixed_clock: self.fixed.elapsed(),
                game_clock: &mut self.game_clock,
            },
        )
    }
}

pub(crate) fn read_input_devices(mut reach: PilotAndFlight) {
    let (pilot, now, mut world) = reach.split();
    let mut batches = match &pilot.thread {
        Some(thread) => thread.lock().map(|t| t.batches()).unwrap_or_default(),
        None => Vec::new(),
    };
    batches.append(&mut pilot.handed);
    pilot.read(batches, now, &mut world);
}

pub(crate) fn read_keyboard(
    mut keys: MessageReader<KeyboardInput>,
    mut focus: MessageReader<WindowFocused>,
    mut reach: PilotAndFlight,
) {
    let (pilot, now, mut world) = reach.split();
    for key in keys.read() {
        let down = key.state.is_pressed();
        if down && pilot.mode == InputMode::Menu {
            // In a menu the arrows move the cursor, and a held arrow repeats.
            let input = match key.key_code {
                KeyCode::ArrowUp => Some(MenuInput::Up),
                KeyCode::ArrowDown => Some(MenuInput::Down),
                KeyCode::ArrowLeft => Some(MenuInput::Left),
                KeyCode::ArrowRight => Some(MenuInput::Right),
                KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space if !key.repeat => {
                    Some(MenuInput::Select)
                }
                KeyCode::Backspace if !key.repeat => Some(MenuInput::Back),
                KeyCode::KeyQ if !key.repeat => Some(MenuInput::PreviousTab),
                KeyCode::KeyE if !key.repeat => Some(MenuInput::NextTab),
                _ => None,
            };
            if let Some(input) = input {
                pilot.menu_queue.push((None, input));
            }
        }
        if down && !key.repeat {
            match key.key_code {
                KeyCode::KeyR => pilot.fire(Action::Reset, now, &mut world),
                KeyCode::Escape => pilot.fire(Action::Pause, now, &mut world),
                _ => {}
            }
        }
        if !key.repeat
            && let Some(name) = key_name(key.key_code)
            && let Some(named) = Key::named(name)
        {
            let events = pilot.inputs.key(named, down, now);
            pilot.handle(events, &mut world);
        }
    }
    for change in focus.read() {
        if !change.focused {
            let events = pilot.inputs.focus_lost(now);
            pilot.handle(events, &mut world);
        }
    }
}

/// A key's name as Input Device profiles write it, for the keys a profile
/// may bind to a Virtual Switch.
fn key_name(code: KeyCode) -> Option<&'static str> {
    use KeyCode as K;
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const F_KEYS: [&str; 12] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ];
    let letters = [
        K::KeyA,
        K::KeyB,
        K::KeyC,
        K::KeyD,
        K::KeyE,
        K::KeyF,
        K::KeyG,
        K::KeyH,
        K::KeyI,
        K::KeyJ,
        K::KeyK,
        K::KeyL,
        K::KeyM,
        K::KeyN,
        K::KeyO,
        K::KeyP,
        K::KeyQ,
        K::KeyR,
        K::KeyS,
        K::KeyT,
        K::KeyU,
        K::KeyV,
        K::KeyW,
        K::KeyX,
        K::KeyY,
        K::KeyZ,
    ];
    let digits = [
        K::Digit0,
        K::Digit1,
        K::Digit2,
        K::Digit3,
        K::Digit4,
        K::Digit5,
        K::Digit6,
        K::Digit7,
        K::Digit8,
        K::Digit9,
    ];
    let f_keys = [
        K::F1,
        K::F2,
        K::F3,
        K::F4,
        K::F5,
        K::F6,
        K::F7,
        K::F8,
        K::F9,
        K::F10,
        K::F11,
        K::F12,
    ];
    if let Some(i) = letters.iter().position(|k| *k == code) {
        return Some(LETTERS[i]);
    }
    if let Some(i) = digits.iter().position(|k| *k == code) {
        return Some(DIGITS[i]);
    }
    if let Some(i) = f_keys.iter().position(|k| *k == code) {
        return Some(F_KEYS[i]);
    }
    Some(match code {
        K::Space => "Space",
        K::Enter => "Enter",
        K::Tab => "Tab",
        K::Backspace => "Backspace",
        K::Escape => "Escape",
        K::ArrowUp => "Up",
        K::ArrowDown => "Down",
        K::ArrowLeft => "Left",
        K::ArrowRight => "Right",
        K::ShiftLeft => "LeftShift",
        K::ShiftRight => "RightShift",
        K::ControlLeft => "LeftCtrl",
        K::ControlRight => "RightCtrl",
        K::AltLeft => "LeftAlt",
        K::AltRight => "RightAlt",
        _ => return None,
    })
}
