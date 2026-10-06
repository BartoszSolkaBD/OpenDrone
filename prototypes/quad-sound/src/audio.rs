//! PROTOTYPE (#34). The Firewheel graph, used directly (no Bevy):
//!
//! ```text
//! QuadVoiceNode --0--> ListenerNode --0--> Quad volume ----\
//!               --1-->              --1--> Crashes volume --+--> Master --> out
//! hit samplers ----1-->                                    |
//! background sampler -----------------> Background volume -+
//! menu samplers ----------------------> Menus volume ------/
//! ```
//!
//! The same graph runs live on the sound device (cpal) or offline into a WAV file.

use crate::quads::{QuadKind, Tuning, Volumes};
use crate::synth::{AudioStats, ListenerNode, NodeShared, QuadVoiceNode, SoundFrame};
use firewheel::{
    channel_config::NonZeroChannelCount,
    diff::{Diff, PathBuilder},
    nodes::{
        sampler::{RepeatMode, SamplerConfig, SamplerNode},
        volume::{VolumeNode, VolumeNodeConfig},
    },
    node::NodeID,
    FirewheelConfig, FirewheelContext, Volume,
};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Resource = firewheel::collector::ArcGc<dyn firewheel::sample_resource::SampleResource + Send + Sync + 'static>;

#[derive(serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ClipEntry {
    pub group: String,
    pub file: String,
    pub title: String,
    pub author: String,
    pub source_page: String,
    pub licence: String,
    pub sha256: String,
    pub duration_s: f32,
    pub notes: String,
}

#[derive(serde::Deserialize, Default)]
struct Manifest {
    clip: Vec<ClipEntry>,
}

pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

pub fn load_manifest() -> Vec<ClipEntry> {
    let p = assets_dir().join("clips.toml");
    match std::fs::read_to_string(&p).ok().and_then(|s| toml::from_str::<Manifest>(&s).ok()) {
        Some(m) => m.clip,
        None => {
            eprintln!("[clips] no manifest at {}", p.display());
            Vec::new()
        }
    }
}

/// Decode a clip to f32 at the stream's rate. Long "hit" recordings are cut around their loudest
/// moment (from 50 ms before to 600 ms after), so they work as one hit.
pub fn decode_clip(file: &str, sr: u32, trim_hit: bool) -> Option<Vec<Vec<f32>>> {
    let path = assets_dir().join(file);
    let probed = symphonium::probe_from_file(&path, None).ok()?;
    let decoded = symphonium::decode_f32(probed, &Default::default(), NonZeroU32::new(sr), None, None).ok()?;
    let mut data = decoded.data;
    if trim_hit && data[0].len() > (sr as usize) * 3 / 2 {
        let ch0 = &data[0];
        let win = (sr as usize) / 200;
        let mut best = (0usize, 0.0f32);
        let mut prev = 0.0;
        for (k, c) in ch0.chunks(win).enumerate() {
            let e: f32 = c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32;
            // A transient: energy jumping above the previous window.
            let score = e - prev * 0.5;
            if score > best.1 {
                best = (k * win, score);
            }
            prev = e;
        }
        let start = best.0.saturating_sub(sr as usize / 20);
        let end = (best.0 + sr as usize * 6 / 10).min(ch0.len());
        for ch in data.iter_mut() {
            *ch = ch[start..end].to_vec();
            let n = ch.len();
            let fade = (sr as usize / 50).min(n / 4);
            for i in 0..fade {
                ch[n - 1 - i] *= i as f32 / fade as f32;
            }
        }
    }
    // Files play as recorded (#34 reaction 14): the Map's level and the sound block's hit level
    // set the loudness.
    Some(data)
}

pub struct Engine {
    pub cx: FirewheelContext,
    pub stats: Arc<AudioStats>,
    pub sr: u32,
    voice_id: NodeID,
    listener_id: NodeID,
    vol_ids: [NodeID; 5],
    vols: [VolumeNode; 5],
    hit_ids: [NodeID; 3],
    hits: [SamplerNode; 3],
    hit_rr: usize,
    bg_id: NodeID,
    bg: SamplerNode,
    bg_file: String,
    menu_ids: [NodeID; 2],
    menus: [SamplerNode; 2],
    menu_rr: usize,
    pub voice: QuadVoiceNode,
    pub listener: ListenerNode,
    clip_cache: HashMap<String, Resource>,
    pub manifest: Vec<ClipEntry>,
    pub paused: bool,
}

const MASTER: usize = 0;
const QUAD: usize = 1;
const CRASH: usize = 2;
const BG: usize = 3;
const MENU: usize = 4;

impl Engine {
    /// Build the graph. Returns the engine and the two frame writers (voice, listener) the
    /// simulation publishes each tick to.
    pub fn new(tuning: &Tuning, kind: QuadKind) -> (Engine, [triple_buffer::Input<SoundFrame>; 2]) {
        let mut cx = FirewheelContext::new(FirewheelConfig::default());
        let stats = Arc::new(AudioStats::default());
        let (in_v, out_v) = triple_buffer::triple_buffer(&SoundFrame::default());
        let (in_l, out_l) = triple_buffer::triple_buffer(&SoundFrame::default());
        let voice = QuadVoiceNode { block: *tuning.block(kind), standing: tuning.listener.where_you_stand };
        let listener = ListenerNode { p: tuning.listener };
        let voice_id = cx
            .add_node(voice, Some(NodeShared { frames: Some(Arc::new(Mutex::new(Some(out_v)))), stats: Some(stats.clone()) }))
            .unwrap();
        let listener_id = cx
            .add_node(listener, Some(NodeShared { frames: Some(Arc::new(Mutex::new(Some(out_l)))), stats: Some(stats.clone()) }))
            .unwrap();
        let mono = Some(VolumeNodeConfig { channels: NonZeroChannelCount::MONO });
        let stereo = Some(VolumeNodeConfig { channels: NonZeroChannelCount::STEREO });
        let v = tuning.volumes;
        let vols = [
            VolumeNode::from_linear(v.master),
            VolumeNode::from_linear(v.quad),
            VolumeNode::from_linear(v.crashes),
            VolumeNode::from_linear(v.background),
            VolumeNode::from_linear(v.menus),
        ];
        let vol_ids = [
            cx.add_node(vols[0], stereo).unwrap(),
            cx.add_node(vols[1], mono).unwrap(),
            cx.add_node(vols[2], mono).unwrap(),
            cx.add_node(vols[3], stereo).unwrap(),
            cx.add_node(vols[4], stereo).unwrap(),
        ];
        let out = cx.graph_out_node_id();
        cx.connect(voice_id, listener_id, &[(0, 0), (1, 1)], false).unwrap();
        cx.connect(listener_id, vol_ids[QUAD], &[(0, 0)], false).unwrap();
        cx.connect(listener_id, vol_ids[CRASH], &[(1, 0)], false).unwrap();
        cx.connect(vol_ids[QUAD], vol_ids[MASTER], &[(0, 0), (0, 1)], false).unwrap();
        cx.connect(vol_ids[CRASH], vol_ids[MASTER], &[(0, 0), (0, 1)], false).unwrap();
        cx.connect(vol_ids[BG], vol_ids[MASTER], &[(0, 0), (1, 1)], false).unwrap();
        cx.connect(vol_ids[MENU], vol_ids[MASTER], &[(0, 0), (1, 1)], false).unwrap();
        cx.connect(vol_ids[MASTER], out, &[(0, 0), (1, 1)], false).unwrap();

        let mono_sampler = Some(SamplerConfig { channels: NonZeroChannelCount::MONO, ..Default::default() });
        let hits = [SamplerNode::default(); 3];
        let hit_ids = [
            cx.add_node(hits[0], mono_sampler).unwrap(),
            cx.add_node(hits[1], mono_sampler).unwrap(),
            cx.add_node(hits[2], mono_sampler).unwrap(),
        ];
        for id in hit_ids {
            cx.connect(id, listener_id, &[(0, 1)], false).unwrap();
        }
        let bg = SamplerNode { repeat_mode: RepeatMode::RepeatEndlessly, ..Default::default() };
        let bg_id = cx.add_node(bg, None).unwrap();
        cx.connect(bg_id, vol_ids[BG], &[(0, 0), (1, 1)], false).unwrap();
        let menus = [SamplerNode::default(); 2];
        let menu_ids = [cx.add_node(menus[0], None).unwrap(), cx.add_node(menus[1], None).unwrap()];
        for id in menu_ids {
            cx.connect(id, vol_ids[MENU], &[(0, 0), (1, 1)], false).unwrap();
        }
        let engine = Engine {
            cx,
            stats,
            sr: 48000,
            voice_id,
            listener_id,
            vol_ids,
            vols,
            hit_ids,
            hits,
            hit_rr: 0,
            bg_id,
            bg,
            bg_file: String::new(),
            menu_ids,
            menus,
            menu_rr: 0,
            voice,
            listener,
            clip_cache: HashMap::new(),
            manifest: load_manifest(),
            paused: false,
        };
        (engine, [in_v, in_l])
    }

    pub fn active(&self) -> bool {
        self.cx.is_active()
    }

    /// Push the current tuning into the nodes (only what changed becomes events).
    pub fn apply_tuning(&mut self, tuning: &Tuning, kind: QuadKind) {
        let new_voice = QuadVoiceNode { block: *tuning.block(kind), standing: tuning.listener.where_you_stand };
        let new_listener = ListenerNode { p: tuning.listener };
        // While no stream runs, nothing is sent: the processors are built from the values the
        // nodes were added with, so the next activation diffs against those.
        if self.active() {
            new_voice.diff(&self.voice, PathBuilder::default(), &mut self.cx.event_queue(self.voice_id));
            new_listener.diff(&self.listener, PathBuilder::default(), &mut self.cx.event_queue(self.listener_id));
            self.voice = new_voice;
            self.listener = new_listener;
        }
        self.set_volumes(&tuning.volumes, tuning);
    }

    fn set_volumes(&mut self, v: &Volumes, tuning: &Tuning) {
        let bg_db = if self.bg_file == tuning.clips.bando_background {
            tuning.clips.bando_level_db
        } else {
            tuning.clips.skate_park_level_db
        };
        let bg_level = 10f32.powf(bg_db / 20.0);
        let pause = if self.paused { v.pause_background } else { 1.0 };
        // Sliders use Firewheel's Volume::Linear (amplitude = value²). The Map's level and the
        // pause dip are amplitudes, so they go in under the square root.
        let want = [v.master, v.quad, v.crashes, v.background * (bg_level * pause).max(0.0).sqrt(), v.menus];
        for k in 0..5 {
            let new = VolumeNode { volume: Volume::Linear(want[k]), ..self.vols[k] };
            if self.active() {
                new.diff(&self.vols[k], PathBuilder::default(), &mut self.cx.event_queue(self.vol_ids[k]));
                self.vols[k] = new;
            }
        }
    }

    fn clip(&mut self, file: &str, trim_hit: bool) -> Option<Resource> {
        if file.is_empty() {
            return None;
        }
        let key = format!("{file}@{}", self.sr);
        if let Some(c) = self.clip_cache.get(&key) {
            return Some(c.clone());
        }
        let data: Resource = decode_clip(file, self.sr, trim_hit)?.into();
        self.clip_cache.insert(key, data.clone());
        Some(data)
    }

    pub fn play_hit(&mut self, file: &str, gain: f32) {
        if !self.active() {
            return;
        }
        let Some(data) = self.clip(file, true) else { return };
        let k = self.hit_rr;
        self.hit_rr = (k + 1) % 3;
        let id = self.hit_ids[k];
        self.cx.queue_event_for(id, SamplerNode::set_dyn_sample_event(data));
        let mut new = self.hits[k];
        // Firewheel's Volume::Linear is a slider value (amplitude = value²); this gain is an amplitude.
        new.volume = Volume::Linear(gain.max(0.0).sqrt());
        new.start_or_restart();
        new.diff(&self.hits[k], PathBuilder::default(), &mut self.cx.event_queue(id));
        self.hits[k] = new;
    }

    pub fn play_menu(&mut self, file: &str) {
        if !self.active() {
            return;
        }
        let Some(data) = self.clip(file, false) else { return };
        let k = self.menu_rr;
        self.menu_rr = (k + 1) % 2;
        let id = self.menu_ids[k];
        self.cx.queue_event_for(id, SamplerNode::set_dyn_sample_event(data));
        let mut new = self.menus[k];
        new.start_or_restart();
        new.diff(&self.menus[k], PathBuilder::default(), &mut self.cx.event_queue(id));
        self.menus[k] = new;
    }

    pub fn set_background(&mut self, file: &str, tuning: &Tuning) {
        if !self.active() || file == self.bg_file {
            return;
        }
        self.bg_file = file.to_string();
        let Some(data) = self.clip(file, false) else { return };
        self.cx.queue_event_for(self.bg_id, SamplerNode::set_dyn_sample_event(data));
        let mut new = self.bg;
        new.start_or_restart();
        new.diff(&self.bg, PathBuilder::default(), &mut self.cx.event_queue(self.bg_id));
        self.bg = new;
        self.set_volumes(&tuning.volumes, tuning);
    }

    pub fn set_paused(&mut self, paused: bool, tuning: &Tuning) {
        self.paused = paused;
        self.set_volumes(&tuning.volumes, tuning);
    }

    pub fn update(&mut self) {
        if let Err(e) = self.cx.update() {
            eprintln!("[audio] update error: {e:?}");
        }
    }
}
