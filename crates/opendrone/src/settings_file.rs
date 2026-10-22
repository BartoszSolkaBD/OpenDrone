//! The settings file: one pilot per install, one file, saved the moment a
//! setting changes (#68).
//!
//! Every setting a pilot can change lives in this one file, and each is owned
//! where ADR-0003 puts it: the Flight Mode, the Assists and the Packet Rate
//! belong to the Flight Controller and the Simulation, and the camera,
//! accessibility and screen settings to the game. The file only keeps them
//! (and the Preset, the last Map and the last Quad). A Quad or a Map is named
//! by its id, such as `opendrone/whoop-65`, never by a path.
//!
//! The file is TOML, in plain sections:
//!
//! ```toml
//! format = 1
//!
//! [pilot]
//! first_launch_done = true
//! preset = "beginner"
//! quad = "opendrone/whoop-65"
//! map = "opendrone/skate-park"
//!
//! [accessibility]
//! text_size = 100          # percent: 100, 125 or 150
//! reduce_motion = false
//!
//! [flying]
//! flight_mode = "Acro"
//! input_smoothing = false
//! endless_battery = false
//! auto_arm = false
//! packet_rate = 250        # Hz
//!
//! [camera]
//! video_look = "analog"
//! aspect = "4:3"
//!
//! [camera.quads."opendrone/whoop-65"]
//! tilt = 30.0              # degrees
//! fov = 160.0              # degrees
//! ```
//!
//! Beside it, in the same folder, go the saved flights (`saved-flights/`) and,
//! later, the `Packs` folder and the Blackbox folder (ADR-0003).
//!
//! A missing key keeps its default, so a file from an older OpenDrone still
//! reads. A key this OpenDrone doesn't know stays in the file when it saves,
//! so a newer OpenDrone's settings aren't lost. A value that isn't one it
//! accepts is ignored and said so, and the setting keeps its default; a
//! `[camera.quads]` entry it ignores stays in the file as the pilot wrote it,
//! until the pilot sets that Quad's camera. A file
//! that isn't TOML at all is kept aside as `settings.broken.toml`, and the
//! game starts fresh.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use opendrone_input::profile::FlightMode;
use opendrone_sim::PacketRate;
use toml::{Table, Value};

use crate::camera::controls::{CameraSettings, QuadCamera};
use crate::camera::lens::Aspect;
use crate::camera::look::VideoLook;

/// The file's format. A file with a higher number was made by a newer
/// OpenDrone.
pub const FORMAT: i64 = 1;

/// The settings file's name.
pub const FILE_NAME: &str = "settings.toml";

/// Where a file this version couldn't read is kept.
pub const BROKEN_FILE_NAME: &str = "settings.broken.toml";

/// The folder saved flights go to, inside the settings folder.
pub const SAVED_FLIGHTS: &str = "saved-flights";

/// A starting set of settings, picked on First Launch from how much the pilot
/// has flown. A Preset sets Flight Mode, Rates, Assists, each Quad's Camera
/// Tilt and FOV, and Breakup (#13, #14); what each sets arrives with the
/// Settings drawer (#69). Until then the file only remembers which one the
/// pilot chose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Preset {
    #[default]
    Beginner,
    Intermediate,
    Pro,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Beginner, Preset::Intermediate, Preset::Pro];

    /// The name on screen.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Beginner => "Beginner",
            Preset::Intermediate => "Intermediate",
            Preset::Pro => "Pro",
        }
    }

    /// The name in the settings file.
    fn key(self) -> &'static str {
        match self {
            Preset::Beginner => "beginner",
            Preset::Intermediate => "intermediate",
            Preset::Pro => "pro",
        }
    }

    fn from_key(key: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.key() == key)
    }
}

/// How big the text is, as a share of its usual size (#13's Text size).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextSize {
    #[default]
    Normal,
    Large,
    Larger,
}

impl TextSize {
    pub const ALL: [TextSize; 3] = [TextSize::Normal, TextSize::Large, TextSize::Larger];

    /// 100, 125 or 150.
    pub fn percent(self) -> u32 {
        match self {
            TextSize::Normal => 100,
            TextSize::Large => 125,
            TextSize::Larger => 150,
        }
    }

    fn from_percent(percent: i64) -> Option<TextSize> {
        TextSize::ALL
            .into_iter()
            .find(|t| i64::from(t.percent()) == percent)
    }
}

/// Everything the pilot has set, as the settings file keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotSettings {
    /// First Launch has been passed once.
    pub first_launch_done: bool,
    pub preset: Preset,
    /// The Quad's id, such as `opendrone/whoop-65`.
    pub quad: String,
    /// The Map's id, such as `opendrone/skate-park`.
    pub map: String,
    pub text_size: TextSize,
    pub reduce_motion: bool,
    /// The Flight Mode for an Input Device with no Flight Mode switch.
    pub flight_mode: FlightMode,
    pub input_smoothing: bool,
    pub endless_battery: bool,
    pub auto_arm: bool,
    pub packet_rate: PacketRate,
    /// The Video Look, the Aspect ratio, and each Quad's Camera Tilt and FOV.
    pub camera: CameraSettings,
}

impl Default for PilotSettings {
    /// A new install: the Whoop 65 on the Skate Park, the Beginner Preset,
    /// every Assist off and a 250 Hz Packet Rate.
    fn default() -> PilotSettings {
        PilotSettings {
            first_launch_done: false,
            preset: Preset::Beginner,
            quad: "opendrone/whoop-65".to_string(),
            map: "opendrone/skate-park".to_string(),
            text_size: TextSize::Normal,
            reduce_motion: false,
            flight_mode: FlightMode::Acro,
            input_smoothing: false,
            endless_battery: false,
            auto_arm: false,
            packet_rate: PacketRate::from_hz(250).expect("250 Hz is a Packet Rate"),
            camera: CameraSettings::default(),
        }
    }
}

/// What reading the settings file gave.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub settings: PilotSettings,
    /// Whether there was a file to read.
    pub existed: bool,
    /// Everything in it that couldn't be used, each as a plain sentence. The
    /// settings those were meant for keep their defaults.
    pub problems: Vec<String>,
}

/// The settings file in its folder.
#[derive(Clone, Debug)]
pub struct SettingsFile {
    folder: PathBuf,
    /// What the file held when it was read, so that keys this version doesn't
    /// know are written back.
    kept: Table,
}

impl SettingsFile {
    /// The settings file in `folder`.
    pub fn in_folder(folder: PathBuf) -> SettingsFile {
        SettingsFile {
            folder,
            kept: Table::new(),
        }
    }

    /// The folder the operating system keeps an app's settings in, for
    /// OpenDrone: `~/Library/Application Support/OpenDrone` on macOS,
    /// `%APPDATA%\OpenDrone` on Windows, and `$XDG_CONFIG_HOME/opendrone` (or
    /// `~/.config/opendrone`) elsewhere. `None` when the computer doesn't say
    /// where.
    pub fn usual_folder() -> Option<PathBuf> {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        let home = || var("HOME").map(PathBuf::from);
        match std::env::consts::OS {
            "macos" => Some(home()?.join("Library/Application Support/OpenDrone")),
            "windows" => Some(PathBuf::from(var("APPDATA")?).join("OpenDrone")),
            _ => match var("XDG_CONFIG_HOME") {
                Some(config) => Some(PathBuf::from(config).join("opendrone")),
                None => Some(home()?.join(".config/opendrone")),
            },
        }
    }

    /// The folder the file is in.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The file itself.
    pub fn path(&self) -> PathBuf {
        self.folder.join(FILE_NAME)
    }

    /// Where Save Flight writes: `saved-flights/` beside the file (#73).
    pub fn saved_flights_folder(&self) -> PathBuf {
        self.folder.join(SAVED_FLIGHTS)
    }

    /// Reads the file. It never fails: a missing file gives a new install's
    /// settings, a key or value it can't use is ignored and listed in
    /// [`Loaded::problems`], and a file that isn't TOML is kept aside as
    /// `settings.broken.toml`.
    pub fn load(&mut self) -> Loaded {
        let path = self.path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Loaded {
                    settings: PilotSettings::default(),
                    existed: false,
                    problems: Vec::new(),
                };
            }
            Err(error) => {
                return Loaded {
                    settings: PilotSettings::default(),
                    existed: true,
                    problems: vec![format!(
                        "the settings file {} can't be read ({error}), so OpenDrone starts with its usual settings",
                        path.display()
                    )],
                };
            }
        };
        let table = match text.parse::<Table>() {
            Ok(table) => table,
            Err(error) => {
                let aside = self.folder.join(BROKEN_FILE_NAME);
                let kept = match fs::rename(&path, &aside) {
                    Ok(()) => format!("It is kept as {}.", aside.display()),
                    Err(rename) => format!("It couldn't be moved aside ({rename})."),
                };
                return Loaded {
                    settings: PilotSettings::default(),
                    existed: true,
                    problems: vec![format!(
                        "the settings file {} isn't readable ({}), so OpenDrone starts with its usual settings. {kept}",
                        path.display(),
                        error.message()
                    )],
                };
            }
        };
        let mut problems = Vec::new();
        let settings = settings_from(&table, &mut problems);
        self.kept = table;
        Loaded {
            settings,
            existed: true,
            problems,
        }
    }

    /// Writes the settings, whole, at once: a new file beside the old one,
    /// then a rename, so a crash never leaves half a file.
    pub fn save(&self, settings: &PilotSettings) -> io::Result<()> {
        fs::create_dir_all(&self.folder)?;
        let table = table_of(settings, self.kept.clone());
        let text =
            toml::to_string_pretty(&table).map_err(|error| io::Error::other(error.to_string()))?;
        let path = self.path();
        let new = self.folder.join(format!("{FILE_NAME}.new"));
        fs::write(&new, text)?;
        fs::rename(&new, &path)
    }
}

/// The settings a table holds, each missing one at its default and each bad
/// one said in `problems`.
pub fn settings_from(table: &Table, problems: &mut Vec<String>) -> PilotSettings {
    let mut settings = PilotSettings::default();
    match table.get("format") {
        None => {}
        Some(Value::Integer(n)) if *n <= FORMAT => {}
        Some(Value::Integer(n)) => problems.push(format!(
            "the settings file is format {n}, which a newer OpenDrone made; this one reads format {FORMAT}, so it keeps what it can read"
        )),
        Some(_) => problems.push("`format` should be a whole number".to_string()),
    }
    let mut reader = Reader { table, problems };

    if let Some(done) = reader.boolean("pilot", "first_launch_done") {
        settings.first_launch_done = done;
    }
    if let Some(key) = reader.string("pilot", "preset") {
        match Preset::from_key(&key) {
            Some(preset) => settings.preset = preset,
            None => reader.refuse("pilot", "preset", "beginner, intermediate or pro"),
        }
    }
    if let Some(id) = reader.string("pilot", "quad") {
        settings.quad = id;
    }
    if let Some(id) = reader.string("pilot", "map") {
        settings.map = id;
    }
    if let Some(percent) = reader.integer("accessibility", "text_size") {
        match TextSize::from_percent(percent) {
            Some(size) => settings.text_size = size,
            None => reader.refuse("accessibility", "text_size", "100, 125 or 150"),
        }
    }
    if let Some(on) = reader.boolean("accessibility", "reduce_motion") {
        settings.reduce_motion = on;
    }
    if let Some(word) = reader.string("flying", "flight_mode") {
        match FlightMode::from_word(&word) {
            Some(mode) => settings.flight_mode = mode,
            None => reader.refuse("flying", "flight_mode", "Acro, Horizon or Angle"),
        }
    }
    if let Some(on) = reader.boolean("flying", "input_smoothing") {
        settings.input_smoothing = on;
    }
    if let Some(on) = reader.boolean("flying", "endless_battery") {
        settings.endless_battery = on;
    }
    if let Some(on) = reader.boolean("flying", "auto_arm") {
        settings.auto_arm = on;
    }
    if let Some(hz) = reader.integer("flying", "packet_rate") {
        match u32::try_from(hz).ok().and_then(PacketRate::from_hz) {
            Some(rate) => settings.packet_rate = rate,
            None => reader.refuse(
                "flying",
                "packet_rate",
                "50, 100, 150, 250, 333, 500 or 1000 (Hz)",
            ),
        }
    }
    if let Some(look) = reader.string("camera", "video_look") {
        match look.as_str() {
            "analog" => settings.camera.look = VideoLook::Analog,
            "digital" => settings.camera.look = VideoLook::Digital,
            _ => reader.refuse("camera", "video_look", "analog or digital"),
        }
    }
    if let Some(aspect) = reader.string("camera", "aspect") {
        match aspect.as_str() {
            "4:3" => settings.camera.aspect = Aspect::FourThree,
            "16:9" => settings.camera.aspect = Aspect::SixteenNine,
            _ => reader.refuse("camera", "aspect", "4:3 or 16:9"),
        }
    }
    if let Some(Value::Table(quads)) = table
        .get("camera")
        .and_then(Value::as_table)
        .and_then(|camera| camera.get("quads"))
    {
        for (id, quad) in quads {
            let Some(quad) = quad.as_table() else {
                problems.push(format!(
                    "[camera.quads.\"{id}\"] should be a section, so it is ignored; it stays in the file as it is"
                ));
                continue;
            };
            use crate::camera::controls::{FOV_RANGE, TILT_RANGE};
            let tilt = quad_number(quad, "tilt", TILT_RANGE);
            let fov = quad_number(quad, "fov", FOV_RANGE);
            match (tilt, fov) {
                (Ok(tilt), Ok(fov)) => settings.camera.set_quad(id, QuadCamera { tilt, fov }),
                (tilt, fov) => {
                    let why: Vec<String> = [tilt.err(), fov.err()].into_iter().flatten().collect();
                    problems.push(format!(
                        "[camera.quads.\"{id}\"] is ignored ({}), so that Quad's camera keeps its own values; the entry stays in the file as it is",
                        why.join("; ")
                    ));
                }
            }
        }
    }
    settings
}

/// One number of a Quad's camera entry, or the plain reason it can't be used.
fn quad_number(entry: &Table, key: &str, range: (f64, f64)) -> Result<f64, String> {
    let value = entry.get(key).ok_or(format!("{key} is missing"))?;
    let n = value
        .as_float()
        .or_else(|| value.as_integer().map(|i| i as f64))
        .ok_or(format!("{key} should be a number"))?;
    if (range.0..=range.1).contains(&n) {
        Ok(n)
    } else {
        Err(format!(
            "{key} is {n}, which is outside {} to {} degrees",
            range.0, range.1
        ))
    }
}

/// What each `Reader` method looks up: a key in a section, of one kind.
struct Reader<'a> {
    table: &'a Table,
    problems: &'a mut Vec<String>,
}

impl Reader<'_> {
    fn value(&mut self, section: &str, key: &str) -> Option<&Value> {
        let section_table = match self.table.get(section) {
            None => return None,
            Some(Value::Table(t)) => t,
            Some(_) => {
                self.problems
                    .push(format!("[{section}] should be a section"));
                return None;
            }
        };
        section_table.get(key)
    }

    fn boolean(&mut self, section: &str, key: &str) -> Option<bool> {
        let value = self.value(section, key)?;
        match value.as_bool() {
            Some(b) => Some(b),
            None => {
                self.refuse(section, key, "true or false");
                None
            }
        }
    }

    fn integer(&mut self, section: &str, key: &str) -> Option<i64> {
        let value = self.value(section, key)?;
        match value.as_integer() {
            Some(n) => Some(n),
            None => {
                self.refuse(section, key, "a whole number");
                None
            }
        }
    }

    fn string(&mut self, section: &str, key: &str) -> Option<String> {
        let value = self.value(section, key)?;
        match value.as_str() {
            Some(s) => Some(s.to_string()),
            None => {
                self.refuse(section, key, "text");
                None
            }
        }
    }

    fn refuse(&mut self, section: &str, key: &str, wants: &str) {
        self.problems.push(format!(
            "{section}.{key} should be {wants}, so it keeps its usual value"
        ));
    }
}

/// The table to write: the keys this version knows set from `settings`, on
/// top of `base`, which holds whatever the file had.
pub fn table_of(settings: &PilotSettings, mut base: Table) -> Table {
    fn section<'t>(table: &'t mut Table, name: &str) -> &'t mut Table {
        let entry = table
            .entry(name.to_string())
            .or_insert_with(|| Value::Table(Table::new()));
        if !entry.is_table() {
            *entry = Value::Table(Table::new());
        }
        entry.as_table_mut().expect("it is a table now")
    }
    base.insert("format".to_string(), Value::Integer(FORMAT));
    {
        let pilot = section(&mut base, "pilot");
        pilot.insert(
            "first_launch_done".into(),
            Value::Boolean(settings.first_launch_done),
        );
        pilot.insert(
            "preset".into(),
            Value::String(settings.preset.key().to_string()),
        );
        pilot.insert("quad".into(), Value::String(settings.quad.clone()));
        pilot.insert("map".into(), Value::String(settings.map.clone()));
    }
    {
        let accessibility = section(&mut base, "accessibility");
        accessibility.insert(
            "text_size".into(),
            Value::Integer(i64::from(settings.text_size.percent())),
        );
        accessibility.insert(
            "reduce_motion".into(),
            Value::Boolean(settings.reduce_motion),
        );
    }
    {
        let flying = section(&mut base, "flying");
        flying.insert(
            "flight_mode".into(),
            Value::String(settings.flight_mode.word().to_string()),
        );
        flying.insert(
            "input_smoothing".into(),
            Value::Boolean(settings.input_smoothing),
        );
        flying.insert(
            "endless_battery".into(),
            Value::Boolean(settings.endless_battery),
        );
        flying.insert("auto_arm".into(), Value::Boolean(settings.auto_arm));
        flying.insert(
            "packet_rate".into(),
            Value::Integer(i64::from(settings.packet_rate.hz())),
        );
    }
    {
        let camera = section(&mut base, "camera");
        camera.insert(
            "video_look".into(),
            Value::String(
                match settings.camera.look {
                    VideoLook::Analog => "analog",
                    VideoLook::Digital => "digital",
                }
                .to_string(),
            ),
        );
        camera.insert(
            "aspect".into(),
            Value::String(
                match settings.camera.aspect {
                    Aspect::FourThree => "4:3",
                    Aspect::SixteenNine => "16:9",
                }
                .to_string(),
            ),
        );
        // The entries the reader ignored stay as the pilot wrote them. Every
        // entry it understood is in `settings`, so what `base` holds for an id
        // that isn't there is one it rejected; the pilot's own change for an
        // id replaces it.
        let mut quads = match camera.remove("quads") {
            Some(Value::Table(kept)) => kept,
            _ => Table::new(),
        };
        quads.retain(|id, _| !settings.camera.quads().any(|(known, _)| known == id));
        for (id, quad) in settings.camera.quads() {
            let mut one = Table::new();
            one.insert("tilt".into(), Value::Float(quad.tilt));
            one.insert("fov".into(), Value::Float(quad.fov));
            quads.insert(id.clone(), Value::Table(one));
        }
        camera.insert("quads".into(), Value::Table(quads));
    }
    base
}
