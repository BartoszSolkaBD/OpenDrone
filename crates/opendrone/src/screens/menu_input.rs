//! Moving around the menus with an Input Device (#23, #19 §7).
//!
//! - **A Radio:** the right stick moves the cursor, a flick of yaw to the
//!   right selects, and a flick of yaw to the left goes back. Menus never read
//!   the throttle or the switches, so a Radio with a switch left on, or its
//!   throttle held up, never does anything in a menu. It reads the
//!   roll, pitch and yaw Channels, which already come out of the channel
//!   mapping, so it works on any stick order the profile knows.
//! - **A Gamepad:** the D-pad or the left stick moves, ✕ selects, ○ goes back,
//!   and L1 and R1 switch tabs. Options (or Start) pauses: that is the Pause
//!   Action, which the profile binds.
//! - **The keyboard** and the mouse always work too.
//!
//! A move repeats while it is held: after half a second, then every 150 ms.
//! A stick or button already held when a menu opens (the pilot paused with
//! the stick over) does nothing until it is let go, so opening the Pause Menu
//! in a turn never selects the item under the cursor.
//!
//! Everything here is plain code on plain numbers, so the readable checks
//! drive it with no device.

use std::time::Duration;

use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::raw::PadState;

/// One thing a pilot does in a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuInput {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    /// L1: the tab to the left.
    PreviousTab,
    /// R1: the tab to the right.
    NextTab,
    /// The Pause Action: Esc, Options, or a switch the pilot bound.
    Pause,
}

/// A held move waits this long before it repeats...
pub const REPEAT_AFTER: Duration = Duration::from_millis(500);

/// ...and then repeats this often.
pub const REPEAT_EVERY: Duration = Duration::from_millis(150);

/// Half of a Channel's full travel, in µs: 988 to 2012 around 1500.
const HALF_TRAVEL_US: f64 = 512.0;
const CENTRE_US: f64 = 1500.0;

/// How far a stick must go from centre to move the cursor, and how near
/// centre it must come back to let go (a share of full travel).
const MOVE_AT: f64 = 0.5;
const MOVE_RELEASED: f64 = 0.25;

/// How far yaw must go to count as a flick, and how near centre it must come
/// back before it can flick again.
const FLICK_AT: f64 = 0.7;
const FLICK_RELEASED: f64 = 0.3;

/// A SDL gamepad stick's full travel, from the centre.
const PAD_TRAVEL: f64 = 32767.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn input(self) -> MenuInput {
        match self {
            Direction::Up => MenuInput::Up,
            Direction::Down => MenuInput::Down,
            Direction::Left => MenuInput::Left,
            Direction::Right => MenuInput::Right,
        }
    }
}

/// A move being held: which, and when it repeats next (`None`: it was held
/// before the menu opened, so it only counts once it is let go).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    direction: Direction,
    next: Option<Duration>,
}

/// Turns what is held each frame into moves, with repeats.
#[derive(Clone, Debug, Default)]
struct Hold {
    held: Option<Held>,
}

impl Hold {
    /// One frame at `now`: `wanted` is the direction pushed now, if any, and
    /// `keeps` whether the held direction is still pushed, with the room
    /// (hysteresis) a stick needs. `first` says this is the first frame the
    /// menu is read.
    fn update(
        &mut self,
        now: Duration,
        wanted: Option<Direction>,
        keeps: impl Fn(Direction) -> bool,
        first: bool,
    ) -> Option<MenuInput> {
        if let Some(held) = &mut self.held {
            if keeps(held.direction) {
                return match held.next {
                    Some(next) if next <= now => {
                        held.next = Some(now + REPEAT_EVERY);
                        Some(held.direction.input())
                    }
                    _ => None,
                };
            }
            self.held = None;
        }
        let direction = wanted?;
        if first {
            self.held = Some(Held {
                direction,
                next: None,
            });
            return None;
        }
        self.held = Some(Held {
            direction,
            next: Some(now + REPEAT_AFTER),
        });
        Some(direction.input())
    }
}

/// A Radio's menu: its right stick and its yaw.
#[derive(Clone, Debug, Default)]
pub struct RadioMenu {
    started: bool,
    stick: Hold,
    /// Which way yaw flicked and hasn't come back yet.
    flicked: Option<bool>,
}

impl RadioMenu {
    pub fn new() -> RadioMenu {
        RadioMenu::default()
    }

    /// Forgets everything, so what is held now only counts once it is let go.
    pub fn restart(&mut self) {
        *self = RadioMenu::default();
    }

    /// One frame's reading at `now`: the roll, pitch and yaw Channels in µs.
    /// Roll right and pitch up read high, as they do to a Flight Controller.
    /// The throttle and the switches are never read.
    pub fn read(&mut self, now: Duration, roll: f64, pitch: f64, yaw: f64) -> Vec<MenuInput> {
        let first = !self.started;
        self.started = true;
        let roll = deflection(roll);
        let pitch = deflection(pitch);
        let yaw = deflection(yaw);
        let mut moves = Vec::new();

        let wanted = push(roll, pitch, MOVE_AT, Direction::Right, Direction::Up);
        let keeps = |direction: Direction| match direction {
            Direction::Right => roll >= MOVE_RELEASED,
            Direction::Left => roll <= -MOVE_RELEASED,
            Direction::Up => pitch >= MOVE_RELEASED,
            Direction::Down => pitch <= -MOVE_RELEASED,
        };
        moves.extend(self.stick.update(now, wanted, keeps, first));

        match self.flicked {
            Some(_) if yaw.abs() < FLICK_RELEASED => self.flicked = None,
            Some(_) => {}
            None if yaw.abs() >= FLICK_AT => {
                self.flicked = Some(yaw > 0.0);
                if !first {
                    moves.push(if yaw > 0.0 {
                        MenuInput::Select
                    } else {
                        MenuInput::Back
                    });
                }
            }
            None => {}
        }
        moves
    }
}

/// A stick's deflection from centre, from −1 to +1.
fn deflection(us: f64) -> f64 {
    (us - CENTRE_US) / HALF_TRAVEL_US
}

/// The direction a stick with these two deflections is pushed, if either
/// passes `at`: the stronger one wins. `positive_x` is the direction of a
/// positive horizontal deflection, and `positive_y` of a positive vertical one.
fn push(
    x: f64,
    y: f64,
    at: f64,
    positive_x: Direction,
    positive_y: Direction,
) -> Option<Direction> {
    let opposite = |d: Direction| match d {
        Direction::Up => Direction::Down,
        Direction::Down => Direction::Up,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    };
    if x.abs() >= y.abs() {
        (x.abs() >= at).then(|| {
            if x > 0.0 {
                positive_x
            } else {
                opposite(positive_x)
            }
        })
    } else {
        (y.abs() >= at).then(|| {
            if y > 0.0 {
                positive_y
            } else {
                opposite(positive_y)
            }
        })
    }
}

/// A Gamepad's menu: its D-pad, left stick and buttons.
#[derive(Clone, Debug, Default)]
pub struct PadMenu {
    /// The buttons that were down at the last frame, for ✕, ○, L1 and R1.
    before: Option<[bool; 4]>,
    hold: Hold,
}

impl PadMenu {
    pub fn new() -> PadMenu {
        PadMenu::default()
    }

    /// Forgets everything, so what is held now only counts once it is let go.
    pub fn restart(&mut self) {
        *self = PadMenu::default();
    }

    /// One frame's reading at `now`.
    pub fn read(&mut self, now: Duration, pad: &PadState) -> Vec<MenuInput> {
        let first = self.before.is_none();
        let buttons = [
            pad.button(PadButton::South),
            pad.button(PadButton::East),
            pad.button(PadButton::LeftShoulder),
            pad.button(PadButton::RightShoulder),
        ];
        let before = self.before.replace(buttons).unwrap_or(buttons);
        let mut moves = Vec::new();

        let dpad = [
            (PadButton::DpadUp, Direction::Up),
            (PadButton::DpadDown, Direction::Down),
            (PadButton::DpadLeft, Direction::Left),
            (PadButton::DpadRight, Direction::Right),
        ]
        .into_iter()
        .find(|(button, _)| pad.button(*button))
        .map(|(_, direction)| direction);
        // SDL: stick right and down read positive.
        let x = f64::from(pad.axis(PadAxis::LeftX)) / PAD_TRAVEL;
        let y = f64::from(pad.axis(PadAxis::LeftY)) / PAD_TRAVEL;
        let stick = push(x, y, MOVE_AT, Direction::Right, Direction::Down);
        let wanted = dpad.or(stick);
        let keeps = |direction: Direction| {
            let pressed = |button| pad.button(button);
            match direction {
                Direction::Up => pressed(PadButton::DpadUp) || y <= -MOVE_RELEASED,
                Direction::Down => pressed(PadButton::DpadDown) || y >= MOVE_RELEASED,
                Direction::Left => pressed(PadButton::DpadLeft) || x <= -MOVE_RELEASED,
                Direction::Right => pressed(PadButton::DpadRight) || x >= MOVE_RELEASED,
            }
        };
        moves.extend(self.hold.update(now, wanted, keeps, first));

        let pressed = |i: usize| buttons[i] && !before[i];
        if pressed(0) {
            moves.push(MenuInput::Select);
        }
        if pressed(1) {
            moves.push(MenuInput::Back);
        }
        if pressed(2) {
            moves.push(MenuInput::PreviousTab);
        }
        if pressed(3) {
            moves.push(MenuInput::NextTab);
        }
        moves
    }
}
