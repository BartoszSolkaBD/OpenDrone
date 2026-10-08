//! The input thread: SDL 3.4 on a thread of its own, reading every Input
//! Device about 3,000 times a second and stamping what it reads with the
//! computer's clock (ADR-0018).
//!
//! - SDL is built from source and statically linked with only its joystick
//!   and HIDAPI parts; on Linux it is a no-window build.
//! - Samples are stamped when this thread reads them, not with the device's
//!   or the operating system's own time, so it polls well above the fastest
//!   device's Report Rate: stamps are accurate to about 0.3 ms.
//! - SDL's signal handlers are off, so Ctrl+C still stops the game and the
//!   test runner (#28). Sticks keep working when the window isn't focused.
//! - A DualSense's motion sensors are switched on as its heartbeat: over USB
//!   it then reports 250 times a second even at rest (#27).
//!
//! This is the only module that calls SDL. Everything it reads goes out as
//! plain [`Batch`]es, which [`Inputs`](crate::Inputs) turns into Channels.

use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_int};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sdl3_sys::everything::*;

use crate::controls::{PadAxis, PadButton};
use crate::device::DeviceInfo;
use crate::profile::Connection;
use crate::raw::{Batch, DeviceState, PadState, Raw, SdlId};

/// How often the thread reads SDL: about 3,000 times a second, so stamps are
/// accurate to about 0.3 ms (ADR-0018). The thread sleeps until each poll is
/// due, so a sleep that runs long is made up by a shorter next one. A plain
/// sleep may be coarser on Windows; that needs checking on real Windows
/// hardware.
pub const POLL_EVERY: Duration = Duration::from_nanos(333_333);

/// SDL is one per program, so only one input thread may run at a time.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// The running input thread. Dropping it stops the thread and shuts SDL
/// down.
pub struct InputThread {
    batches: mpsc::Receiver<Batch>,
    stop: Arc<AtomicBool>,
    polls: Arc<AtomicU64>,
    started: Instant,
    sdl_version: String,
    handle: Option<JoinHandle<()>>,
}

impl InputThread {
    /// Starts SDL on its own thread. Fails, with SDL's reason, if SDL can't
    /// start, or if an input thread is already running.
    pub fn start() -> Result<InputThread, String> {
        if RUNNING.swap(true, Ordering::SeqCst) {
            return Err("an input thread is already running; SDL is one per program".into());
        }
        let started = Instant::now();
        let stop = Arc::new(AtomicBool::new(false));
        let polls = Arc::new(AtomicU64::new(0));
        let (batch_tx, batches) = mpsc::channel();
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let thread = {
            let stop = Arc::clone(&stop);
            let polls = Arc::clone(&polls);
            std::thread::Builder::new()
                .name("opendrone-input".into())
                .spawn(move || run(started, &stop, &polls, &batch_tx, &ready_tx))
        };
        let handle = match thread {
            Ok(handle) => handle,
            Err(error) => {
                RUNNING.store(false, Ordering::SeqCst);
                return Err(format!("the input thread couldn't start: {error}"));
            }
        };
        match ready.recv() {
            Ok(Ok(sdl_version)) => Ok(InputThread {
                batches,
                stop,
                polls,
                started,
                sdl_version,
                handle: Some(handle),
            }),
            Ok(Err(reason)) => {
                let _ = handle.join();
                RUNNING.store(false, Ordering::SeqCst);
                Err(reason)
            }
            Err(_) => {
                let _ = handle.join();
                RUNNING.store(false, Ordering::SeqCst);
                Err("the input thread stopped while SDL was starting".into())
            }
        }
    }

    /// Every batch read since the last call, oldest first.
    pub fn batches(&self) -> Vec<Batch> {
        self.batches.try_iter().collect()
    }

    /// The input thread's clock: the time since it started. Batches are
    /// stamped on this clock.
    pub fn now(&self) -> Duration {
        self.started.elapsed()
    }

    /// When the thread started, on the computer's clock: the zero of every
    /// stamp.
    pub fn started(&self) -> Instant {
        self.started
    }

    /// How many times the thread has read SDL so far.
    pub fn polls(&self) -> u64 {
        self.polls.load(Ordering::Relaxed)
    }

    /// SDL's version, such as "3.4.18".
    pub fn sdl_version(&self) -> &str {
        &self.sdl_version
    }
}

impl Drop for InputThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// One opened device.
struct Opened {
    joystick: *mut SDL_Joystick,
    gamepad: *mut SDL_Gamepad,
}

fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        // SAFETY: SDL hands back a valid, NUL-terminated string it owns,
        // which stays alive until the next SDL call on this thread.
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

fn run(
    started: Instant,
    stop: &AtomicBool,
    polls: &AtomicU64,
    batches: &mpsc::Sender<Batch>,
    ready: &mpsc::SyncSender<Result<String, String>>,
) {
    // SAFETY: plain SDL calls, made only on this thread, with static
    // NUL-terminated strings.
    let started_ok = unsafe {
        SDL_SetHint(SDL_HINT_NO_SIGNAL_HANDLERS, c"1".as_ptr());
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
        SDL_Init(SDL_INIT_GAMEPAD)
    };
    if !started_ok {
        let reason = text(SDL_GetError());
        let _ = ready.send(Err(format!("SDL couldn't start: {reason}")));
        return;
    }
    let version = SDL_GetVersion();
    let _ = ready.send(Ok(format!(
        "{}.{}.{}",
        SDL_VERSIONNUM_MAJOR(version),
        SDL_VERSIONNUM_MINOR(version),
        SDL_VERSIONNUM_MICRO(version)
    )));

    let mut opened: BTreeMap<u32, Opened> = BTreeMap::new();
    let mut buffer: Vec<MaybeUninit<SDL_Event>> = (0..256).map(|_| MaybeUninit::uninit()).collect();
    let mut due = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let mut events = Vec::new();
        // SAFETY: SDL is started, and every call stays on this thread.
        unsafe { SDL_UpdateJoysticks() };
        loop {
            // SAFETY: `buffer` has room for the number of events asked for;
            // SDL writes that many at most and says how many.
            let count = unsafe {
                SDL_PeepEvents(
                    buffer.as_mut_ptr().cast::<SDL_Event>(),
                    c_int::try_from(buffer.len()).unwrap_or(c_int::MAX),
                    SDL_GETEVENT,
                    SDL_EVENT_FIRST.0,
                    SDL_EVENT_LAST.0,
                )
            };
            let Ok(count) = usize::try_from(count) else {
                break;
            };
            for event in &buffer[..count] {
                // SAFETY: SDL wrote the first `count` events.
                let event = unsafe { event.assume_init_ref() };
                read_event(event, &mut opened, &mut events);
            }
            if count < buffer.len() {
                break;
            }
        }
        let at = started.elapsed();
        polls.fetch_add(1, Ordering::Relaxed);
        if !events.is_empty() && batches.send(Batch { at, events }).is_err() {
            break;
        }
        due += POLL_EVERY;
        let now = Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        } else if now - due > POLL_EVERY {
            // Far behind, as after plugging a device in: start the beat again
            // rather than polling in a burst.
            due = now;
        }
    }
    for device in opened.values() {
        close(device);
    }
    // SAFETY: every device is closed; SDL shuts down on the thread it
    // started on.
    unsafe { SDL_Quit() };
}

fn close(device: &Opened) {
    // SAFETY: each pointer came from SDL's open call and is closed once.
    unsafe {
        if !device.gamepad.is_null() {
            SDL_CloseGamepad(device.gamepad);
        } else if !device.joystick.is_null() {
            SDL_CloseJoystick(device.joystick);
        }
    }
}

/// Turns one SDL event into what [`Inputs`](crate::Inputs) reads.
fn read_event(event: &SDL_Event, opened: &mut BTreeMap<u32, Opened>, out: &mut Vec<Raw>) {
    // SAFETY: every SDL event starts with its type, and each arm reads the
    // part of the union that type says was written.
    unsafe {
        let kind = event.r#type;
        if kind == SDL_EVENT_JOYSTICK_ADDED.0 {
            let which = event.jdevice.which;
            if !opened.contains_key(&which.0)
                && let Some((device, raw)) = open(which)
            {
                opened.insert(which.0, device);
                out.push(raw);
            }
        } else if kind == SDL_EVENT_JOYSTICK_REMOVED.0 {
            let which = event.jdevice.which.0;
            if let Some(device) = opened.remove(&which) {
                close(&device);
                out.push(Raw::Removed {
                    device: SdlId(which),
                });
            }
        } else if kind == SDL_EVENT_JOYSTICK_AXIS_MOTION.0 {
            let e = event.jaxis;
            out.push(Raw::Axis {
                device: SdlId(e.which.0),
                axis: e.axis,
                value: e.value,
            });
        } else if kind == SDL_EVENT_JOYSTICK_BUTTON_DOWN.0 || kind == SDL_EVENT_JOYSTICK_BUTTON_UP.0
        {
            let e = event.jbutton;
            out.push(Raw::Button {
                device: SdlId(e.which.0),
                button: e.button,
                down: e.down,
            });
        } else if kind == SDL_EVENT_GAMEPAD_AXIS_MOTION.0 {
            let e = event.gaxis;
            if let Some(axis) = PadAxis::from_index(usize::from(e.axis)) {
                out.push(Raw::PadAxis {
                    device: SdlId(e.which.0),
                    axis,
                    value: e.value,
                });
            }
        } else if kind == SDL_EVENT_GAMEPAD_BUTTON_DOWN.0 || kind == SDL_EVENT_GAMEPAD_BUTTON_UP.0 {
            let e = event.gbutton;
            if let Some(button) = PadButton::from_index(usize::from(e.button)) {
                out.push(Raw::PadButton {
                    device: SdlId(e.which.0),
                    button,
                    down: e.down,
                });
            }
        } else if kind == SDL_EVENT_GAMEPAD_SENSOR_UPDATE.0 {
            let device = SdlId(event.gsensor.which.0);
            let report = Raw::Report { device };
            if !out.contains(&report) {
                out.push(report);
            }
        }
    }
}

/// Opens a device that was just plugged in: as a gamepad when SDL reads it
/// as one, otherwise as a plain joystick. Reads what it is and every
/// control's value now, since SDL reports only changes.
fn open(which: SDL_JoystickID) -> Option<(Opened, Raw)> {
    // SAFETY: `which` came from SDL's added event; every pointer is checked
    // before use, and every call stays on this thread.
    unsafe {
        let gamepad = if SDL_IsGamepad(which) {
            SDL_OpenGamepad(which)
        } else {
            std::ptr::null_mut()
        };
        let joystick = if gamepad.is_null() {
            SDL_OpenJoystick(which)
        } else {
            SDL_GetGamepadJoystick(gamepad)
        };
        if joystick.is_null() {
            return None;
        }
        let device = Opened { joystick, gamepad };
        let axes = usize::try_from(SDL_GetNumJoystickAxes(joystick)).unwrap_or(0);
        let buttons = usize::try_from(SDL_GetNumJoystickButtons(joystick)).unwrap_or(0);
        let mut state = DeviceState::new(axes, buttons, !gamepad.is_null());
        let mut heartbeat = false;
        for (i, value) in state.axes.iter_mut().enumerate() {
            *value = SDL_GetJoystickAxis(joystick, c_int::try_from(i).unwrap_or(0));
        }
        for (i, down) in state.buttons.iter_mut().enumerate() {
            *down = SDL_GetJoystickButton(joystick, c_int::try_from(i).unwrap_or(0));
        }
        if !gamepad.is_null() {
            let mut pad = PadState::at_rest();
            for axis in PadAxis::ALL {
                let index = c_int::try_from(axis.index()).unwrap_or(0);
                pad.axes[axis.index()] = SDL_GetGamepadAxis(gamepad, SDL_GamepadAxis(index));
            }
            for button in PadButton::ALL {
                let index = c_int::try_from(button.index()).unwrap_or(0);
                pad.buttons[button.index()] =
                    SDL_GetGamepadButton(gamepad, SDL_GamepadButton(index));
            }
            state.pad = Some(pad);
            // The motion sensors are the heartbeat of a pad that reports at
            // rest (#27); their readings themselves aren't used. Only a
            // heartbeat that really switched on lets silence count as lost.
            heartbeat = SDL_GamepadHasSensor(gamepad, SDL_SENSOR_GYRO)
                && SDL_SetGamepadSensorEnabled(gamepad, SDL_SENSOR_GYRO, true);
        }
        let info = DeviceInfo {
            name: text(SDL_GetJoystickName(joystick)),
            usb_vendor: SDL_GetJoystickVendor(joystick),
            usb_product: SDL_GetJoystickProduct(joystick),
            sdl_gamepad: !gamepad.is_null(),
            connection: connection(joystick),
            heartbeat,
        };
        Some((
            device,
            Raw::Added {
                device: SdlId(which.0),
                info,
                state,
            },
        ))
    }
}

/// USB or Bluetooth, from the bus SDL writes into the device's GUID.
///
/// # Safety
///
/// `joystick` must be an open joystick.
unsafe fn connection(joystick: *mut SDL_Joystick) -> Option<Connection> {
    // SAFETY: the caller passes an open joystick.
    let guid = unsafe { SDL_GetJoystickGUID(joystick) };
    match u16::from_le_bytes([guid.data[0], guid.data[1]]) {
        0x03 => Some(Connection::Usb),
        0x05 => Some(Connection::Bluetooth),
        _ => None,
    }
}
