//! The Quad's sound on the sound device, from the sound's own thread.
//!
//! The thread owns the Firewheel graph and the device's stream. It asks for
//! a 256-frame block, about 5 ms at 48 kHz, and takes the device's own block
//! if the device can't do that; Firewheel's own default, 1024 frames, is too
//! slow. With no device it runs silent and tries again, a little later each
//! time. If the device goes away, it moves to the new default device as soon
//! as the old stream lets go of the graph, and keeps trying until one opens
//! or the game stops. It never blocks: every turn it looks for the stop
//! signal first. The game never waits for it: each frame's Quad state goes in
//! with no waiting, and the sound device's own thread reads the newest.
//!
//! What the thread needs from the device sits behind [`Output`], so a check
//! can give it a fake device and no real audio.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use firewheel::FirewheelContext;
use firewheel::cpal::{CpalConfig, CpalOutputConfig, CpalStream};

use crate::graph::{FrameReaders, FrameWriter, frames, graph};
use crate::{QuadFrame, QuadSound};

/// The block the game asks the sound device for, in frames: about 5 ms at
/// 48 kHz (#34).
pub const BLOCK_FRAMES_ASKED: u32 = 256;

/// How often the sound's thread keeps the graph going, looks for the stop
/// signal and checks the device.
const TURN: Duration = Duration::from_millis(5);

/// How long it waits after the first failed try to open a device. The wait
/// doubles after each failure, up to [`RETRY_AT_MOST`].
const RETRY_FIRST: Duration = Duration::from_millis(250);

/// The longest it waits between two tries to open a device.
const RETRY_AT_MOST: Duration = Duration::from_secs(3);

/// Where the sound is playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device {
    /// The sound's thread is making its first try to open a device.
    Opening,
    /// Playing on the device: its name, if it gave one, the block it took,
    /// in frames, and its sample rate, in Hz.
    Playing {
        name: Option<String>,
        block_frames: u32,
        sample_rate: u32,
    },
    /// The sound is silent, and still trying. The device it played on went
    /// away: why the last try failed, or that the device went away and the
    /// old stream hasn't let go of the graph yet. It plays again on the
    /// first device that opens.
    Lost(String),
    /// No sound device could be opened, so the game runs silent; why not.
    /// The sound's thread keeps trying, so a device that turns up later is
    /// used.
    None(String),
}

/// What the sound's thread needs from the sound device. Firewheel and the
/// operating system's device are the real one; a check gives a fake.
pub trait Output {
    /// Keeps the graph going: called every turn, whether or not a stream is
    /// open.
    fn keep_going(&mut self);

    /// Whether the graph is free for a new stream. It isn't while a stream
    /// that went away still holds it.
    fn graph_free(&self) -> bool;

    /// Opens the default device, and says where the sound now plays.
    fn open(&mut self) -> Result<Device, String>;

    /// Whether the open stream has gone away: the device was unplugged, or
    /// failed.
    fn lost(&mut self) -> bool;

    /// Lets go of the stream that went away. It may take a while to let go
    /// of the graph; the thread keeps going meanwhile.
    fn close(&mut self);
}

/// The Quad's sound, playing on its own thread.
///
/// Dropping it stops the thread and lets go of the device.
pub struct Sound {
    frames: Mutex<FrameWriter>,
    device: Arc<Mutex<Device>>,
    stop: mpsc::Sender<()>,
}

impl Sound {
    /// Starts `quad`'s sound on the default sound device, on a new thread.
    /// It never fails: with no device, or no thread, it runs silent, and
    /// [`Sound::device`] says why.
    pub fn start(quad: QuadSound) -> Sound {
        Sound::start_with(move |readers| {
            graph(quad, readers)
                .map(Firewheel::new)
                .map_err(|problem| format!("the sound can't be made: {problem}"))
        })
    }

    /// Starts the sound's thread on the device layer `make` makes, on that
    /// thread. This is for a check with a fake device layer: nothing is made
    /// from the Quad's state. A `make` that fails leaves the game silent,
    /// and [`Sound::device`] says why.
    pub fn start_on<O: Output + 'static>(
        make: impl FnOnce() -> Result<O, String> + Send + 'static,
    ) -> Sound {
        Sound::start_with(move |_readers| make())
    }

    fn start_with<O: Output + 'static>(
        make: impl FnOnce(FrameReaders) -> Result<O, String> + Send + 'static,
    ) -> Sound {
        let device = Arc::new(Mutex::new(Device::Opening));
        let (stop, stopped) = mpsc::channel();
        let (writer, readers) = frames();
        let sound = Sound {
            frames: Mutex::new(writer),
            device: device.clone(),
            stop,
        };
        let spawned = thread::Builder::new()
            .name("opendrone-sound".to_string())
            .spawn(move || match make(readers) {
                Ok(output) => run(output, &device, &stopped),
                Err(problem) => say(&device, Device::None(problem)),
            });
        if let Err(problem) = spawned {
            say(
                &sound.device,
                Device::None(format!("the sound's thread can't start: {problem}")),
            );
        }
        sound
    }

    /// Hands the sound the Quad's newest state, never waiting for it.
    pub fn hand_over(&self, frame: QuadFrame) {
        if let Ok(mut frames) = self.frames.lock() {
            frames.write(frame);
        }
    }

    /// Where the sound is playing, or why it's silent.
    pub fn device(&self) -> Device {
        self.device
            .lock()
            .map(|d| d.clone())
            .unwrap_or_else(|_| Device::None("the sound's thread stopped".to_string()))
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

fn say(shared: &Mutex<Device>, device: Device) {
    if let Ok(mut d) = shared.lock() {
        *d = device;
    }
}

/// How long to wait after each failed try to open a device.
struct Backoff(Duration);

impl Backoff {
    fn new() -> Backoff {
        Backoff(RETRY_FIRST)
    }

    /// The wait after this failure. The next one is twice as long, up to the
    /// most.
    fn next(&mut self) -> Duration {
        let wait = self.0;
        self.0 = (wait * 2).min(RETRY_AT_MOST);
        wait
    }
}

/// The sound's thread: keeps the graph going, opens the device, and moves
/// to the new default device when it goes away, until told to stop. It
/// never waits on the old stream or on a device: it looks again next turn,
/// so the stop signal always comes first.
fn run(mut output: impl Output, device: &Mutex<Device>, stop: &mpsc::Receiver<()>) {
    let mut playing = false;
    let mut was_lost = false;
    let mut backoff = Backoff::new();
    let mut next_try = Instant::now();
    loop {
        match stop.recv_timeout(TURN) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if playing && output.lost() {
            output.close();
            playing = false;
            was_lost = true;
            say(
                device,
                Device::Lost("the sound device went away".to_string()),
            );
            backoff = Backoff::new();
            next_try = Instant::now();
        }
        if !playing && Instant::now() >= next_try && output.graph_free() {
            match output.open() {
                Ok(now_playing) => {
                    playing = true;
                    was_lost = false;
                    backoff = Backoff::new();
                    say(device, now_playing);
                }
                Err(problem) => {
                    say(
                        device,
                        if was_lost {
                            Device::Lost(problem)
                        } else {
                            Device::None(problem)
                        },
                    );
                    next_try = Instant::now() + backoff.next();
                }
            }
        }
        output.keep_going();
    }
}

/// Firewheel's graph and the operating system's sound device.
struct Firewheel {
    cx: FirewheelContext,
    stream: Option<CpalStream>,
}

impl Firewheel {
    fn new(cx: FirewheelContext) -> Firewheel {
        Firewheel { cx, stream: None }
    }
}

impl Output for Firewheel {
    fn keep_going(&mut self) {
        let _ = self.cx.update();
    }

    fn graph_free(&self) -> bool {
        !self.cx.is_active()
    }

    fn open(&mut self) -> Result<Device, String> {
        let (stream, playing) = open(&mut self.cx)?;
        self.stream = Some(stream);
        Ok(playing)
    }

    fn lost(&mut self) -> bool {
        let Some(stream) = &mut self.stream else {
            return false;
        };
        // Errors arrive here; whether to reopen is `output_stream_ok`'s.
        for _ in stream.poll_status() {}
        !stream.output_stream_ok()
    }

    fn close(&mut self) {
        self.stream = None;
    }
}

/// Opens the default sound device, asking for a 256-frame block first, then
/// for the device's own.
fn open(cx: &mut FirewheelContext) -> Result<(CpalStream, Device), String> {
    let mut problem = String::new();
    for block in [Some(BLOCK_FRAMES_ASKED), None] {
        let config = CpalConfig {
            output: CpalOutputConfig {
                desired_block_frames: block,
                ..Default::default()
            },
            input: None,
        };
        match CpalStream::new(cx, config) {
            Ok(stream) => {
                let info = stream.info();
                let playing = Device::Playing {
                    name: info.out_device_id.as_ref().map(|id| id.to_string()),
                    block_frames: info.max_block_frames.get(),
                    sample_rate: info.sample_rate.get(),
                };
                return Ok((stream, playing));
            }
            Err(e) => problem = e.to_string(),
        }
    }
    Err(problem)
}
