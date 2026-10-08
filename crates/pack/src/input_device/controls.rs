//! The controls an Input Device has, as SDL names them, and the positions a
//! Radio's switches take.
//!
//! - A **Radio** is read as SDL's plain joystick. EdgeTX sends the model's
//!   CH1–CH8 as axes 0–7 and CH9–CH32 as buttons 0–23 (#18), so a Radio
//!   profile names its controls by channel.
//! - A **Gamepad** is read through SDL's gamepad layer, which names every
//!   stick, trigger and button by its position (`south`, `right_shoulder`,
//!   …), the same on a DualSense, an Xbox pad or a Switch Pro (#19 §8).
//!
//! SDL scales every axis to −32768…32767 and every trigger to 0…32767.

use core::fmt;

/// The most a Radio sends: CH1–CH32.
pub const RADIO_CHANNELS: u8 = 32;

/// A Radio's channels CH1–CH8 arrive as axes; CH9 and up as buttons.
pub const RADIO_AXIS_CHANNELS: u8 = 8;

/// SDL's lowest axis value: −100 %.
pub const AXIS_MIN: f64 = -32768.0;

/// SDL's highest axis value: +100 %.
pub const AXIS_MAX: f64 = 32767.0;

/// One 8-bit step on SDL's scale (65535 / 255), the DualSense's resolution.
pub const EIGHT_BIT_STEP: f64 = 257.0;

/// An SDL axis value as a percentage of travel: −100 % is SDL's lowest value,
/// +100 % its highest, and 0 % is 0. A trigger reads 0 % at rest.
pub fn percent_of(value: f64) -> f64 {
    if value >= 0.0 {
        value / AXIS_MAX * 100.0
    } else {
        value / -AXIS_MIN * 100.0
    }
}

/// The SDL axis value at a percentage of travel, the inverse of
/// [`percent_of`].
pub fn value_at_percent(percent: f64) -> f64 {
    if percent >= 0.0 {
        percent / 100.0 * AXIS_MAX
    } else {
        percent / 100.0 * -AXIS_MIN
    }
}

/// A Gamepad stick axis, by SDL's position name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stick {
    LeftX,
    LeftY,
    RightX,
    RightY,
}

impl Stick {
    pub const ALL: [Stick; 4] = [Stick::LeftX, Stick::LeftY, Stick::RightX, Stick::RightY];

    pub fn name(self) -> &'static str {
        match self {
            Stick::LeftX => "left_x",
            Stick::LeftY => "left_y",
            Stick::RightX => "right_x",
            Stick::RightY => "right_y",
        }
    }

    pub fn from_name(name: &str) -> Option<Stick> {
        Stick::ALL.into_iter().find(|s| s.name() == name)
    }

    /// SDL's gamepad axis number for this stick.
    pub fn pad_axis(self) -> PadAxis {
        match self {
            Stick::LeftX => PadAxis::LeftX,
            Stick::LeftY => PadAxis::LeftY,
            Stick::RightX => PadAxis::RightX,
            Stick::RightY => PadAxis::RightY,
        }
    }

    /// Up and down sticks: SDL reads them negative when pushed up, so they
    /// are turned round to read "up is high".
    pub fn is_vertical(self) -> bool {
        matches!(self, Stick::LeftY | Stick::RightY)
    }
}

/// A Gamepad trigger, by SDL's position name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trigger {
    Left,
    Right,
}

impl Trigger {
    pub const ALL: [Trigger; 2] = [Trigger::Left, Trigger::Right];

    pub fn name(self) -> &'static str {
        match self {
            Trigger::Left => "left_trigger",
            Trigger::Right => "right_trigger",
        }
    }

    pub fn from_name(name: &str) -> Option<Trigger> {
        Trigger::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn pad_axis(self) -> PadAxis {
        match self {
            Trigger::Left => PadAxis::LeftTrigger,
            Trigger::Right => PadAxis::RightTrigger,
        }
    }
}

/// SDL's six gamepad axes, in SDL's order (`SDL_GamepadAxis`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

impl PadAxis {
    pub const COUNT: usize = 6;
    pub const ALL: [PadAxis; 6] = [
        PadAxis::LeftX,
        PadAxis::LeftY,
        PadAxis::RightX,
        PadAxis::RightY,
        PadAxis::LeftTrigger,
        PadAxis::RightTrigger,
    ];

    /// The axis with SDL's number `index`.
    pub fn from_index(index: usize) -> Option<PadAxis> {
        PadAxis::ALL.get(index).copied()
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// SDL's gamepad buttons by position, in SDL's order (`SDL_GamepadButton`).
/// SDL keeps them positional on Nintendo layouts too, so `south` is the
/// bottom face button on every pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Misc1,
    RightPaddle1,
    LeftPaddle1,
    RightPaddle2,
    LeftPaddle2,
    Touchpad,
    Misc2,
    Misc3,
    Misc4,
    Misc5,
    Misc6,
}

impl PadButton {
    pub const COUNT: usize = 26;
    pub const ALL: [PadButton; 26] = [
        PadButton::South,
        PadButton::East,
        PadButton::West,
        PadButton::North,
        PadButton::Back,
        PadButton::Guide,
        PadButton::Start,
        PadButton::LeftStick,
        PadButton::RightStick,
        PadButton::LeftShoulder,
        PadButton::RightShoulder,
        PadButton::DpadUp,
        PadButton::DpadDown,
        PadButton::DpadLeft,
        PadButton::DpadRight,
        PadButton::Misc1,
        PadButton::RightPaddle1,
        PadButton::LeftPaddle1,
        PadButton::RightPaddle2,
        PadButton::LeftPaddle2,
        PadButton::Touchpad,
        PadButton::Misc2,
        PadButton::Misc3,
        PadButton::Misc4,
        PadButton::Misc5,
        PadButton::Misc6,
    ];

    /// The button with SDL's number `index`.
    pub fn from_index(index: usize) -> Option<PadButton> {
        PadButton::ALL.get(index).copied()
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// SDL's position name, as profiles write it, such as `right_shoulder`.
    pub fn name(self) -> &'static str {
        match self {
            PadButton::South => "south",
            PadButton::East => "east",
            PadButton::West => "west",
            PadButton::North => "north",
            PadButton::Back => "back",
            PadButton::Guide => "guide",
            PadButton::Start => "start",
            PadButton::LeftStick => "left_stick",
            PadButton::RightStick => "right_stick",
            PadButton::LeftShoulder => "left_shoulder",
            PadButton::RightShoulder => "right_shoulder",
            PadButton::DpadUp => "dpad_up",
            PadButton::DpadDown => "dpad_down",
            PadButton::DpadLeft => "dpad_left",
            PadButton::DpadRight => "dpad_right",
            PadButton::Misc1 => "misc1",
            PadButton::RightPaddle1 => "right_paddle1",
            PadButton::LeftPaddle1 => "left_paddle1",
            PadButton::RightPaddle2 => "right_paddle2",
            PadButton::LeftPaddle2 => "left_paddle2",
            PadButton::Touchpad => "touchpad",
            PadButton::Misc2 => "misc2",
            PadButton::Misc3 => "misc3",
            PadButton::Misc4 => "misc4",
            PadButton::Misc5 => "misc5",
            PadButton::Misc6 => "misc6",
        }
    }

    pub fn from_name(name: &str) -> Option<PadButton> {
        PadButton::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// A Gamepad control a device's own label can name: a button or a trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadControl {
    Button(PadButton),
    Trigger(Trigger),
}

impl PadControl {
    /// SDL's position name, such as `right_shoulder` or `right_trigger`.
    pub fn name(self) -> &'static str {
        match self {
            PadControl::Button(b) => b.name(),
            PadControl::Trigger(t) => t.name(),
        }
    }

    pub fn from_name(name: &str) -> Option<PadControl> {
        PadButton::from_name(name)
            .map(PadControl::Button)
            .or_else(|| Trigger::from_name(name).map(PadControl::Trigger))
    }
}

/// Where a Radio switch sits. Switch channels are read by range, so a
/// switch never needs calibrating: the bottom third of travel is low, the
/// middle third middle, the top third high. A button channel (CH9 and up) is
/// pressed or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    Low,
    Middle,
    High,
    Pressed,
}

impl Position {
    pub fn word(self) -> &'static str {
        match self {
            Position::Low => "low",
            Position::Middle => "middle",
            Position::High => "high",
            Position::Pressed => "pressed",
        }
    }

    pub fn from_word(word: &str) -> Option<Position> {
        [
            Position::Low,
            Position::Middle,
            Position::High,
            Position::Pressed,
        ]
        .into_iter()
        .find(|p| p.word() == word)
    }

    /// Where a switch on an axis channel sits, read by thirds of travel.
    pub fn of_axis(value: i16) -> Position {
        let third = -AXIS_MIN / 3.0;
        let value = f64::from(value);
        if value < -third {
            Position::Low
        } else if value > third {
            Position::High
        } else {
            Position::Middle
        }
    }
}

/// A keyboard key, as the game names it. Keys can drive Virtual Switches; they
/// never fly a Quad.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(String);

impl Key {
    /// The key with this name: a letter `A`–`Z`, a digit `0`–`9`, `F1`–`F12`,
    /// or one of [`Key::NAMED`]. `None` for any other name.
    pub fn named(name: &str) -> Option<Key> {
        let ok = match name.as_bytes() {
            [c] => c.is_ascii_uppercase() || c.is_ascii_digit(),
            [b'F', rest @ ..] if !rest.is_empty() => std::str::from_utf8(rest)
                .ok()
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=12).contains(&n) && !rest.starts_with(b"0")),
            _ => Key::NAMED.contains(&name),
        };
        ok.then(|| Key(name.to_string()))
    }

    /// The keys with a name of more than one character, besides `F1`–`F12`.
    pub const NAMED: &'static [&'static str] = &[
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "Escape",
        "Up",
        "Down",
        "Left",
        "Right",
        "LeftShift",
        "RightShift",
        "LeftCtrl",
        "RightCtrl",
        "LeftAlt",
        "RightAlt",
    ];

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
