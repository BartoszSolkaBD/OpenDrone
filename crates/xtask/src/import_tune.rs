//! `cargo xtask import-tune`: imports a quad's Betaflight `diff all` (or
//! `diff`, or `dump`, from Betaflight 4.3 or newer) as a Quad definition's
//! `tune.txt`, in Betaflight 2026.6's names with every line marked
//! ([ADR-0008], [ADR-0015]).
//!
//! The translating is the Flight Controller's Betaflight CLI translator,
//! `opendrone_flight_controller::cli::import_tune`, which opens no files;
//! this reads the export and the Quad's name, writes the Tune, and prints
//! what was translated and left out. The Whoop 65's Tune is made this way
//! from the Meteor65 Pro's `diff all`:
//!
//! ```sh
//! cargo xtask import-tune docs/research/quad-settings/meteor65-pro.diff-all.txt packs/opendrone/quads/whoop-65
//! ```
//!
//! [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use opendrone_flight_controller::cli::import_tune;
use opendrone_pack::read_quad_file;

use crate::packs::repo;

pub const USAGE: &str = "Usage: cargo xtask import-tune <export file> (<quad folder> | --print)";

pub fn run(args: &[String]) -> ExitCode {
    let [export, target] = args else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match import(Path::new(export), target) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn import(export: &Path, target: &str) -> Result<ExitCode, String> {
    let root = repo()?;
    let text =
        fs::read_to_string(export).map_err(|e| format!("Can't read {}: {e}", export.display()))?;
    let imported = match import_tune(&text) {
        Ok(imported) => imported,
        Err(refusal) => {
            println!("{} can't be imported:\n{refusal}", export.display());
            return Ok(ExitCode::FAILURE);
        }
    };
    let source = shown(&root, export);
    if target == "--print" {
        let name = imported
            .craft_name
            .clone()
            .unwrap_or_else(|| "This quad".into());
        print!("{}", imported.tune_txt(&name, &source));
        eprint!("\n{}", imported.report());
        return Ok(ExitCode::SUCCESS);
    }
    let folder = PathBuf::from(target);
    let quad_toml = folder.join("quad.toml");
    let quad_text = fs::read_to_string(&quad_toml)
        .map_err(|e| format!("Can't read {}: {e}", quad_toml.display()))?;
    let quad = read_quad_file(&shown(&root, &quad_toml), &quad_text)
        .map_err(|problems| format!("{} can't be read:\n{problems}", quad_toml.display()))?;
    let tune = folder.join("tune.txt");
    fs::write(&tune, imported.tune_txt(&quad.name, &source))
        .map_err(|e| format!("Can't write {}: {e}", tune.display()))?;
    println!("Wrote {}.", shown(&root, &tune));
    print!("{}", imported.report());
    if imported.problems.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        println!(
            "The Pack checker refuses this Tune until those are fixed: change each by hand and mark it `hand-set: <reason>`."
        );
        Ok(ExitCode::FAILURE)
    }
}

/// A path as the repo names it, with forward slashes, when it's inside the
/// repo.
fn shown(root: &Path, path: &Path) -> String {
    let full = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let relative = full.strip_prefix(&root).unwrap_or(path);
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
