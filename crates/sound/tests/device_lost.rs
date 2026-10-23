//! A sound device that goes away mid-flight (#263): the sound keeps trying
//! until a device opens or the game stops, and says what it is doing. These
//! use a fake device layer and no real audio, so they run on CI's machines.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use opendrone_sound::{Device, Output, Sound};

/// The fake sound device layer's world, which a check changes from outside.
#[derive(Default)]
struct World {
    /// The default device now, or none.
    default: Option<&'static str>,
    /// The device the stream plays on, if one is open.
    playing_on: Option<&'static str>,
    /// Whether the old stream still holds the graph after it was closed.
    old_stream_holds_the_graph: bool,
    /// Whether a stream that went away is still being let go of.
    holding: bool,
    /// How many times the thread tried to open a device.
    tries: u32,
    /// Whether the thread let go of the whole device layer, as it does when
    /// it stops.
    let_go_of_everything: bool,
}

struct Fake(Arc<Mutex<World>>);

impl Output for Fake {
    fn keep_going(&mut self) {}

    fn graph_free(&self) -> bool {
        !self.0.lock().unwrap().holding
    }

    fn open(&mut self) -> Result<Device, String> {
        let mut world = self.0.lock().unwrap();
        world.tries += 1;
        let name = world.default.ok_or("no sound device")?;
        world.playing_on = Some(name);
        Ok(Device::Playing {
            name: Some(name.to_string()),
            block_frames: 256,
            sample_rate: 48_000,
        })
    }

    fn lost(&mut self) -> bool {
        let world = self.0.lock().unwrap();
        world.playing_on.is_some() && world.playing_on != world.default
    }

    fn close(&mut self) {
        let mut world = self.0.lock().unwrap();
        world.playing_on = None;
        world.holding = world.old_stream_holds_the_graph;
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.0.lock().unwrap().let_go_of_everything = true;
    }
}

fn start(world: &Arc<Mutex<World>>) -> Sound {
    let world = world.clone();
    Sound::start_on(move || Ok(Fake(world)))
}

fn wait_for(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < within, "{what}: not within {within:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn plays_on(sound: &Sound, name: &str) -> bool {
    matches!(sound.device(), Device::Playing { name: Some(n), .. } if n == name)
}

fn lost(sound: &Sound) -> bool {
    matches!(sound.device(), Device::Lost(_))
}

#[test]
fn a_device_lost_while_the_old_stream_holds_the_graph_plays_on_the_new_device_once_it_lets_go() {
    // Rule: when the sound device is lost, the sound moves to the new
    // default device (sound.md, "Settings and screens"). It used to give up
    // for good if the old stream still held the graph after a 2 s wait.
    let world = Arc::new(Mutex::new(World {
        default: Some("headphones"),
        ..World::default()
    }));
    let sound = start(&world);
    wait_for("playing on the headphones", Duration::from_secs(5), || {
        plays_on(&sound, "headphones")
    });

    // The headphones come out, and the old stream keeps the graph.
    {
        let mut w = world.lock().unwrap();
        w.default = None;
        w.old_stream_holds_the_graph = true;
    }
    wait_for("the loss is noticed", Duration::from_secs(5), || {
        lost(&sound)
    });

    // Past the 2 s the old code waited, the sound is still trying, and says
    // so. A new device is there, but the graph is still held.
    std::thread::sleep(Duration::from_millis(2_500));
    world.lock().unwrap().default = Some("speakers");
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        lost(&sound),
        "while the old stream holds the graph it is {:?}",
        sound.device()
    );

    // The old stream lets go, and the sound plays on the new device.
    world.lock().unwrap().holding = false;
    wait_for("playing on the speakers", Duration::from_secs(5), || {
        plays_on(&sound, "speakers")
    });
}

#[test]
fn a_lost_device_with_no_new_one_keeps_trying_with_pauses_and_plays_when_one_appears() {
    // Rule: the sound keeps trying until a device opens (#263), a little
    // later after each failure, so it doesn't spin.
    let world = Arc::new(Mutex::new(World {
        default: Some("headphones"),
        ..World::default()
    }));
    let sound = start(&world);
    wait_for("playing on the headphones", Duration::from_secs(5), || {
        plays_on(&sound, "headphones")
    });
    let tries_before = world.lock().unwrap().tries;

    world.lock().unwrap().default = None;
    wait_for("the loss is noticed", Duration::from_secs(5), || {
        lost(&sound)
    });
    std::thread::sleep(Duration::from_millis(1_500));
    let tries = world.lock().unwrap().tries - tries_before;
    assert!(
        (2..=10).contains(&tries),
        "{tries} tries in 1.5 s with no device: it should keep trying, with pauses"
    );
    assert!(lost(&sound), "with no device it is {:?}", sound.device());

    world.lock().unwrap().default = Some("speakers");
    wait_for("playing on the speakers", Duration::from_secs(8), || {
        plays_on(&sound, "speakers")
    });
}

#[test]
fn the_wait_for_the_old_stream_ends_at_once_when_the_game_stops() {
    // Rule: the game never waits for the sound (sound.md). The old wait of
    // 2 s ignored the stop signal.
    let world = Arc::new(Mutex::new(World {
        default: Some("headphones"),
        ..World::default()
    }));
    let sound = start(&world);
    wait_for("playing on the headphones", Duration::from_secs(5), || {
        plays_on(&sound, "headphones")
    });
    {
        let mut w = world.lock().unwrap();
        w.default = None;
        w.old_stream_holds_the_graph = true;
    }
    wait_for("the loss is noticed", Duration::from_secs(5), || {
        lost(&sound)
    });

    let stopped = Instant::now();
    drop(sound);
    wait_for(
        "the sound's thread lets go of the device",
        Duration::from_secs(5),
        || world.lock().unwrap().let_go_of_everything,
    );
    assert!(
        stopped.elapsed() < Duration::from_millis(500),
        "stopping took {:?}",
        stopped.elapsed()
    );
}

#[test]
fn a_sound_started_with_no_device_says_so_and_plays_when_one_appears() {
    // Rule: with no device the game runs silent and says so; it uses a
    // device that turns up later (#263).
    let world = Arc::new(Mutex::new(World::default()));
    let sound = start(&world);
    wait_for("it says there is no device", Duration::from_secs(5), || {
        matches!(sound.device(), Device::None(_))
    });
    world.lock().unwrap().default = Some("speakers");
    wait_for("playing on the speakers", Duration::from_secs(8), || {
        plays_on(&sound, "speakers")
    });
}
