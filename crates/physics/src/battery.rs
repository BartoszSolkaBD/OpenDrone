//! The battery: its voltage curve, sag, recovery and charge counting (E13,
//! E14 in #10; flight-dynamics research §5.3).
//!
//! - **Resting voltage.** Each cell's open-circuit voltage comes from the
//!   Quad definition's voltage curve at the charge left, straight lines
//!   between its points. Past empty the last stretch carries on down, so a
//!   flat pack fades: there is no cutoff, and no special case for 1S.
//! - **Sag.** Under load the voltage drops at once by the current times the
//!   pack's resistance and its connector's. A slower part builds up and dies
//!   away with the pack's recovery time: one resistor–capacitor pair, as
//!   Bauersfeld & Scaramuzza fitted to ten 4S–6S packs (flight-dynamics research §5.3, its source S7).
//!   Their pair's voltage settles at `k` times the power each cell gives per
//!   amp-hour of its capacity, with `k` = 0.00104846 V per W/Ah (their
//!   Table I); the Quad definition holds only the time it takes, `recovery`.
//! - **The voltage under load** follows from the power the ESCs draw: with
//!   `E` the resting voltage less the slow part and `R` both resistances,
//!   `V = (E + √(E² − 4·R·P)) / 2`, so the current is `P / V` (their eq. for
//!   a load of fixed power). Braking gives some power back, and then the
//!   current is negative.
//! - **Charge** is counted as current over time.

use opendrone_maths::{Fingerprinter, functions};

/// Bauersfeld & Scaramuzza's `k`: the slow part's settled voltage per cell,
/// in volts per watt per amp-hour of capacity.
const SLOW_SAG_PER_POWER: f64 = 0.001_048_46;

/// One coulomb in amp-hours.
const AMP_HOURS_PER_COULOMB: f64 = 1.0 / 3600.0;

/// The battery's numbers from the Quad definition.
#[derive(Clone, Debug, PartialEq)]
pub struct BatteryParameters {
    pub cells: u32,
    /// In coulombs.
    pub capacity: f64,
    /// The open-circuit voltage of one cell: points from full to empty, as
    /// (charge left from 0 to 1, volts).
    pub voltage_curve: Vec<(f64, f64)>,
    /// The pack's own resistance, in ohms.
    pub resistance: f64,
    /// The connector's and leads', in ohms.
    pub connector: f64,
    /// The slow part's time constant, in seconds.
    pub recovery: f64,
}

impl BatteryParameters {
    /// One cell's resting voltage with `charge` left (0 to 1; below 0 past
    /// empty).
    pub fn resting_cell_voltage(&self, charge: f64) -> f64 {
        let curve = &self.voltage_curve;
        let Some(&(top, full)) = curve.first() else {
            return 0.0;
        };
        if charge >= top || curve.len() == 1 {
            return full;
        }
        for pair in curve.windows(2) {
            let ((high, upper), (low, lower)) = (pair[0], pair[1]);
            if charge >= low {
                return lower + (upper - lower) * (charge - low) / (high - low);
            }
        }
        // Past the curve's last point: the last stretch carries on down.
        let ((high, upper), (low, lower)) = (curve[curve.len() - 2], curve[curve.len() - 1]);
        let volts = lower + (upper - lower) * (charge - low) / (high - low);
        functions::max(volts, 0.0)
    }
}

/// The battery's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Battery {
    /// Charge drawn since the start, in coulombs: negative charge left counts
    /// past empty.
    charge_used: f64,
    /// Charge left at the start, as a share of the capacity.
    starting_charge: f64,
    /// The slow part of the sag, per cell, in volts.
    slow_sag: f64,
    /// The voltage at its terminals after the last step.
    voltage: f64,
    /// The current after the last step, in amps.
    current: f64,
}

/// What the battery reports after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatteryOutput {
    /// At its terminals, past the connector, in volts.
    pub voltage: f64,
    /// Drawn from it, in amps (negative while braking gives some back).
    pub current: f64,
    /// Drawn since the start, in coulombs.
    pub charge_used: f64,
    /// Left, as a share of the capacity: below 0 past empty.
    pub charge: f64,
    /// How far the voltage sits below the pack's resting voltage at this
    /// charge, in volts.
    pub sag: f64,
}

impl Battery {
    /// At rest, with `charge` left (a share of the capacity).
    pub(crate) fn new(parameters: &BatteryParameters, charge: f64) -> Battery {
        Battery {
            charge_used: 0.0,
            starting_charge: charge,
            slow_sag: 0.0,
            voltage: f64::from(parameters.cells) * parameters.resting_cell_voltage(charge),
            current: 0.0,
        }
    }

    /// The voltage at its terminals after the last step.
    pub(crate) fn voltage(&self) -> f64 {
        self.voltage
    }

    fn charge(&self, parameters: &BatteryParameters) -> f64 {
        self.starting_charge - self.charge_used / parameters.capacity
    }

    /// Gives `power` watts (negative: takes them back) for `dt` seconds.
    pub(crate) fn step(&mut self, parameters: &BatteryParameters, power: f64, dt: f64) {
        let cells = f64::from(parameters.cells);
        let resting = cells * parameters.resting_cell_voltage(self.charge(parameters));
        let open = resting - cells * self.slow_sag;
        let resistance = parameters.resistance + parameters.connector;
        let voltage = if resistance > 0.0 {
            // V² − E·V + R·P = 0. When the load asks for more than the pack
            // can give, it gives what it can: half its open voltage.
            let room = open * open - 4.0 * resistance * power;
            (open + functions::max(room, 0.0).sqrt()) / 2.0
        } else {
            open
        };
        let current = if voltage > 0.0 { power / voltage } else { 0.0 };
        self.charge_used += current * dt;

        let capacity_ah = parameters.capacity * AMP_HOURS_PER_COULOMB;
        let cell_power_per_ah = voltage * current / (cells * capacity_ah);
        let settled = SLOW_SAG_PER_POWER * cell_power_per_ah;
        self.slow_sag += (settled - self.slow_sag) * -functions::exp_m1(-dt / parameters.recovery);
        self.voltage = voltage;
        self.current = current;
    }

    pub(crate) fn output(&self, parameters: &BatteryParameters) -> BatteryOutput {
        let charge = self.charge(parameters);
        let resting = f64::from(parameters.cells) * parameters.resting_cell_voltage(charge);
        BatteryOutput {
            voltage: self.voltage,
            current: self.current,
            charge_used: self.charge_used,
            charge,
            sag: resting - self.voltage,
        }
    }

    /// Feeds everything it remembers into a fingerprint.
    pub(crate) fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64s(&[
            self.charge_used,
            self.starting_charge,
            self.slow_sag,
            self.voltage,
            self.current,
        ]);
    }
}
