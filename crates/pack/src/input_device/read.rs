//! Reading and checking an Input Device profile, `input-devices/<id>.toml`
//! (#19 §8). Every problem names its file, its line and a plain sentence, and
//! all of a file's problems are listed at once.

use std::collections::BTreeMap;

use super::controls::{
    AXIS_MAX, Key, PadButton, PadControl, Position, RADIO_AXIS_CHANNELS, RADIO_CHANNELS, Stick,
    value_at_percent,
};
use super::{
    Action, ActionSource, Connection, DeviceFacts, Ends, FlightMode, FlightModeSwitch,
    GamepadCalibration, GamepadChannels, GamepadSetup, GamepadThrottle, InputDeviceProfile, Kind,
    Match, OnOffSwitch, PadStick, Press, PressStyle, RadioCalibration, RadioChannels, RadioSetup,
    RadioStick, Setup, StickCalibration, Switches, ThrottleZero,
};
use crate::document::{Document, FORMAT, Item, Problems, Table};
use crate::migration::{self, FileKind, PACK_STEPS, Step};
use crate::units::{self, Dimension};

const TOP_KEYS: &[&str] = &[
    "format",
    "name",
    "kind",
    "match",
    "report_rate",
    "reports_at_rest",
    "labels",
    "channels",
    "switches",
    "actions",
    "calibration",
];

/// The stick functions, in the order a profile writes them.
const STICKS: &[&str] = &["roll", "pitch", "throttle", "yaw"];

/// The switch Channels with fixed meanings (ADR-0017).
const SWITCHES: &[&str] = &["arm", "flight_mode", "crash_flip"];

/// Reads and checks one Input Device profile. `id` is its id, such as
/// `opendrone/dualsense`; `file` is how problems name the file. An older
/// file is upgraded in memory first, like every Pack file.
pub fn read_input_device_file(
    id: &str,
    file: &str,
    text: &str,
) -> Result<InputDeviceProfile, Problems> {
    read_input_device_file_with_steps(id, file, text, PACK_STEPS)
}

/// [`read_input_device_file`], upgrading an older file with `steps` instead
/// of [`PACK_STEPS`]: the steps must lead to the newest format, [`FORMAT`].
/// The readable checks give it a synthetic step.
pub fn read_input_device_file_with_steps(
    id: &str,
    file: &str,
    text: &str,
    steps: &[Step],
) -> Result<InputDeviceProfile, Problems> {
    let text = migration::upgraded_with(file, text, FileKind::InputDevice, FORMAT, steps)?;
    let doc = Document::parse(file, &text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    if doc.is_newer() {
        return Err(problems);
    }
    let root = doc.root();
    root.refuse_unknown(TOP_KEYS, &mut problems);
    let name = root.text("name", &mut problems).and_then(|(name, item)| {
        if name.trim().is_empty() {
            problems.push(item.problem("`name` can't be empty"));
            None
        } else {
            Some(name.to_string())
        }
    });
    let kind = root
        .require("kind", &mut problems)
        .and_then(|item| match item.as_str() {
            Some("Radio") => Some(Kind::Radio),
            Some("Gamepad") => Some(Kind::Gamepad),
            _ => {
                problems.push(item.problem(format!(
                    "\"{}\" isn't a kind of Input Device: write \"Radio\" or \"Gamepad\"",
                    item.as_str().unwrap_or("this")
                )));
                None
            }
        });
    let matches = root
        .require("match", &mut problems)
        .and_then(|item| read_match(&item, &mut problems));
    let report_rate = per_connection(&root, "report_rate", &mut problems, |item, problems| {
        let text = item.text(problems)?;
        match units::parse_quantity(text).and_then(|q| q.as_a(Dimension::PER_SECOND)) {
            Ok(rate) if rate > 0.0 => Some(rate),
            Ok(_) => {
                problems.push(item.problem(format!(
                    "`{}` must be above zero, such as \"250 Hz\"",
                    item.key()
                )));
                None
            }
            Err(p) => {
                problems.push(item.problem(p.0));
                None
            }
        }
    });
    let reports_at_rest =
        per_connection(&root, "reports_at_rest", &mut problems, |item, problems| {
            let value = item.boolean();
            if value.is_none() {
                problems.push(item.problem(format!(
                    "`{}` is written true or false, without quotes",
                    item.key()
                )));
            }
            value
        });
    let labels = match (root.get("labels"), kind) {
        (Some(item), Some(Kind::Radio)) => {
            problems.push(item.problem(
                "a Radio's controls are named by channel, so only a Gamepad profile has [labels]",
            ));
            Vec::new()
        }
        (Some(item), _) => read_labels(&item, &mut problems),
        (None, _) => Vec::new(),
    };
    let facts = name.zip(matches).map(|(name, matches)| DeviceFacts {
        name,
        matches,
        report_rate,
        reports_at_rest,
        labels,
    });
    let setup = match kind {
        Some(Kind::Radio) => read_radio(&root, &mut problems).map(Setup::Radio),
        Some(Kind::Gamepad) => facts
            .as_ref()
            .and_then(|facts| read_gamepad(&root, facts, &mut problems))
            .map(Setup::Gamepad),
        None => None,
    };
    match (facts, setup) {
        (Some(facts), Some(setup)) => problems.or(InputDeviceProfile {
            id: id.to_string(),
            facts,
            setup,
        }),
        _ => Err(problems),
    }
}

/// `match = { usb_vendor = "1209", usb_product = "4F54", product_name = "…" }`
/// or `match = "any"`.
fn read_match(item: &Item, problems: &mut Problems) -> Option<Match> {
    if item.as_str() == Some("any") {
        return Some(Match::Any);
    }
    if !item.is_table() {
        problems.push(item.problem(
            "`match` is { usb_vendor = \"…\", usb_product = \"…\", product_name = \"…\" }, or \"any\" for a fallback such as \"Any Radio\"",
        ));
        return None;
    }
    let table = item.table(problems)?;
    table.refuse_unknown(&["usb_vendor", "usb_product", "product_name"], problems);
    let mut id = |key: &str| {
        let (text, item) = table.text(key, problems)?;
        if text.len() == 4 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
            u16::from_str_radix(text, 16).ok()
        } else {
            problems.push(item.problem(format!(
                "`{key}` is a USB id: four hex digits in quotes, such as \"1209\""
            )));
            None
        }
    };
    let usb_vendor = id("usb_vendor");
    let usb_product = id("usb_product");
    let product_name = table
        .text("product_name", problems)
        .and_then(|(name, item)| {
            if name.trim().is_empty() {
                problems.push(item.problem(
                    "`product_name` can't be empty: it's found anywhere in the device's name",
                ));
                None
            } else {
                Some(name.to_string())
            }
        });
    Some(Match::Device {
        usb_vendor: usb_vendor?,
        usb_product: usb_product?,
        product_name: product_name?,
    })
}

/// A table of values per connection, such as `report_rate = { usb = "1000 Hz" }`.
fn per_connection<T>(
    root: &Table,
    key: &str,
    problems: &mut Problems,
    read: impl Fn(&Item, &mut Problems) -> Option<T>,
) -> BTreeMap<Connection, T> {
    let mut values = BTreeMap::new();
    let Some(table) = root.get(key).and_then(|item| item.table(problems)) else {
        return values;
    };
    for (word, item) in table.entries() {
        let Some(connection) = Connection::from_word(&word) else {
            problems.push(item.problem(format!(
                "`{word}` isn't a connection OpenDrone knows: write `usb` or `bluetooth`"
            )));
            continue;
        };
        if let Some(value) = read(&item, problems) {
            values.insert(connection, value);
        }
    }
    values
}

/// `[labels]`: a Gamepad's own names for SDL's positions, such as
/// `R1 = "right_shoulder"`.
fn read_labels(item: &Item, problems: &mut Problems) -> Vec<(String, PadControl)> {
    let Some(table) = item.table(problems) else {
        return Vec::new();
    };
    let mut labels = Vec::new();
    for (label, item) in table.entries() {
        if PadControl::from_name(&label).is_some() {
            problems.push(item.problem(format!(
                "\"{label}\" is already SDL's name for a position, so it can't be a label too"
            )));
            continue;
        }
        let Some(name) = item.text(problems) else {
            continue;
        };
        match PadControl::from_name(name) {
            Some(control) => labels.push((label, control)),
            None => problems.push(item.problem(format!(
                "\"{name}\" isn't one of SDL's gamepad positions: write a button such as \"south\" or \"right_shoulder\", or \"left_trigger\" or \"right_trigger\""
            ))),
        }
    }
    labels
}

/// The section `[name]`, or a problem if it's missing.
fn section<'d, 't>(
    root: &Table<'d, 't>,
    name: &str,
    problems: &mut Problems,
) -> Option<Table<'d, 't>> {
    root.table(name, problems)
}

/// An optional section: missing means empty.
fn optional_section<'d, 't>(
    root: &Table<'d, 't>,
    name: &str,
    problems: &mut Problems,
) -> Option<Table<'d, 't>> {
    root.get(name).and_then(|item| item.table(problems))
}

/// A whole number from `least` to `most`.
fn whole(item: &Item, least: i64, most: i64, problems: &mut Problems) -> Option<i64> {
    match item.integer() {
        Some(n) if (least..=most).contains(&n) => Some(n),
        _ => {
            problems.push(item.problem(format!(
                "`{}` is a whole number from {least} to {most}, without quotes",
                item.key()
            )));
            None
        }
    }
}

/// An optional `reverse = true`.
fn reverse(table: &Table, problems: &mut Problems) -> bool {
    match table.get("reverse") {
        None => false,
        Some(item) => item.boolean().unwrap_or_else(|| {
            problems.push(item.problem("`reverse` is written true or false, without quotes"));
            false
        }),
    }
}

fn read_radio(root: &Table, problems: &mut Problems) -> Option<RadioSetup> {
    let channels = section(root, "channels", problems).and_then(|table| {
        table.refuse_unknown(STICKS, problems);
        let mut stick = |name: &str| -> Option<RadioStick> {
            let item = table.require(name, problems)?;
            let entry = item.table(problems)?;
            entry.refuse_unknown(&["channel", "reverse"], problems);
            let channel = entry.require("channel", problems).and_then(|c| {
                whole(&c, 1, i64::from(RADIO_AXIS_CHANNELS), problems)
            });
            let reverse = reverse(&entry, problems);
            Some(RadioStick {
                channel: u8::try_from(channel?).ok()?,
                reverse,
            })
        };
        let (roll, pitch, throttle, yaw) = (stick("roll"), stick("pitch"), stick("throttle"), stick("yaw"));
        let channels = RadioChannels {
            roll: roll?,
            pitch: pitch?,
            throttle: throttle?,
            yaw: yaw?,
        };
        let all = channels.all();
        for (i, a) in all.iter().enumerate() {
            if all[..i].iter().any(|b| b.channel == a.channel) {
                problems.push(table.problem(format!(
                    "CH{} carries two sticks: each of roll, pitch, throttle and yaw needs its own channel",
                    a.channel
                )));
                return None;
            }
        }
        Some(channels)
    });
    let stick_channels: Vec<u8> = channels
        .map(|c| c.all().iter().map(|s| s.channel).collect())
        .unwrap_or_default();
    let switches = read_switches(root, Kind::Radio, None, &stick_channels, problems);
    let switch_channels: Vec<u8> = switches.as_ref().map(switch_channels).unwrap_or_default();
    let actions = read_actions(
        root,
        Kind::Radio,
        None,
        &stick_channels,
        &switch_channels,
        problems,
    );
    let calibration = section(root, "calibration", problems).and_then(|table| {
        table.refuse_unknown(STICKS, problems);
        let roll = stick_calibration(&table, "roll", true, problems);
        let pitch = stick_calibration(&table, "pitch", true, problems);
        let yaw = stick_calibration(&table, "yaw", true, problems);
        let throttle = ends(&table, "throttle", false, problems);
        Some(RadioCalibration {
            roll: roll?,
            pitch: pitch?,
            yaw: yaw?,
            throttle: throttle?,
        })
    });
    Some(RadioSetup {
        channels: channels?,
        switches: switches?,
        actions: actions?,
        calibration: calibration?,
    })
}

fn read_gamepad(
    root: &Table,
    facts: &DeviceFacts,
    problems: &mut Problems,
) -> Option<GamepadSetup> {
    let channels = section(root, "channels", problems).and_then(|table| {
        table.refuse_unknown(STICKS, problems);
        let stick_of = |entry: &Table, problems: &mut Problems| -> Option<Stick> {
            let (name, item) = entry.text("stick", problems)?;
            let stick = Stick::from_name(name);
            if stick.is_none() {
                problems.push(item.problem(format!(
                    "\"{name}\" isn't a stick: write \"left_x\", \"left_y\", \"right_x\" or \"right_y\""
                )));
            }
            stick
        };
        let pad_stick = |name: &str, problems: &mut Problems| -> Option<PadStick> {
            let entry = table.require(name, problems)?.table(problems)?;
            entry.refuse_unknown(&["stick", "reverse"], problems);
            let stick = stick_of(&entry, problems);
            let reverse = reverse(&entry, problems);
            Some(PadStick {
                stick: stick?,
                reverse,
            })
        };
        let roll = pad_stick("roll", problems);
        let pitch = pad_stick("pitch", problems);
        let yaw = pad_stick("yaw", problems);
        let throttle = table.require("throttle", problems).and_then(|item| {
            let entry = item.table(problems)?;
            if let Some(trigger) = entry.get("trigger") {
                entry.refuse_unknown(&["trigger"], problems);
                let name = trigger.text(problems)?;
                return match facts.control_named(name) {
                    Some(PadControl::Trigger(t)) => Some(GamepadThrottle::Trigger(t)),
                    _ => {
                        problems.push(trigger.problem(format!(
                            "\"{name}\" isn't one of this Gamepad's triggers: write a label from [labels], or \"left_trigger\" or \"right_trigger\""
                        )));
                        None
                    }
                };
            }
            entry.refuse_unknown(&["stick", "zero"], problems);
            let stick = {
                let (name, item) = entry.text("stick", problems)?;
                let stick = Stick::from_name(name);
                if stick.is_none() {
                    problems.push(item.problem(format!(
                        "\"{name}\" isn't a stick: write \"left_x\", \"left_y\", \"right_x\" or \"right_y\", or use trigger = \"…\""
                    )));
                }
                stick
            };
            let zero = entry.text("zero", problems).and_then(|(word, item)| match word {
                "at rest" => Some(ThrottleZero::AtRest),
                "at bottom" => Some(ThrottleZero::AtBottom),
                _ => {
                    problems.push(item.problem(format!(
                        "\"{word}\" isn't where a throttle stick reads zero: write \"at rest\" or \"at bottom\""
                    )));
                    None
                }
            });
            Some(GamepadThrottle::Stick {
                stick: stick?,
                zero: zero?,
            })
        });
        let channels = GamepadChannels {
            roll: roll?,
            pitch: pitch?,
            yaw: yaw?,
            throttle: throttle?,
        };
        let mut used = vec![channels.roll.stick, channels.pitch.stick, channels.yaw.stick];
        if let GamepadThrottle::Stick { stick, .. } = channels.throttle {
            used.push(stick);
        }
        for (i, a) in used.iter().enumerate() {
            if used[..i].contains(a) {
                problems.push(table.problem(format!(
                    "the stick axis {} carries two Channels: each of roll, pitch, throttle and yaw needs its own",
                    a.name()
                )));
                return None;
            }
        }
        Some(channels)
    });
    let switches = read_switches(root, Kind::Gamepad, Some(facts), &[], problems);
    let actions = read_actions(root, Kind::Gamepad, Some(facts), &[], &[], problems);
    let calibration = section(root, "calibration", problems).and_then(|table| {
        table.refuse_unknown(
            &[
                "left_x",
                "left_y",
                "right_x",
                "right_y",
                "left_trigger",
                "right_trigger",
            ],
            problems,
        );
        let left_x = stick_calibration(&table, "left_x", false, problems);
        let left_y = stick_calibration(&table, "left_y", false, problems);
        let right_x = stick_calibration(&table, "right_x", false, problems);
        let right_y = stick_calibration(&table, "right_y", false, problems);
        let trigger = |name: &str, problems: &mut Problems| match table.get(name) {
            Some(_) => ends(&table, name, true, problems),
            None => Some(Ends::full_trigger()),
        };
        let left_trigger = trigger("left_trigger", problems);
        let right_trigger = trigger("right_trigger", problems);
        Some(GamepadCalibration {
            left_x: left_x?,
            left_y: left_y?,
            right_x: right_x?,
            right_y: right_y?,
            left_trigger: left_trigger?,
            right_trigger: right_trigger?,
        })
    });
    Some(GamepadSetup {
        channels: channels?,
        switches: switches?,
        actions: actions?,
        calibration: calibration?,
    })
}

/// Every Radio channel a switch uses.
fn switch_channels(switches: &Switches) -> Vec<u8> {
    let on_off = |s: &Option<OnOffSwitch>| match s {
        Some(OnOffSwitch::Channel { channel, .. }) => Some(*channel),
        _ => None,
    };
    let flight_mode = match &switches.flight_mode {
        Some(FlightModeSwitch::Channel { channel, .. }) => Some(*channel),
        _ => None,
    };
    [
        on_off(&switches.arm),
        flight_mode,
        on_off(&switches.crash_flip),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The source keys a switch or Action may use on this kind of device.
fn sources(kind: Kind, keys: bool) -> &'static [&'static str] {
    match (kind, keys) {
        (Kind::Radio, true) => &["channel", "key"],
        (Kind::Radio, false) => &["channel"],
        (Kind::Gamepad, true) => &["button", "key"],
        (Kind::Gamepad, false) => &["button"],
    }
}

/// Which source an entry names: exactly one of `allowed`.
fn one_source<'d, 't>(
    entry: &Table<'d, 't>,
    what: &str,
    allowed: &[&str],
    problems: &mut Problems,
) -> Option<(String, Item<'d, 't>)> {
    let given: Vec<(String, Item<'d, 't>)> = ["channel", "button", "key"]
        .iter()
        .filter_map(|key| entry.get(key).map(|item| (key.to_string(), item)))
        .collect();
    let list = allowed
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(" or ");
    match given.as_slice() {
        [(key, item)] if allowed.contains(&key.as_str()) => Some((key.clone(), item.clone())),
        [(key, item)] => {
            problems.push(item.problem(format!(
                "{what} can't use a `{key}` on this kind of Input Device: give it a {list}"
            )));
            None
        }
        [] => {
            problems.push(entry.problem(format!("{what} needs a source: a {list}")));
            None
        }
        _ => {
            problems.push(entry.problem(format!(
                "{what} has more than one source; each switch Channel and Action has one source at a time: give it one {list}"
            )));
            None
        }
    }
}

/// A Gamepad button by the device's label or SDL's position name.
fn pad_button(item: &Item, facts: &DeviceFacts, problems: &mut Problems) -> Option<PadButton> {
    let name = item.text(problems)?;
    match facts.control_named(name) {
        Some(PadControl::Button(button)) => Some(button),
        Some(PadControl::Trigger(_)) => {
            problems.push(item.problem(format!(
                "\"{name}\" is a trigger, not a button: a trigger can only carry the throttle"
            )));
            None
        }
        None => {
            problems.push(item.problem(format!(
                "\"{name}\" isn't a button on this Gamepad: write a label from [labels], or SDL's position name, such as \"south\" or \"right_shoulder\""
            )));
            None
        }
    }
}

/// A keyboard key's name.
fn key(item: &Item, problems: &mut Problems) -> Option<Key> {
    let name = item.text(problems)?;
    let key = Key::named(name);
    if key.is_none() {
        problems.push(item.problem(format!(
            "\"{name}\" isn't a key OpenDrone knows: write a letter A–Z, a digit 0–9, F1–F12, or one of {}",
            Key::NAMED.join(", ")
        )));
    }
    key
}

/// A Radio channel a switch or Action reads, which no stick uses.
fn switch_channel(item: &Item, sticks: &[u8], problems: &mut Problems) -> Option<u8> {
    let channel = u8::try_from(whole(item, 1, i64::from(RADIO_CHANNELS), problems)?).ok()?;
    if sticks.contains(&channel) {
        problems.push(item.problem(format!(
            "CH{channel} carries a stick, so it can't be a switch or an Action"
        )));
        return None;
    }
    Some(channel)
}

/// A switch position: low, middle or high on CH1–CH8, pressed on CH9–CH32.
fn position(entry: &Table, key: &str, channel: u8, problems: &mut Problems) -> Option<Position> {
    let (word, item) = entry.text(key, problems)?;
    let position = Position::from_word(word);
    let fits = match position {
        Some(Position::Pressed) => channel > RADIO_AXIS_CHANNELS,
        Some(_) => channel <= RADIO_AXIS_CHANNELS,
        None => false,
    };
    if !fits {
        let choices = if channel <= RADIO_AXIS_CHANNELS {
            "\"low\", \"middle\" or \"high\" (CH1–CH8 are read by range)"
        } else {
            "\"pressed\" (CH9–CH32 are buttons)"
        };
        problems.push(item.problem(format!(
            "\"{word}\" isn't a position CH{channel} can take: write {choices}"
        )));
        return None;
    }
    position
}

fn read_switches(
    root: &Table,
    kind: Kind,
    facts: Option<&DeviceFacts>,
    sticks: &[u8],
    problems: &mut Problems,
) -> Option<Switches> {
    let Some(table) = optional_section(root, "switches", problems) else {
        return Some(Switches::default());
    };
    table.refuse_unknown(SWITCHES, problems);
    let mut ok = true;
    let mut on_off = |name: &str, problems: &mut Problems| -> Option<OnOffSwitch> {
        let entry = table.get(name)?.table(problems)?;
        let read = (|| {
            let (source, item) =
                one_source(&entry, &format!("`{name}`"), sources(kind, true), problems)?;
            if source == "channel" {
                entry.refuse_unknown(&["channel", "on"], problems);
                let channel = switch_channel(&item, sticks, problems)?;
                let on = position(&entry, "on", channel, problems)?;
                return Some(OnOffSwitch::Channel { channel, on });
            }
            entry.refuse_unknown(&[source.as_str(), "press"], problems);
            let press = press_source(&source, &item, facts, problems);
            let style = entry.text("press", problems).and_then(|(word, item)| match word {
                "toggle" => Some(PressStyle::Toggle),
                "hold" => Some(PressStyle::Hold),
                _ => {
                    problems.push(item.problem(format!(
                        "\"{word}\" isn't how a Virtual Switch answers: write \"toggle\" or \"hold\""
                    )));
                    None
                }
            });
            Some(OnOffSwitch::Virtual {
                source: press?,
                style: style?,
            })
        })();
        if read.is_none() {
            ok = false;
        }
        read
    };
    let arm = on_off("arm", problems);
    let crash_flip = on_off("crash_flip", problems);
    let flight_mode = table.get("flight_mode").and_then(|item| {
        let entry = item.table(problems)?;
        let read = (|| {
            let (source, item) =
                one_source(&entry, "`flight_mode`", sources(kind, true), problems)?;
            if source == "channel" {
                entry.refuse_unknown(&["channel", "low", "middle", "high"], problems);
                let channel = switch_channel(&item, sticks, problems)?;
                if channel > RADIO_AXIS_CHANNELS {
                    problems.push(item.problem(format!(
                        "CH{channel} is a button, but a Flight Mode switch needs a switch on CH1–CH8"
                    )));
                    return None;
                }
                let mut mode = |key: &str| {
                    let (word, item) = entry.text(key, problems)?;
                    let mode = FlightMode::from_word(word);
                    if mode.is_none() {
                        problems.push(item.problem(format!(
                            "\"{word}\" isn't a Flight Mode: write \"Acro\", \"Horizon\" or \"Angle\""
                        )));
                    }
                    mode
                };
                let (low, middle, high) = (mode("low"), mode("middle"), mode("high"));
                return Some(FlightModeSwitch::Channel {
                    channel,
                    low: low?,
                    middle: middle?,
                    high: high?,
                });
            }
            entry.refuse_unknown(&[source.as_str(), "modes"], problems);
            let press = press_source(&source, &item, facts, problems);
            let modes = match entry.get("modes") {
                None => Some(vec![FlightMode::Acro, FlightMode::Angle]),
                Some(list) => {
                    let items = list.array(problems)?;
                    let mut modes = Vec::new();
                    for item in &items {
                        match item.as_str().and_then(FlightMode::from_word) {
                            Some(mode) if !modes.contains(&mode) => modes.push(mode),
                            Some(mode) => problems.push(item.problem(format!(
                                "{} is listed twice",
                                mode.word()
                            ))),
                            None => problems.push(item.problem(
                                "each of `modes` is a Flight Mode: \"Acro\", \"Horizon\" or \"Angle\"",
                            )),
                        }
                    }
                    if modes.len() < 2 {
                        problems.push(list.problem(
                            "`modes` lists the Flight Modes each press steps through, at least two, such as [\"Acro\", \"Angle\"]",
                        ));
                        return None;
                    }
                    Some(modes)
                }
            };
            Some(FlightModeSwitch::Steps {
                source: press?,
                modes: modes?,
            })
        })();
        if read.is_none() {
            ok = false;
        }
        read
    });
    ok.then_some(Switches {
        arm,
        flight_mode,
        crash_flip,
    })
}

/// A Virtual Switch's button or key.
fn press_source(
    source: &str,
    item: &Item,
    facts: Option<&DeviceFacts>,
    problems: &mut Problems,
) -> Option<Press> {
    match (source, facts) {
        ("button", Some(facts)) => pad_button(item, facts, problems).map(Press::Button),
        _ => key(item, problems).map(Press::Key),
    }
}

fn read_actions(
    root: &Table,
    kind: Kind,
    facts: Option<&DeviceFacts>,
    sticks: &[u8],
    switch_channels: &[u8],
    problems: &mut Problems,
) -> Option<BTreeMap<Action, ActionSource>> {
    let mut actions = BTreeMap::new();
    let Some(table) = optional_section(root, "actions", problems) else {
        return Some(actions);
    };
    let mut ok = true;
    for (name, item) in table.entries() {
        let Some(action) = Action::from_name(&name) else {
            problems.push(item.problem(format!(
                "`{name}` isn't an Action OpenDrone knows; it knows {}",
                Action::ALL
                    .iter()
                    .map(|a| format!("`{}`", a.name()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
            ok = false;
            continue;
        };
        let read = (|| {
            let entry = item.table(problems)?;
            let what = format!("the Action `{name}`");
            let (source, source_item) = one_source(&entry, &what, sources(kind, false), problems)?;
            if source == "channel" {
                entry.refuse_unknown(&["channel", "on"], problems);
                let channel = switch_channel(&source_item, sticks, problems)?;
                if switch_channels.contains(&channel) {
                    problems.push(source_item.problem(format!(
                        "CH{channel} already drives a switch, so an Action can't use it"
                    )));
                    return None;
                }
                let on = position(&entry, "on", channel, problems)?;
                return Some(ActionSource::Channel { channel, on });
            }
            entry.refuse_unknown(&["button"], problems);
            let facts = facts?;
            pad_button(&source_item, facts, problems).map(ActionSource::Button)
        })();
        match read {
            Some(source) => {
                actions.insert(action, source);
            }
            None => ok = false,
        }
    }
    ok.then_some(actions)
}

/// A percentage, such as "-100 %", within −100 % and +100 % (`signed`) or
/// 0 % and 100 %.
fn percent(item: &Item, signed: bool, problems: &mut Problems) -> Option<f64> {
    let text = item.text(problems)?;
    let share = match units::parse_quantity(text).and_then(|q| q.as_a(Dimension::PERCENT)) {
        Ok(share) => share,
        Err(p) => {
            problems.push(item.problem(p.0));
            return None;
        }
    };
    let low = if signed { -1.0 } else { 0.0 };
    if !(low..=1.0).contains(&share) {
        problems.push(item.problem(format!(
            "`{}` must be from {} to {}",
            item.key(),
            if signed { "-100 %" } else { "0 %" },
            if signed { "+100 %" } else { "100 %" }
        )));
        return None;
    }
    Some(share * 100.0)
}

/// A centring stick's calibration: `{ min, centre, max, deadband }`. On a
/// Gamepad `min` and `max` may be left out: −100 % and +100 %.
fn stick_calibration(
    table: &Table,
    name: &str,
    ends_needed: bool,
    problems: &mut Problems,
) -> Option<StickCalibration> {
    let entry = table.require(name, problems)?.table(problems)?;
    entry.refuse_unknown(&["min", "centre", "max", "deadband"], problems);
    let value = |key: &str, default: Option<f64>, problems: &mut Problems| match (
        entry.get(key),
        default,
    ) {
        (Some(item), _) => percent(&item, key != "deadband", problems),
        (None, Some(default)) => Some(default),
        (None, None) => {
            entry.require(key, problems);
            None
        }
    };
    let end_default = |end: f64| if ends_needed { None } else { Some(end) };
    let min = value("min", end_default(-100.0), problems);
    let centre = value("centre", None, problems);
    let max = value("max", end_default(100.0), problems);
    let deadband = value("deadband", None, problems);
    let (min, centre, max, deadband) = (min?, centre?, max?, deadband?);
    if !(min < centre && centre < max) {
        problems.push(entry.problem(format!("{name}'s centre must lie between its min and max")));
        return None;
    }
    if deadband >= centre - min || deadband >= max - centre {
        problems.push(entry.problem(format!(
            "{name}'s deadband is wider than the stick's travel on one side of its centre"
        )));
        return None;
    }
    Some(StickCalibration::from_percent(min, centre, max, deadband))
}

/// A throttle stick's or a trigger's ends: `{ min, max }`.
fn ends(table: &Table, name: &str, trigger: bool, problems: &mut Problems) -> Option<Ends> {
    let entry = table.require(name, problems)?.table(problems)?;
    entry.refuse_unknown(&["min", "max"], problems);
    let min = entry
        .require("min", problems)
        .and_then(|item| percent(&item, !trigger, problems));
    let max = entry
        .require("max", problems)
        .and_then(|item| percent(&item, !trigger, problems));
    let (min, max) = (min?, max?);
    if min >= max {
        problems.push(entry.problem(format!("{name}'s min must be below its max")));
        return None;
    }
    let at = |percent: f64| {
        if trigger {
            percent / 100.0 * AXIS_MAX
        } else {
            value_at_percent(percent)
        }
    };
    Some(Ends {
        min: at(min),
        max: at(max),
    })
}
