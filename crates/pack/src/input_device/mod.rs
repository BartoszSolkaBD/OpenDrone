//! Input Device profiles: what turns one Input Device's axes, buttons and
//! switches into Channels and Actions (#19 §8). A Pack holds one per device
//! model, as `input-devices/<id>.toml`; [`read_input_device_file`] reads and
//! checks it.
//!
//! A profile has two halves:
//!
//! - **The device's facts** ([`DeviceFacts`]): its name, how it is matched,
//!   its Report Rate and whether it reports at rest, per connection, and (on
//!   a Gamepad) the device's own names for SDL's buttons. They always come
//!   from the current Pack, so a game update that adds one reaches every
//!   pilot.
//! - **Its setup** ([`Setup`]): the kind, channel mapping, switches, Actions
//!   and Calibration. A Pack's setup is only a starting point. The pilot's
//!   own copy (`opendrone_input::PilotCopy`) holds what setup measured or the
//!   pilot chose, and rewrites only this half, plus a measured Report Rate
//!   where the Pack has none.
//!
//! `opendrone-input` turns a device's values into Channels with these.
//! [`switches_from_aux`] turns pasted Betaflight `aux` lines into a Radio's
//! switches.

mod aux_paste;
pub mod controls;
mod read;

use std::collections::BTreeMap;

use controls::{
    AXIS_MAX, AXIS_MIN, Key, PadButton, PadControl, Position, Stick, Trigger, value_at_percent,
};

pub use aux_paste::{PastedSwitches, switches_from_aux};
pub use read::{read_input_device_file, read_input_device_file_with_steps};

/// Radio or Gamepad (#19 §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Radio,
    Gamepad,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Radio => "Radio",
            Kind::Gamepad => "Gamepad",
        }
    }
}

/// How an Input Device reaches the computer. Report Rates and whether a
/// device reports at rest are written per connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Connection {
    Usb,
    Bluetooth,
}

impl Connection {
    pub const ALL: [Connection; 2] = [Connection::Usb, Connection::Bluetooth];

    /// The word a profile writes, such as `usb`.
    pub fn word(self) -> &'static str {
        match self {
            Connection::Usb => "usb",
            Connection::Bluetooth => "bluetooth",
        }
    }

    pub fn from_word(word: &str) -> Option<Connection> {
        Connection::ALL.into_iter().find(|c| c.word() == word)
    }
}

/// How a profile finds its devices (#19 §1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Match {
    /// USB vendor and product ids, plus the product name found anywhere in
    /// the device's name, ignoring case. Never by firmware version.
    Device {
        usb_vendor: u16,
        usb_product: u16,
        product_name: String,
    },
    /// The fallback for every device of the profile's kind that has no
    /// profile of its own: "Any Radio" and "Any Gamepad".
    Any,
}

/// One Input Device profile, as the Pack checker hands it over.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDeviceProfile {
    /// The profile's id, such as `opendrone/radiomaster-pocket`.
    pub id: String,
    pub facts: DeviceFacts,
    pub setup: Setup,
}

/// What a profile says about the device itself. These always come from the
/// current Pack; a pilot's copy never changes them.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFacts {
    /// The on-screen name, such as "Radiomaster Pocket".
    pub name: String,
    pub matches: Match,
    /// The Report Rate in reports a second, per connection, where it was
    /// measured: 1000 for the Pocket over USB with RF off, 250 for the
    /// DualSense over USB. Fallback profiles carry none; setup measures it.
    pub report_rate: BTreeMap<Connection, f64>,
    /// Whether the device keeps reporting while its sticks rest, per
    /// connection (#27). Such a device counts as lost after 1 s of silence. A
    /// connection left out reports only changes, so only an unplug loses it.
    pub reports_at_rest: BTreeMap<Connection, bool>,
    /// A Gamepad's own names for SDL's positions, such as `R1` for
    /// `right_shoulder` on a DualSense, in the order the profile writes them.
    pub labels: Vec<(String, PadControl)>,
}

impl DeviceFacts {
    /// The label this device prints on a control, or SDL's position name.
    pub fn label_of(&self, control: PadControl) -> &str {
        self.labels
            .iter()
            .find(|(_, c)| *c == control)
            .map_or(control.name(), |(label, _)| label.as_str())
    }

    /// The control a label names: one of this device's labels, or SDL's
    /// position name.
    pub fn control_named(&self, name: &str) -> Option<PadControl> {
        self.labels
            .iter()
            .find(|(label, _)| label == name)
            .map(|(_, c)| *c)
            .or_else(|| PadControl::from_name(name))
    }
}

/// The half of a profile that setup and the pilot may change: the kind, and
/// everything that turns the device's controls into Channels and Actions.
#[derive(Clone, Debug, PartialEq)]
pub enum Setup {
    Radio(RadioSetup),
    Gamepad(GamepadSetup),
}

impl Setup {
    pub fn kind(&self) -> Kind {
        match self {
            Setup::Radio(_) => Kind::Radio,
            Setup::Gamepad(_) => Kind::Gamepad,
        }
    }

    pub fn switches(&self) -> &Switches {
        match self {
            Setup::Radio(radio) => &radio.switches,
            Setup::Gamepad(pad) => &pad.switches,
        }
    }

    pub fn actions(&self) -> &BTreeMap<Action, ActionSource> {
        match self {
            Setup::Radio(radio) => &radio.actions,
            Setup::Gamepad(pad) => &pad.actions,
        }
    }
}

/// A Radio's setup. Its stick mode is set in the radio, so the sim only needs
/// each stick's channel and whether it runs reversed (#19 §4).
#[derive(Clone, Debug, PartialEq)]
pub struct RadioSetup {
    pub channels: RadioChannels,
    pub switches: Switches,
    pub actions: BTreeMap<Action, ActionSource>,
    pub calibration: RadioCalibration,
}

/// Which of the Radio's channels carries each stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadioChannels {
    pub roll: RadioStick,
    pub pitch: RadioStick,
    pub throttle: RadioStick,
    pub yaw: RadioStick,
}

/// One stick on a Radio: its channel, CH1–CH8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadioStick {
    pub channel: u8,
    pub reverse: bool,
}

impl RadioStick {
    pub fn on(channel: u8) -> RadioStick {
        RadioStick {
            channel,
            reverse: false,
        }
    }
}

impl RadioChannels {
    /// EdgeTX's AETR order, as the Pocket sends it out of the box: roll,
    /// pitch, throttle, yaw on CH1–CH4.
    pub fn aetr() -> RadioChannels {
        RadioChannels {
            roll: RadioStick::on(1),
            pitch: RadioStick::on(2),
            throttle: RadioStick::on(3),
            yaw: RadioStick::on(4),
        }
    }

    /// The four sticks, in the order roll, pitch, throttle, yaw.
    pub fn all(&self) -> [RadioStick; 4] {
        [self.roll, self.pitch, self.throttle, self.yaw]
    }
}

/// A Radio stick's Calibration, in SDL's axis units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadioCalibration {
    pub roll: StickCalibration,
    pub pitch: StickCalibration,
    pub yaw: StickCalibration,
    pub throttle: Ends,
}

/// One centring stick's Calibration, in SDL's axis units (−32768…32767):
/// where its ends and centre are, and the deadband around the centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickCalibration {
    pub min: f64,
    pub centre: f64,
    pub max: f64,
    /// Half the width of the band around the centre that reads as centre.
    pub deadband: f64,
}

impl StickCalibration {
    /// A stick that trusts the device: −100 %, 0 %, +100 % and no deadband.
    pub fn full() -> StickCalibration {
        StickCalibration {
            min: AXIS_MIN,
            centre: 0.0,
            max: AXIS_MAX,
            deadband: 0.0,
        }
    }

    /// A stick read from percentages of travel, as a profile writes them.
    pub fn from_percent(min: f64, centre: f64, max: f64, deadband: f64) -> StickCalibration {
        StickCalibration {
            min: value_at_percent(min),
            centre: value_at_percent(centre),
            max: value_at_percent(max),
            deadband: deadband / 100.0 * -AXIS_MIN,
        }
    }
}

/// The two ends of a throttle stick or a trigger, in SDL's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ends {
    pub min: f64,
    pub max: f64,
}

impl Ends {
    /// A throttle stick's full travel: −100 % to +100 %.
    pub fn full_stick() -> Ends {
        Ends {
            min: AXIS_MIN,
            max: AXIS_MAX,
        }
    }

    /// A trigger's full travel: 0 % to 100 %.
    pub fn full_trigger() -> Ends {
        Ends {
            min: 0.0,
            max: AXIS_MAX,
        }
    }
}

/// A Gamepad's setup: which stick or trigger carries each Channel, its
/// Virtual Switches, Actions and Calibration.
#[derive(Clone, Debug, PartialEq)]
pub struct GamepadSetup {
    pub channels: GamepadChannels,
    pub switches: Switches,
    pub actions: BTreeMap<Action, ActionSource>,
    pub calibration: GamepadCalibration,
}

/// Which stick carries roll, pitch and yaw, and how the throttle is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamepadChannels {
    pub roll: PadStick,
    pub pitch: PadStick,
    pub yaw: PadStick,
    pub throttle: GamepadThrottle,
}

/// One centring stick axis on a Gamepad. Up and right read high.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadStick {
    pub stick: Stick,
    pub reverse: bool,
}

impl PadStick {
    pub fn on(stick: Stick) -> PadStick {
        PadStick {
            stick,
            reverse: false,
        }
    }
}

/// How a Gamepad's throttle is read (#19 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadThrottle {
    /// A centring stick axis, read in one of two styles.
    Stick { stick: Stick, zero: ThrottleZero },
    /// A trigger: released is no throttle, squeezed is full.
    Trigger(Trigger),
}

/// Where a Gamepad throttle stick reads zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrottleZero {
    /// The default: the stick at rest is no throttle and fully up is full;
    /// pulling down does nothing. Letting go cuts the throttle, and arming
    /// works from rest.
    AtRest,
    /// Full travel: fully down is no throttle, the stick at rest is half.
    AtBottom,
}

impl ThrottleZero {
    /// The words a profile writes: `zero = "at rest"` or `"at bottom"`.
    pub fn words(self) -> &'static str {
        match self {
            ThrottleZero::AtRest => "at rest",
            ThrottleZero::AtBottom => "at bottom",
        }
    }
}

/// The three throttle styles a Gamepad offers: zero at rest (the default),
/// full travel, and a trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrottleStyle {
    ZeroAtRest,
    FullTravel,
    Trigger(Trigger),
}

/// Which stick carries what, as the RC stick modes 1–4 lay it out. A Gamepad
/// flies Mode 2 by default (#19 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickMode {
    /// Left stick: pitch and yaw. Right stick: throttle and roll.
    Mode1,
    /// Left stick: throttle and yaw. Right stick: pitch and roll.
    Mode2,
    /// Left stick: pitch and roll. Right stick: throttle and yaw.
    Mode3,
    /// Left stick: throttle and roll. Right stick: pitch and yaw.
    Mode4,
}

impl GamepadChannels {
    /// The sticks laid out in a stick mode, with a throttle style.
    pub fn in_mode(mode: StickMode, throttle: ThrottleStyle) -> GamepadChannels {
        use Stick::{LeftX, LeftY, RightX, RightY};
        // (roll, pitch, yaw, throttle stick)
        let (roll, pitch, yaw, throttle_stick) = match mode {
            StickMode::Mode1 => (RightX, LeftY, LeftX, RightY),
            StickMode::Mode2 => (RightX, RightY, LeftX, LeftY),
            StickMode::Mode3 => (LeftX, LeftY, RightX, RightY),
            StickMode::Mode4 => (LeftX, RightY, RightX, LeftY),
        };
        let throttle = match throttle {
            ThrottleStyle::ZeroAtRest => GamepadThrottle::Stick {
                stick: throttle_stick,
                zero: ThrottleZero::AtRest,
            },
            ThrottleStyle::FullTravel => GamepadThrottle::Stick {
                stick: throttle_stick,
                zero: ThrottleZero::AtBottom,
            },
            ThrottleStyle::Trigger(trigger) => GamepadThrottle::Trigger(trigger),
        };
        GamepadChannels {
            roll: PadStick::on(roll),
            pitch: PadStick::on(pitch),
            yaw: PadStick::on(yaw),
            throttle,
        }
    }

    /// The stick mode these sticks follow, if they follow one.
    pub fn mode(&self) -> Option<StickMode> {
        let style = self.throttle_style();
        [
            StickMode::Mode1,
            StickMode::Mode2,
            StickMode::Mode3,
            StickMode::Mode4,
        ]
        .into_iter()
        .find(|&mode| {
            let laid_out = GamepadChannels::in_mode(mode, style);
            let same_throttle = match (laid_out.throttle, self.throttle) {
                (
                    GamepadThrottle::Stick { stick: a, .. },
                    GamepadThrottle::Stick { stick: b, .. },
                ) => a == b,
                _ => true,
            };
            laid_out.roll.stick == self.roll.stick
                && laid_out.pitch.stick == self.pitch.stick
                && laid_out.yaw.stick == self.yaw.stick
                && same_throttle
        })
    }

    pub fn throttle_style(&self) -> ThrottleStyle {
        match self.throttle {
            GamepadThrottle::Stick {
                zero: ThrottleZero::AtRest,
                ..
            } => ThrottleStyle::ZeroAtRest,
            GamepadThrottle::Stick {
                zero: ThrottleZero::AtBottom,
                ..
            } => ThrottleStyle::FullTravel,
            GamepadThrottle::Trigger(trigger) => ThrottleStyle::Trigger(trigger),
        }
    }
}

/// A Gamepad's Calibration: each stick axis's ends, centre and deadband, and
/// each trigger's ends, in SDL's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadCalibration {
    pub left_x: StickCalibration,
    pub left_y: StickCalibration,
    pub right_x: StickCalibration,
    pub right_y: StickCalibration,
    pub left_trigger: Ends,
    pub right_trigger: Ends,
}

impl GamepadCalibration {
    pub fn stick(&self, stick: Stick) -> &StickCalibration {
        match stick {
            Stick::LeftX => &self.left_x,
            Stick::LeftY => &self.left_y,
            Stick::RightX => &self.right_x,
            Stick::RightY => &self.right_y,
        }
    }

    pub fn stick_mut(&mut self, stick: Stick) -> &mut StickCalibration {
        match stick {
            Stick::LeftX => &mut self.left_x,
            Stick::LeftY => &mut self.left_y,
            Stick::RightX => &mut self.right_x,
            Stick::RightY => &mut self.right_y,
        }
    }

    pub fn trigger(&self, trigger: Trigger) -> &Ends {
        match trigger {
            Trigger::Left => &self.left_trigger,
            Trigger::Right => &self.right_trigger,
        }
    }
}

/// The Flight Mode switch's three modes. The Flight Controller sees AUX2 low
/// for Acro, middle for Horizon and high for Angle (ADR-0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlightMode {
    Acro,
    Horizon,
    Angle,
}

impl FlightMode {
    pub const ALL: [FlightMode; 3] = [FlightMode::Acro, FlightMode::Horizon, FlightMode::Angle];

    pub fn word(self) -> &'static str {
        match self {
            FlightMode::Acro => "Acro",
            FlightMode::Horizon => "Horizon",
            FlightMode::Angle => "Angle",
        }
    }

    pub fn from_word(word: &str) -> Option<FlightMode> {
        FlightMode::ALL.into_iter().find(|m| m.word() == word)
    }
}

/// The three switch Channels with fixed meanings (ADR-0017), and what drives
/// each. Each switch Channel has one source at a time; one left out has no
/// source, so the game decides it (Auto-arm, or the Flight Mode setting and
/// the Preset).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// AUX1: high is armed.
    pub arm: Option<OnOffSwitch>,
    /// AUX2: low Acro, middle Horizon, high Angle.
    pub flight_mode: Option<FlightModeSwitch>,
    /// AUX3: high is Crash Flip.
    pub crash_flip: Option<OnOffSwitch>,
}

/// What drives Arm or Crash Flip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnOffSwitch {
    /// A Radio switch on a channel, on in one position: a third of an axis
    /// channel's travel (CH1–CH8), or a button channel pressed (CH9–CH32).
    Channel { channel: u8, on: Position },
    /// A Virtual Switch, driven by a Gamepad button or a keyboard key.
    Virtual { source: Press, style: PressStyle },
}

/// What drives the Flight Mode switch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlightModeSwitch {
    /// A Radio switch on an axis channel, one Flight Mode per position.
    Channel {
        channel: u8,
        low: FlightMode,
        middle: FlightMode,
        high: FlightMode,
    },
    /// A Virtual Switch: each press steps to the next of these modes (Acro
    /// and Angle by default), starting from the first (#19 §5).
    Steps {
        source: Press,
        modes: Vec<FlightMode>,
    },
}

/// A button or key that drives a Virtual Switch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Press {
    Button(PadButton),
    Key(Key),
}

/// How a Virtual Switch answers its button or key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressStyle {
    /// Each press flips it. An Arm toggle also turns itself off whenever the
    /// game reports a disarm, so the next press always means "arm now".
    Toggle,
    /// On only while held, as Crash Flip is.
    Hold,
}

impl PressStyle {
    pub fn word(self) -> &'static str {
        match self {
            PressStyle::Toggle => "toggle",
            PressStyle::Hold => "hold",
        }
    }
}

/// A command that isn't flying (the Input deep dive). The game fires it;
/// a profile only says which control is bound to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Pause,
    Reset,
    CameraTiltUp,
    CameraTiltDown,
    FovWider,
    FovNarrower,
    SaveFlight,
}

impl Action {
    pub const ALL: [Action; 7] = [
        Action::Pause,
        Action::Reset,
        Action::CameraTiltUp,
        Action::CameraTiltDown,
        Action::FovWider,
        Action::FovNarrower,
        Action::SaveFlight,
    ];

    /// The name a profile writes in `[actions]`, such as `camera_tilt_up`.
    pub fn name(self) -> &'static str {
        match self {
            Action::Pause => "pause",
            Action::Reset => "reset",
            Action::CameraTiltUp => "camera_tilt_up",
            Action::CameraTiltDown => "camera_tilt_down",
            Action::FovWider => "fov_wider",
            Action::FovNarrower => "fov_narrower",
            Action::SaveFlight => "save_flight",
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// The control an Action is bound to on one Input Device. The keyboard's own
/// Action keys belong to the game's settings, not to a device's profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionSource {
    /// A Radio channel in one position: a CH5–CH8 position or a CH9–CH32
    /// button, never a channel a switch already uses (#19 §6).
    Channel {
        channel: u8,
        on: Position,
    },
    Button(PadButton),
}

impl InputDeviceProfile {
    pub fn kind(&self) -> Kind {
        self.setup.kind()
    }

    /// The Report Rate over `connection`, if known.
    pub fn report_rate(&self, connection: Option<Connection>) -> Option<f64> {
        connection.and_then(|c| self.facts.report_rate.get(&c).copied())
    }

    /// Whether the device keeps reporting at rest over `connection`. Unknown
    /// connections count as reporting only changes.
    pub fn reports_at_rest(&self, connection: Option<Connection>) -> bool {
        connection
            .and_then(|c| self.facts.reports_at_rest.get(&c).copied())
            .unwrap_or(false)
    }
}
