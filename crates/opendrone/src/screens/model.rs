//! What each screen holds and how a pilot moves between them: plain code, no
//! Bevy, so the readable checks drive it with a list of presses.
//!
//! [`Menus`] is the model. A press ([`MenuInput`]) goes in with the facts the
//! game knows ([`Facts`]: the Input Devices, the Maps and Quads in the Packs,
//! whether the Quad is in the air), and out come the [`Effect`]s the game must
//! carry out: start the flight, reset it, freeze the Simulation, go back to
//! the Hub. The screen as a pilot sees it comes from [`Menus::page`], again
//! with the facts, as plain data that the view draws.
//!
//! The flow is the one #23 settled (variant A):
//!
//! ```text
//! First Launch -> Input Device step -> Hub -> loading screen -> Free Flight
//!                                       ^                          |  Esc
//!                                       +---- Back to the Hub ---- Pause Menu
//!                                                                  (Resume, Reset, Map, Quad, Settings, Quit)
//! ```
//!
//! A returning pilot reaches the Launch Spot in one press: FLY, selected on
//! arrival. A new install takes three when it skips Input Device setup:
//! answer the question, Set up later, FLY.
//!
//! Every setting is one stop in a menu, changed with left and right, and
//! saved the moment it changes ([`Effect::SettingsChanged`]). There are no
//! drop-down lists.

use std::path::PathBuf;
use std::time::Duration;

use opendrone_input::DeviceId;
use opendrone_input::profile::Kind;

use super::LONG_NOTICE_FOR;
use super::menu_input::MenuInput;
use crate::camera::controls::CameraSettings;
use crate::settings_file::{FlyingDevice, PilotSettings, Preset, TextSize};

/// How long a short notice stays on a screen.
pub const NOTICE_FOR: Duration = Duration::from_secs(6);

/// Resume counts down this many seconds when the Quad was in the air.
pub const COUNTDOWN_SECONDS: u32 = 3;

/// A screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The first of First Launch's two steps: Text size, Reduce motion, the
    /// flashing-images notice and "Have you flown FPV before?".
    FirstLaunch,
    /// The second step: the Input Device.
    InputDeviceStep,
    Hub,
    /// After FLY, and after swapping the Map or Quad: the world is made and
    /// its shaders compile.
    Loading,
    /// Free Flight. The Simulation runs, unless a 3-2-1 is counting.
    Flying,
    PauseMenu,
    /// The Pause Menu's Map or Quad picker.
    Picker(Pick),
    Settings,
}

/// What a picker picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Map,
    Quad,
}

/// The answers to "Have you flown FPV before?", each picking a starting
/// Preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Experience {
    NeverFlown,
    ALittle,
    Yes,
    Skip,
}

impl Experience {
    pub const ALL: [Experience; 4] = [
        Experience::NeverFlown,
        Experience::ALittle,
        Experience::Yes,
        Experience::Skip,
    ];

    /// The words on the button.
    pub fn answer(self) -> &'static str {
        match self {
            Experience::NeverFlown => "No, never",
            Experience::ALittle => "A little: in a sim, or line of sight",
            Experience::Yes => "Yes, I fly real quads in Acro",
            Experience::Skip => "Skip",
        }
    }

    /// The Preset it starts on: No, never is Beginner; A little is
    /// Intermediate; Yes is Pro; Skip is Beginner.
    pub fn preset(self) -> Preset {
        match self {
            Experience::NeverFlown | Experience::Skip => Preset::Beginner,
            Experience::ALittle => Preset::Intermediate,
            Experience::Yes => Preset::Pro,
        }
    }
}

/// Something a pilot can point at on a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemId {
    TextSize,
    ReduceMotion,
    AutoArm,
    FlyingDevice,
    Answer(Experience),
    SetUp,
    SetUpLater,
    ContinueWithout,
    Fly,
    MapChip,
    QuadChip,
    PresetChip,
    OpenSettings,
    Quit,
    Resume,
    Reset,
    OpenMapPicker,
    OpenQuadPicker,
    BackToHub,
    PickMap(usize),
    PickQuad(usize),
    Back,
}

/// How an item is drawn and what a press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// FLY: the big button.
    Fly,
    Button,
    /// A setting: one stop, changed with left and right. Select goes to the
    /// next value.
    Stop,
}

/// One thing on a screen a pilot can stop on.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub kind: ItemKind,
    pub label: String,
    /// A setting's value.
    pub value: Option<String>,
    /// A line of small print under it.
    pub note: Option<String>,
    /// A picture beside it: a Quad's, in the Quad picker.
    pub picture: Option<PathBuf>,
    /// The item is what's chosen now, in a picker.
    pub current: bool,
}

impl Item {
    fn new(id: ItemId, kind: ItemKind, label: impl Into<String>) -> Item {
        Item {
            id,
            kind,
            label: label.into(),
            value: None,
            note: None,
            picture: None,
            current: false,
        }
    }

    fn button(id: ItemId, label: impl Into<String>) -> Item {
        Item::new(id, ItemKind::Button, label)
    }

    fn stop(id: ItemId, label: impl Into<String>, value: impl Into<String>) -> Item {
        Item {
            value: Some(value.into()),
            ..Item::new(id, ItemKind::Stop, label)
        }
    }
}

/// A Map a pilot can pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapChoice {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// A Quad a pilot can pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadChoice {
    pub id: String,
    pub name: String,
    pub description: String,
    pub spec_line: String,
    /// Its picture, a file in the Quad's folder.
    pub picture: Option<PathBuf>,
}

/// The Maps and Quads the Packs give, in id order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    pub maps: Vec<MapChoice>,
    pub quads: Vec<QuadChoice>,
}

/// An Input Device that is plugged in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceFacts {
    /// What SDL calls it, such as "DualSense Wireless Controller".
    pub name: String,
    /// The on-screen name of the Input Device profile it flies on, such as
    /// "DualSense": what the Flying device setting names.
    pub profile_name: String,
    pub kind: Kind,
    /// Its profile binds an Arm switch.
    pub arm_bound: bool,
}

/// What the game knows that the screens show.
#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    /// The Input Devices plugged in, in the order they were found.
    pub devices: &'a [DeviceFacts],
    pub catalogue: &'a Catalogue,
    /// The Input Device that flies or will fly: the one that pressed FLY, or
    /// the one the Flying device setting picks. `None` when it isn't known
    /// yet: nothing is plugged in, or several of the same tier tie and the
    /// first to move flies.
    pub flying_with: Option<&'a str>,
    /// Whether the Quad is in the air now ([`quad_is_in_the_air`]).
    pub in_the_air: bool,
}

/// What the game must do after a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Make the flight from the settings' Map and Quad, and show the loading
    /// screen. `by` is the Input Device that pressed the button, which then
    /// flies; `None` keeps the Flying Input Device the flight has, or picks
    /// one if it has none.
    Load {
        by: Option<DeviceId>,
    },
    /// Reset the Quad to its Launch Spot, powered up afresh, and fly on.
    Reset,
    /// Freeze the Simulation: the Pause Menu opens.
    Freeze,
    /// Fly on: the Simulation runs again.
    GoOn,
    /// Leave the flight: the world goes, and no Input Device flies.
    ToHub,
    Quit,
    /// A setting changed: save the file.
    SettingsChanged,
}

/// A short notice, for a few seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Notice {
    text: String,
    until: Duration,
}

/// What a screen shows, as plain data.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub screen: Screen,
    pub title: String,
    /// Plain lines above the items.
    pub lines: Vec<String>,
    pub items: Vec<Item>,
    /// Which item the cursor is on.
    pub cursor: usize,
    /// "Flying with: ..." on the Hub.
    pub flying_with: Option<String>,
    /// The Hub's Pre-flight Warnings. They never block FLY.
    pub warnings: Vec<String>,
    pub notices: Vec<String>,
    pub loading: Option<LoadingPage>,
    /// The 3, 2 or 1 of a Resume.
    pub countdown: Option<u32>,
    /// The flight's world is drawn behind this screen, which then lets it
    /// show through (the Pause Menu, its pickers and its Settings).
    pub over_the_world: bool,
}

/// What the loading screen shows.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadingPage {
    pub map: String,
    pub quad: String,
    pub preset: String,
    /// From 0 to 1.
    pub progress: f32,
    /// What the keys do, with a ⚠ on one with no binding.
    pub keys: Vec<KeyHint>,
}

/// One line of the loading screen's reminder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHint {
    pub action: String,
    /// The key or button, or the words that say it isn't bound.
    pub key: String,
    /// Show the ⚠: nothing is bound.
    pub warning: bool,
}

/// The model of every screen.
#[derive(Clone, Debug)]
pub struct Menus {
    screen: Screen,
    settings: PilotSettings,
    cursor: usize,
    /// Where Settings goes back to, and where the cursor was.
    settings_from: (Screen, usize),
    /// The Quad was in the air when the Pause Menu opened.
    paused_in_air: bool,
    /// A Resume's 3-2-1 ends at this moment.
    resuming_until: Option<Duration>,
    notices: Vec<Notice>,
    progress: f32,
    /// Whether the Flying Input Device's profile binds Arm, for the loading
    /// screen's reminder: set by the game once it knows which device flies.
    arm_bound: Option<bool>,
}

impl Menus {
    /// The screens of a pilot with these settings: First Launch on a new
    /// install, the Hub after.
    pub fn new(settings: PilotSettings) -> Menus {
        let screen = if settings.first_launch_done {
            Screen::Hub
        } else {
            Screen::FirstLaunch
        };
        Menus::starting_on(settings, screen)
    }

    /// The screens, opening on `screen`: for a developer command that shows
    /// one of them.
    pub fn starting_on(settings: PilotSettings, screen: Screen) -> Menus {
        let mut menus = Menus {
            screen,
            settings,
            cursor: 0,
            settings_from: (Screen::Hub, 0),
            paused_in_air: false,
            resuming_until: None,
            notices: Vec::new(),
            progress: 0.0,
            arm_bound: None,
        };
        menus.cursor = menus.first_cursor(screen);
        menus
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The pilot's settings as the screens have them now.
    pub fn settings(&self) -> &PilotSettings {
        &self.settings
    }

    /// Takes the camera settings a flight changed (Camera Tilt and FOV in
    /// flight): the settings file keeps them with the rest. Returns whether
    /// they differ from what was held, and so whether to save.
    pub fn set_camera(&mut self, camera: CameraSettings) -> bool {
        let changed = self.settings.camera != camera;
        self.settings.camera = camera;
        changed
    }

    /// Whether the Input Devices fly the Quad, or move the cursor.
    pub fn input_mode(&self) -> InputMode {
        match self.screen {
            Screen::Flying => InputMode::Flight,
            _ => InputMode::Menu,
        }
    }

    /// Whether the world is drawn: the loading screen shows it, as does
    /// everything in the flight.
    pub fn world_shown(&self) -> bool {
        matches!(
            self.screen,
            Screen::Loading | Screen::Flying | Screen::PauseMenu | Screen::Picker(_)
        ) || (self.screen == Screen::Settings && self.settings_from.0 == Screen::PauseMenu)
    }

    /// Whether the Simulation is frozen: it runs only in Free Flight, and not
    /// while its 3-2-1 counts.
    pub fn frozen(&self) -> bool {
        !(self.screen == Screen::Flying && self.resuming_until.is_none())
    }

    /// A short notice, such as a device connecting, for [`NOTICE_FOR`].
    pub fn notice(&mut self, text: impl Into<String>, now: Duration) {
        self.notice_for(text, now, NOTICE_FOR);
    }

    /// A notice that stays for `how_long`: one that takes longer to read.
    pub fn notice_for(&mut self, text: impl Into<String>, now: Duration, how_long: Duration) {
        self.notices.push(Notice {
            text: text.into(),
            until: now + how_long,
        });
    }

    /// Tells the loading screen how far it is, from 0 to 1.
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Tells the loading screen's reminder whether Arm is bound on the Input
    /// Device that flies.
    pub fn set_arm_bound(&mut self, bound: Option<bool>) {
        self.arm_bound = bound;
    }

    /// The flight is made and its shaders are ready: Free Flight starts.
    pub fn finish_loading(&mut self) {
        if self.screen == Screen::Loading {
            self.screen = Screen::Flying;
            self.resuming_until = None;
        }
    }

    /// The flight couldn't be made: back to where the pilot pressed FLY or
    /// picked, with a notice.
    pub fn fail_loading(&mut self, problem: impl Into<String>, now: Duration) {
        if self.screen == Screen::Loading {
            self.notice(problem, now);
            self.enter(Screen::Hub);
        }
    }

    /// Time passing: notices fade, and a Resume's 3-2-1 ends.
    pub fn tick(&mut self, now: Duration) -> Vec<Effect> {
        self.notices.retain(|n| n.until > now);
        if let Some(until) = self.resuming_until
            && now >= until
        {
            self.resuming_until = None;
            return vec![Effect::GoOn];
        }
        Vec::new()
    }

    /// Puts the cursor on an item, as a mouse over it does. Does nothing if
    /// the screen has no such item.
    pub fn point_at(&mut self, id: ItemId, facts: &Facts<'_>) {
        if let Some(at) = self.items(facts).iter().position(|item| item.id == id) {
            self.cursor = at;
        }
    }

    /// One press, `from` the Input Device that made it (`None`: the keyboard
    /// or the mouse), at `now`.
    pub fn press(
        &mut self,
        input: MenuInput,
        from: Option<DeviceId>,
        facts: &Facts<'_>,
        now: Duration,
    ) -> Vec<Effect> {
        // Pause is the one press that works in flight too.
        if input == MenuInput::Pause {
            return self.pause(now, facts);
        }
        match self.screen {
            Screen::Loading | Screen::Flying => return Vec::new(),
            _ => {}
        }
        let items = self.items(facts);
        let count = items.len().max(1);
        self.cursor = self.cursor.min(count - 1);
        match input {
            MenuInput::Down => {
                self.cursor = (self.cursor + 1) % count;
                Vec::new()
            }
            MenuInput::Up => {
                self.cursor = (self.cursor + count - 1) % count;
                Vec::new()
            }
            MenuInput::Left | MenuInput::Right => {
                let Some(item) = items.get(self.cursor) else {
                    return Vec::new();
                };
                if item.kind == ItemKind::Stop {
                    self.change(item.id, input == MenuInput::Right, facts)
                } else {
                    Vec::new()
                }
            }
            MenuInput::Select => match items.get(self.cursor) {
                Some(item) => self.select(item.id, from, facts, now),
                None => Vec::new(),
            },
            MenuInput::Back => self.back(now),
            MenuInput::PreviousTab | MenuInput::NextTab | MenuInput::Pause => Vec::new(),
        }
    }

    /// The Pause Action: opens the Pause Menu in flight, and in the Pause
    /// Menu goes on. Anywhere else it goes back one screen.
    fn pause(&mut self, now: Duration, facts: &Facts<'_>) -> Vec<Effect> {
        match self.screen {
            Screen::Flying if self.resuming_until.is_some() => {
                // Pause again during the count: back to the menu, still frozen.
                self.resuming_until = None;
                self.enter(Screen::PauseMenu);
                Vec::new()
            }
            Screen::Flying => {
                self.paused_in_air = facts.in_the_air;
                self.enter(Screen::PauseMenu);
                vec![Effect::Freeze]
            }
            Screen::PauseMenu => self.resume(now),
            Screen::Picker(_) | Screen::Settings => self.back(now),
            Screen::FirstLaunch | Screen::InputDeviceStep | Screen::Hub | Screen::Loading => {
                Vec::new()
            }
        }
    }

    fn back(&mut self, now: Duration) -> Vec<Effect> {
        match self.screen {
            Screen::InputDeviceStep => {
                self.enter(Screen::FirstLaunch);
                Vec::new()
            }
            Screen::Settings => {
                let (screen, cursor) = self.settings_from;
                self.screen = screen;
                self.cursor = cursor;
                Vec::new()
            }
            Screen::Picker(_) => {
                self.enter(Screen::PauseMenu);
                Vec::new()
            }
            Screen::PauseMenu => self.resume(now),
            Screen::FirstLaunch | Screen::Hub | Screen::Loading | Screen::Flying => Vec::new(),
        }
    }

    /// Resume: a 3-2-1 first, if the Quad was in the air, because the pilot
    /// needs a moment to get their hands back on the sticks.
    fn resume(&mut self, now: Duration) -> Vec<Effect> {
        self.screen = Screen::Flying;
        if self.paused_in_air {
            self.resuming_until = Some(now + Duration::from_secs(u64::from(COUNTDOWN_SECONDS)));
            Vec::new()
        } else {
            self.resuming_until = None;
            vec![Effect::GoOn]
        }
    }

    fn select(
        &mut self,
        id: ItemId,
        from: Option<DeviceId>,
        facts: &Facts<'_>,
        now: Duration,
    ) -> Vec<Effect> {
        match id {
            ItemId::TextSize | ItemId::ReduceMotion | ItemId::AutoArm | ItemId::FlyingDevice => {
                self.change(id, true, facts)
            }
            ItemId::MapChip | ItemId::QuadChip | ItemId::PresetChip => self.change(id, true, facts),
            ItemId::Answer(experience) => {
                self.settings.preset = experience.preset();
                self.enter(Screen::InputDeviceStep);
                vec![Effect::SettingsChanged]
            }
            ItemId::SetUp => {
                // Setup is its own ticket (#76): until then the pilot goes on
                // to the Hub, and is told so.
                self.notice(
                    "Setting up an Input Device arrives in a later version. Yours flies on its starting profile for now.",
                    now,
                );
                self.leave_first_launch()
            }
            ItemId::SetUpLater | ItemId::ContinueWithout => self.leave_first_launch(),
            ItemId::Fly => self.load(from, facts, now),
            ItemId::OpenSettings => {
                self.settings_from = (self.screen, self.cursor);
                self.enter(Screen::Settings);
                Vec::new()
            }
            ItemId::Quit => vec![Effect::Quit],
            ItemId::Resume => self.resume(now),
            ItemId::Reset => {
                self.screen = Screen::Flying;
                self.resuming_until = None;
                vec![Effect::Reset]
            }
            ItemId::OpenMapPicker => {
                self.enter(Screen::Picker(Pick::Map));
                // The cursor starts on the Map in use.
                if let Some(at) = facts
                    .catalogue
                    .maps
                    .iter()
                    .position(|m| m.id == self.settings.map)
                {
                    self.cursor = at;
                }
                Vec::new()
            }
            ItemId::OpenQuadPicker => {
                self.enter(Screen::Picker(Pick::Quad));
                if let Some(at) = facts
                    .catalogue
                    .quads
                    .iter()
                    .position(|q| q.id == self.settings.quad)
                {
                    self.cursor = at;
                }
                Vec::new()
            }
            ItemId::BackToHub => {
                self.enter(Screen::Hub);
                vec![Effect::ToHub]
            }
            ItemId::PickMap(at) => match facts.catalogue.maps.get(at) {
                Some(map) => {
                    self.settings.map = map.id.clone();
                    let mut effects = vec![Effect::SettingsChanged];
                    effects.extend(self.load(None, facts, now));
                    effects
                }
                None => Vec::new(),
            },
            ItemId::PickQuad(at) => match facts.catalogue.quads.get(at) {
                Some(quad) => {
                    self.settings.quad = quad.id.clone();
                    let mut effects = vec![Effect::SettingsChanged];
                    effects.extend(self.load(None, facts, now));
                    effects
                }
                None => Vec::new(),
            },
            ItemId::Back => self.back(now),
        }
    }

    fn leave_first_launch(&mut self) -> Vec<Effect> {
        self.settings.first_launch_done = true;
        self.enter(Screen::Hub);
        vec![Effect::SettingsChanged]
    }

    fn load(&mut self, by: Option<DeviceId>, facts: &Facts<'_>, now: Duration) -> Vec<Effect> {
        if let Some(name) = self.unplugged_flying_device(facts) {
            self.notice_for(
                format!("{name} isn't plugged in, so the Flying device is Auto for this flight."),
                now,
                LONG_NOTICE_FOR,
            );
        }
        self.progress = 0.0;
        self.arm_bound = None;
        self.enter(Screen::Loading);
        vec![Effect::Load { by }]
    }

    /// Moves to a screen with its cursor in its starting place.
    fn enter(&mut self, screen: Screen) {
        self.screen = screen;
        self.cursor = self.first_cursor(screen);
    }

    /// Where the cursor starts on a screen: on FLY on the Hub, on the first
    /// answer on First Launch, on Resume in the Pause Menu.
    fn first_cursor(&self, screen: Screen) -> usize {
        match screen {
            // Text size and Reduce motion come first, and the cursor starts
            // on the first answer under them.
            Screen::FirstLaunch => 2,
            _ => 0,
        }
    }

    /// Changes a setting one stop forward or back.
    fn change(&mut self, id: ItemId, forward: bool, facts: &Facts<'_>) -> Vec<Effect> {
        fn step<T: Copy + PartialEq>(all: &[T], now: T, forward: bool) -> T {
            let at = all.iter().position(|v| *v == now).unwrap_or(0);
            let count = all.len();
            all[if forward {
                (at + 1) % count
            } else {
                (at + count - 1) % count
            }]
        }
        let before = self.settings.clone();
        match id {
            ItemId::TextSize => {
                self.settings.text_size = step(&TextSize::ALL, self.settings.text_size, forward)
            }
            ItemId::ReduceMotion => self.settings.reduce_motion = !self.settings.reduce_motion,
            ItemId::AutoArm => self.settings.auto_arm = !self.settings.auto_arm,
            ItemId::FlyingDevice => {
                let choices = self.flying_device_choices(facts);
                let at = choices
                    .iter()
                    .position(|c| *c == self.settings.flying_device)
                    .unwrap_or(0);
                let count = choices.len();
                let next = if forward {
                    (at + 1) % count
                } else {
                    (at + count - 1) % count
                };
                self.settings.flying_device = choices[next].clone();
            }
            ItemId::PresetChip => {
                self.settings.preset = step(&Preset::ALL, self.settings.preset, forward)
            }
            ItemId::MapChip => {
                let ids: Vec<&str> = facts.catalogue.maps.iter().map(|m| m.id.as_str()).collect();
                if let Some(id) = stepped_id(&ids, &self.settings.map, forward) {
                    self.settings.map = id;
                }
            }
            ItemId::QuadChip => {
                let ids: Vec<&str> = facts
                    .catalogue
                    .quads
                    .iter()
                    .map(|q| q.id.as_str())
                    .collect();
                if let Some(id) = stepped_id(&ids, &self.settings.quad, forward) {
                    self.settings.quad = id;
                }
            }
            _ => {}
        }
        if self.settings == before {
            Vec::new()
        } else {
            vec![Effect::SettingsChanged]
        }
    }

    /// What the Flying device stop steps through: Auto, then each Input
    /// Device plugged in by its profile's name, then the saved device if it
    /// isn't plugged in.
    fn flying_device_choices(&self, facts: &Facts<'_>) -> Vec<FlyingDevice> {
        let mut choices = vec![FlyingDevice::Auto];
        for device in facts.devices {
            let choice = FlyingDevice::Named(device.profile_name.clone());
            if !choices.contains(&choice) {
                choices.push(choice);
            }
        }
        if !choices.contains(&self.settings.flying_device) {
            choices.push(self.settings.flying_device.clone());
        }
        choices
    }

    /// The saved Flying device's name, when it isn't plugged in: Auto picks
    /// until it is.
    fn unplugged_flying_device(&self, facts: &Facts<'_>) -> Option<String> {
        match &self.settings.flying_device {
            FlyingDevice::Named(name) if !facts.devices.iter().any(|d| d.profile_name == *name) => {
                Some(name.clone())
            }
            _ => None,
        }
    }

    /// The Flying device stop: its value and the plain words under it.
    fn flying_device_item(&self, facts: &Facts<'_>) -> Item {
        let (value, note) = match &self.settings.flying_device {
            FlyingDevice::Auto => (
                FlyingDevice::AUTO.to_string(),
                "Auto flies a Radio if one is plugged in, then a Gamepad. With two of a kind, the first one you move flies. Menus work with any device.".to_string(),
            ),
            FlyingDevice::Named(name) => match self.unplugged_flying_device(facts) {
                None => (
                    name.clone(),
                    "Flies with this device whenever it is plugged in. Menus work with any device.".to_string(),
                ),
                Some(_) => (
                    format!("{name} (not plugged in)"),
                    format!(
                        "{name} isn't plugged in, so Auto picks for now: a Radio if one is plugged in, then a Gamepad."
                    ),
                ),
            },
        };
        Item {
            note: Some(note),
            ..Item::stop(ItemId::FlyingDevice, "Flying device", value)
        }
    }

    /// The items on the current screen, in the order the cursor visits them.
    pub fn items(&self, facts: &Facts<'_>) -> Vec<Item> {
        let settings = &self.settings;
        let on_off = |on: bool| if on { "On" } else { "Off" };
        match self.screen {
            Screen::FirstLaunch => {
                let mut items = vec![
                    Item::stop(
                        ItemId::TextSize,
                        "Text size",
                        format!("{}%", settings.text_size.percent()),
                    ),
                    Item::stop(
                        ItemId::ReduceMotion,
                        "Reduce motion",
                        on_off(settings.reduce_motion),
                    ),
                ];
                items.extend(
                    Experience::ALL
                        .into_iter()
                        .map(|e| Item::button(ItemId::Answer(e), e.answer())),
                );
                items
            }
            Screen::InputDeviceStep => {
                if facts.devices.is_empty() {
                    vec![Item::button(ItemId::ContinueWithout, "Continue without")]
                } else {
                    vec![
                        Item::button(ItemId::SetUp, "Set up"),
                        Item::button(ItemId::SetUpLater, "Set up later"),
                    ]
                }
            }
            Screen::Hub => {
                let map = facts
                    .catalogue
                    .maps
                    .iter()
                    .find(|m| m.id == settings.map)
                    .map_or_else(|| settings.map.clone(), |m| m.name.clone());
                let quad = facts.catalogue.quads.iter().find(|q| q.id == settings.quad);
                let mut quad_chip = Item::stop(
                    ItemId::QuadChip,
                    "Quad",
                    quad.map_or_else(|| settings.quad.clone(), |q| q.name.clone()),
                );
                quad_chip.picture = quad.and_then(|q| q.picture.clone());
                quad_chip.note = quad.map(|q| q.spec_line.clone());
                vec![
                    Item::new(ItemId::Fly, ItemKind::Fly, "FLY"),
                    Item::stop(ItemId::MapChip, "Map", map),
                    quad_chip,
                    Item::stop(ItemId::PresetChip, "Preset", settings.preset.name()),
                    Item::button(ItemId::OpenSettings, "Settings"),
                    Item::button(ItemId::Quit, "Quit"),
                ]
            }
            Screen::Loading | Screen::Flying => Vec::new(),
            Screen::PauseMenu => vec![
                Item::button(ItemId::Resume, "Resume"),
                Item::button(ItemId::Reset, "Reset"),
                Item::button(ItemId::OpenMapPicker, "Map"),
                Item::button(ItemId::OpenQuadPicker, "Quad"),
                Item::button(ItemId::OpenSettings, "Settings"),
                Item::button(ItemId::BackToHub, "Back to the Hub"),
                Item::button(ItemId::Quit, "Quit"),
            ],
            Screen::Picker(Pick::Map) => facts
                .catalogue
                .maps
                .iter()
                .enumerate()
                .map(|(at, map)| Item {
                    note: Some(map.description.clone()),
                    current: map.id == settings.map,
                    ..Item::button(ItemId::PickMap(at), map.name.clone())
                })
                .collect(),
            Screen::Picker(Pick::Quad) => facts
                .catalogue
                .quads
                .iter()
                .enumerate()
                .map(|(at, quad)| Item {
                    note: Some(format!("{} ({})", quad.description, quad.spec_line)),
                    picture: quad.picture.clone(),
                    current: quad.id == settings.quad,
                    ..Item::button(ItemId::PickQuad(at), quad.name.clone())
                })
                .collect(),
            Screen::Settings => vec![
                Item::stop(
                    ItemId::TextSize,
                    "Text size",
                    format!("{}%", settings.text_size.percent()),
                ),
                Item::stop(
                    ItemId::ReduceMotion,
                    "Reduce motion",
                    on_off(settings.reduce_motion),
                ),
                Item::stop(ItemId::AutoArm, "Auto-arm", on_off(settings.auto_arm)),
                self.flying_device_item(facts),
                Item::button(ItemId::Back, "Back"),
            ],
        }
    }

    /// The Pre-flight Warnings on the Hub: something would stop the pilot
    /// flying, or flying well. They never block FLY.
    pub fn warnings(&self, facts: &Facts<'_>) -> Vec<String> {
        let mut warnings = Vec::new();
        if facts.devices.is_empty() {
            warnings.push("No Input Device connected. Plug in your Radio or Gamepad.".to_string());
        }
        if let Some(name) = self.unplugged_flying_device(facts) {
            warnings.push(format!(
                "{name} is your Flying device but isn't plugged in, so Auto picks for now."
            ));
        }
        if !self.settings.auto_arm {
            for device in facts.devices.iter().filter(|d| !d.arm_bound) {
                warnings.push(format!(
                    "Arm isn't bound on your {}. Turn on Auto-arm in Settings.",
                    device.name
                ));
            }
        }
        warnings
    }

    /// The screen as the pilot sees it.
    pub fn page(&self, now: Duration, facts: &Facts<'_>) -> Page {
        let items = self.items(facts);
        let cursor = self.cursor.min(items.len().saturating_sub(1));
        let mut page = Page {
            screen: self.screen,
            title: String::new(),
            lines: Vec::new(),
            items,
            cursor,
            flying_with: None,
            warnings: Vec::new(),
            notices: self.notices.iter().map(|n| n.text.clone()).collect(),
            loading: None,
            countdown: None,
            over_the_world: self.world_shown() && self.screen != Screen::Loading,
        };
        match self.screen {
            Screen::FirstLaunch => {
                page.title = "Welcome to OpenDrone".to_string();
                page.lines = vec![
                    "Flashing images. FPV video static and Breakup can flash. If that bothers you, turn on Reduce motion: it limits flashing to 3 per second.".to_string(),
                    "Have you flown FPV before?".to_string(),
                ];
            }
            Screen::InputDeviceStep => {
                page.title = "Your Input Device".to_string();
                if facts.devices.is_empty() {
                    page.lines = vec!["Plug in your Radio or Gamepad".to_string()];
                } else {
                    page.lines = facts
                        .devices
                        .iter()
                        .map(|d| format!("Connected: {}", d.name))
                        .collect();
                }
            }
            Screen::Hub => {
                page.title = "OpenDrone".to_string();
                page.flying_with = Some(match facts.flying_with {
                    Some(name) => format!("Flying with: {name}"),
                    None if facts.devices.is_empty() => "Flying with: no Input Device".to_string(),
                    None => "Flying with: the first Input Device you move".to_string(),
                });
                page.warnings = self.warnings(facts);
            }
            Screen::Loading => {
                page.title = "Loading".to_string();
                let name_of_map = facts
                    .catalogue
                    .maps
                    .iter()
                    .find(|m| m.id == self.settings.map)
                    .map_or_else(|| self.settings.map.clone(), |m| m.name.clone());
                let name_of_quad = facts
                    .catalogue
                    .quads
                    .iter()
                    .find(|q| q.id == self.settings.quad)
                    .map_or_else(|| self.settings.quad.clone(), |q| q.name.clone());
                page.loading = Some(LoadingPage {
                    map: name_of_map,
                    quad: name_of_quad,
                    preset: self.settings.preset.name().to_string(),
                    progress: self.progress,
                    keys: self.key_hints(),
                });
            }
            Screen::Flying => {
                page.countdown = self.resuming_until.map(|until| {
                    let left = until.saturating_sub(now).as_secs_f64();
                    (left.ceil() as u32).clamp(1, COUNTDOWN_SECONDS)
                });
            }
            Screen::PauseMenu => page.title = "Paused".to_string(),
            Screen::Picker(Pick::Map) => page.title = "Pick a Map".to_string(),
            Screen::Picker(Pick::Quad) => page.title = "Pick a Quad".to_string(),
            Screen::Settings => {
                page.title = "Settings".to_string();
                page.lines = vec![
                    "Every setting takes effect as soon as you change it, and is saved.".to_string(),
                    "Flying, Camera, Graphics, Input Devices, Sound and Packs arrive in later versions.".to_string(),
                ];
            }
        }
        page
    }

    /// The loading screen's reminder of the keys.
    fn key_hints(&self) -> Vec<KeyHint> {
        let hint = |action: &str, key: &str| KeyHint {
            action: action.to_string(),
            key: key.to_string(),
            warning: false,
        };
        let arm = match (self.arm_bound, self.settings.auto_arm) {
            (Some(false), false) | (None, false) => KeyHint {
                action: "Arm".to_string(),
                key: "not bound".to_string(),
                warning: true,
            },
            (Some(false), true) | (None, true) => hint("Arm", "raise the throttle (Auto-arm)"),
            (Some(true), _) => hint("Arm", "your Arm switch"),
        };
        vec![
            arm,
            hint("Reset", "R"),
            hint("Pause", "Esc"),
            hint("Camera Tilt", "Up and Down"),
            hint("FOV", "Right and Left"),
        ]
    }
}

/// Whether the Input Devices fly the Quad or move the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Flight,
    Menu,
}

/// The id after or before `current` among `ids`, wrapping. `None` if there
/// is nothing to pick. An id not in the list steps to the first.
fn stepped_id(ids: &[&str], current: &str, forward: bool) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let at = ids.iter().position(|id| *id == current);
    let next = match (at, forward) {
        (None, _) => 0,
        (Some(at), true) => (at + 1) % ids.len(),
        (Some(at), false) => (at + ids.len() - 1) % ids.len(),
    };
    Some(ids[next].to_string())
}

/// Whether a Quad counts as in the air for Resume's 3-2-1: moving faster
/// than 0.5 m/s, or armed with its motors lifting at least half its weight. A
/// Quad sitting on the ground, armed and idling, is not.
pub fn quad_is_in_the_air(speed: f64, armed: bool, thrust: f64, weight: f64) -> bool {
    speed >= 0.5 || (armed && thrust >= 0.5 * weight)
}
