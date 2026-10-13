//! What the Flight Controller makes of its battery: the voltage through the
//! board's ADC and its low-pass, the cell count, the battery's state (OK,
//! low or critical) and, where the Tune's `current_meter` reads one, the
//! current and the mAh drawn. Reimplemented from the behaviour of Betaflight
//! 2026.6.2's `sensors/battery.c`, `sensors/voltage.c` and
//! `sensors/current.c`, for the OSD ([ADR-0022]) and the beeper.
//!
//! - **The voltage** is read as Betaflight's ADC reads it on a board at
//!   Betaflight's default scale (`vbat_scale` 110, a 10:1 divider, a 3.3 V
//!   12-bit ADC): a whole-number sample, low-passed with
//!   `vbat_display_lpf_period`, then turned into hundredths of a volt with
//!   Betaflight's own rounding. So a 4.35 V pack reads 4.34 V, as on a real
//!   board. It runs 50 times a second.
//! - **Power-up** is settled, as the rest of a fresh Flight Controller is:
//!   on its first loop the low-pass starts at the first reading, and the
//!   battery is found at once (Betaflight waits for the voltage to hold
//!   steady for about a second). A reading that isn't a battery's (below
//!   `vbat_detect_cell_voltage`, or between one cell's full voltage and two
//!   cells' detect voltage) is no battery: NOT PRESENT, with no cell count.
//!   Otherwise the cell count is `force_battery_cell_count`, or the
//!   voltage ÷ `vbat_max_cell_voltage`, rounded down, plus one, as
//!   Betaflight works it out.
//! - **The state** moves 5 times a second: low (`WARNING`) once the
//!   voltage is at or below the cell count × `vbat_warning_cell_voltage`
//!   less `vbat_hysteresis`, for `vbat_duration_for_warning`; critical
//!   once it is at or below the cell count × `vbat_min_cell_voltage` less
//!   the hysteresis, for `vbat_duration_for_critical`; back up a step once
//!   it is above that step's voltage. With `use_vbat_alerts` on, a low or
//!   critical battery sounds the beeper.
//! - **The current**, with `current_meter` ADC or ESC, is the battery current
//!   the physics gives, in hundredths of an amp: low-passed with
//!   `ibat_lpf_period` for the OSD, and added up, unfiltered, into the mAh
//!   drawn, 50 times a second. The ADC's own steps and range aren't
//!   modelled for the current: every board reads it exactly. Betaflight's
//!   `ibat_lpf_period` of 0 would make its filter divide by zero; here it
//!   means no filter.
//!
//! A pack never comes out in the sim, so the battery is found once, at
//! power-up; Reset powers the Flight Controller up afresh.
//!
//! [ADR-0022]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0022-osd-worked-out-beside-the-simulation.md

use opendrone_maths::{Fingerprinter, functions};

use crate::filter::pt1_gain;
use crate::tune::Tune;

/// What the board reads of its battery: the voltage at its pads and the
/// current drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BatteryReading {
    /// In volts. Zero means no battery.
    pub voltage: f64,
    /// In amps, positive when the battery gives current.
    pub current: f64,
}

/// Betaflight's battery state (`batteryState_e`), as the OSD and the beeper
/// read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryState {
    /// `OK`.
    Ok,
    /// `WARNING`: the OSD's `LOW BATTERY`.
    Warning,
    /// `CRITICAL`: the OSD's ` LAND NOW`.
    Critical,
    /// `NOT PRESENT`: no battery voltage.
    NotPresent,
}

impl BatteryState {
    /// Betaflight's name for it (`batteryStateStrings`).
    pub fn name(self) -> &'static str {
        match self {
            BatteryState::Ok => "OK",
            BatteryState::Warning => "WARNING",
            BatteryState::Critical => "CRITICAL",
            BatteryState::NotPresent => "NOT PRESENT",
        }
    }

    fn code(self) -> u64 {
        match self {
            BatteryState::Ok => 0,
            BatteryState::Warning => 1,
            BatteryState::Critical => 2,
            BatteryState::NotPresent => 3,
        }
    }
}

/// The battery as one loop left it, in Betaflight's own units, for the OSD
/// and Scenarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryReadings {
    /// The low-passed voltage, in hundredths of a volt
    /// (`getBatteryVoltage`).
    pub voltage: u16,
    /// The cell count found at power-up; 0 with no battery.
    pub cell_count: u8,
    pub state: BatteryState,
    /// The current, low-passed, in hundredths of an amp (`getAmperage`);
    /// `None` when the Tune's `current_meter` reads none.
    pub current: Option<i32>,
    /// The charge drawn since power-up, in whole mAh (`getMAhDrawn`); `None`
    /// when the Tune's `current_meter` reads none.
    pub mah_drawn: Option<i32>,
}

impl BatteryReadings {
    /// The average cell voltage, in hundredths of a volt
    /// (`getBatteryAverageCellVoltage`): 0 with no battery.
    pub fn average_cell_voltage(&self) -> u16 {
        match self.cell_count {
            0 => 0,
            cells => self.voltage / u16::from(cells),
        }
    }
}

/// The voltage task's rate (`SLOW_VOLTAGE_TASK_FREQ_HZ`), the current's,
/// and the battery alerts' (`TASK_BATTERY_ALERTS`), in µs between runs.
const VOLTAGE_EVERY_US: u64 = 20_000;
const CURRENT_EVERY_US: u64 = 20_000;
const ALERTS_EVERY_US: u64 = 200_000;
/// The most cells Betaflight works out (`MAX_AUTO_DETECT_CELL_COUNT`).
const MOST_CELLS: u16 = 8;

/// The board's ADC: its full scale (12 bits), its reference in mV, and
/// Betaflight's default `vbat_scale`, divider and multiplier.
const ADC_FULL: f64 = 4095.0;
const VREF_MV: u64 = 3300;
const VBAT_SCALE: u64 = 110;
const VBAT_DIVIDER: u64 = 10;
const VBAT_MULTIPLIER: u64 = 1;

/// The sample a 12-bit ADC gives for a battery voltage, through the board's
/// divider: whole steps, cut down, never below 0 or above full scale.
fn adc_sample(volts: f64) -> f64 {
    let pin = volts * (VBAT_DIVIDER as f64) / (VBAT_SCALE as f64) * 1000.0 / VREF_MV as f64;
    (pin * ADC_FULL).floor().clamp(0.0, ADC_FULL)
}

/// Betaflight's `voltageAdcToVoltage`: a sample as hundredths of a volt,
/// rounded as Betaflight rounds it.
fn sample_to_voltage(sample: u64) -> u16 {
    let full = ADC_FULL as u64;
    let hundredths =
        ((sample * VBAT_SCALE * VREF_MV / 10 + full * 5) / (full * VBAT_DIVIDER)) / VBAT_MULTIPLIER;
    hundredths.min(u64::from(u16::MAX)) as u16
}

/// What the battery work asks of the beeper on a loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatteryAlarm {
    Low,
    Critical,
}

/// Betaflight's battery meters and state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Battery {
    /// Whether the first reading has been taken.
    started: bool,
    /// The voltage sample's low-pass: its share and its state.
    voltage_k: f64,
    filtered_sample: f64,
    pub readings: BatteryReadings,
    warning_voltage: u16,
    critical_voltage: u16,
    warning_below: u16,
    critical_below: u16,
    /// When the voltage last stood above the next state's threshold, in ms
    /// (`lastVoltageChangeMs`).
    last_change_ms: u64,
    /// The current's low-pass: its share and its state, in hundredths of an
    /// amp.
    current_k: f64,
    filtered_current: f64,
    /// The mAh drawn, unrounded (`mAhDrawnF`).
    mah: f64,
    /// When the current was last read (`ibatLastServiced`), in µs.
    current_read_at: u64,
    next_voltage: u64,
    next_current: u64,
    next_alerts: u64,
}

impl Battery {
    pub fn new(tune: &Tune) -> Battery {
        let period = |tenths: u8| f64::from(tenths) / 10.0;
        let k = |tenths: u8, every_us: u64| {
            if tenths == 0 {
                1.0
            } else {
                pt1_gain(1.0 / period(tenths), every_us as f64 * 1e-6)
            }
        };
        let measures = tune.current_meter.measures();
        Battery {
            started: false,
            voltage_k: k(tune.vbat_display_lpf_period, VOLTAGE_EVERY_US),
            filtered_sample: 0.0,
            readings: BatteryReadings {
                voltage: 0,
                cell_count: 0,
                state: BatteryState::NotPresent,
                current: measures.then_some(0),
                mah_drawn: measures.then_some(0),
            },
            warning_voltage: 0,
            critical_voltage: 0,
            warning_below: 0,
            critical_below: 0,
            last_change_ms: 0,
            current_k: k(tune.ibat_lpf_period, CURRENT_EVERY_US),
            filtered_current: 0.0,
            mah: 0.0,
            current_read_at: 0,
            next_voltage: 0,
            next_current: 0,
            next_alerts: 0,
        }
    }

    /// One loop's battery work, as Betaflight's tasks would run it by
    /// `now_us`: the voltage and the current 50 times a second, the state 5
    /// times. Gives the alarm the beeper sounds, when the alerts ran and the
    /// battery is low or critical.
    pub fn step(
        &mut self,
        now_us: u64,
        reading: BatteryReading,
        tune: &Tune,
    ) -> Option<BatteryAlarm> {
        if !self.started {
            self.start(reading, tune);
        }
        if now_us >= self.next_voltage {
            self.read_voltage(reading);
            self.next_voltage += VOLTAGE_EVERY_US;
        }
        if now_us >= self.next_current {
            self.read_current(now_us, reading, tune);
            self.next_current += CURRENT_EVERY_US;
        }
        if now_us < self.next_alerts {
            return None;
        }
        self.next_alerts += ALERTS_EVERY_US;
        self.update_state(now_us / 1000, tune);
        if !tune.use_vbat_alerts {
            return None;
        }
        match self.readings.state {
            BatteryState::Warning => Some(BatteryAlarm::Low),
            BatteryState::Critical => Some(BatteryAlarm::Critical),
            BatteryState::Ok | BatteryState::NotPresent => None,
        }
    }

    /// Power-up, settled: the low-pass starts at the first reading, and the
    /// battery is found (`batteryUpdatePresence`).
    fn start(&mut self, reading: BatteryReading, tune: &Tune) {
        self.started = true;
        self.filtered_sample = adc_sample(reading.voltage);
        self.readings.voltage = sample_to_voltage(self.filtered_sample as u64);
        let voltage = self.readings.voltage;
        let detect = tune.vbat_detect_cell_voltage;
        let from_battery = (voltage >= detect && voltage <= tune.vbat_max_cell_voltage)
            || u32::from(voltage) > u32::from(detect) * 2;
        if !from_battery {
            return;
        }
        let cells = if tune.force_battery_cell_count != 0 {
            u16::from(tune.force_battery_cell_count)
        } else {
            (voltage / tune.vbat_max_cell_voltage.max(1) + 1).min(MOST_CELLS)
        };
        self.readings.cell_count = cells as u8;
        self.readings.state = BatteryState::Ok;
        self.warning_voltage = cells.saturating_mul(tune.vbat_warning_cell_voltage);
        self.critical_voltage = cells.saturating_mul(tune.vbat_min_cell_voltage);
        let hysteresis = u16::from(tune.vbat_hysteresis);
        self.warning_below = self.warning_voltage.saturating_sub(hysteresis);
        self.critical_below = self.critical_voltage.saturating_sub(hysteresis);
    }

    /// `voltageMeterADCRefresh`: the sample through its low-pass, cut to a
    /// whole number as Betaflight stores it, then turned into a voltage.
    fn read_voltage(&mut self, reading: BatteryReading) {
        let sample = adc_sample(reading.voltage);
        self.filtered_sample += self.voltage_k * (sample - self.filtered_sample);
        let whole = functions::max(self.filtered_sample.floor(), 0.0) as u64;
        self.readings.voltage = sample_to_voltage(whole);
    }

    /// `batteryUpdateCurrentMeter`: with no battery, or no current meter,
    /// nothing.
    fn read_current(&mut self, now_us: u64, reading: BatteryReading, tune: &Tune) {
        let since = now_us.saturating_sub(self.current_read_at);
        self.current_read_at = now_us;
        if !tune.current_meter.measures() || self.readings.cell_count == 0 {
            return;
        }
        // Hundredths of an amp, cut to a whole number as Betaflight's
        // integers hold them.
        let latest = (reading.current * 100.0).trunc();
        self.filtered_current += self.current_k * (latest - self.filtered_current);
        self.mah += latest * since as f64 / (100.0 * 1000.0 * 3600.0);
        self.readings.current = Some(self.filtered_current.trunc() as i32);
        self.readings.mah_drawn = Some(self.mah.trunc() as i32);
    }

    /// `batteryUpdateVoltageState`.
    fn update_state(&mut self, now_ms: u64, tune: &Tune) {
        let voltage = self.readings.voltage;
        let waited =
            |since: u64, tenths: u8| now_ms.saturating_sub(since) >= u64::from(tenths) * 100;
        match self.readings.state {
            BatteryState::Ok => {
                if voltage <= self.warning_below {
                    if waited(self.last_change_ms, tune.vbat_duration_for_warning) {
                        self.readings.state = BatteryState::Warning;
                    }
                } else {
                    self.last_change_ms = now_ms;
                }
            }
            BatteryState::Warning => {
                if voltage <= self.critical_below {
                    if waited(self.last_change_ms, tune.vbat_duration_for_critical) {
                        self.readings.state = BatteryState::Critical;
                    }
                } else {
                    if voltage > self.warning_voltage {
                        self.readings.state = BatteryState::Ok;
                    }
                    self.last_change_ms = now_ms;
                }
            }
            BatteryState::Critical => {
                if voltage > self.critical_voltage {
                    self.readings.state = BatteryState::Warning;
                    self.last_change_ms = now_ms;
                }
            }
            BatteryState::NotPresent => {}
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.started));
        f.write_f64(self.filtered_sample);
        f.write_u64(u64::from(self.readings.voltage));
        f.write_u64(u64::from(self.readings.cell_count));
        f.write_u64(self.readings.state.code());
        f.write_u64(u64::from(self.warning_voltage));
        f.write_u64(u64::from(self.critical_voltage));
        f.write_u64(self.last_change_ms);
        f.write_f64(self.filtered_current);
        f.write_f64(self.mah);
        for us in [
            self.current_read_at,
            self.next_voltage,
            self.next_current,
            self.next_alerts,
        ] {
            f.write_u64(us);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_high_voltage_cell_reads_4_34_volts_through_the_adc() {
        // 4.35 V on the pads is ADC sample 490 (4.35 × 4095 ÷ 36.3 =
        // 490.7, cut down), which Betaflight turns into 434.86, cut to 434.
        assert_eq!(adc_sample(4.35), 490.0);
        assert_eq!(sample_to_voltage(490), 434);
    }

    #[test]
    fn a_6s_pack_at_25_2_volts_reads_25_19_volts() {
        assert_eq!(sample_to_voltage(adc_sample(25.2) as u64), 2519);
    }
}
