//! PROTOTYPE (#34). Sticks from SDL 3.4 on its own thread (the approach proven in #18, copied
//! from the #28 camera prototype without its Bevy bits), so the Radiomaster Pocket or the
//! DualSense can drive the throttle. Keeps the latest raw value of every axis and button.

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct Device {
    pub name: String,
    pub is_gamepad: bool,
    /// -1..1 per axis (SDL gamepad axes for a gamepad, raw joystick axes otherwise).
    pub axes: Vec<f32>,
    pub buttons: Vec<bool>,
    pub connected: bool,
}

#[derive(Default)]
pub struct InputShared {
    pub devices: Vec<Device>,
    pub sdl_ok: Option<bool>,
    pub error: String,
}

#[derive(Clone, Default)]
pub struct SdlInput(pub Arc<Mutex<InputShared>>);

#[cfg(feature = "sdl")]
pub fn start() -> SdlInput {
    let shared = SdlInput::default();
    let s2 = shared.clone();
    std::thread::Builder::new().name("sdl-input".into()).spawn(move || run(s2)).expect("spawn sdl thread");
    shared
}

#[cfg(not(feature = "sdl"))]
pub fn start() -> SdlInput {
    let s = SdlInput::default();
    s.0.lock().unwrap().sdl_ok = Some(false);
    s.0.lock().unwrap().error = "built without SDL".into();
    s
}

#[cfg(feature = "sdl")]
fn run(shared: SdlInput) {
    use sdl3_sys::everything::*;
    use std::collections::HashMap;
    use std::ffi::CStr;
    use std::mem::MaybeUninit;

    unsafe {
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
        SDL_SetHint(SDL_HINT_NO_SIGNAL_HANDLERS, c"1".as_ptr());
    }
    let ok = unsafe { SDL_Init(SDL_INIT_GAMEPAD) };
    {
        let mut s = shared.0.lock().unwrap();
        s.sdl_ok = Some(ok);
        if !ok {
            s.error = unsafe { CStr::from_ptr(SDL_GetError()) }.to_string_lossy().into_owned();
            return;
        }
    }
    struct Open {
        joy: *mut SDL_Joystick,
        pad: *mut SDL_Gamepad,
        idx: usize,
    }
    let mut open: HashMap<u32, Open> = HashMap::new();
    let mut events: Vec<MaybeUninit<SDL_Event>> = Vec::with_capacity(256);
    unsafe { events.set_len(256) };
    loop {
        unsafe { SDL_UpdateJoysticks() };
        loop {
            let n = unsafe {
                SDL_PeepEvents(events.as_mut_ptr() as *mut SDL_Event, 256, SDL_GETEVENT, SDL_EVENT_FIRST.0, SDL_EVENT_LAST.0)
            };
            if n <= 0 {
                break;
            }
            let mut s = shared.0.lock().unwrap();
            for e in &events[..n as usize] {
                let e = unsafe { e.assume_init_ref() };
                let ty = unsafe { e.r#type };
                if ty == SDL_EVENT_JOYSTICK_ADDED.0 {
                    let ev = unsafe { e.jdevice };
                    if open.contains_key(&ev.which.0) {
                        continue;
                    }
                    let (joy, pad) = unsafe {
                        if SDL_IsGamepad(ev.which) {
                            let g = SDL_OpenGamepad(ev.which);
                            (if g.is_null() { std::ptr::null_mut() } else { SDL_GetGamepadJoystick(g) }, g)
                        } else {
                            (SDL_OpenJoystick(ev.which), std::ptr::null_mut())
                        }
                    };
                    if joy.is_null() {
                        continue;
                    }
                    let name = unsafe {
                        let p = SDL_GetJoystickName(joy);
                        if p.is_null() { "?".into() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
                    };
                    let n_axes = if pad.is_null() { unsafe { SDL_GetNumJoystickAxes(joy) }.max(0) as usize } else { 6 };
                    let n_buttons = unsafe { SDL_GetNumJoystickButtons(joy) }.max(0) as usize;
                    let mut axes = vec![0.0; n_axes];
                    if pad.is_null() {
                        for (i, a) in axes.iter_mut().enumerate() {
                            *a = unsafe { SDL_GetJoystickAxis(joy, i as i32) } as f32 / 32767.0;
                        }
                    }
                    eprintln!("[input] connected: {name} ({n_axes} axes, gamepad: {})", !pad.is_null());
                    s.devices.push(Device { name, is_gamepad: !pad.is_null(), axes, buttons: vec![false; n_buttons], connected: true });
                    open.insert(ev.which.0, Open { joy, pad, idx: s.devices.len() - 1 });
                } else if ty == SDL_EVENT_JOYSTICK_REMOVED.0 {
                    let ev = unsafe { e.jdevice };
                    if let Some(o) = open.remove(&ev.which.0) {
                        if let Some(d) = s.devices.get_mut(o.idx) {
                            d.connected = false;
                            d.axes.iter_mut().for_each(|a| *a = 0.0);
                        }
                        unsafe {
                            if !o.pad.is_null() {
                                SDL_CloseGamepad(o.pad);
                            } else {
                                SDL_CloseJoystick(o.joy);
                            }
                        }
                    }
                } else if ty == SDL_EVENT_JOYSTICK_AXIS_MOTION.0 {
                    let ev = unsafe { e.jaxis };
                    if let Some(o) = open.get(&ev.which.0) {
                        if o.pad.is_null() {
                            if let Some(a) = s.devices.get_mut(o.idx).and_then(|d| d.axes.get_mut(ev.axis as usize)) {
                                *a = (ev.value as f32 / 32767.0).clamp(-1.0, 1.0);
                            }
                        }
                    }
                } else if ty == SDL_EVENT_GAMEPAD_AXIS_MOTION.0 {
                    let ev = unsafe { e.gaxis };
                    if let Some(o) = open.get(&ev.which.0) {
                        if let Some(a) = s.devices.get_mut(o.idx).and_then(|d| d.axes.get_mut(ev.axis as usize)) {
                            *a = (ev.value as f32 / 32767.0).clamp(-1.0, 1.0);
                        }
                    }
                } else if ty == SDL_EVENT_JOYSTICK_BUTTON_DOWN.0 || ty == SDL_EVENT_JOYSTICK_BUTTON_UP.0 {
                    let ev = unsafe { e.jbutton };
                    if let Some(o) = open.get(&ev.which.0) {
                        if let Some(b) = s.devices.get_mut(o.idx).and_then(|d| d.buttons.get_mut(ev.button as usize)) {
                            *b = ev.down;
                        }
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Map a device to sticks. Radio: AETR on axes 0-3 (EdgeTX default), Arm on axis 4 (CH5).
/// Gamepad: Mode 2 (left stick throttle/yaw, right stick roll/pitch); throttle from the left stick
/// (spring-centred, so rest = 50%) or from R2.
pub struct Sticks {
    pub throttle: f32,
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    /// Radio's CH5 switch, if it has one.
    pub arm: Option<bool>,
}

pub fn read_sticks(d: &Device, gamepad_r2_throttle: bool) -> Sticks {
    let ax = |i: usize| d.axes.get(i).copied().unwrap_or(0.0);
    if d.is_gamepad {
        let throttle = if gamepad_r2_throttle { (ax(5) + 1.0) * 0.5 } else { ((-ax(1)) + 1.0) * 0.5 };
        Sticks { throttle: throttle.clamp(0.0, 1.0), roll: ax(2), pitch: -ax(3), yaw: ax(0), arm: None }
    } else {
        Sticks {
            throttle: ((ax(2) + 1.0) * 0.5).clamp(0.0, 1.0),
            roll: ax(0),
            pitch: ax(1),
            yaw: ax(3),
            arm: if d.axes.len() > 4 { Some(ax(4) > 0.3) } else { None },
        }
    }
}
