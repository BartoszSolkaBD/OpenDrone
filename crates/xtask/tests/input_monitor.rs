//! `cargo xtask input-monitor` starts SDL on its own thread and reports what
//! it sees, running the real command. CI's machines have no Input Device, so
//! there it says so plainly; on a desk with a device plugged in it shows it.
//! Basis: Rule (#61).

use std::path::Path;
use std::process::Command;

#[test]
fn the_input_monitor_starts_sdl_and_says_plainly_what_is_connected() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["input-monitor", "--seconds", "1"])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .expect("xtask runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(
        text.starts_with("Watching Input Devices through SDL 3.4."),
        "{text}"
    );
    assert!(
        text.contains("No Input Device is connected. Plug in a Radio or a Gamepad")
            || text.contains("] Found "),
        "{text}"
    );
}

#[test]
fn the_input_monitor_refuses_a_time_that_isnt_a_number() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["input-monitor", "--seconds", "soon"])
        .output()
        .expect("xtask runs");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--seconds takes a number"));
}
