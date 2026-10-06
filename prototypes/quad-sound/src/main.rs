//! PROTOTYPE (#34): "Does the live Quad sound feel right, on board and from where you stand?"
//! Throwaway. See README.md. Run with ./run.sh.

mod analyze;
mod audio;
mod beeps;
mod dsp;
mod input;
mod map;
mod offline;
mod paths;
mod quads;
mod script;
mod sim;
mod synth;
mod ui;

use std::path::{Path, PathBuf};

pub fn tuning_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tuning")
}

fn load_tuning(args: &[String]) -> quads::Tuning {
    if let Some(i) = args.iter().position(|a| a == "--load") {
        if let Some(f) = args.get(i + 1) {
            let p = if Path::new(f).exists() { PathBuf::from(f) } else { tuning_dir().join(f) };
            match quads::Tuning::load(&p) {
                Ok(t) => {
                    eprintln!("[tuning] loaded {}", p.display());
                    return t;
                }
                Err(e) => eprintln!("[tuning] could not load {}: {e}", p.display()),
            }
        }
    }
    let current = tuning_dir().join("current.toml");
    if current.exists() {
        if let Ok(t) = quads::Tuning::load(&current) {
            eprintln!("[tuning] loaded {}", current.display());
            return t;
        }
    }
    quads::Tuning::default()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tuning = load_tuning(&args);
    match args.first().map(|s| s.as_str()) {
        Some("render") => {
            let names: Vec<String> = args[1..].iter().filter(|a| !a.starts_with("--") && !a.ends_with(".toml")).cloned().collect();
            offline::render(&names, &tuning, &offline::default_out_dir());
        }
        Some("bench") => {
            let report = offline::bench(&tuning);
            println!("{report}");
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("results");
            std::fs::create_dir_all(&dir).ok();
            std::fs::write(dir.join("bench.md"), &report).ok();
            eprintln!("[bench] wrote results/bench.md");
        }
        Some("nodevice") => offline::nodevice(&tuning),
        Some("analyze") => analyze::analyze(&args[1..]),
        Some("defaults") => {
            let p = tuning_dir().join("defaults.toml");
            quads::Tuning::default().save(&p).unwrap();
            println!("wrote {}", p.display());
        }
        Some("cliplevels") => {
            for c in audio::load_manifest() {
                match audio::decode_clip(&c.file, 48000, c.group == "hits") {
                    Some(d) => {
                        let ch0 = &d[0];
                        let rms = (ch0.iter().map(|x| x * x).sum::<f32>() / ch0.len().max(1) as f32).sqrt();
                        let peak = ch0.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
                        println!("{:60} {:6.1} s  rms {:6.1} dBFS  peak {:6.1} dBFS", c.file, ch0.len() as f32 / 48000.0, 20.0 * rms.max(1e-9).log10(), 20.0 * peak.max(1e-9).log10());
                    }
                    None => println!("{:60} could not decode", c.file),
                }
            }
        }
        _ => {
            let no_device = args.iter().any(|a| a == "--no-device");
            // --selftest N: run N seconds silently (master 0), save a screenshot, quit.
            let selftest = args.iter().position(|a| a == "--selftest").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok());
            ui::run(tuning, no_device, selftest);
        }
    }
}
