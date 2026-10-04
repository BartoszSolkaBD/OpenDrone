//! PROTOTYPE. The SDL 3 input thread: initialises SDL's joystick + gamepad subsystems on a
//! plain `std::thread` (never the main thread), polls as fast as it is told to, and records every
//! raw change SDL reports, with SDL's own timestamp. No deadzone, no smoothing, no filtering.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sdl3_sys::everything::*;

/// What one recorded line means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Axis,
    Button,
    Hat,
    Gyro,
    Accel,
    Added,
    Removed,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Axis => "axis",
            Kind::Button => "button",
            Kind::Hat => "hat",
            Kind::Gyro => "gyro",
            Kind::Accel => "accel",
            Kind::Added => "added",
            Kind::Removed => "removed",
        }
    }
}

/// One raw input change, exactly as SDL delivered it.
#[derive(Clone, Copy, Debug)]
pub struct Rec {
    /// SDL's event timestamp (nanoseconds, SDL clock).
    pub t_sdl: u64,
    /// When our thread pulled it off SDL's queue (nanoseconds since the probe started).
    pub t_rx: u64,
    /// Device clock for sensor readings (nanoseconds), 0 otherwise.
    pub sensor_ts: u64,
    pub dev: u32,
    pub value: i32,
    pub phase: u8,
    pub kind: Kind,
    pub idx: u8,
}

/// Everything SDL tells us about one Input Device when it is opened.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DeviceInfo {
    pub instance_id: u32,
    pub name: String,
    pub path: String,
    pub vendor_id: String,
    pub product_id: String,
    pub product_version: u16,
    pub firmware_version: u16,
    pub serial: String,
    pub guid: String,
    pub bus: String,
    pub sdl_driver: String,
    pub joystick_type: String,
    pub connection: String,
    pub is_gamepad: bool,
    pub gamepad_type: String,
    pub num_axes: i32,
    pub num_buttons: i32,
    pub num_hats: i32,
    pub has_gyro: bool,
    pub has_accel: bool,
    /// SDL's own claim of the sensor rate (Hz), read after the sensors are switched on.
    pub sdl_sensor_rate_hz: Option<f32>,
    pub initial_axis_state: Vec<Option<i16>>,
    pub opened_at_s: f64,
    pub removed_at_s: Option<f64>,
}

impl DeviceInfo {
    pub fn vendor_product(&self) -> (u16, u16) {
        let v = u16::from_str_radix(self.vendor_id.trim_start_matches("0x"), 16).unwrap_or(0);
        let p = u16::from_str_radix(self.product_id.trim_start_matches("0x"), 16).unwrap_or(0);
        (v, p)
    }
}

/// Facts about the thread SDL ran on.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct InitInfo {
    pub sdl_version: String,
    pub sdl_revision: String,
    pub thread_name: String,
    /// `pthread_main_np()` on macOS: 1 = main thread, 0 = not. None elsewhere.
    pub pthread_main_np: Option<i32>,
    pub qos_requested: bool,
    pub qos_result: Option<i32>,
    pub sdl_init_ok: bool,
    pub sdl_init_ms: f64,
    pub sdl_error: String,
    pub poll_sleep_us: u64,
}

/// What the main thread sees, refreshed about ten times a second.
#[derive(Clone, Debug, Default)]
pub struct Live {
    pub init: Option<InitInfo>,
    pub devices: Vec<LiveDevice>,
    pub poll_hz: f64,
    pub poll_max_gap_ms: f64,
    pub any_sensors: bool,
    pub sensors_on: bool,
}

#[derive(Clone, Debug, Default)]
pub struct LiveDevice {
    pub name: String,
    pub summary: String,
    pub axes: Vec<i16>,
    pub buttons: Vec<bool>,
    pub hats: Vec<u8>,
    pub updates_per_s: f64,
    pub sensor_per_s: f64,
}

pub struct ThreadConfig {
    pub poll_sleep_us: u64,
    pub qos: bool,
    pub t0: Instant,
    pub phase: Arc<AtomicU8>,
    pub stop: Arc<AtomicBool>,
    pub enable_sensors: Arc<AtomicBool>,
    pub live: Arc<Mutex<Live>>,
    /// Test the analysis code with an SDL virtual joystick (NOT hardware; never a measurement).
    pub selftest: bool,
}

pub struct ThreadResult {
    pub init: InitInfo,
    pub devices: Vec<DeviceInfo>,
    pub records: Vec<Rec>,
    /// (phase, microseconds since previous poll) for every poll-loop iteration.
    pub poll_intervals: Vec<(u8, u32)>,
}

struct Open {
    info_idx: usize,
    joystick: *mut SDL_Joystick,
    gamepad: *mut SDL_Gamepad,
    sensors_enabled: bool,
    // live counters
    last_axis_ts: u64,
    updates_in_window: u32,
    sensor_in_window: u32,
    hats: Vec<u8>,
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn joystick_type_name(t: SDL_JoystickType) -> &'static str {
    match t.0 {
        1 => "gamepad",
        2 => "wheel",
        3 => "arcade stick",
        4 => "flight stick",
        5 => "dance pad",
        6 => "guitar",
        7 => "drum kit",
        8 => "arcade pad",
        9 => "throttle",
        _ => "unknown",
    }
}

fn connection_name(c: SDL_JoystickConnectionState) -> &'static str {
    match c.0 {
        1 => "wired",
        2 => "wireless",
        0 => "unknown",
        _ => "invalid",
    }
}

fn decode_guid(g: &SDL_GUID) -> (String, String) {
    let bus = u16::from_le_bytes([g.data[0], g.data[1]]);
    let bus = match bus {
        0x03 => "USB".to_string(),
        0x05 => "Bluetooth".to_string(),
        0x00 => "unknown".to_string(),
        other => format!("0x{other:02x}"),
    };
    let sig = g.data[14];
    let driver = match sig {
        b'h' => "HIDAPI (SDL's own HID driver)".to_string(),
        b'v' => "virtual".to_string(),
        b'r' => "RAWINPUT".to_string(),
        b'w' => "Windows.Gaming.Input".to_string(),
        b'x' => "XInput".to_string(),
        b's' => "Steam".to_string(),
        b'm' => "MFi / GameController framework".to_string(),
        0 => {
            if cfg!(target_os = "macos") {
                "platform driver (IOKit on macOS)".to_string()
            } else if cfg!(target_os = "windows") {
                "platform driver (DirectInput)".to_string()
            } else {
                "platform driver (evdev on Linux)".to_string()
            }
        }
        other => format!("signature 0x{other:02x}"),
    };
    (bus, driver)
}

#[cfg(target_os = "macos")]
fn set_qos_user_interactive() -> i32 {
    unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0) }
}

#[cfg(target_os = "macos")]
fn is_main_thread() -> Option<i32> {
    unsafe extern "C" {
        fn pthread_main_np() -> i32;
    }
    Some(unsafe { pthread_main_np() })
}

#[cfg(not(target_os = "macos"))]
fn is_main_thread() -> Option<i32> {
    None
}

pub fn spawn(cfg: ThreadConfig) -> std::thread::JoinHandle<ThreadResult> {
    std::thread::Builder::new()
        .name("sdl-input".into())
        .spawn(move || run(cfg))
        .expect("spawn input thread")
}

fn run(cfg: ThreadConfig) -> ThreadResult {
    let mut init = InitInfo {
        thread_name: std::thread::current().name().unwrap_or("?").to_string(),
        pthread_main_np: is_main_thread(),
        qos_requested: cfg.qos,
        poll_sleep_us: cfg.poll_sleep_us,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if cfg.qos {
        init.qos_result = Some(set_qos_user_interactive());
    }

    let v = SDL_GetVersion();
    init.sdl_version = format!(
        "{}.{}.{}",
        SDL_VERSIONNUM_MAJOR(v),
        SDL_VERSIONNUM_MINOR(v),
        SDL_VERSIONNUM_MICRO(v)
    );
    init.sdl_revision = cstr(SDL_GetRevision());

    let t_init = Instant::now();
    // GAMEPAD implies JOYSTICK (and EVENTS). No video: Bevy/winit owns the window.
    let ok = unsafe { SDL_Init(SDL_INIT_GAMEPAD) };
    init.sdl_init_ms = t_init.elapsed().as_secs_f64() * 1000.0;
    init.sdl_init_ok = ok;
    if !ok {
        init.sdl_error = cstr(SDL_GetError());
    }
    if let Ok(mut live) = cfg.live.lock() {
        live.init = Some(init.clone());
    }

    let mut devices: Vec<DeviceInfo> = Vec::new();
    let mut records: Vec<Rec> = Vec::with_capacity(1 << 20);
    let mut poll_intervals: Vec<(u8, u32)> = Vec::with_capacity(1 << 19);
    let mut open: HashMap<u32, Open> = HashMap::new();

    if !ok {
        return ThreadResult { init, devices, records, poll_intervals };
    }
    let mut selftest = if cfg.selftest { SelfTest::attach() } else { None };

    let sleep = Duration::from_micros(cfg.poll_sleep_us);
    let mut events: Vec<MaybeUninit<SDL_Event>> = Vec::with_capacity(256);
    unsafe { events.set_len(256) };

    let mut last_poll = Instant::now();
    let mut window_start = Instant::now();
    let mut polls_in_window: u32 = 0;
    let mut max_gap_in_window = Duration::ZERO;
    let mut sensors_on = false;

    while !cfg.stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        let gap = now - last_poll;
        last_poll = now;
        let phase = cfg.phase.load(Ordering::Relaxed);
        poll_intervals.push((phase, gap.as_micros().min(u32::MAX as u128) as u32));
        polls_in_window += 1;
        if gap > max_gap_in_window {
            max_gap_in_window = gap;
        }

        if let Some(st) = selftest.as_mut() {
            st.drive(phase, cfg.t0.elapsed().as_secs_f64());
        }
        unsafe { SDL_UpdateJoysticks() };

        loop {
            let n = unsafe {
                SDL_PeepEvents(
                    events.as_mut_ptr() as *mut SDL_Event,
                    events.len() as i32,
                    SDL_GETEVENT,
                    SDL_EVENT_FIRST.0,
                    SDL_EVENT_LAST.0,
                )
            };
            if n <= 0 {
                break;
            }
            let t_rx = cfg.t0.elapsed().as_nanos() as u64;
            for e in &events[..n as usize] {
                let e = unsafe { e.assume_init_ref() };
                handle_event(e, t_rx, phase, &cfg, &mut devices, &mut records, &mut open);
            }
            if (n as usize) < events.len() {
                break;
            }
        }

        if cfg.enable_sensors.load(Ordering::Relaxed) && !sensors_on {
            sensors_on = true;
            for o in open.values_mut() {
                enable_sensors(o, &mut devices);
            }
        }

        if now - window_start >= Duration::from_millis(250) {
            let secs = (now - window_start).as_secs_f64();
            publish_live(&cfg, &devices, &mut open, polls_in_window, secs, max_gap_in_window, sensors_on);
            window_start = now;
            polls_in_window = 0;
            max_gap_in_window = Duration::ZERO;
        }

        std::thread::sleep(sleep);
    }

    for o in open.values() {
        unsafe {
            if !o.gamepad.is_null() {
                SDL_CloseGamepad(o.gamepad);
            } else if !o.joystick.is_null() {
                SDL_CloseJoystick(o.joystick);
            }
        }
    }
    unsafe { SDL_Quit() };

    ThreadResult { init, devices, records, poll_intervals }
}

fn enable_sensors(o: &mut Open, devices: &mut [DeviceInfo]) {
    if o.gamepad.is_null() || o.sensors_enabled {
        return;
    }
    let info = &mut devices[o.info_idx];
    unsafe {
        if info.has_gyro {
            SDL_SetGamepadSensorEnabled(o.gamepad, SDL_SENSOR_GYRO, true);
            info.sdl_sensor_rate_hz = Some(SDL_GetGamepadSensorDataRate(o.gamepad, SDL_SENSOR_GYRO));
        }
        if info.has_accel {
            SDL_SetGamepadSensorEnabled(o.gamepad, SDL_SENSOR_ACCEL, true);
        }
    }
    o.sensors_enabled = info.has_gyro || info.has_accel;
}

#[allow(clippy::too_many_arguments)]
fn handle_event(
    e: &SDL_Event,
    t_rx: u64,
    phase: u8,
    cfg: &ThreadConfig,
    devices: &mut Vec<DeviceInfo>,
    records: &mut Vec<Rec>,
    open: &mut HashMap<u32, Open>,
) {
    let ty = unsafe { e.r#type };
    let rec = |t_sdl: u64, dev: u32, kind: Kind, idx: u8, value: i32, sensor_ts: u64| Rec {
        t_sdl,
        t_rx,
        sensor_ts,
        dev,
        value,
        phase,
        kind,
        idx,
    };

    if ty == SDL_EVENT_JOYSTICK_ADDED.0 {
        let ev = unsafe { e.jdevice };
        let id = ev.which.0;
        if open.contains_key(&id) {
            return;
        }
        let (joystick, gamepad) = unsafe {
            if SDL_IsGamepad(ev.which) {
                let g = SDL_OpenGamepad(ev.which);
                (if g.is_null() { std::ptr::null_mut() } else { SDL_GetGamepadJoystick(g) }, g)
            } else {
                (SDL_OpenJoystick(ev.which), std::ptr::null_mut())
            }
        };
        if joystick.is_null() {
            eprintln!("[input] could not open device {id}: {}", cstr(SDL_GetError()));
            return;
        }
        let info = describe(id, joystick, gamepad, cfg.t0.elapsed().as_secs_f64());
        println!(
            "[input] connected: {} ({} {}:{}, {}, {}, {} axes, {} buttons)",
            info.name, info.bus, info.vendor_id, info.product_id, info.connection, info.sdl_driver, info.num_axes,
            info.num_buttons
        );
        let hats = vec![0u8; info.num_hats.max(0) as usize];
        devices.push(info);
        let mut o = Open {
            info_idx: devices.len() - 1,
            joystick,
            gamepad,
            sensors_enabled: false,
            last_axis_ts: 0,
            updates_in_window: 0,
            sensor_in_window: 0,
            hats,
        };
        if cfg.enable_sensors.load(Ordering::Relaxed) {
            enable_sensors(&mut o, devices);
        }
        open.insert(id, o);
        records.push(rec(ev.timestamp, id, Kind::Added, 0, 0, 0));
    } else if ty == SDL_EVENT_JOYSTICK_REMOVED.0 {
        let ev = unsafe { e.jdevice };
        let id = ev.which.0;
        if let Some(o) = open.remove(&id) {
            devices[o.info_idx].removed_at_s = Some(cfg.t0.elapsed().as_secs_f64());
            println!("[input] disconnected: {}", devices[o.info_idx].name);
            unsafe {
                if !o.gamepad.is_null() {
                    SDL_CloseGamepad(o.gamepad);
                } else {
                    SDL_CloseJoystick(o.joystick);
                }
            }
        }
        records.push(rec(ev.timestamp, id, Kind::Removed, 0, 0, 0));
    } else if ty == SDL_EVENT_JOYSTICK_AXIS_MOTION.0 {
        let ev = unsafe { e.jaxis };
        let id = ev.which.0;
        if let Some(o) = open.get_mut(&id) {
            if ev.timestamp != o.last_axis_ts {
                o.last_axis_ts = ev.timestamp;
                o.updates_in_window += 1;
            }
        }
        records.push(rec(ev.timestamp, id, Kind::Axis, ev.axis, ev.value as i32, 0));
    } else if ty == SDL_EVENT_JOYSTICK_BUTTON_DOWN.0 || ty == SDL_EVENT_JOYSTICK_BUTTON_UP.0 {
        let ev = unsafe { e.jbutton };
        records.push(rec(ev.timestamp, ev.which.0, Kind::Button, ev.button, ev.down as i32, 0));
    } else if ty == SDL_EVENT_JOYSTICK_HAT_MOTION.0 {
        let ev = unsafe { e.jhat };
        if let Some(o) = open.get_mut(&ev.which.0) {
            if let Some(h) = o.hats.get_mut(ev.hat as usize) {
                *h = ev.value;
            }
        }
        records.push(rec(ev.timestamp, ev.which.0, Kind::Hat, ev.hat, ev.value as i32, 0));
    } else if ty == SDL_EVENT_GAMEPAD_SENSOR_UPDATE.0 {
        let ev = unsafe { e.gsensor };
        let kind = if ev.sensor == SDL_SENSOR_GYRO.0 {
            Kind::Gyro
        } else if ev.sensor == SDL_SENSOR_ACCEL.0 {
            Kind::Accel
        } else {
            return;
        };
        if kind == Kind::Gyro {
            if let Some(o) = open.get_mut(&ev.which.0) {
                o.sensor_in_window += 1;
            }
        }
        records.push(rec(ev.timestamp, ev.which.0, kind, 0, 0, ev.sensor_timestamp));
    }
}

fn describe(id: u32, j: *mut SDL_Joystick, g: *mut SDL_Gamepad, now_s: f64) -> DeviceInfo {
    unsafe {
        let guid = SDL_GetJoystickGUID(j);
        let mut buf = [0 as c_char; 33];
        SDL_GUIDToString(guid, buf.as_mut_ptr(), buf.len() as i32);
        let (bus, sdl_driver) = decode_guid(&guid);
        let num_axes = SDL_GetNumJoystickAxes(j);
        let initial_axis_state = (0..num_axes.max(0))
            .map(|a| {
                let mut s: i16 = 0;
                if SDL_GetJoystickAxisInitialState(j, a, &mut s) { Some(s) } else { None }
            })
            .collect();
        let (gamepad_type, has_gyro, has_accel) = if g.is_null() {
            (String::new(), false, false)
        } else {
            (
                cstr(SDL_GetGamepadStringForType(SDL_GetGamepadType(g))),
                SDL_GamepadHasSensor(g, SDL_SENSOR_GYRO),
                SDL_GamepadHasSensor(g, SDL_SENSOR_ACCEL),
            )
        };
        DeviceInfo {
            instance_id: id,
            name: cstr(SDL_GetJoystickName(j)),
            path: cstr(SDL_GetJoystickPath(j)),
            vendor_id: format!("0x{:04X}", SDL_GetJoystickVendor(j)),
            product_id: format!("0x{:04X}", SDL_GetJoystickProduct(j)),
            product_version: SDL_GetJoystickProductVersion(j),
            firmware_version: SDL_GetJoystickFirmwareVersion(j),
            serial: cstr(SDL_GetJoystickSerial(j)),
            guid: cstr(buf.as_ptr()),
            bus,
            sdl_driver,
            joystick_type: joystick_type_name(SDL_GetJoystickType(j)).to_string(),
            connection: connection_name(SDL_GetJoystickConnectionState(j)).to_string(),
            is_gamepad: !g.is_null(),
            gamepad_type,
            num_axes,
            num_buttons: SDL_GetNumJoystickButtons(j),
            num_hats: SDL_GetNumJoystickHats(j),
            has_gyro,
            has_accel,
            sdl_sensor_rate_hz: None,
            initial_axis_state,
            opened_at_s: now_s,
            removed_at_s: None,
        }
    }
}

fn publish_live(
    cfg: &ThreadConfig,
    devices: &[DeviceInfo],
    open: &mut HashMap<u32, Open>,
    polls: u32,
    secs: f64,
    max_gap: Duration,
    sensors_on: bool,
) {
    let mut live_devs = Vec::new();
    let mut ids: Vec<&u32> = open.keys().collect();
    ids.sort();
    let ids: Vec<u32> = ids.into_iter().copied().collect();
    for id in ids {
        let o = open.get_mut(&id).unwrap();
        let info = &devices[o.info_idx];
        let axes = (0..info.num_axes.max(0)).map(|a| unsafe { SDL_GetJoystickAxis(o.joystick, a) }).collect();
        let buttons =
            (0..info.num_buttons.max(0)).map(|b| unsafe { SDL_GetJoystickButton(o.joystick, b) }).collect();
        live_devs.push(LiveDevice {
            name: info.name.clone(),
            summary: format!(
                "{}:{} {} {} | {}",
                info.vendor_id, info.product_id, info.bus, info.connection, info.sdl_driver
            ),
            axes,
            buttons,
            hats: o.hats.clone(),
            updates_per_s: o.updates_in_window as f64 / secs,
            sensor_per_s: o.sensor_in_window as f64 / secs,
        });
        o.updates_in_window = 0;
        o.sensor_in_window = 0;
    }
    let any_sensors = open.values().any(|o| {
        let i = &devices[o.info_idx];
        i.has_gyro || i.has_accel
    });
    if let Ok(mut live) = cfg.live.lock() {
        live.devices = live_devs;
        live.poll_hz = polls as f64 / secs;
        live.poll_max_gap_ms = max_gap.as_secs_f64() * 1000.0;
        live.any_sensors = any_sensors;
        live.sensors_on = sensors_on;
    }
}

/// SELF-TEST ONLY: an SDL virtual joystick shaped like an EdgeTX radio (8 axes, 24 buttons,
/// 11-bit values, a new "report" every 1 ms), pushed through SDL's normal event path so the
/// statistics and summary code can be checked without hardware. Its numbers are not measurements.
struct SelfTest {
    id: SDL_JoystickID,
    j: *mut SDL_Joystick,
    last_report_s: f64,
}

impl SelfTest {
    fn attach() -> Option<SelfTest> {
        let name = c"SELF-TEST virtual device (not hardware)";
        let mut desc = SDL_VirtualJoystickDesc::default();
        desc.vendor_id = 0x1209;
        desc.product_id = 0x4F54;
        desc.naxes = 8;
        desc.nbuttons = 24;
        desc.name = name.as_ptr();
        let id = unsafe { SDL_AttachVirtualJoystick(&desc) };
        if id.0 == 0 {
            eprintln!("[selftest] could not attach virtual joystick: {}", cstr(SDL_GetError()));
            return None;
        }
        Some(SelfTest { id, j: std::ptr::null_mut(), last_report_s: 0.0 })
    }

    fn drive(&mut self, phase: u8, t: f64) {
        use crate::Phase;
        if self.j.is_null() {
            self.j = unsafe { SDL_GetJoystickFromID(self.id) };
            if self.j.is_null() {
                return;
            }
        }
        // A "report" every whole millisecond, like a 1 kHz device, independent of the poll rate.
        let tick = (t * 1000.0).floor();
        if tick == self.last_report_s {
            return;
        }
        self.last_report_s = tick;
        let t = tick / 1000.0;
        // x in -1..1 -> EdgeTX 0..2048 -> SDL -32768..32767 (as SDL's IOKit scaling does)
        let raw = |x: f64| -> i16 {
            let r = ((x.clamp(-1.0, 1.0) + 1.0) * 1024.0).round();
            ((r * 65535.0 / 2048.0) - 32768.0).round() as i16
        };
        let w = 2.0 * std::f64::consts::PI * 1.5 * t;
        let mut axes = [0.0f64; 8];
        axes[2] = -1.0; // throttle low
        match Phase::from_u8(phase) {
            Phase::Circles | Phase::Unfocused | Phase::Sensors => {
                axes = [w.cos(), w.sin(), (w * 1.1).sin(), (w * 1.1).cos(), 0.0, 0.0, 0.0, 0.0];
            }
            Phase::Yaw => axes[3] = w.sin(),
            Phase::Extremes => {
                let s = 0.2 * w;
                axes = [s.cos(), s.sin(), s.sin(), s.cos(), 0.0, 0.0, 0.0, 0.0];
            }
            Phase::Switches => {
                let k = (t * 2.0) as i64;
                for (i, a) in axes.iter_mut().enumerate().skip(4) {
                    *a = [-1.0, 0.0, 1.0][((k + i as i64) % 3) as usize];
                }
                for b in 0..24 {
                    unsafe { SDL_SetJoystickVirtualButton(self.j, b, (k + b as i64) % 4 == 0) };
                }
            }
            _ => {}
        }
        for (i, a) in axes.iter().enumerate() {
            unsafe { SDL_SetJoystickVirtualAxis(self.j, i as i32, raw(*a)) };
        }
    }
}
