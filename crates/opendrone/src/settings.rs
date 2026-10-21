//! What a flight starts from: the pilot's settings that belong to the
//! Simulation (ADR-0003), and which Quad on which Map.
//!
//! The pilot's own settings live in the settings file
//! ([`crate::settings_file`]); [`Settings::from_pilot`] turns them into what
//! the Simulation is set up with. The physics rate and the random seed aren't
//! the pilot's to change.

use opendrone_input::profile::FlightMode;
use opendrone_sim::{PacketRate, PhysicsRate, Rates};

use crate::settings_file::PilotSettings;

/// What a flight starts from: the pilot's settings that belong to the
/// Simulation (ADR-0003), and which Quad on which Map.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The Quad's id, such as `opendrone/whoop-65`.
    pub quad: String,
    /// The Map's id, such as `opendrone/skate-park`.
    pub map: String,
    /// 8 kHz: the alpha's physics rate.
    pub physics_rate: PhysicsRate,
    /// The Packet Rate, 250 Hz unless the pilot picked another.
    pub packet_rate: PacketRate,
    /// Betaflight 2026.6's default Rates, until the Settings drawer (#69)
    /// lets a pilot paste their own.
    pub rates: Rates,
    /// The Flight Mode for an Input Device with no Flight Mode switch, such
    /// as the DualSense: Acro, as a real quad flies, unless the pilot picked
    /// another.
    pub flight_mode: FlightMode,
    /// The Assists, all off unless the pilot turned them on.
    pub input_smoothing: bool,
    pub endless_battery: bool,
    pub auto_arm: bool,
    /// The Simulation's random seed (its Prop Wash flicker). Fixed, so two
    /// flights flown the same way fly the same.
    pub random_seed: u64,
}

impl Settings {
    /// The same settings on another Map, by id.
    pub fn on_map(mut self, map: &str) -> Settings {
        self.map = map.to_string();
        self
    }

    /// A new install's settings, flying the Quad `quad` (an id after
    /// `opendrone/`) on the Test Map `flat-floor`: what the readable checks
    /// and the developer commands fly with, so they never read the pilot's
    /// settings file.
    pub fn defaults(quad: &str) -> Settings {
        Settings {
            quad: format!("opendrone/{quad}"),
            map: "test/flat-floor".to_string(),
            ..Settings::from_pilot(&PilotSettings::default())
        }
    }

    /// The settings a flight starts from, from the pilot's settings file.
    pub fn from_pilot(pilot: &PilotSettings) -> Settings {
        Settings {
            quad: pilot.quad.clone(),
            map: pilot.map.clone(),
            physics_rate: PhysicsRate::from_hz(8000).expect("8 kHz is a rate"),
            packet_rate: pilot.packet_rate,
            rates: Rates::BETAFLIGHT_DEFAULT,
            flight_mode: pilot.flight_mode,
            input_smoothing: pilot.input_smoothing,
            endless_battery: pilot.endless_battery,
            auto_arm: pilot.auto_arm,
            random_seed: 1,
        }
    }
}
