//! Setting up a flight: the Quad and the Map from the Packs, the settings,
//! and where the Quad starts.

use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use opendrone_maths::{Attitude, DEGREE, PilotAngles, Vec3};
use opendrone_pack::{MapDefinition, Packs, QuadDefinition};
use opendrone_physics::PROP_DISC_THICKNESS;
use opendrone_sim::{
    InputDeviceFacts, Mount, OurFlightController, QuadSetUp, QuadShape, QuadState, SetUp,
    Simulation, StartingMotors, sensor_readings,
};

use crate::settings::Settings;

/// Everything a flight starts from: the Quad and the Map, already checked by
/// the Pack checker, and the settings.
#[derive(Resource, Clone, Debug)]
pub struct FlightPlan {
    pub quad: QuadDefinition,
    pub map: MapDefinition,
    pub settings: Settings,
}

impl FlightPlan {
    /// Finds the settings' Quad and Map in the Packs.
    pub fn from_packs(packs: &Packs, settings: Settings) -> Result<FlightPlan, String> {
        let quad = packs
            .quad(&settings.quad)
            .map_err(|p| format!("can't use the Quad \"{}\":\n{p}", settings.quad))?;
        if let Err(missing) = &quad.flight_controller {
            return Err(format!(
                "the Quad \"{}\" can't fly with our Flight Controller yet: its Tune doesn't spell out {}",
                settings.quad,
                missing.join(", ")
            ));
        }
        let map = packs
            .map(&settings.map)
            .map_err(|p| format!("can't use the Map \"{}\":\n{p}", settings.map))?;
        Ok(FlightPlan {
            quad,
            map,
            settings,
        })
    }

    /// Where the Quad starts, and where Reset puts it: the Map's Launch Spot,
    /// landed on the ground there, level, facing the spot's compass heading,
    /// still. A Test Map has no Launch Spot, so the Quad sits at its origin,
    /// landed on the floor, its nose north.
    pub fn launch_spot(&self) -> QuadState {
        let (at, heading) = match &self.map.launch_spot {
            Some(spot) => (spot.position, spot.heading * DEGREE),
            None => (Vec3::ZERO, 0.0),
        };
        QuadState {
            position: at + Vec3::new(0.0, 0.0, landed_height(&self.quad.parameters.shape)),
            velocity: Vec3::ZERO,
            attitude: Attitude::from_pilot_angles(PilotAngles {
                roll: 0.0,
                pitch: 0.0,
                heading,
            }),
            rotation: Vec3::ZERO,
        }
    }

    /// A fresh Simulation: the Quad on its Launch Spot, powered up as Reset
    /// powers it up (a full battery, a fresh Flight Controller, its ESCs
    /// playing their start-up tones), flown by the Flying Input Device whose
    /// facts are given, or by none yet.
    pub fn simulation(&self, device: Option<InputDeviceFacts>) -> Result<Simulation, String> {
        let settings = &self.settings;
        let tune = self
            .quad
            .flight_controller
            .clone()
            .map_err(|missing| format!("the Tune doesn't spell out {}", missing.join(", ")))?;
        let start = self.launch_spot();
        let parameters = self.quad.parameters.clone();
        let flight_controller = OurFlightController::new(
            tune,
            settings.rates.clone(),
            settings.physics_rate,
            settings.packet_rate,
            false,
            settings.auto_arm,
            &sensor_readings(&start, false, parameters.gyro_range),
        );
        Simulation::new(SetUp {
            physics_rate: settings.physics_rate,
            world: self.map.world,
            map: self.map.shapes.clone(),
            random_seed: settings.random_seed,
            quads: vec![QuadSetUp {
                parameters,
                start,
                launch_spot: start,
                motors: StartingMotors::PoweringUp,
                battery: 1.0,
                mount: Mount::Free,
                packet_rate: settings.packet_rate,
                input_device: device,
                input_smoothing: settings.input_smoothing,
                endless_battery: settings.endless_battery,
                flight_controller: Box::new(flight_controller),
            }],
        })
        .map_err(|error| format!("the Simulation can't start: {error:?}"))
    }
}

/// How high a level Quad's centre of mass sits when it rests on a flat
/// floor: as high as its lowest part reaches below the centre of mass. That
/// is the body box, the pack, the prop discs or, on a whoop, the duct rings.
pub fn landed_height(shape: &QuadShape) -> f64 {
    let mut lowest = [
        -shape.body.z / 2.0,
        shape.pack_height - shape.pack.z / 2.0,
        shape.rotor_height - PROP_DISC_THICKNESS / 2.0,
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min);
    if let Some(rings) = shape.duct_rings {
        lowest = lowest.min(shape.rotor_height - rings.height / 2.0);
    }
    -lowest
}

/// The game's folder, for the built-in Pack, the Scenarios and the material
/// library: see [`find_repo_folder`]. When neither the program's folder nor
/// the working folder has them, it is the repo the game was built from.
pub fn repo_folder() -> PathBuf {
    let program = std::env::current_exe().ok();
    let here = std::env::current_dir().ok();
    find_repo_folder(program.as_deref(), here.as_deref())
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// Where the game finds its files, a folder that holds both `packs/` and
/// `scenarios/`:
///
/// - the folder the program itself is in, when it holds them: a downloaded
///   build, which `cargo xtask package` lays out that way (#81), so it finds
///   its own files wherever it's started from, by a double-click too;
/// - else the nearest folder at or above the working folder that holds them:
///   the repo, when the game runs with `cargo run`.
pub fn find_repo_folder(program: Option<&Path>, here: Option<&Path>) -> Option<PathBuf> {
    let holds_the_game = |f: &Path| f.join("packs").is_dir() && f.join("scenarios").is_dir();
    program
        .and_then(Path::parent)
        .filter(|f| holds_the_game(f))
        .map(Path::to_path_buf)
        .or_else(|| {
            here.and_then(|here| here.ancestors().find(|f| holds_the_game(f)))
                .map(Path::to_path_buf)
        })
}

/// The built-in Pack and the other Packs in the repo's `packs/` folder, each
/// checked by the Pack checker.
pub fn open_packs(repo: &Path) -> Result<Packs, String> {
    Packs::open(&repo.join("packs"), "packs").map_err(|p| p.to_string())
}
