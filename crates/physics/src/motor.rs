//! The motor model: each motor's speed from the battery's voltage (ADR-0006).
//!
//! # The props
//!
//! Thrust and the air's drag torque on a prop grow with the square of its
//! speed (flight-dynamics research §3.1): `T = k_f·ω²` and `Q = k_m·ω²`, with
//! `k_f = C_T·ρ·D⁴ / 4π²` and `k_m = C_P·ρ·D⁵ / 8π³` from the prop's unitless
//! thrust and power coefficients, its diameter `D` and the Map's air density
//! `ρ`. Spinning backwards (Crash Flip), a prop gives the Quad definition's
//! reverse shares of both.
//!
//! # The motor
//!
//! Drela's first-order model (research §3.2), with the winding's inductance
//! left out, so the current is worked out from the voltage directly:
//!
//! - the ESC puts its drive share `d` of the battery's voltage `V` across the
//!   motor (Bluejay drives "damped light", so this holds at any drive, braking
//!   included);
//! - the motor's back-voltage is `ω / KV` (`KV` in rad/s per volt);
//! - so the current through it is `i = (d·V − ω/KV) / R`, with `R` the
//!   winding resistance;
//! - its torque is `(i − i₀) / KV`, where `i₀` is the no-load current: the
//!   motor's own friction and iron losses. A Quad definition gives `i₀` at one
//!   voltage; here it grows in step with speed from nothing at a standstill,
//!   so it equals the given value at that voltage's no-load speed. That is
//!   the physics of it: the iron's eddy and hysteresis losses grow with
//!   speed, which is why makers quote `i₀` at a stated voltage and why
//!   Drela's QPROP lets it vary with speed; the 5″'s 1.2 A at 10 V becomes
//!   about 2 A at full speed on 6S. (A constant `i₀` would also hold a motor
//!   still under Bluejay's start-up power limit, which real motors overcome.)
//! - Its ESC draws `d·i` from the battery: the power `d·V·i`, at the battery's
//!   voltage.
//!
//! Where the motor's torque meets the prop's, the motor holds a steady speed,
//! the positive root of `k_m·ω² + (1/(R·KV²) + i₀/(KV·ω₀))·ω = d·V/(R·KV)`,
//! where `ω₀` is the no-load speed at the voltage `i₀` was measured at. That
//! is how speed, thrust and current follow the battery's voltage: at a fixed
//! command, thrust falls about with the voltage squared.
//!
//! # Spin-up and slow-down
//!
//! A motor never reaches its steady speed at once. It approaches it as a first
//! lag, with the Quad's spin-up time when speeding up and its slow-down time
//! when slowing (research §1.2, E5 and E9): the slow-down time is the ESC's
//! braking. The current is whatever turns the motor and its prop
//! (`rotor_inertia`) that way, and the ESC can never give more than its
//! command's drive when speeding up, nor less than none (full braking) when
//! slowing; if the lag asks for more, the motor follows what that drive
//! gives. With its ESC switched off a motor turns freely, slowed only by its
//! prop and its own losses.

use crate::commands::SpinDirection;
use crate::esc::Drive;
use opendrone_maths::functions;

/// A motor's numbers from the Quad definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorParameters {
    /// Speed per volt of back-voltage, in rad/s per volt.
    pub kv: f64,
    pub poles: u32,
    /// In ohms: the motor's and its ESC's, lumped together.
    pub winding_resistance: f64,
    /// In amps, measured at `no_load_voltage`.
    pub no_load_current: f64,
    /// In volts.
    pub no_load_voltage: f64,
    /// The spin-up time constant, in seconds.
    pub spin_up: f64,
    /// The slow-down time constant, in seconds: the ESC's braking.
    pub slow_down: f64,
}

/// A prop's numbers from the Quad definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropParameters {
    /// In metres.
    pub diameter: f64,
    /// C_T in T = C_T·ρ·n²·D⁴, with n in turns a second.
    pub thrust_coefficient: f64,
    /// C_P in P = C_P·ρ·n³·D⁵.
    pub power_coefficient: f64,
    /// The prop and the motor's bell, in kg·m².
    pub rotor_inertia: f64,
    /// Spinning backwards, the thrust as a share of forward's at the same
    /// speed.
    pub reverse_thrust: f64,
    /// Spinning backwards, the drag torque as a share of forward's.
    pub reverse_torque: f64,
}

/// One motor and prop's numbers, ready to use in the Map's air.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Model {
    kv: f64,
    resistance: f64,
    /// The no-load current per rad/s of speed.
    loss_per_speed: f64,
    inertia: f64,
    spin_up: f64,
    slow_down: f64,
    k_f: f64,
    k_m: f64,
    reverse_thrust: f64,
    reverse_torque: f64,
}

impl Model {
    pub(crate) fn new(motor: &MotorParameters, prop: &PropParameters, air_density: f64) -> Model {
        let pi = core::f64::consts::PI;
        let d2 = prop.diameter * prop.diameter;
        let no_load_speed =
            motor.kv * (motor.no_load_voltage - motor.no_load_current * motor.winding_resistance);
        Model {
            kv: motor.kv,
            resistance: motor.winding_resistance,
            loss_per_speed: if no_load_speed > 0.0 {
                motor.no_load_current / no_load_speed
            } else {
                0.0
            },
            inertia: prop.rotor_inertia,
            spin_up: motor.spin_up,
            slow_down: motor.slow_down,
            k_f: prop.thrust_coefficient * air_density * d2 * d2 / (4.0 * pi * pi),
            k_m: prop.power_coefficient * air_density * d2 * d2 * prop.diameter
                / (8.0 * pi * pi * pi),
            reverse_thrust: prop.reverse_thrust,
            reverse_torque: prop.reverse_torque,
        }
    }

    /// The thrust at `speed` (rad/s, positive the normal way), in newtons
    /// along the motor's axis: negative when spinning backwards.
    pub(crate) fn thrust(&self, speed: f64) -> f64 {
        let thrust = self.k_f * speed * speed;
        if speed >= 0.0 {
            thrust
        } else {
            -self.reverse_thrust * thrust
        }
    }

    /// The speed, spinning the normal way, whose thrust is `thrust` newtons
    /// (none for no thrust).
    pub(crate) fn settled_speed(&self, thrust: f64) -> f64 {
        if thrust > 0.0 && self.k_f > 0.0 {
            (thrust / self.k_f).sqrt()
        } else {
            0.0
        }
    }

    /// The air's drag torque on the prop at `speed`, in N·m, with the speed's
    /// sign: it always works against the spin.
    pub(crate) fn drag_torque(&self, speed: f64) -> f64 {
        let torque = self.k_m * speed * speed.abs();
        if speed >= 0.0 {
            torque
        } else {
            self.reverse_torque * torque
        }
    }

    /// The motor's no-load current at `speed`, with its sign.
    fn loss_current(&self, speed: f64) -> f64 {
        self.loss_per_speed * speed
    }

    /// The speed the motor holds at drive `drive` from `volts`, spinning
    /// `direction`, in rad/s with its sign.
    pub(crate) fn steady_speed(&self, drive: f64, volts: f64, direction: SpinDirection) -> f64 {
        let (sign, k_m) = match direction {
            SpinDirection::Normal => (1.0, self.k_m),
            SpinDirection::Reversed => (-1.0, self.reverse_torque * self.k_m),
        };
        // k_m·ω² + b·ω − c = 0, from the current making the prop's torque.
        let b = 1.0 / (self.resistance * self.kv * self.kv) + self.loss_per_speed / self.kv;
        let c = drive * volts / (self.resistance * self.kv);
        if c <= 0.0 {
            return 0.0;
        }
        let speed = if k_m > 0.0 {
            2.0 * c / (b + (b * b + 4.0 * k_m * c).sqrt())
        } else {
            c / b
        };
        sign * speed
    }

    /// The current that holds `speed` steady against the prop and the
    /// motor's losses, in amps.
    pub(crate) fn steady_current(&self, speed: f64) -> f64 {
        self.kv * self.drag_torque(speed) + self.loss_current(speed)
    }

    /// The voltage across the motor that holds `speed` steady.
    pub(crate) fn steady_voltage(&self, speed: f64) -> f64 {
        speed / self.kv + self.steady_current(speed) * self.resistance
    }
}

/// One motor's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Motor {
    /// In rad/s, positive the normal way.
    pub(crate) speed: f64,
    /// The drive its ESC gave in the last step, as a share of the battery's
    /// voltage.
    pub(crate) drive: f64,
    /// Through the motor, in amps.
    pub(crate) current: f64,
    /// What the ESC took from the battery to drive it, in watts: the drive's
    /// voltage times the current (negative while braking gives some back).
    pub(crate) power: f64,
}

impl Motor {
    pub(crate) const STOPPED: Motor = Motor {
        speed: 0.0,
        drive: 0.0,
        current: 0.0,
        power: 0.0,
    };

    /// Spinning steadily at `speed`, from `volts`.
    pub(crate) fn steady(model: &Model, speed: f64, volts: f64) -> Motor {
        let current = model.steady_current(speed);
        let voltage = model.steady_voltage(speed);
        Motor {
            speed,
            drive: if volts > 0.0 {
                functions::min((voltage / volts).abs(), 1.0)
            } else {
                0.0
            },
            current,
            power: voltage * current,
        }
    }

    /// Moves the motor on by `dt` seconds, driven as its ESC says from a
    /// battery at `volts`.
    pub(crate) fn step(&mut self, model: &Model, drive: Drive, volts: f64, dt: f64) {
        let speed = self.speed;
        let Drive::On {
            throttle,
            direction,
        } = drive
        else {
            // Every switch off: no current, so only the prop and the motor's
            // own losses slow it. It never turns back past a standstill.
            let torque = model.loss_current(speed) / model.kv + model.drag_torque(speed);
            let next = speed - torque / model.inertia * dt;
            self.speed = if next * speed <= 0.0 { 0.0 } else { next };
            self.drive = 0.0;
            self.current = 0.0;
            self.power = 0.0;
            return;
        };

        let target = model.steady_speed(throttle, volts, direction);
        let speeding_up = target * speed >= 0.0 && target.abs() > speed.abs();
        let time_constant = if speeding_up {
            model.spin_up
        } else {
            model.slow_down
        };
        let lagged = target + (speed - target) * functions::exp(-dt / time_constant);

        // The current that turns the motor and prop that way.
        let back_voltage = speed / model.kv;
        let wanted = model.kv * (model.inertia * (lagged - speed) / dt + model.drag_torque(speed))
            + model.loss_current(speed);
        // What the ESC can give: from no drive (full braking) up to its
        // command, or, while slowing, down from the drive it gave last.
        let most = if speeding_up {
            throttle
        } else {
            functions::max(throttle, self.drive)
        };
        let (low, high) = match direction {
            SpinDirection::Normal => (0.0, most * volts),
            SpinDirection::Reversed => (-most * volts, 0.0),
        };
        let lowest = (low - back_voltage) / model.resistance;
        let highest = (high - back_voltage) / model.resistance;
        let current = functions::min(functions::max(wanted, lowest), highest);
        self.speed = if wanted < lowest || wanted > highest {
            let torque =
                (current - model.loss_current(speed)) / model.kv - model.drag_torque(speed);
            speed + torque / model.inertia * dt
        } else {
            lagged
        };
        let voltage = back_voltage + current * model.resistance;
        self.drive = if volts > 0.0 {
            (voltage / volts).abs()
        } else {
            0.0
        };
        self.current = current;
        self.power = voltage * current;
    }
}
