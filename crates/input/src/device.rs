//! Telling devices apart: what SDL says about a device, which kind it is,
//! and which Input Device profile it gets (#19 §1).

use crate::profile::{Connection, InputDeviceProfile, Kind, Match};

/// The USB ids every EdgeTX and OpenTX radio shares (pid.codes 1209:4F54).
pub const EDGETX_VENDOR: u16 = 0x1209;
pub const EDGETX_PRODUCT: u16 = 0x4F54;

/// What SDL tells us about a device when it is plugged in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// SDL's name for it: the USB manufacturer and product strings, such as
    /// "EdgeTX Radiomaster Pocket Joystick" or "DualSense Wireless Controller".
    pub name: String,
    pub usb_vendor: u16,
    pub usb_product: u16,
    /// Whether SDL reads it through its own gamepad drivers or its gamepad
    /// database (`SDL_IsGamepad`).
    pub sdl_gamepad: bool,
    /// USB or Bluetooth, when SDL can tell.
    pub connection: Option<Connection>,
    /// Whether the input thread switched on a heartbeat: a report that
    /// arrives even while the sticks rest, such as a DualSense's motion
    /// sensors (#27). Without one, a device counts as lost only when
    /// unplugged, whatever its profile says, so a resting pad whose sensors
    /// didn't start never trips Failsafe.
    pub heartbeat: bool,
}

impl DeviceInfo {
    /// The pilot's copy of a profile is kept per device model: the USB ids
    /// plus the name. Not per unit, because there's no reliable serial.
    pub fn model(&self) -> DeviceModel {
        DeviceModel {
            usb_vendor: self.usb_vendor,
            usb_product: self.usb_product,
            name: self.name.clone(),
        }
    }
}

/// A device model: its USB ids plus its name. The key the pilot's copies are
/// kept by, and how a re-plugged device is recognised as the same one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceModel {
    pub usb_vendor: u16,
    pub usb_product: u16,
    pub name: String,
}

/// Which kind a device is, before the pilot says otherwise (#19 §1):
///
/// - every EdgeTX or OpenTX radio (USB 1209:4F54) is a Radio, even where SDL
///   reads it as a gamepad, as it does on Linux;
/// - a device SDL reads as a gamepad is a Gamepad;
/// - anything else is a Radio.
///
/// Setup shows the guess with a "This is a Gamepad / Radio" switch.
pub fn kind_of(device: &DeviceInfo) -> Kind {
    if device.usb_vendor == EDGETX_VENDOR && device.usb_product == EDGETX_PRODUCT {
        Kind::Radio
    } else if device.sdl_gamepad {
        Kind::Gamepad
    } else {
        Kind::Radio
    }
}

/// Whether a profile's match fits this device: the same USB ids, and the
/// product name found anywhere in the device's name, ignoring case. A
/// fallback (`match = "any"`) never matches by itself.
pub fn matches(profile: &InputDeviceProfile, device: &DeviceInfo) -> bool {
    match &profile.facts.matches {
        Match::Device {
            usb_vendor,
            usb_product,
            product_name,
        } => {
            *usb_vendor == device.usb_vendor
                && *usb_product == device.usb_product
                && device
                    .name
                    .to_lowercase()
                    .contains(&product_name.to_lowercase())
        }
        Match::Any => false,
    }
}

/// The profile a device gets, from every checked profile:
///
/// 1. a profile that matches it; when several do, the one with the longest
///    product name, then the first by id;
/// 2. otherwise the fallback of the device's kind ("Any Radio" or "Any
///    Gamepad"), the first by id.
///
/// `None` only when no profile matches and no fallback of its kind exists.
pub fn find_profile<'p>(
    device: &DeviceInfo,
    profiles: &'p [InputDeviceProfile],
) -> Option<&'p InputDeviceProfile> {
    let mut by_id: Vec<&InputDeviceProfile> = profiles.iter().collect();
    by_id.sort_by(|a, b| a.id.cmp(&b.id));
    let name_length = |p: &InputDeviceProfile| match &p.facts.matches {
        Match::Device { product_name, .. } => product_name.len(),
        Match::Any => 0,
    };
    let best = by_id.iter().filter(|p| matches(p, device)).fold(
        None::<&&InputDeviceProfile>,
        |best, p| match best {
            Some(b) if name_length(b) >= name_length(p) => Some(b),
            _ => Some(p),
        },
    );
    if let Some(profile) = best {
        return Some(profile);
    }
    let kind = kind_of(device);
    by_id
        .into_iter()
        .find(|p| p.facts.matches == Match::Any && p.kind() == kind)
}
