//! The pilot's own copy of an Input Device profile (#19 §1).

use std::collections::BTreeMap;

use crate::profile::{Connection, InputDeviceProfile, Setup};

/// The pilot's own copy of a profile, made by setup and Calibration and kept
/// with their settings, per device model. It rewrites only the setup (the
/// kind, channels, switches, Actions and Calibration, with the throttle style
/// and stick mode in the channels) and adds a measured Report Rate where the
/// Pack has none. The device's facts always come from the current Pack. No
/// Pack can change the copy, and it is never written back into a Pack.
/// Saving it with the settings is the game's job.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotCopy {
    pub setup: Setup,
    /// Whether the Calibration in `setup` was measured, for the "isn't
    /// calibrated" Pre-flight Warning.
    pub calibrated: bool,
    /// Report Rates setup measured, per connection, in reports a second.
    pub measured_report_rate: BTreeMap<Connection, f64>,
}

impl PilotCopy {
    /// A copy that starts as the Pack profile's setup, not yet calibrated.
    pub fn of(profile: &InputDeviceProfile) -> PilotCopy {
        PilotCopy {
            setup: profile.setup.clone(),
            calibrated: false,
            measured_report_rate: BTreeMap::new(),
        }
    }

    /// The Pack's profile with this copy laid over it: the copy's setup, the
    /// Pack's facts, and a measured Report Rate only where the Pack has
    /// none. "Reset to the Pack's starting profile" simply drops the copy.
    pub fn laid_over(&self, pack: &InputDeviceProfile) -> InputDeviceProfile {
        let mut facts = pack.facts.clone();
        for (&connection, &rate) in &self.measured_report_rate {
            facts.report_rate.entry(connection).or_insert(rate);
        }
        InputDeviceProfile {
            id: pack.id.clone(),
            facts,
            setup: self.setup.clone(),
        }
    }
}
