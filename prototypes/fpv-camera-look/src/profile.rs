//! PROTOTYPE (#28). How Breakup builds up along the preset paths, at each Breakup level, without
//! rendering anything: the numbers behind "is there time to react?". Written by `--signal-profile`.

use crate::map::MapState;
use crate::paths::{paths_for, SampledPath, PATHS};
use crate::settings::{BreakupLevel, SignalTuning, Tuning};
use crate::signal::{breakup_from, path};
use std::fmt::Write as _;

/// Round 1's Video Signal values, for comparison.
pub fn round1(s: &SignalTuning) -> SignalTuning {
    SignalTuning {
        wall_db_per_m: 60.0,
        wall_max_m: 0.6,
        open_surface_m: 0.3,
        analog_clean_dbm: -78.0,
        analog_lost_dbm: -92.0,
        digital_perfect_dbm: -82.0,
        digital_lost_dbm: -93.0,
        flutter_db: 2.0,
        fresnel_m: 0.0,
        smoothing_s: 0.0,
        ..s.clone()
    }
}

struct Run {
    /// (time s, analog breakup, digital breakup) every step.
    samples: Vec<(f32, f32, f32)>,
}

fn simulate(map: &MapState, p: &SampledPath, s: &SignalTuning, tuning: &Tuning, level: BreakupLevel) -> Run {
    let rx = map.launch_pos + bevy::prelude::Vec3::Y * s.receiver_height_m;
    let mult = match level {
        BreakupLevel::Off => 1.0,
        BreakupLevel::Light => s.light_power_mult,
        BreakupLevel::Medium => s.medium_power_mult,
        BreakupLevel::Realistic => s.realistic_power_mult,
    };
    let tx = 10.0 * (tuning.vtx_mw() * mult).max(1e-6).log10();
    let mut t = 0.0f32;
    let mut d = 0.0f32;
    let mut level_dbm: Option<f32> = None;
    let mut out = Vec::new();
    while d < p.length() {
        let (pos, v) = p.at(d);
        let step = 0.2f32;
        let dt = step / v.max(0.3);
        let pp = path(&map.geom, pos, rx, s);
        let target = tx - pp.fspl_db - pp.wall_db;
        let lvl = match level_dbm {
            None => target,
            Some(prev) if s.smoothing_s > 0.0 => prev + (target - prev) * (dt / s.smoothing_s).min(1.0),
            Some(_) => target,
        };
        level_dbm = Some(lvl);
        let (mut a, mut dg) = breakup_from(lvl, s);
        if level == BreakupLevel::Light {
            a = a.min(s.light_cap_analog);
            dg = dg.min(s.light_cap_digital.min(0.99));
        }
        out.push((t, a, dg));
        t += dt;
        d += step;
    }
    Run { samples: out }
}

/// Share of flight time in each band, and the warning time before the picture is (nearly) gone.
fn describe(run: &Run, which: usize) -> String {
    let n = run.samples.len().max(1) as f32;
    let get = |s: &(f32, f32, f32)| if which == 0 { s.1 } else { s.2 };
    let mut bands = [0usize; 4];
    for s in &run.samples {
        let b = get(s);
        let i = if b < 0.1 {
            0
        } else if b < 0.5 {
            1
        } else if b < 0.9 {
            2
        } else {
            3
        };
        bands[i] += 1;
    }
    // Each time the picture goes (nearly) to nothing, how long since it was last fine?
    let mut warnings: Vec<f32> = Vec::new();
    let mut last_fine = 0.0f32;
    let mut was_gone = false;
    for s in &run.samples {
        let b = get(s);
        if b < 0.1 {
            last_fine = s.0;
        }
        let gone = b >= 0.9;
        if gone && !was_gone {
            warnings.push(s.0 - last_fine);
        }
        was_gone = gone;
    }
    let warn = if warnings.is_empty() {
        "never".to_string()
    } else {
        let min = warnings.iter().cloned().fold(f32::MAX, f32::min);
        format!("{} times, warning {:.1} s at the shortest", warnings.len(), min)
    };
    format!(
        "{:.0}% / {:.0}% / {:.0}% / {:.0}% | {}",
        bands[0] as f32 / n * 100.0,
        bands[1] as f32 / n * 100.0,
        bands[2] as f32 / n * 100.0,
        bands[3] as f32 / n * 100.0,
        warn
    )
}

/// Spots to check by hand (Blender coordinates from the #17 scripts).
const BANDO_SPOTS: &[(&str, f32, f32, f32)] = &[
    ("Yard, 10 m from the building", 0.0, -12.0, 2.0),
    ("Ground floor, open east half", 6.0, -3.0, 1.5),
    ("Ground floor SE room (behind its south wall)", -3.0, -5.0, 1.3),
    ("Ground floor SW room", -9.0, -5.0, 1.3),
    ("Ground floor corridor, middle", -6.0, 0.0, 1.3),
    ("Ground floor NW room (2 rooms in)", -9.0, 5.0, 1.3),
    ("Lift shaft, bottom", 1.5, 0.0, 1.0),
    ("Floor 1 NW room", -9.0, 5.0, 4.7),
    ("Floor 2, north side", 3.0, 6.0, 8.3),
    ("North yard, behind the building", 0.0, 15.0, 2.0),
    ("Roof", 0.0, 0.0, 18.5),
];

fn spots(map: &MapState, tuning: &Tuning, md: &mut String) {
    let s2 = tuning.signal.clone();
    let s1 = round1(&s2);
    let rx = map.launch_pos + bevy::prelude::Vec3::Y * s2.receiver_height_m;
    let _ = writeln!(md, "Spot check: straight-line walls, received level on Realistic, and Breakup (Analog / Digital, 0-100%) at Light, Medium and Realistic. Round 1 -> Round 2.\n");
    let _ = writeln!(md, "| Spot | Walls on the line | Level dBm R1 -> R2 | Light | Medium | Realistic |");
    let _ = writeln!(md, "|---|---|---|---|---|---|");
    for &(name, x, y, z) in BANDO_SPOTS {
        let pos = crate::map::b(x, y, z);
        let mut cells: Vec<String> = Vec::new();
        let mut lvl = [0.0f32; 2];
        let mut walls = 0;
        for (k, s) in [&s1, &s2].iter().enumerate() {
            let p = path(&map.geom, pos, rx, s);
            walls = p.walls + p.open_surfaces;
            lvl[k] = 10.0 * tuning.vtx_mw().log10() - p.fspl_db - p.wall_db;
        }
        for level in [BreakupLevel::Light, BreakupLevel::Medium, BreakupLevel::Realistic] {
            let mut parts = Vec::new();
            for (k, s) in [&s1, &s2].iter().enumerate() {
                let mult = match level {
                    BreakupLevel::Light => s.light_power_mult,
                    BreakupLevel::Medium => s.medium_power_mult,
                    _ => s.realistic_power_mult,
                };
                let (mut a, mut d) = breakup_from(lvl[k] + 10.0 * mult.log10(), s);
                if level == BreakupLevel::Light {
                    a = a.min(s.light_cap_analog);
                    d = d.min(s.light_cap_digital);
                }
                parts.push(format!("{:.0}/{:.0}", a * 100.0, d * 100.0));
            }
            cells.push(parts.join(" -> "));
        }
        let _ = writeln!(md, "| {name} | {walls} | {:.0} -> {:.0} | {} | {} | {} |", lvl[0], lvl[1], cells[0], cells[1], cells[2]);
    }
    let _ = writeln!(md);
}

pub fn report(map: &MapState, tuning: &Tuning) -> String {
    let mut md = String::new();
    let cur = map.current.unwrap_or_default();
    if cur == crate::settings::MapKind::Bando {
        spots(map, tuning, &mut md);
    }
    let _ = writeln!(md, "## {} (Whoop 65, {} mW VTX)\n", cur.label(), tuning.signal.vtx_mw_whoop);
    let _ = writeln!(md, "Share of the path's flight time that is clean / noisy (<50%) / heavy (50-90%) / (nearly) gone (90%+), and how much warning the pilot gets: the time from the picture last being clean (<10%) to it being (nearly) gone.\n");
    let _ = writeln!(md, "| Path | Level | Look | Round 1 settings | Round 2 settings |");
    let _ = writeln!(md, "|---|---|---|---|---|");
    let r1 = round1(&tuning.signal);
    for idx in paths_for(cur) {
        let p = SampledPath::new(&PATHS[idx]);
        for level in [BreakupLevel::Light, BreakupLevel::Medium, BreakupLevel::Realistic] {
            let a1 = simulate(map, &p, &r1, tuning, level);
            let a2 = simulate(map, &p, &tuning.signal, tuning, level);
            for (look, which) in [("Analog", 0usize), ("Digital", 1usize)] {
                let _ = writeln!(
                    md,
                    "| {} | {} | {} | {} | {} |",
                    PATHS[idx].name,
                    level.label(),
                    look,
                    describe(&a1, which),
                    describe(&a2, which)
                );
            }
        }
    }
    md
}
