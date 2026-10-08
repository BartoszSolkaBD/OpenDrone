//! Readable checks for `cargo xtask migrate`, the format migration tool, run
//! as the real command. What a step does to the files is proved in
//! `opendrone-pack`'s and the Scenario runner's `tests/migration.rs`, with
//! synthetic steps; here, the command finds steps by name and lists them.

use std::path::Path;
use std::process::Command;

/// Runs `cargo xtask <args>` in the repo, and returns whether it passed and
/// what it printed.
fn xtask(args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .expect("xtask runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), text)
}

#[test]
fn on_its_own_the_command_says_each_format_and_lists_the_steps() {
    let (passed, text) = xtask(&["migrate"]);
    assert!(passed, "{text}");
    let first = text.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("Scenarios are format ")
            && first.contains(", and Pack files, Test Quads and Test Maps are format "),
        "{text}"
    );
    assert!(
        text.contains("There are no format migration steps yet.")
            || text.contains("The steps, each run with `cargo xtask migrate <step>`:"),
        "{text}"
    );
}

#[test]
fn a_step_that_doesnt_exist_is_refused_and_the_steps_are_listed() {
    let (passed, text) = xtask(&["migrate", "add-wind-assist"]);
    assert!(!passed);
    assert!(
        text.contains("There's no format migration step called \"add-wind-assist\"."),
        "{text}"
    );
    assert!(text.contains("Scenarios are format "), "{text}");
}
