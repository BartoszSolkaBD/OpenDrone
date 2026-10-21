//! The Quad's sound made with no sound device: the live graph, driven block
//! by block, for tests and renders (ADR-0023).

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use audioadapter_buffers::direct::InterleavedSlice;
use firewheel::backend::BackendProcessInfo;
use firewheel::node::StreamStatus;
use firewheel::processor::FirewheelProcessor;
use firewheel::{ActivateInfo, FirewheelContext};

use crate::graph::{FrameWriter, frames, graph};
use crate::{QuadFrame, QuadSound};

/// The same graph [`Sound`](crate::Sound) plays, made block by block with
/// no device.
pub struct Offline {
    // Dropped first: the context waits for its processor to go when it's
    // dropped.
    processor: FirewheelProcessor,
    cx: FirewheelContext,
    frames: FrameWriter,
    sample_rate: u32,
    block_frames: usize,
    played: Duration,
    stereo: Vec<f32>,
}

impl Offline {
    /// `quad`'s sound at `sample_rate` (Hz), made `block_frames` frames at a
    /// time, as a device would ask for them.
    pub fn new(quad: QuadSound, sample_rate: u32, block_frames: u32) -> Result<Offline, String> {
        let (frames, readers) = frames();
        let mut cx = graph(quad, readers)?;
        let processor = cx
            .activate(ActivateInfo {
                sample_rate: NonZeroU32::new(sample_rate).ok_or("a sample rate of 0 Hz")?,
                max_block_frames: NonZeroU32::new(block_frames).ok_or("a block of 0 frames")?,
                num_stream_in_channels: 0,
                num_stream_out_channels: 2,
                input_to_output_latency_seconds: 0.0,
            })
            .map_err(|e| e.to_string())?;
        cx.update().map_err(|e| e.to_string())?;
        Ok(Offline {
            cx,
            processor,
            frames,
            sample_rate,
            block_frames: block_frames as usize,
            played: Duration::ZERO,
            stereo: vec![0.0; block_frames as usize * 2],
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Hands the graph `frame`, as the game does each frame, then makes
    /// `seconds` of sound, rounded up to whole blocks: the left channel's
    /// samples, from −1 to 1 at full scale. The right channel is the same.
    pub fn play(&mut self, frame: QuadFrame, seconds: f64) -> Vec<f32> {
        self.frames.write(frame);
        let wanted = (seconds * f64::from(self.sample_rate)).ceil() as usize;
        let mut left = Vec::with_capacity(wanted + self.block_frames);
        while left.len() < wanted {
            let _ = self.cx.update();
            let n = self.block_frames;
            let silence: [f32; 0] = [];
            let input = InterleavedSlice::new(&silence[..], 0, n);
            let output = InterleavedSlice::new_mut(&mut self.stereo[..], 2, n);
            let (Ok(input), Ok(mut output)) = (input, output) else {
                break;
            };
            self.processor.process(
                &input,
                &mut output,
                BackendProcessInfo {
                    frames: n,
                    process_timestamp: Some(Instant::now()),
                    duration_since_stream_start: self.played,
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                    process_to_playback_delay: None,
                },
            );
            self.played += Duration::from_secs_f64(n as f64 / f64::from(self.sample_rate));
            left.extend(self.stereo.chunks(2).map(|frame| frame[0]));
        }
        left
    }
}
