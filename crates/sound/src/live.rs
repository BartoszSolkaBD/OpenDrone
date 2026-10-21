//! The Quad's sound on the sound device, from the sound's own thread.
//!
//! The thread owns the Firewheel graph and the device's stream. It asks for
//! a 256-frame block, about 5 ms at 48 kHz, and takes the device's own block
//! if the device can't do that; Firewheel's own default, 1024 frames, is too
//! slow. With no device it runs silent and tries again every few seconds. If
//! the device goes away, it moves to the new default device at once. The
//! game never waits for it: each frame's Quad state goes in with no waiting,
//! and the sound device's own thread reads the newest.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use firewheel::FirewheelContext;
use firewheel::cpal::{CpalConfig, CpalOutputConfig, CpalStream};

use crate::graph::{FrameWriter, frames, graph};
use crate::{QuadFrame, QuadSound};

/// The block the game asks the sound device for, in frames: about 5 ms at
/// 48 kHz (#34).
pub const BLOCK_FRAMES_ASKED: u32 = 256;

/// How often the sound's thread keeps the graph going and checks the device.
const TURN: Duration = Duration::from_millis(5);

/// How long it waits before looking for a sound device again.
const RETRY: Duration = Duration::from_secs(3);

/// How long it waits for a stream that went away to let go of the graph.
const LET_GO: Duration = Duration::from_secs(2);

/// Where the sound is playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device {
    /// The sound's thread is still opening the device.
    Opening,
    /// Playing on the device: its name, if it gave one, the block it took,
    /// in frames, and its sample rate, in Hz.
    Playing {
        name: Option<String>,
        block_frames: u32,
        sample_rate: u32,
    },
    /// No sound device could be opened, so the game runs silent; why not.
    None(String),
}

/// The Quad's sound, playing on the sound's own thread.
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
            .spawn(move || match graph(quad, readers) {
                Ok(cx) => run(cx, &device, &stopped),
                Err(problem) => say(
                    &device,
                    Device::None(format!("the sound can't be made: {problem}")),
                ),
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

/// The sound's thread: keeps the graph going, opens the device, and moves
/// to the new default device when it goes away, until told to stop.
fn run(mut cx: FirewheelContext, device: &Mutex<Device>, stop: &mpsc::Receiver<()>) {
    let mut stream: Option<CpalStream> = None;
    let mut next_try = Instant::now();
    loop {
        match stop.recv_timeout(TURN) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if let Some(s) = &mut stream {
            // Errors arrive here; whether to reopen is `output_stream_ok`'s.
            for _ in s.poll_status() {}
            if !s.output_stream_ok() {
                stream = None;
                let let_go = Instant::now();
                while cx.is_active() && let_go.elapsed() < LET_GO {
                    let _ = cx.update();
                    thread::sleep(Duration::from_millis(1));
                }
                say(device, Device::Opening);
                next_try = Instant::now();
            }
        }
        if stream.is_none() && Instant::now() >= next_try && !cx.is_active() {
            match open(&mut cx) {
                Ok((s, playing)) => {
                    stream = Some(s);
                    say(device, playing);
                }
                Err(problem) => {
                    say(device, Device::None(problem));
                    next_try = Instant::now() + RETRY;
                }
            }
        }
        let _ = cx.update();
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
