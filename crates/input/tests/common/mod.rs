//! What the readable checks share: the recorded device traces committed in
//! `tests/traces/`, replayed as the input thread's batches, and the built-in
//! Pack's Input Device profiles, read by the Pack checker.
//!
//! The traces are the SDL3 input probe's recordings from #18, #27 and #30
//! (the `prototype/sdl3-input-probe` branch), trimmed. Each file's header
//! says which run, which stretch, the device, and every control's value
//! where the stretch starts. Rows sharing a stamp were read in one poll, so
//! they form one batch, as they would from the input thread.
//!
//! The probe recorded SDL's joystick events. The DualSense goes through SDL's
//! own HIDAPI driver, which sends its joystick axes and buttons in SDL's
//! gamepad order, so a gamepad's events here are made from them as SDL's
//! gamepad layer makes them: triggers rescaled from −32768…32767 to
//! 0…32767, buttons 0–10 kept, 11 the touchpad, 12 the microphone (misc1),
//! and the D-pad from the hat.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::{Connection, InputDeviceProfile};
use opendrone_input::raw::PadState;
use opendrone_input::{Batch, DeviceInfo, DeviceState, InputEvent, Inputs, Raw, SdlId};

/// The probe's one device.
pub const DEVICE: SdlId = SdlId(1);

/// One recorded change.
#[derive(Clone, Debug)]
pub struct Row {
    /// When the probe's input thread read it.
    pub at: Duration,
    /// The probe's step, such as `circles`.
    pub step: String,
    pub kind: String,
    pub index: usize,
    pub value: i32,
}

/// One trimmed recording.
#[derive(Clone, Debug)]
pub struct Trace {
    pub device: DeviceInfo,
    /// Every control's value where the recording starts.
    pub start: DeviceState,
    pub rows: Vec<Row>,
}

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every Input Device profile in the built-in Pack, through the Pack checker.
pub fn built_in_profiles() -> Vec<InputDeviceProfile> {
    let packs = opendrone_pack::Packs::open(&repo().join("packs"), "packs")
        .expect("the built-in Packs open");
    assert!(
        packs.problems().is_empty(),
        "the built-in Pack has problems:\n{}",
        packs.problems()
    );
    packs.input_devices().into_iter().cloned().collect()
}

/// One built-in profile by id, such as `opendrone/dualsense`.
pub fn profile(id: &str) -> InputDeviceProfile {
    built_in_profiles()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("the built-in Pack has no profile {id}"))
}

/// The SDL gamepad layer's trigger: −32768…32767 becomes 0…32767
/// (`SDL_gamepad.c`, `HandleJoystickAxis`).
fn trigger(value: i32) -> i16 {
    let normalized = (value as f32 + 32768.0) / 65535.0;
    (normalized * 32767.0) as i16
}

/// The gamepad button a DualSense joystick button is (`SDL_hidapi_ps5.c`).
fn pad_button(index: usize) -> Option<PadButton> {
    match index {
        0..=10 => PadButton::from_index(index),
        11 => Some(PadButton::Touchpad),
        12 => Some(PadButton::Misc1),
        _ => None,
    }
}

/// The D-pad buttons a hat value holds down (SDL's hat bits: up 1, right 2,
/// down 4, left 8).
fn dpad(hat: i32) -> [(PadButton, bool); 4] {
    [
        (PadButton::DpadUp, hat & 1 != 0),
        (PadButton::DpadRight, hat & 2 != 0),
        (PadButton::DpadDown, hat & 4 != 0),
        (PadButton::DpadLeft, hat & 8 != 0),
    ]
}

impl Trace {
    /// Reads `tests/traces/<name>`.
    pub fn read(name: &str) -> Trace {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/traces")
            .join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()));
        let mut device = None;
        let mut axes: Vec<i16> = Vec::new();
        let mut buttons_down: Vec<usize> = Vec::new();
        let mut hat = 0;
        let mut rows = Vec::new();
        for line in text.lines() {
            if let Some(header) = line.strip_prefix("# device: ") {
                let field = |key: &str| {
                    header
                        .split("; ")
                        .find_map(|part| part.strip_prefix(&format!("{key} = ")))
                        .unwrap_or_else(|| panic!("{name}: the device line has no {key}"))
                        .to_string()
                };
                let usb = field("usb");
                let (vendor, product) = usb.split_once(':').expect("usb = VVVV:PPPP");
                device = Some(DeviceInfo {
                    name: field("name"),
                    usb_vendor: u16::from_str_radix(vendor, 16).unwrap(),
                    usb_product: u16::from_str_radix(product, 16).unwrap(),
                    sdl_gamepad: field("sdl_gamepad") == "true",
                    connection: (field("bus") == "USB").then_some(Connection::Usb),
                    heartbeat: field("heartbeat") == "true",
                });
            } else if let Some(list) = line.strip_prefix("# axes at start: ") {
                axes = list.split(',').map(|v| v.parse().unwrap()).collect();
            } else if let Some(list) = line.strip_prefix("# buttons down at start:") {
                buttons_down = list
                    .split(',')
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| v.trim().parse().unwrap())
                    .collect();
            } else if let Some(value) = line.strip_prefix("# hat at start: ") {
                hat = value.parse().unwrap();
            } else if line.starts_with('#') || line.starts_with("t_rx_ns") || line.is_empty() {
                continue;
            } else {
                let parts: Vec<&str> = line.split(',').collect();
                rows.push(Row {
                    at: Duration::from_nanos(parts[0].parse().unwrap()),
                    step: parts[1].to_string(),
                    kind: parts[2].to_string(),
                    index: parts[3].parse().unwrap(),
                    value: parts[4].parse().unwrap(),
                });
            }
        }
        let device = device.unwrap_or_else(|| panic!("{name} has no device line"));
        let gamepad = device.sdl_gamepad;
        let buttons = if gamepad { 13 } else { 24 };
        let mut start = DeviceState::new(axes.len(), buttons, gamepad);
        start.axes.clone_from(&axes);
        for &b in &buttons_down {
            start.buttons[b] = true;
        }
        if let Some(pad) = start.pad.as_mut() {
            *pad = PadState::at_rest();
            for (i, &value) in axes.iter().enumerate().take(PadAxis::COUNT) {
                pad.axes[i] = if i >= 4 {
                    trigger(i32::from(value))
                } else {
                    value
                };
            }
            for &b in &buttons_down {
                if let Some(button) = pad_button(b) {
                    pad.buttons[button.index()] = true;
                }
            }
            for (button, down) in dpad(hat) {
                pad.buttons[button.index()] = down;
            }
        }
        Trace {
            device,
            start,
            rows,
        }
    }

    /// The trace as the input thread's batches: the device plugged in first
    /// (at its `added` row, or with the first row), then each poll's changes.
    pub fn batches(&self) -> Vec<Batch> {
        let mut batches: Vec<Batch> = Vec::new();
        let added = Raw::Added {
            device: DEVICE,
            info: self.device.clone(),
            state: self.start.clone(),
        };
        let mut pending_add = Some(added);
        let mut hat = 0;
        if let Some(pad) = &self.start.pad {
            hat = i32::from(pad.button(PadButton::DpadUp))
                | i32::from(pad.button(PadButton::DpadRight)) << 1
                | i32::from(pad.button(PadButton::DpadDown)) << 2
                | i32::from(pad.button(PadButton::DpadLeft)) << 3;
        }
        let gamepad = self.device.sdl_gamepad;
        for row in &self.rows {
            if batches.last().is_none_or(|b| b.at != row.at) {
                batches.push(Batch {
                    at: row.at,
                    events: Vec::new(),
                });
            }
            let events = &mut batches.last_mut().unwrap().events;
            if let Some(add) = pending_add.take() {
                events.push(add);
                if row.kind == "added" {
                    continue;
                }
            }
            match row.kind.as_str() {
                "axis" => {
                    let value = i16::try_from(row.value).unwrap();
                    events.push(Raw::Axis {
                        device: DEVICE,
                        axis: u8::try_from(row.index).unwrap(),
                        value,
                    });
                    if gamepad && let Some(axis) = PadAxis::from_index(row.index) {
                        let value = if row.index >= 4 {
                            trigger(row.value)
                        } else {
                            value
                        };
                        events.push(Raw::PadAxis {
                            device: DEVICE,
                            axis,
                            value,
                        });
                    }
                }
                "button" => {
                    let down = row.value != 0;
                    events.push(Raw::Button {
                        device: DEVICE,
                        button: u8::try_from(row.index).unwrap(),
                        down,
                    });
                    if gamepad && let Some(button) = pad_button(row.index) {
                        events.push(Raw::PadButton {
                            device: DEVICE,
                            button,
                            down,
                        });
                    }
                }
                "hat" => {
                    for ((button, down), (_, was)) in dpad(row.value).into_iter().zip(dpad(hat)) {
                        if down != was {
                            events.push(Raw::PadButton {
                                device: DEVICE,
                                button,
                                down,
                            });
                        }
                    }
                    hat = row.value;
                }
                "report" => events.push(Raw::Report { device: DEVICE }),
                "removed" => events.push(Raw::Removed { device: DEVICE }),
                "added" => {}
                other => panic!("a trace row of an unknown kind: {other}"),
            }
        }
        batches
    }

    /// When the probe's step `step` first appears.
    pub fn step_starts(&self, step: &str) -> Duration {
        self.rows
            .iter()
            .find(|r| r.step == step)
            .unwrap_or_else(|| panic!("no step {step}"))
            .at
    }

    /// One axis's values with their stamps, the start value first.
    pub fn axis(&self, index: usize) -> Vec<(Duration, i16)> {
        let mut values = vec![(Duration::ZERO, self.start.axes[index])];
        values.extend(
            self.rows
                .iter()
                .filter(|r| r.kind == "axis" && r.index == index)
                .map(|r| (r.at, i16::try_from(r.value).unwrap())),
        );
        values
    }

    /// When the last row was read.
    pub fn end(&self) -> Duration {
        self.rows.last().map_or(Duration::ZERO, |r| r.at)
    }
}

/// Plays batches into `inputs`, as the game would, checking for silent
/// devices every 10 ms of the recording between them. Returns every event.
pub fn replay(inputs: &mut Inputs, batches: Vec<Batch>) -> Vec<InputEvent> {
    let mut events = Vec::new();
    let mut checked = Duration::ZERO;
    for batch in batches {
        while checked + Duration::from_millis(10) < batch.at {
            checked += Duration::from_millis(10);
            events.extend(inputs.check_silence(checked));
        }
        events.extend(inputs.take(batch));
    }
    events
}

/// Every Channels event, with its stamp.
pub fn channels(events: &[InputEvent]) -> Vec<(Duration, opendrone_input::Channels)> {
    events
        .iter()
        .filter_map(|e| match e {
            InputEvent::Channels { at, channels, .. } => Some((*at, *channels)),
            _ => None,
        })
        .collect()
}

/// The Channels in force at `at`: the last ones handed over at or before it.
pub fn channels_at(events: &[InputEvent], at: Duration) -> opendrone_input::Channels {
    channels(events)
        .into_iter()
        .take_while(|(when, _)| *when <= at)
        .last()
        .expect("Channels were handed over by then")
        .1
}

/// Seconds as a stamp.
pub fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}
