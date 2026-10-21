//! Real flight logs, turned into what Physics Scenarios play back: a motor
//! Input Track of the recorded motor commands, and a recorded flight to
//! compare the run with (`docs/data-policy.md`,
//! `docs/verification/reading-a-scenario.md`).
//!
//! - `tii-logs <folder>`: TII's *Race Against the Machine* flights (data
//!   CC BY 4.0), from `<folder>`, where TII's own downloader put them. It
//!   writes the trimmed CSV files the Freestyle 5″'s Scenarios in
//!   `scenarios/quads/freestyle-5/tii-logs/` play back, which are committed
//!   with their attribution notice beside them.
//! - `download neurobem`: downloads one flight of UZH's NeuroBEM dataset,
//!   which states no licence, so it is never committed, and writes the files
//!   its Scenarios in `scenarios/quads/freestyle-5/neurobem/` play back,
//!   beside them, where git ignores them. Those Scenarios skip while the
//!   files are missing.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use opendrone_maths::{DEGREE, Vec3};

/// A flight log's rows, by column name.
struct Table {
    names: Vec<String>,
    rows: Vec<Vec<f64>>,
}

impl Table {
    /// Reads a CSV file whose first line names its columns. Cells that
    /// aren't numbers (TII's picture file names) read as nothing.
    fn read(path: &Path) -> Result<Table, String> {
        let text =
            fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
        Table::parse(&path.display().to_string(), &text)
    }

    /// The same, from a file's text, naming it `name` in problems.
    fn parse(name: &str, text: &str) -> Result<Table, String> {
        let mut lines = text.lines();
        let names: Vec<String> = lines
            .next()
            .ok_or_else(|| format!("{name} is empty"))?
            .split(',')
            .map(|name| name.trim().to_string())
            .collect();
        let rows = lines
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                line.split(',')
                    .map(|cell| cell.trim().parse().unwrap_or(f64::NAN))
                    .collect()
            })
            .collect();
        Ok(Table { names, rows })
    }

    fn column(&self, name: &str) -> Result<usize, String> {
        self.names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| format!("the log has no `{name}` column"))
    }
}

/// One stretch of a flight that a Scenario plays back.
pub struct Window {
    /// The Scenario's file name without `.toml`; the CSV files are
    /// `<name>-motors.csv` and `<name>-recorded.csv`.
    pub name: &'static str,
    /// The flight, as TII names it.
    pub flight: &'static str,
    /// Where the window starts in the flight, in seconds of TII's
    /// `elapsed_time`, and how long it lasts.
    pub start: f64,
    pub length: f64,
}

/// The windows of TII's flights the Freestyle 5″'s Scenarios play back:
/// a still hover, a punch-out and two stretches slowing down, from each of
/// two runs of the same autonomous ellipse.
pub const TII_WINDOWS: &[Window] = &[
    Window {
        name: "hovering-in-flight-01a",
        flight: "flight-01a-ellipse",
        start: 3.0,
        length: 0.6,
    },
    Window {
        name: "punching-out-in-flight-01a",
        flight: "flight-01a-ellipse",
        start: 9.9,
        length: 0.3,
    },
    Window {
        name: "slowing-from-5-m-s-in-flight-01a",
        flight: "flight-01a-ellipse",
        start: 15.3,
        length: 0.2,
    },
    Window {
        name: "slowing-from-3-m-s-in-flight-01a",
        flight: "flight-01a-ellipse",
        start: 15.6,
        length: 0.2,
    },
    Window {
        name: "hovering-in-flight-04a",
        flight: "flight-04a-ellipse",
        start: 6.0,
        length: 0.6,
    },
    Window {
        name: "punching-out-in-flight-04a",
        flight: "flight-04a-ellipse",
        start: 10.9,
        length: 0.3,
    },
    Window {
        name: "slowing-from-5-m-s-in-flight-04a",
        flight: "flight-04a-ellipse",
        start: 16.2,
        length: 0.2,
    },
    Window {
        name: "slowing-from-3-m-s-in-flight-04a",
        flight: "flight-04a-ellipse",
        start: 16.5,
        length: 0.2,
    },
];

/// Where in each TII flight the quad hovers still: the motor commands' trim
/// and the accelerometer's offsets are read there.
const TII_STILL_HOVER: (f64, f64) = (3.0, 6.0);

/// The attribution every TII file carries (CC BY 4.0).
const TII_CREDIT: &str = "\
# From TII Racing's \"Race Against the Machine\" dataset, v3.0.0
# (https://github.com/tii-racing/drone-racing-dataset), by Bosello et al.,
# licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
# Trimmed and converted by `cargo xtask tii-logs`: see NOTICE.md beside it.";

/// `cargo xtask tii-logs <folder>`.
pub fn run_tii(args: &[String]) -> ExitCode {
    let [folder] = args else {
        eprintln!(
            "Usage: cargo xtask tii-logs <folder>\n\n<folder> holds TII's flights as their downloader unpacks them (data_downloader.sh in https://github.com/tii-racing/drone-racing-dataset): each flight's <flight>_500hz_freq_sync.csv somewhere inside it."
        );
        return ExitCode::from(2);
    };
    match write_tii(Path::new(folder)) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn write_tii(folder: &Path) -> Result<String, String> {
    let root = crate::materials::repo()?;
    let out = root.join("scenarios/quads/freestyle-5/tii-logs");
    fs::create_dir_all(&out).map_err(|e| format!("can't make {}: {e}", out.display()))?;
    let mut report = String::new();
    let mut flight: Option<(&str, TiiFlight)> = None;
    for window in TII_WINDOWS {
        if flight
            .as_ref()
            .is_none_or(|(name, _)| *name != window.flight)
        {
            let file = find(folder, &format!("{}_500hz_freq_sync.csv", window.flight))?;
            flight = Some((window.flight, TiiFlight::read(&file)?));
        }
        let (_, log) = flight.as_ref().expect("read just above");
        let (motors, recorded, start) = log.window(window)?;
        for (ending, text) in [("motors", &motors), ("recorded", &recorded)] {
            let path = out.join(format!("{}-{ending}.csv", window.name));
            fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        }
        let _ = writeln!(report, "{}: its Scenario starts\n{start}", window.name);
    }
    Ok(report)
}

/// The first file called `name` in `folder` or the folders inside it, in
/// name order.
fn find(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(folder)
        .map_err(|e| format!("can't read {}: {e}", folder.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for entry in &entries {
        if entry.file_name().is_some_and(|n| n == name) {
            return Ok(entry.clone());
        }
    }
    for entry in entries.iter().filter(|e| e.is_dir()) {
        if let Ok(found) = find(entry, name) {
            return Ok(found);
        }
    }
    Err(format!("there's no {name} in {}", folder.display()))
}

/// One TII flight at 500 Hz, in OpenDrone's axes.
struct TiiFlight {
    rows: Vec<TiiRow>,
    /// Each motor's command over the still hover, as a share of the four's
    /// mean: the trim the Flight Controller gave this quad to hold it level
    /// and straight.
    trim: [f64; 4],
    /// The accelerometer's forward and left reading in the still hover,
    /// where the quad's true felt acceleration has neither: it sits about a
    /// degree off the thrust's axis.
    offset: [f64; 2],
}

struct TiiRow {
    time: f64,
    /// Each motor's command, from 0 to 1, in Betaflight's motor order.
    motors: [f64; 4],
    volts: f64,
    /// East, north and up, in metres and m/s (TII's motion capture: x, y, z).
    position: Vec3,
    velocity: Vec3,
    /// The body's forward, left and up axes in the world's (east, north,
    /// up).
    axes: [Vec3; 3],
    /// The gyro, in body axes (forward, left, up), in rad/s.
    gyro: Vec3,
    /// The accelerometer, in body axes, in m/s².
    felt: Vec3,
}

impl TiiFlight {
    fn read(path: &Path) -> Result<TiiFlight, String> {
        let table = Table::read(path)?;
        let col = |name: &str| table.column(name);
        let time = col("elapsed_time")?;
        let motors = [
            col("thrust[0]")?,
            col("thrust[1]")?,
            col("thrust[2]")?,
            col("thrust[3]")?,
        ];
        let volts = col("vbat")?;
        let position = [col("drone_x")?, col("drone_y")?, col("drone_z")?];
        let velocity = [
            col("drone_velocity_linear_x")?,
            col("drone_velocity_linear_y")?,
            col("drone_velocity_linear_z")?,
        ];
        let mut rot = [0; 9];
        for (k, c) in rot.iter_mut().enumerate() {
            *c = col(&format!("drone_rot[{k}]"))?;
        }
        let gyro = [col("gyro_x")?, col("gyro_y")?, col("gyro_z")?];
        let accel = [col("accel_x")?, col("accel_y")?, col("accel_z")?];
        let v3 = |row: &[f64], c: [usize; 3]| Vec3::new(row[c[0]], row[c[1]], row[c[2]]);
        let rows: Vec<TiiRow> = table
            .rows
            .iter()
            .map(|row| TiiRow {
                time: row[time],
                motors: motors.map(|c| row[c]),
                volts: row[volts],
                position: v3(row, position),
                velocity: v3(row, velocity),
                // TII writes the body-to-world turn's matrix column by
                // column (its data_interpolation.py transposes it), so each
                // three numbers are one body axis in the world.
                axes: [
                    Vec3::new(row[rot[0]], row[rot[1]], row[rot[2]]),
                    Vec3::new(row[rot[3]], row[rot[4]], row[rot[5]]),
                    Vec3::new(row[rot[6]], row[rot[7]], row[rot[8]]),
                ],
                gyro: v3(row, gyro),
                felt: v3(row, accel),
            })
            .collect();
        let still: Vec<&TiiRow> = rows
            .iter()
            .filter(|r| r.time >= TII_STILL_HOVER.0 && r.time < TII_STILL_HOVER.1)
            .collect();
        if still.is_empty() {
            return Err(format!(
                "{} has no still hover from {} s to {} s",
                path.display(),
                TII_STILL_HOVER.0,
                TII_STILL_HOVER.1
            ));
        }
        let n = still.len() as f64;
        let mean_motor: [f64; 4] =
            core::array::from_fn(|k| still.iter().map(|r| r.motors[k]).sum::<f64>() / n);
        let all = mean_motor.iter().sum::<f64>() / 4.0;
        Ok(TiiFlight {
            trim: mean_motor.map(|m| all / m),
            offset: [
                still.iter().map(|r| r.felt.x).sum::<f64>() / n,
                still.iter().map(|r| r.felt.y).sum::<f64>() / n,
            ],
            rows,
        })
    }

    /// The window's motor Input Track, its recorded flight, and the lines of
    /// its Scenario's starting state.
    fn window(&self, window: &Window) -> Result<(String, String, String), String> {
        let rows: Vec<&TiiRow> = self
            .rows
            .iter()
            .filter(|r| {
                r.time >= window.start - 1e-6 && r.time <= window.start + window.length + 1e-6
            })
            .collect();
        let first = rows
            .first()
            .ok_or_else(|| format!("{} has no rows at {} s", window.flight, window.start))?;
        let at = |r: &TiiRow| ((r.time - first.time) * 500.0).round() / 500.0;

        let mut motors = format!(
            "{TII_CREDIT}\n#\n# {} from {} s to {} s: each motor's command as Betaflight sent it, times\n# its trim, its share of the four's mean in the still hover from {} s to {} s\n# ({}), which held this quad level and straight. The Scenario's Test Quad\n# is level and straight with all four equal.\n{}\n",
            window.flight,
            fixed(window.start, 3),
            fixed(window.start + window.length, 3),
            TII_STILL_HOVER.0,
            TII_STILL_HOVER.1,
            self.trim
                .iter()
                .map(|t| fixed(*t, 4))
                .collect::<Vec<_>>()
                .join(", "),
            opendrone_scenario::MOTOR_TRACK_HEADER
        );
        let mut last: Option<[String; 4]> = None;
        for r in &rows {
            let now: [String; 4] = core::array::from_fn(|k| {
                fixed((r.motors[k] * self.trim[k]).clamp(0.0, 1.0) * 100.0, 2)
            });
            let cells: Vec<String> = (0..4)
                .map(|k| match &last {
                    Some(held) if held[k] == now[k] => String::new(),
                    _ => now[k].clone(),
                })
                .collect();
            if last.is_some() && cells.iter().all(String::is_empty) {
                continue;
            }
            let _ = writeln!(motors, "{},{}", fixed(at(r), 3), cells.join(","));
            last = Some(now);
        }

        let mut recorded = format!(
            "{TII_CREDIT}\n#\n# {} from {} s to {} s: what the quad's accelerometer felt, in its own axes\n# (forward, left, up), after Betaflight's 10 Hz low-pass, so about 22 ms\n# late, less its forward and left reading in the still hover ({} and {} m/s²),\n# where it should read none; and the battery's voltage, to 0.1 V.\ntime (s),felt acceleration forward (m/s²),felt acceleration left (m/s²),felt acceleration up (m/s²),battery voltage (V)\n",
            window.flight,
            fixed(window.start, 3),
            fixed(window.start + window.length, 3),
            fixed(self.offset[0], 3),
            fixed(self.offset[1], 3),
        );
        for r in &rows {
            let _ = writeln!(
                recorded,
                "{},{},{},{},{}",
                fixed(at(r), 3),
                fixed(r.felt.x - self.offset[0], 3),
                fixed(r.felt.y - self.offset[1], 3),
                fixed(r.felt.z, 3),
                fixed(r.volts, 2)
            );
        }

        Ok((motors, recorded, starting_state(first)))
    }
}

/// A row's place, attitude, speed and rotation, as a Scenario's `[start]`
/// writes them.
fn starting_state(r: &TiiRow) -> String {
    let [forward, left, up] = r.axes;
    // Heading, then pitch, then roll, as pilots read them: the nose's
    // compass direction, how far it points up, and how far the left side
    // is up (right side down).
    let pitch = opendrone_maths::functions::asin(forward.z.clamp(-1.0, 1.0));
    let heading = opendrone_maths::functions::atan2(forward.x, forward.y);
    let roll = opendrone_maths::functions::atan2(left.z, up.z);
    let heading = (heading / DEGREE).rem_euclid(360.0);
    format!(
        "position = \"{} m east, {} m north, {} m up\"\nattitude = \"roll {}°, pitch {}°, heading {}°\"\nspeed    = \"{} m/s east, {} m/s north, {} m/s up\"\nrotation = \"roll {} °/s, pitch {} °/s, yaw {} °/s\"\n",
        fixed(r.position.x, 3),
        fixed(r.position.y, 3),
        fixed(r.position.z, 3),
        fixed(roll / DEGREE, 2),
        fixed(pitch / DEGREE, 2),
        fixed(heading, 2),
        fixed(r.velocity.x, 3),
        fixed(r.velocity.y, 3),
        fixed(r.velocity.z, 3),
        fixed(r.gyro.x / DEGREE, 1),
        fixed(-r.gyro.y / DEGREE, 1),
        fixed(-r.gyro.z / DEGREE, 1),
    )
}

/// Where NeuroBEM's processed flights are published (no licence stated).
const NEUROBEM_URL: &str = "https://download.ifi.uzh.ch/rpg/NeuroBEM/processed_data.zip";

/// The flight its Scenarios play back: the second part of a CPC race
/// trajectory flown at a thrust-to-weight of 2.475, as NeuroBEM's
/// Flights.txt lists 2021-02-05-15-46-49.
const NEUROBEM_SEGMENT: &str = "processed_data/merged_2021-02-05-15-46-49_seg_2.csv";

/// The Test Quad that stands in for NeuroBEM's quad.
const NEUROBEM_QUAD: &str = "test/freestyle-5-neurobem-quad";

/// The battery voltage the Test Quad's bench-like pack holds: four cells at
/// 4.00 V each at its 50% charge, with no resistance.
const NEUROBEM_VOLTS: f64 = 16.0;

/// The windows of that flight NeuroBEM's Scenarios play back. Each starts
/// in a still hover, so its Scenario's starting state ("level, still") is
/// nothing taken from the data.
pub const NEUROBEM_WINDOWS: &[Window] = &[Window {
    name: "rolling-out-of-a-hover",
    flight: "2021-02-05-15-46-49",
    start: 9.785,
    length: 0.5,
}];

/// How long before each window its still hover is read, for the motors'
/// trim.
const NEUROBEM_TRIM_BEFORE: f64 = 0.2;

/// The attribution every downloaded NeuroBEM file carries.
const NEUROBEM_CREDIT: &str = "\
# From UZH's NeuroBEM dataset (https://rpg.ifi.uzh.ch/NeuroBEM.html), by
# Bauersfeld, Kaufmann, Foehn and Scaramuzza, RSS 2021. It states no licence,
# so it is never committed: `cargo xtask download neurobem` made this file,
# and git ignores it (docs/data-policy.md).";

/// `cargo xtask download neurobem [--zip <processed_data.zip>]`.
pub fn run_download(args: &[String]) -> ExitCode {
    let zip = match args {
        [data] if data == "neurobem" => None,
        [data, flag, zip] if data == "neurobem" && flag == "--zip" => Some(PathBuf::from(zip)),
        _ => {
            eprintln!(
                "Usage: cargo xtask download neurobem [--zip <processed_data.zip>]\n\nDownloads NeuroBEM's processed flights ({NEUROBEM_URL}, about 630 MB) into target/downloads/neurobem/, unless --zip names a copy already here, and writes the files its Scenarios in scenarios/quads/freestyle-5/neurobem/ play back. NeuroBEM states no licence, so git ignores them (docs/data-policy.md)."
            );
            return ExitCode::from(2);
        }
    };
    match write_neurobem(zip) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn write_neurobem(zip: Option<PathBuf>) -> Result<String, String> {
    let root = crate::materials::repo()?;
    let zip = match zip {
        Some(zip) => zip,
        None => {
            let zip = root.join("target/downloads/neurobem/processed_data.zip");
            if !zip.is_file() {
                println!("Downloading {NEUROBEM_URL} (about 630 MB)…");
                crate::textures::download(NEUROBEM_URL, &zip)?;
            }
            zip
        }
    };
    let file = fs::File::open(&zip).map_err(|e| format!("can't open {}: {e}", zip.display()))?;
    let mut archive = ::zip::ZipArchive::new(file)
        .map_err(|e| format!("{} isn't a zip archive NeuroBEM made: {e}", zip.display()))?;
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(NEUROBEM_SEGMENT)
            .map_err(|e| format!("{} has no {NEUROBEM_SEGMENT}: {e}", zip.display()))?,
        &mut text,
    )
    .map_err(|e| format!("can't read {NEUROBEM_SEGMENT}: {e}"))?;
    let table = Table::parse(NEUROBEM_SEGMENT, &text)?;

    let repo = opendrone_scenario::Repo { root: root.clone() };
    let packs = repo
        .packs()
        .map_err(|p| format!("the Packs can't be read: {p}"))?;
    let quad = packs
        .quad(NEUROBEM_QUAD)
        .map_err(|p| format!("the Test Quad {NEUROBEM_QUAD} can't be read: {p}"))?;
    let air = packs
        .map("test/empty-air")
        .map_err(|p| format!("the Test Map test/empty-air can't be read: {p}"))?
        .world
        .air_density;
    let drive = |speed: f64, change: f64| {
        opendrone_physics::drive_holding(
            &quad.parameters.motors,
            &quad.parameters.props,
            air,
            speed,
            change,
            NEUROBEM_VOLTS,
        )
    };

    let out = root.join("scenarios/quads/freestyle-5/neurobem");
    let mut report = String::new();
    for window in NEUROBEM_WINDOWS {
        let (motors, recorded) = neurobem_window(&table, window, &drive)?;
        for (ending, text) in [("motors", &motors), ("recorded", &recorded)] {
            let path = out.join(format!("{}-{ending}.csv", window.name));
            fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        }
        let _ = writeln!(
            report,
            "Wrote {}'s files in {}.",
            window.name,
            out.display()
        );
    }
    Ok(report)
}

/// A NeuroBEM window's motor Input Track and recorded flight.
fn neurobem_window(
    table: &Table,
    window: &Window,
    drive: &dyn Fn(f64, f64) -> f64,
) -> Result<(String, String), String> {
    let col = |name: &str| table.column(name);
    let t = col("t")?;
    let speeds = [col("mot 1")?, col("mot 2")?, col("mot 3")?, col("mot 4")?];
    let changes = [
        col("dmot 1")?,
        col("dmot 2")?,
        col("dmot 3")?,
        col("dmot 4")?,
    ];
    let rates = [col("ang vel x")?, col("ang vel y")?, col("ang vel z")?];
    let felt = [col("acc x")?, col("acc y")?, col("acc z")?];
    let rows_in = |from: f64, to: f64| -> Vec<&Vec<f64>> {
        table
            .rows
            .iter()
            .filter(|r| r[t] >= from - 1e-6 && r[t] <= to + 1e-6)
            .collect()
    };
    // The trim: each motor's thrust (its speed squared) in the still hover
    // just before the window, as a share of the four's mean, taken back off
    // its speed, as the TII files take the trim off the commands.
    let still = rows_in(window.start - NEUROBEM_TRIM_BEFORE, window.start);
    if still.is_empty() {
        return Err(format!(
            "NeuroBEM's flight has no rows before {} s",
            window.start
        ));
    }
    let n = still.len() as f64;
    let squares: [f64; 4] = core::array::from_fn(|k| {
        still
            .iter()
            .map(|r| r[speeds[k]] * r[speeds[k]])
            .sum::<f64>()
            / n
    });
    let mean = squares.iter().sum::<f64>() / 4.0;
    let trim: [f64; 4] = squares.map(|s| (mean / s).sqrt());

    let rows = rows_in(window.start, window.start + window.length);
    let first = rows
        .first()
        .ok_or_else(|| format!("NeuroBEM's flight has no rows at {} s", window.start))?;
    // NeuroBEM's rows are 2.5 ms apart (400 Hz), 20 steps at 8 kHz.
    let at = |r: &Vec<f64>| ((r[t] - first[t]) * 400.0).round() / 400.0;

    let mut motors = format!(
        "{NEUROBEM_CREDIT}\n#\n# Flight {} from {} s to {} s. NeuroBEM logged each motor's speed, not the\n# command that turned it, so each command here is the drive that holds the\n# Test Quad {NEUROBEM_QUAD}'s motor on that speed (times its trim,\n# {}, from the still hover just before), as it gains speed at the logged\n# rate, from its pack's steady {NEUROBEM_VOLTS} V: opendrone_physics::drive_holding.\n{}\n",
        window.flight,
        fixed(window.start, 4),
        fixed(window.start + window.length, 4),
        trim.iter()
            .map(|x| fixed(*x, 4))
            .collect::<Vec<_>>()
            .join(", "),
        opendrone_scenario::MOTOR_TRACK_HEADER
    );
    let mut last: Option<[String; 4]> = None;
    for r in &rows {
        let now: [String; 4] = core::array::from_fn(|k| {
            fixed(
                drive(r[speeds[k]] * trim[k], r[changes[k]] * trim[k]) * 100.0,
                2,
            )
        });
        let cells: Vec<String> = (0..4)
            .map(|k| match &last {
                Some(held) if held[k] == now[k] => String::new(),
                _ => now[k].clone(),
            })
            .collect();
        if last.is_some() && cells.iter().all(String::is_empty) {
            continue;
        }
        let _ = writeln!(motors, "{},{}", fixed(at(r), 4), cells.join(","));
        last = Some(now);
    }

    let rpm = 60.0 / (2.0 * core::f64::consts::PI);
    let mut recorded = format!(
        "{NEUROBEM_CREDIT}\n#\n# Flight {} from {} s to {} s: the body rates (NeuroBEM's, filtered from the\n# gyro and the motion capture), what the quad felt in its own axes, and each\n# motor's logged speed.\ntime (s),roll rate (°/s),pitch rate (°/s),yaw rate (°/s),felt acceleration forward (m/s²),felt acceleration left (m/s²),felt acceleration up (m/s²),motor 1 speed (RPM),motor 2 speed (RPM),motor 3 speed (RPM),motor 4 speed (RPM)\n",
        window.flight,
        fixed(window.start, 4),
        fixed(window.start + window.length, 4),
    );
    for r in &rows {
        let _ = writeln!(
            recorded,
            "{},{},{},{},{},{},{},{},{},{},{}",
            fixed(at(r), 4),
            fixed(r[rates[0]] / DEGREE, 2),
            fixed(-r[rates[1]] / DEGREE, 2),
            fixed(-r[rates[2]] / DEGREE, 2),
            fixed(r[felt[0]], 3),
            fixed(r[felt[1]], 3),
            fixed(r[felt[2]], 3),
            fixed(r[speeds[0]] * rpm, 0),
            fixed(r[speeds[1]] * rpm, 0),
            fixed(r[speeds[2]] * rpm, 0),
            fixed(r[speeds[3]] * rpm, 0),
        );
    }
    Ok((motors, recorded))
}

/// A number with `places` decimals, without a minus on zero.
fn fixed(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    if text
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        text.trim_start_matches('-').to_string()
    } else {
        text
    }
}
