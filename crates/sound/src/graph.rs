//! The Firewheel graph, the same live and offline:
//!
//! ```text
//! the four motor voices and the frame hum --> On the Quad --> the device
//! ```

use firewheel::{FirewheelConfig, FirewheelContext};

use crate::on_the_quad::OnTheQuadNode;
use crate::voice::{Frames, FramesSlot, VoiceNode};
use crate::{QuadFrame, QuadSound};

/// Hands each new Quad state to the graph's nodes, never waiting for them.
pub(crate) struct FrameWriter([triple_buffer::Input<QuadFrame>; 2]);

impl FrameWriter {
    pub(crate) fn write(&mut self, frame: QuadFrame) {
        for input in &mut self.0 {
            input.write(frame);
        }
    }
}

/// Where each of the graph's two nodes reads the newest Quad state.
pub(crate) struct FrameReaders([Frames; 2]);

/// A writer and the readers it feeds.
pub(crate) fn frames() -> (FrameWriter, FrameReaders) {
    let (voice_in, voice_out) = triple_buffer::triple_buffer(&QuadFrame::default());
    let (mic_in, mic_out) = triple_buffer::triple_buffer(&QuadFrame::default());
    (
        FrameWriter([voice_in, mic_in]),
        FrameReaders([voice_out, mic_out]),
    )
}

/// A graph making `quad`'s sound from what `readers` read, not yet on a
/// device. Firewheel's context stays on the thread that makes it.
pub(crate) fn graph(quad: QuadSound, readers: FrameReaders) -> Result<FirewheelContext, String> {
    let FrameReaders([voice_frames, mic_frames]) = readers;
    let mut cx = FirewheelContext::new(FirewheelConfig::default());
    let voice = cx
        .add_node(VoiceNode { quad }, Some(FramesSlot::new(voice_frames)))
        .map_err(|e| e.to_string())?;
    let on_the_quad = cx
        .add_node(OnTheQuadNode { quad }, Some(FramesSlot::new(mic_frames)))
        .map_err(|e| e.to_string())?;
    let out = cx.graph_out_node_id();
    cx.connect(voice, on_the_quad, &[(0, 0)], false)
        .map_err(|e| e.to_string())?;
    cx.connect(on_the_quad, out, &[(0, 0), (1, 1)], false)
        .map_err(|e| e.to_string())?;
    Ok(cx)
}
