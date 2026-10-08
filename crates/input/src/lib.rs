//! Input Devices on their own thread, on SDL 3.4 ([ADR-0018]).
//!
//! An edge crate ([ADR-0003]): it talks to the operating system, so it is one
//! of the two crates that may use `unsafe`. It knows only devices and
//! Channels; the game turns keys, buttons and switches into Actions, and
//! turns the computer's clock into Simulation Time.
//!
//! How a device's values become Channels:
//!
//! 1. **The input thread** ([`thread`]) reads every Input Device through SDL
//!    about 3,000 times a second and stamps what it reads with the
//!    computer's clock. Everything one poll brought arrives as one [`Batch`].
//! 2. **[`Inputs`]** takes those batches. It finds each device's kind and
//!    profile ([`device`]), keeps its values, throws away values that arrive
//!    with an unplug, counts a silent device as lost, and hands the game
//!    [`InputEvent`]s: devices found, lost and back, and Channels whenever
//!    they change.
//! 3. **Channels** ([`channels`]) come from the device's Input Device profile
//!    ([`profile`], as the Pack checker reads it, with the pilot's
//!    [`PilotCopy`] laid over): channel mapping, Calibration, a
//!    Betaflight-style deadband on roll, pitch and yaw, the throttle style,
//!    and the switches, including Virtual Switches ([`switches`]).
//!
//! The Calibration maths ([`calibration`]) and the check that a Radio still
//! transmits ([`transmitting`]) are plain functions too. Everything but the
//! thread is plain code, so the readable checks feed it recorded device
//! traces with no hardware attached.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0018]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0018-input-through-sdl3-on-its-own-thread.md

pub mod calibration;
pub mod channels;
mod copy;
pub mod device;
pub mod inputs;
pub mod raw;
pub mod switches;
pub mod thread;
pub mod transmitting;

/// Input Device profiles, as the Pack checker reads them.
pub use opendrone_pack::input_device as profile;
/// The controls a device has, as SDL names them.
pub use opendrone_pack::input_device::controls;

pub use channels::Channels;
pub use copy::PilotCopy;
pub use device::{DeviceInfo, DeviceModel, find_profile, kind_of};
pub use inputs::{Device, DeviceId, InputEvent, Inputs, Lost};
pub use profile::{InputDeviceProfile, Kind};
pub use raw::{Batch, DeviceState, Raw, SdlId};
pub use thread::InputThread;
