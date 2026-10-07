//! What a Quad definition (`quad.toml`) holds: every section and key, and
//! what kind of value each takes (#16 §4, with #26 §5, #28 §8 and #32 §6).
//!
//! The reader refuses any key not listed here, and every number is read
//! through the shared unit list in the form and bounds written here.

use crate::units::Dimension;

/// What kind of value a key takes.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A physics number: `{ value, confidence, source }`, and an Estimate adds
    /// the `range` it may move within. The Simulation receives it, so the
    /// fingerprint covers it.
    Physics(Form),
    /// One of the FPV Camera's limits (#28 §8): written like a physics number,
    /// but the camera sits outside the Simulation, so the fingerprint doesn't
    /// cover it.
    CameraLimit(Form),
    /// A number with its unit and no Confidence: a camera default or a value
    /// in the sound block.
    Plain(Form),
    /// A count, written as a bare whole number, such as `blades = 3`.
    Count { least: i64 },
    /// One of a few words, such as `direction = "props-in"`.
    Choice(&'static [&'static str]),
    /// `true` or `false`.
    YesNo,
}

impl Kind {
    /// Whether the value carries a Confidence and a source.
    pub fn has_confidence(self) -> bool {
        matches!(self, Kind::Physics(_) | Kind::CameraLimit(_))
    }

    pub fn form(self) -> Option<Form> {
        match self {
            Kind::Physics(form) | Kind::CameraLimit(form) | Kind::Plain(form) => Some(form),
            _ => None,
        }
    }
}

/// How a number is written.
#[derive(Clone, Copy, Debug)]
pub enum Form {
    /// One number, such as `"23.0 g"`.
    One(Dimension, Bounds),
    /// Labelled parts, such as `"roll 70, pitch 90, yaw 140 g·cm²"`.
    Parts(&'static [&'static str], Dimension, Bounds),
    /// A box, `"box 64 × 10 × 6 mm"`: front to back, side to side, top to
    /// bottom.
    Box,
    /// A number and the condition it holds at, such as `"1.2 A at 10 V"`.
    At(Dimension, Dimension, Bounds),
    /// A curve of points, such as `"4.35 V at 100%, 3.30 V at 0%"`.
    Curve(Dimension, Dimension),
}

impl Form {
    /// The kind of number an absolute range is written in.
    pub fn ranged_dimension(self) -> Dimension {
        match self {
            Form::One(d, _) | Form::Parts(_, d, _) | Form::At(d, _, _) | Form::Curve(d, _) => d,
            Form::Box => Dimension::LENGTH,
        }
    }
}

/// Where a number must lie, in SI units.
#[derive(Clone, Copy, Debug)]
pub enum Bounds {
    Any,
    AboveZero,
    ZeroOrMore,
    /// From 0% to 100%.
    Share,
    /// Between two values, inclusive, in SI units, with how to write them.
    Between(f64, f64, &'static str),
}

impl Bounds {
    /// `None` when `value` is inside; otherwise what it must be.
    pub fn refuses(self, value: f64) -> Option<String> {
        match self {
            Bounds::Any => None,
            Bounds::AboveZero if value <= 0.0 => Some("must be above zero".into()),
            Bounds::ZeroOrMore if value < 0.0 => Some("can't be below zero".into()),
            Bounds::Share if !(0.0..=1.0).contains(&value) => {
                Some("must be from 0% to 100%".into())
            }
            Bounds::Between(low, high, text) if !(low..=high).contains(&value) => {
                Some(format!("must be {text}"))
            }
            _ => None,
        }
    }
}

/// Whether a key must be there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Always,
    /// Only on a Quad with ducts: the reader checks it goes with `[ducts]`.
    WithDucts,
    /// Only on a Quad with a buzzer: the reader checks it goes with
    /// `[sound] buzzer = true`.
    WithBuzzer,
}

/// One key in a section.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub name: &'static str,
    pub kind: Kind,
    pub need: Need,
}

/// One section, such as `[props]`.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub name: &'static str,
    pub keys: &'static [Key],
    /// Only `[ducts]` may be left out: a Quad without ducts has none.
    pub optional: bool,
    /// Whether the Simulation receives this section's values, so the
    /// fingerprint covers them. The camera and the sound sit outside it.
    pub simulated: bool,
}

impl Section {
    pub fn key(&self, name: &str) -> Option<&'static Key> {
        self.keys.iter().find(|key| key.name == name)
    }
}

/// The text keys at the top of the file: on-screen text and the picture.
pub const TOP_TEXT: &[&str] = &["name", "description", "spec_line", "picture"];

const fn key(name: &'static str, kind: Kind) -> Key {
    Key {
        name,
        kind,
        need: Need::Always,
    }
}

const fn only(need: Need, name: &'static str, kind: Kind) -> Key {
    Key { name, kind, need }
}

use Bounds::{AboveZero, Any, Share, ZeroOrMore};
use Kind::{CameraLimit, Count, Physics, Plain, YesNo};

const fn one(dimension: Dimension, bounds: Bounds) -> Form {
    Form::One(dimension, bounds)
}

/// Degrees in radians.
const DEGREE: f64 = core::f64::consts::PI / 180.0;

/// Every section of a Quad definition, in the order a file writes them.
pub const SECTIONS: &[Section] = &[
    Section {
        name: "camera",
        optional: false,
        simulated: false,
        keys: &[
            key("lens", Kind::Choice(&["fisheye"])),
            key(
                "position",
                Plain(Form::Parts(
                    &["forward", "left", "up"],
                    Dimension::LENGTH,
                    Any,
                )),
            ),
            key(
                "camera_tilt",
                Plain(one(
                    Dimension::ANGLE,
                    Bounds::Between(0.0, 80.0 * DEGREE, "from 0° to 80°"),
                )),
            ),
            key(
                "fov",
                Plain(one(
                    Dimension::ANGLE,
                    Bounds::Between(90.0 * DEGREE, 170.0 * DEGREE, "from 90° to 170°"),
                )),
            ),
            key("vtx_power", Plain(one(Dimension::POWER, AboveZero))),
            key(
                "analog_dynamic_range",
                CameraLimit(one(Dimension::STOPS, AboveZero)),
            ),
            key(
                "analog_lines",
                CameraLimit(one(Dimension::LINES, AboveZero)),
            ),
            key(
                "analog_sharpness",
                CameraLimit(one(Dimension::TVL, AboveZero)),
            ),
        ],
    },
    Section {
        name: "frame",
        optional: false,
        simulated: true,
        keys: &[
            key("dry_mass", Physics(one(Dimension::MASS, AboveZero))),
            key("diagonal", Physics(one(Dimension::LENGTH, AboveZero))),
            key(
                "inertia",
                Physics(Form::Parts(
                    &["roll", "pitch", "yaw"],
                    Dimension::INERTIA,
                    AboveZero,
                )),
            ),
            key("rotor_height", Physics(one(Dimension::LENGTH, Any))),
            key(
                "drag_area",
                Physics(Form::Parts(
                    &["front", "side", "top"],
                    Dimension::AREA,
                    ZeroOrMore,
                )),
            ),
        ],
    },
    Section {
        name: "collision",
        optional: false,
        simulated: true,
        keys: &[
            key("body", Physics(Form::Box)),
            key("pack", Physics(Form::Box)),
            key("pack_height", Physics(one(Dimension::LENGTH, Any))),
            only(
                Need::WithDucts,
                "duct_rings",
                Physics(Form::Parts(
                    &["inside", "wall", "tall"],
                    Dimension::LENGTH,
                    AboveZero,
                )),
            ),
            key(
                "bounce",
                Physics(one(
                    Dimension::NONE,
                    Bounds::Between(0.0, 1.0, "from 0 to 1"),
                )),
            ),
            key("friction", Physics(one(Dimension::NONE, ZeroOrMore))),
        ],
    },
    Section {
        name: "props",
        optional: false,
        simulated: true,
        keys: &[
            key("diameter", Physics(one(Dimension::LENGTH, AboveZero))),
            key("blades", Count { least: 2 }),
            key("direction", Kind::Choice(&["props-in", "props-out"])),
            key(
                "thrust_coefficient",
                Physics(one(Dimension::NONE, AboveZero)),
            ),
            key(
                "power_coefficient",
                Physics(one(Dimension::NONE, AboveZero)),
            ),
            key(
                "rotor_drag",
                Physics(one(Dimension::PER_SECOND, ZeroOrMore)),
            ),
            key("rotor_inertia", Physics(one(Dimension::INERTIA, AboveZero))),
            key("reverse_thrust", Physics(one(Dimension::PERCENT, Share))),
            key("reverse_torque", Physics(one(Dimension::PERCENT, Share))),
            key("grip", Physics(one(Dimension::NONE, ZeroOrMore))),
        ],
    },
    Section {
        name: "motors",
        optional: false,
        simulated: true,
        keys: &[
            key("kv", Physics(one(Dimension::KV, AboveZero))),
            key("poles", Count { least: 4 }),
            key(
                "winding_resistance",
                Physics(one(Dimension::RESISTANCE, AboveZero)),
            ),
            key(
                "no_load_current",
                Physics(Form::At(Dimension::CURRENT, Dimension::VOLTAGE, ZeroOrMore)),
            ),
            key("spin_up", Physics(one(Dimension::TIME, AboveZero))),
            key("slow_down", Physics(one(Dimension::TIME, AboveZero))),
            key("start_wait", Physics(one(Dimension::TIME, ZeroOrMore))),
            key("restart_tries", Count { least: 0 }),
            key(
                "startup_power_limit",
                Physics(one(Dimension::PERCENT, Share)),
            ),
        ],
    },
    Section {
        name: "battery",
        optional: false,
        simulated: true,
        keys: &[
            key("cells", Count { least: 1 }),
            key("chemistry", Kind::Choice(&["LiPo", "LiHV", "Li-ion"])),
            key("full", Physics(one(Dimension::VOLTAGE, PER_CELL))),
            key("empty", Physics(one(Dimension::VOLTAGE, PER_CELL))),
            key("capacity", Physics(one(Dimension::CHARGE, AboveZero))),
            key("mass", Physics(one(Dimension::MASS, AboveZero))),
            key(
                "voltage_curve",
                Physics(Form::Curve(Dimension::VOLTAGE, Dimension::PERCENT)),
            ),
            key(
                "resistance",
                Physics(one(Dimension::RESISTANCE, ZeroOrMore)),
            ),
            key("recovery", Physics(one(Dimension::TIME, AboveZero))),
            key("connector", Physics(one(Dimension::RESISTANCE, ZeroOrMore))),
        ],
    },
    Section {
        name: "ducts",
        optional: true,
        simulated: true,
        keys: &[
            key("ram_drag", Physics(one(Dimension::PER_SECOND, ZeroOrMore))),
            key(
                "nose_up_offset",
                Physics(one(Dimension::LENGTH, ZeroOrMore)),
            ),
        ],
    },
    Section {
        name: "feel",
        optional: false,
        simulated: true,
        keys: &[
            key(
                "prop_wash_strength",
                Physics(one(Dimension::PERCENT, Share)),
            ),
            key(
                "prop_wash_flicker",
                Physics(one(Dimension::PER_SECOND, ZeroOrMore)),
            ),
            key(
                "ground_effect_body",
                Physics(one(Dimension::NONE, ZeroOrMore)),
            ),
        ],
    },
    Section {
        name: "board",
        optional: false,
        simulated: true,
        keys: &[key(
            "gyro_range",
            Physics(one(Dimension::ROTATION_SPEED, AboveZero)),
        )],
    },
    Section {
        name: "sound",
        // Only the ESC start-up melody reaches the Simulation (it sets when
        // the motors answer), so the fingerprint covers just that key.
        simulated: false,
        optional: false,
        keys: &[
            key("buzzer", YesNo),
            key("esc_melody", Kind::Choice(&["Bluejay default"])),
        ],
    },
    Section {
        name: "sound_block",
        optional: false,
        simulated: false,
        keys: &[
            key("blade_pass_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("harmonics", Count { least: 1 }),
            key("harmonic_rolloff", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("blade_mismatch", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("shaft_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("loudness_exponent", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("whoosh_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("whoosh_centre", Plain(one(Dimension::NONE, AboveZero))),
            key("whoosh_q", Plain(one(Dimension::NONE, AboveZero))),
            key("whoosh_swish", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("motor_whine", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("load_brightness", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("load_loudness", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("speed_wobble", Plain(one(Dimension::PERCENT, Share))),
            key("tone_roughness", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("hiss_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("hiss_corner", Plain(one(Dimension::PER_SECOND, AboveZero))),
            key("body_peak", Plain(one(Dimension::PER_SECOND, AboveZero))),
            key("body_peak_gain", Plain(one(Dimension::DECIBELS, Any))),
            key("body_peak_q", Plain(one(Dimension::NONE, AboveZero))),
            key("hum_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key(
                "hum_first_mode",
                Plain(one(Dimension::PER_SECOND, AboveZero)),
            ),
            key("hum_first_mode_q", Plain(one(Dimension::NONE, AboveZero))),
            key(
                "hum_second_mode",
                Plain(one(Dimension::PER_SECOND, AboveZero)),
            ),
            key("hum_second_mode_q", Plain(one(Dimension::NONE, AboveZero))),
            key("hum_follow", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("esc_beep_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key(
                "esc_beep_brightness",
                Plain(one(Dimension::NONE, ZeroOrMore)),
            ),
            key("esc_ring", Plain(one(Dimension::PER_SECOND, AboveZero))),
            only(
                Need::WithBuzzer,
                "buzzer_pitch",
                Plain(one(Dimension::PER_SECOND, AboveZero)),
            ),
            only(
                Need::WithBuzzer,
                "buzzer_level",
                Plain(one(Dimension::NONE, ZeroOrMore)),
            ),
            key("tick_level", Plain(one(Dimension::NONE, ZeroOrMore))),
            key("tick_ring", Plain(one(Dimension::PER_SECOND, AboveZero))),
            key("tick_decay", Plain(one(Dimension::TIME, AboveZero))),
            key(
                "where_you_stand_level",
                Plain(one(Dimension::DECIBELS, Any)),
            ),
            key("hit_level", Plain(one(Dimension::NONE, ZeroOrMore))),
        ],
    },
];

/// A battery voltage is written per cell, so it lies between these.
const PER_CELL: Bounds = Bounds::Between(2.5, 4.5, "a voltage per cell, from 2.5 V to 4.5 V");

/// The section with this name.
pub fn section(name: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|section| section.name == name)
}

/// The key `section.key`, such as `props.grip`.
pub fn find(name: &str) -> Option<(&'static Section, &'static Key)> {
    let (section_name, key_name) = name.split_once('.')?;
    let section = section(section_name)?;
    Some((section, section.key(key_name)?))
}
