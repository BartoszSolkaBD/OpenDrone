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

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use opendrone_input::channels::flight_mode_us;
use opendrone_input::controls::Key;
use opendrone_input::profile::{Action, FlightMode};
use opendrone_input::{Batch, DeviceId, InputEvent, InputThread, Inputs, Lost};
use opendrone_sim::{Channel, Channels, FlightInput, InputDeviceFacts, ReportRate};

use crate::clock::SimulationClock;
use crate::set_up::FlightPlan;
use crate::stepping::Flight;

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
        }
    }

    /// One frame's reading, before its ticks: the input thread's batches
    /// since the last frame, read at `now` on its clock. Their Channels,
    /// lost and back enter the flight as Flight Inputs; the first device
    /// whose sticks move starts the flight; Actions fire; and if the Quad
    /// disarmed during the last frame's steps, every Arm Virtual Switch turns
    /// itself off.
    pub fn read(&mut self, batches: Vec<Batch>, now: Duration, world: &mut FlightWorld<'_>) {
        self.clock.follow(now, world.game_clock.elapsed());

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
    fn input_clock(&self, real: &Time<Real>) -> Duration {
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
                    ..
                } => {
                    info!(
                        "Input Device found: {name} ({})",
                        profile.as_deref().unwrap_or("no profile fits it")
                    );
                    if let Some(found) = self.inputs.device(device) {
                        self.found_with.insert(device, found.channels());
                    }
                }
                InputEvent::Channels {
                    device,
                    at,
                    channels,
                } => {
                    if self.flying == Some(device) {
                        self.forward(world, at, FlightInput::Channels(self.channels(&channels)));
                    } else if self.flying.is_none() && self.sticks_moved(device, &channels) {
                        queue.extend(self.start_flight(device, at, world));
                    }
                }
                InputEvent::Lost { device, at, why } => {
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
        let flight = world.plan.simulation(Some(facts)).and_then(Flight::new);
        let flight = match flight {
            Ok(flight) => flight,
            Err(problem) => {
                error!("the flight can't start: {problem}");
                return Vec::new();
            }
        };
        *world.flight = flight;
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

    fn forward(&self, world: &mut FlightWorld<'_>, at: Duration, input: FlightInput) {
        world.flight.input(self.clock.simulation_time(at), input);
    }

    /// Fires an Action.
    fn fire(&self, action: Action, at: Duration, world: &mut FlightWorld<'_>) {
        match action {
            Action::Reset => {
                info!("Reset");
                self.forward(world, at, FlightInput::Reset);
            }
            Action::Pause => toggle_pause(world.game_clock),
            // Camera Tilt, FOV and Save Flight arrive with their tickets.
            _ => {}
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
struct PilotAndFlight<'w> {
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

fn read_input_devices(mut reach: PilotAndFlight) {
    let (pilot, now, mut world) = reach.split();
    let batches = match &pilot.thread {
        Some(thread) => thread.lock().map(|t| t.batches()).unwrap_or_default(),
        None => Vec::new(),
    };
    pilot.read(batches, now, &mut world);
}

fn read_keyboard(
    mut keys: MessageReader<KeyboardInput>,
    mut focus: MessageReader<WindowFocused>,
    mut reach: PilotAndFlight,
) {
    let (pilot, now, mut world) = reach.split();
    for key in keys.read() {
        let down = key.state.is_pressed();
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
