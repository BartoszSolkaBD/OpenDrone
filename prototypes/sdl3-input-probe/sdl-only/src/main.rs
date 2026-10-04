//! PROTOTYPE (issue #18): initialise SDL 3.4's gamepad subsystem on a spawned thread, list the
//! Input Devices it sees, poll for a moment, quit. Proves the from-source static build links and
//! runs on this OS. No window, no devices required.
use sdl3_sys::everything::*;
use std::ffi::CStr;

fn main() {
    let h = std::thread::Builder::new()
        .name("sdl-input".into())
        .spawn(|| unsafe {
            let v = SDL_GetVersion();
            println!(
                "SDL {}.{}.{} on thread {:?}",
                SDL_VERSIONNUM_MAJOR(v),
                SDL_VERSIONNUM_MINOR(v),
                SDL_VERSIONNUM_MICRO(v),
                std::thread::current().name()
            );
            if !SDL_Init(SDL_INIT_GAMEPAD) {
                let e = CStr::from_ptr(SDL_GetError()).to_string_lossy();
                println!("SDL_Init(GAMEPAD) FAILED: {e}");
                std::process::exit(1);
            }
            let t = std::time::Instant::now();
            let mut polls = 0u32;
            while t.elapsed() < std::time::Duration::from_millis(500) {
                SDL_UpdateJoysticks();
                polls += 1;
                std::thread::sleep(std::time::Duration::from_micros(250));
            }
            let mut n = 0;
            let ids = SDL_GetJoysticks(&mut n);
            println!("SDL_Init(GAMEPAD) OK; {polls} polls in 0.5 s; {n} joystick(s) present");
            for i in 0..n as usize {
                let name = SDL_GetJoystickNameForID(*ids.add(i));
                if !name.is_null() {
                    println!("  - {}", CStr::from_ptr(name).to_string_lossy());
                }
            }
            SDL_free(ids as *mut _);
            SDL_Quit();
        })
        .unwrap();
    h.join().unwrap();
}
