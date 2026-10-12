//! The tracer's fixed settings. The screens ticket brings the settings file
//! and the menus to change them; until then every flight uses these.

use opendrone_input::profile::FlightMode;
use opendrone_sim::{PacketRate, PhysicsRate, Rates};

/// The Quads the tracer flies, by the id after `opendrone/`.
pub const QUADS: [&str; 2] = ["whoop-65", "freestyle-5"];

/// What a flight starts from: the pilot's settings that belong to the
/// Simulation (ADR-0003), and which Quad on which Map.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The Quad's id, such as `opendrone/whoop-65`.
    pub quad: String,
    /// The Map's id. The tracer flies the flat floor, a Test Map.
    pub map: String,
    /// 8 kHz: the alpha's physics rate.
    pub physics_rate: PhysicsRate,
    /// 250 Hz, the default Packet Rate.
    pub packet_rate: PacketRate,
    /// Betaflight 2026.6's default Rates.
    pub rates: Rates,
    /// The Flight Mode for an Input Device with no Flight Mode switch, such
    /// as the DualSense: Acro, as a real quad flies.
    pub flight_mode: FlightMode,
    /// The Assists, all off.
    pub input_smoothing: bool,
    pub endless_battery: bool,
    pub auto_arm: bool,
    /// The Simulation's random seed (its Prop Wash flicker). Fixed, so two
    /// flights flown the same way fly the same.
    pub random_seed: u64,
}

impl Settings {
    /// The fixed defaults, flying the Quad `quad` (an id after `opendrone/`).
    pub fn tracer(quad: &str) -> Settings {
        Settings {
            quad: format!("opendrone/{quad}"),
            map: "test/flat-floor".to_string(),
            physics_rate: PhysicsRate::from_hz(8000).expect("8 kHz is a rate"),
            packet_rate: PacketRate::from_hz(250).expect("250 Hz is a Packet Rate"),
            rates: Rates::BETAFLIGHT_DEFAULT,
            flight_mode: FlightMode::Acro,
            input_smoothing: false,
            endless_battery: false,
            auto_arm: false,
            random_seed: 1,
        }
    }
}
