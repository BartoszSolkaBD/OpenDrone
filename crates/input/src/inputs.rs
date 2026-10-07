//! Every Input Device the game knows of, and what it hands the game: devices
//! found, lost and back, and each device's Channels whenever they change,
//! stamped with the computer's clock.
//!
//! [`Inputs`] is plain code with no SDL in it. The input thread's batches go
//! in ([`Inputs::take`]), and so do recorded traces in the readable checks,
//! so both run exactly the same code.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::channels::{Channels, channel_position, channels};
use crate::controls::Key;
use crate::copy::PilotCopy;
use crate::device::{DeviceInfo, DeviceModel, find_profile, kind_of};
use crate::profile::{Action, ActionSource, InputDeviceProfile, Kind, Press};
use crate::raw::{Batch, DeviceState, Raw, SdlId, drop_values_arriving_with_removals};
use crate::switches::VirtualSwitches;
use crate::transmitting::{Transmitting, still_transmitting};

/// A device that keeps reporting at rest counts as lost after this long
/// without a report, exactly as if it were unplugged (#27).
pub const SILENT_FOR: Duration = Duration::from_secs(1);

/// How many recent stick changes are kept to tell whether a Radio still
/// transmits.
const CHANGES_KEPT: usize = 200;

/// Our own number for an Input Device. A device unplugged and plugged back
/// in keeps its number, so the game sees it come back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub u32);

/// Why a device counts as lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lost {
    /// The operating system removed it.
    Unplugged,
    /// It reports at rest, and stopped for [`SILENT_FOR`].
    Silent,
}

/// What [`Inputs`] hands the game, each stamped with the computer's clock.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// A device was plugged in for the first time while the game runs, and
    /// matched to a profile (`None` when no profile fits).
    Found {
        device: DeviceId,
        at: Duration,
        name: String,
        kind: Kind,
        profile: Option<String>,
    },
    /// A device was unplugged or went silent.
    Lost {
        device: DeviceId,
        at: Duration,
        why: Lost,
    },
    /// A lost device was plugged back in or reported again.
    Back { device: DeviceId, at: Duration },
    /// A device's Channels changed.
    Channels {
        device: DeviceId,
        at: Duration,
        channels: Channels,
    },
}

/// One Input Device as [`Inputs`] keeps it.
#[derive(Clone, Debug)]
pub struct Device {
    id: DeviceId,
    info: DeviceInfo,
    sdl: Option<SdlId>,
    /// The profile it flies on: the Pack's, with the pilot's copy laid over.
    profile: Option<InputDeviceProfile>,
    calibrated: bool,
    state: DeviceState,
    virtuals: VirtualSwitches,
    channels: Channels,
    lost: Option<Lost>,
    last_heard: Duration,
    stick_changes: VecDeque<Duration>,
}

impl Device {
    pub fn id(&self) -> DeviceId {
        self.id
    }

    /// What SDL said about it when it was last plugged in.
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// The profile it flies on, with the pilot's copy laid over the Pack's.
    pub fn profile(&self) -> Option<&InputDeviceProfile> {
        self.profile.as_ref()
    }

    /// Its kind: its profile's, or the guess when no profile fits.
    pub fn kind(&self) -> Kind {
        self.profile
            .as_ref()
            .map_or_else(|| kind_of(&self.info), InputDeviceProfile::kind)
    }

    /// Whether the pilot's copy holds a measured Calibration. An
    /// uncalibrated device flies on its Pack profile's starting values.
    pub fn calibrated(&self) -> bool {
        self.calibrated
    }

    /// Whether it is lost, and why.
    pub fn lost(&self) -> Option<Lost> {
        self.lost
    }

    /// Every control's current value: every button's state included.
    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn channels(&self) -> Channels {
        self.channels
    }

    /// The Actions whose control is in its bound position right now. The
    /// game fires an Action once, when its control moves into that position.
    pub fn actions_held(&self) -> Vec<Action> {
        let Some(profile) = &self.profile else {
            return Vec::new();
        };
        profile
            .setup
            .actions()
            .iter()
            .filter(|(_, source)| match source {
                ActionSource::Channel { channel, on } => {
                    channel_position(&self.state, *channel) == Some(*on)
                }
                ActionSource::Button(button) => self
                    .state
                    .pad
                    .as_ref()
                    .is_some_and(|pad| pad.button(*button)),
            })
            .map(|(action, _)| *action)
            .collect()
    }

    /// Whether this Radio still transmits, from its recent stick changes
    /// (#30): the RF-on Pre-flight Warning and setup's update-rate check.
    pub fn transmitting(&self) -> Transmitting {
        let changes: Vec<Duration> = self.stick_changes.iter().copied().collect();
        still_transmitting(&changes)
    }

    fn connected(&self) -> bool {
        self.sdl.is_some()
    }
}

/// Every Input Device, its profile and its Channels.
#[derive(Clone, Debug)]
pub struct Inputs {
    profiles: Vec<InputDeviceProfile>,
    copies: BTreeMap<DeviceModel, PilotCopy>,
    devices: Vec<Device>,
    flying: Option<DeviceId>,
    keys_held: Vec<Key>,
}

impl Inputs {
    /// Starts with the checked Input Device profiles of every Pack.
    pub fn new(profiles: Vec<InputDeviceProfile>) -> Inputs {
        Inputs {
            profiles,
            copies: BTreeMap::new(),
            devices: Vec::new(),
            flying: None,
            keys_held: Vec::new(),
        }
    }

    /// Every device seen since the game started, connected or not, in the
    /// order they were found.
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.get(id.0 as usize)
    }

    /// The Flying Input Device: its Channels fly the Quad, and keys drive
    /// the Virtual Switches its profile binds to keys. The game picks it.
    pub fn flying(&self) -> Option<DeviceId> {
        self.flying
    }

    /// Lays the pilot's copy over a device model's Pack profile, or with
    /// `None` resets it to the Pack's starting profile.
    pub fn set_copy(
        &mut self,
        model: DeviceModel,
        copy: Option<PilotCopy>,
        at: Duration,
    ) -> Vec<InputEvent> {
        match copy {
            Some(copy) => self.copies.insert(model.clone(), copy),
            None => self.copies.remove(&model),
        };
        let ids: Vec<DeviceId> = self
            .devices
            .iter()
            .filter(|d| d.info.model() == model)
            .map(|d| d.id)
            .collect();
        let mut events = Vec::new();
        for id in ids {
            let (profile, calibrated) = self.profile_for(&self.devices[id.0 as usize].info);
            let device = &mut self.devices[id.0 as usize];
            device.profile = profile;
            device.calibrated = calibrated;
            self.refresh(id, at, &mut events);
        }
        events
    }

    /// Picks the Flying Input Device. Keys held for the old one let go.
    pub fn set_flying(&mut self, device: Option<DeviceId>, at: Duration) -> Vec<InputEvent> {
        let mut events = Vec::new();
        let old = std::mem::replace(&mut self.flying, device);
        for id in [old, device].into_iter().flatten() {
            self.refresh(id, at, &mut events);
        }
        events
    }

    /// Takes one poll's changes from the input thread. Values that arrive in
    /// the same instant as their device's removal are thrown away.
    pub fn take(&mut self, mut batch: Batch) -> Vec<InputEvent> {
        drop_values_arriving_with_removals(&mut batch);
        let at = batch.at;
        let mut events = Vec::new();
        let mut touched: Vec<DeviceId> = Vec::new();
        for raw in batch.events {
            match raw {
                Raw::Added {
                    device: sdl,
                    info,
                    state,
                } => {
                    let id = self.add(sdl, info, state, at, &mut events);
                    touched.push(id);
                }
                Raw::Removed { device: sdl } => {
                    if let Some(id) = self.by_sdl(sdl) {
                        let device = &mut self.devices[id.0 as usize];
                        device.sdl = None;
                        device.lost = Some(Lost::Unplugged);
                        events.push(InputEvent::Lost {
                            device: id,
                            at,
                            why: Lost::Unplugged,
                        });
                    }
                }
                raw => {
                    let Some(id) = self.by_sdl(raw.device()) else {
                        continue;
                    };
                    let device = &mut self.devices[id.0 as usize];
                    device.last_heard = at;
                    if device.lost == Some(Lost::Silent) {
                        device.lost = None;
                        events.push(InputEvent::Back { device: id, at });
                    }
                    if device.state.apply(&raw) && !touched.contains(&id) {
                        touched.push(id);
                    }
                }
            }
        }
        for id in touched {
            self.refresh(id, at, &mut events);
        }
        events
    }

    /// Counts a device as lost when it keeps reporting at rest and has said
    /// nothing for [`SILENT_FOR`]. Devices that report only changes, like
    /// the Pocket, are lost only when unplugged: a resting Pocket can send
    /// nothing at all. Call it often, with the input thread's clock.
    pub fn check_silence(&mut self, now: Duration) -> Vec<InputEvent> {
        let mut events = Vec::new();
        for device in &mut self.devices {
            let reports_at_rest = device
                .profile
                .as_ref()
                .is_some_and(|p| p.reports_at_rest(device.info.connection));
            if device.connected()
                && device.lost.is_none()
                && reports_at_rest
                && now.saturating_sub(device.last_heard) >= SILENT_FOR
            {
                device.lost = Some(Lost::Silent);
                events.push(InputEvent::Lost {
                    device: device.id,
                    at: now,
                    why: Lost::Silent,
                });
            }
        }
        events
    }

    /// A keyboard key went down or up. Keys drive only the Virtual Switches
    /// of the Flying Input Device's profile, and travel in its Channels.
    /// They never fly a Quad.
    pub fn key(&mut self, key: Key, down: bool, at: Duration) -> Vec<InputEvent> {
        self.keys_held.retain(|k| *k != key);
        if down {
            self.keys_held.push(key);
        }
        self.refresh_flying(at)
    }

    /// The game window lost focus: every held key counts as let go.
    pub fn focus_lost(&mut self, at: Duration) -> Vec<InputEvent> {
        self.keys_held.clear();
        self.refresh_flying(at)
    }

    /// The game reports that the Quad disarmed, for any reason: Reset, a
    /// Failsafe drop, a blocked arm or Crash Flip's disarm. Every Arm toggle
    /// turns itself off, so the next press always means "arm now".
    pub fn disarmed(&mut self, at: Duration) -> Vec<InputEvent> {
        let mut events = Vec::new();
        let ids: Vec<DeviceId> = self.devices.iter().map(|d| d.id).collect();
        for id in ids {
            if self.devices[id.0 as usize].virtuals.disarmed() {
                self.refresh(id, at, &mut events);
            }
        }
        events
    }

    fn refresh_flying(&mut self, at: Duration) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if let Some(id) = self.flying {
            self.refresh(id, at, &mut events);
        }
        events
    }

    fn by_sdl(&self, sdl: SdlId) -> Option<DeviceId> {
        self.devices
            .iter()
            .find(|d| d.sdl == Some(sdl))
            .map(|d| d.id)
    }

    fn profile_for(&self, info: &DeviceInfo) -> (Option<InputDeviceProfile>, bool) {
        let Some(pack) = find_profile(info, &self.profiles) else {
            return (None, false);
        };
        match self.copies.get(&info.model()) {
            Some(copy) => (Some(copy.laid_over(pack)), copy.calibrated),
            None => (Some(pack.clone()), false),
        }
    }

    fn add(
        &mut self,
        sdl: SdlId,
        info: DeviceInfo,
        state: DeviceState,
        at: Duration,
        events: &mut Vec<InputEvent>,
    ) -> DeviceId {
        let returning = self
            .devices
            .iter()
            .find(|d| !d.connected() && d.info.model() == info.model())
            .map(|d| d.id);
        let (profile, calibrated) = self.profile_for(&info);
        if let Some(id) = returning {
            let device = &mut self.devices[id.0 as usize];
            device.sdl = Some(sdl);
            device.info = info;
            device.profile = profile;
            device.calibrated = calibrated;
            device.state = state;
            device.lost = None;
            device.last_heard = at;
            events.push(InputEvent::Back { device: id, at });
            return id;
        }
        let id = DeviceId(u32::try_from(self.devices.len()).unwrap_or(u32::MAX));
        events.push(InputEvent::Found {
            device: id,
            at,
            name: info.name.clone(),
            kind: profile
                .as_ref()
                .map_or_else(|| kind_of(&info), InputDeviceProfile::kind),
            profile: profile.as_ref().map(|p| p.id.clone()),
        });
        let mut device = Device {
            id,
            info,
            sdl: Some(sdl),
            profile,
            calibrated,
            state,
            virtuals: VirtualSwitches::new(),
            channels: Channels::at_rest(),
            lost: None,
            last_heard: at,
            stick_changes: VecDeque::new(),
        };
        if let Some(profile) = &device.profile {
            device.channels = channels(&profile.setup, &device.state, &device.virtuals);
        }
        events.push(InputEvent::Channels {
            device: id,
            at,
            channels: device.channels,
        });
        self.devices.push(device);
        id
    }

    /// Works a device's Channels out again, and hands them over if they
    /// changed.
    fn refresh(&mut self, id: DeviceId, at: Duration, events: &mut Vec<InputEvent>) {
        let flying = self.flying == Some(id);
        let keys = &self.keys_held;
        let Some(device) = self.devices.get_mut(id.0 as usize) else {
            return;
        };
        if !device.connected() {
            return;
        }
        let Some(profile) = &device.profile else {
            return;
        };
        let state = &device.state;
        device
            .virtuals
            .update(profile.setup.switches(), |press| match press {
                Press::Button(button) => state.pad.as_ref().is_some_and(|pad| pad.button(*button)),
                Press::Key(key) => flying && keys.contains(key),
            });
        let new = channels(&profile.setup, state, &device.virtuals);
        let old = device.channels;
        if new == old {
            return;
        }
        if (new.roll, new.pitch, new.throttle, new.yaw)
            != (old.roll, old.pitch, old.throttle, old.yaw)
        {
            if device.stick_changes.len() == CHANGES_KEPT {
                device.stick_changes.pop_front();
            }
            device.stick_changes.push_back(at);
        }
        device.channels = new;
        events.push(InputEvent::Channels {
            device: id,
            at,
            channels: new,
        });
    }
}
