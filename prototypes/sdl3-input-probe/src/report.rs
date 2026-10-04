//! PROTOTYPE. Turns the raw records into rates, jitter and resolution, and writes
//! `summary.md` (for people), `stats.json` (for agents) and `events.csv` (raw, every change).

use crate::Phase;
use crate::input_thread::{DeviceInfo, InitInfo, Kind, Rec, ThreadResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

/// Gaps longer than this are "the stick was not moving", not a sample interval.
const PAUSE_NS: u64 = 50_000_000;
/// Histogram bucket edges in milliseconds.
const EDGES_MS: [f64; 14] = [0.0, 0.25, 0.5, 0.75, 1.25, 1.75, 2.5, 3.5, 4.5, 6.0, 8.0, 12.0, 20.0, 50.0];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntervalStats {
    pub samples: usize,
    pub intervals: usize,
    pub pauses_excluded: usize,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub p05_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub jitter_std_ms: f64,
    pub rate_from_median_hz: f64,
    pub rate_from_mean_hz: f64,
    /// Most updates seen in any 100 ms window, per second. A slow-moving stick doesn't change
    /// value on every report, so averages understate the report rate; the busiest window doesn't.
    #[serde(default)]
    pub peak_100ms_hz: f64,
    /// Share of gaps that land within one poll period of a whole multiple of 1, 2 and 4 ms.
    /// A device reporting every N ms only ever produces gaps that are multiples of N.
    #[serde(default)]
    pub on_grid_share: Vec<(f64, f64)>,
    pub histogram: Vec<(String, usize)>,
}

const GRIDS_MS: [f64; 3] = [1.0, 2.0, 4.0];
const GRID_TOLERANCE_MS: f64 = 0.4;

impl IntervalStats {
    pub fn grid(&self, g: f64) -> f64 {
        self.on_grid_share.iter().find(|(x, _)| *x == g).map(|(_, s)| *s).unwrap_or(f64::NAN)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AxisResolution {
    pub axis: u8,
    pub min: i32,
    pub max: i32,
    pub distinct_values: usize,
    pub step_min: Option<i32>,
    pub step_mode: Option<i32>,
    pub levels_full_range: Option<f64>,
    pub bits: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhaseActivity {
    pub phase: String,
    pub axis_changes: BTreeMap<u8, usize>,
    pub buttons_pressed: BTreeSet<u8>,
    pub hat_changes: usize,
    pub most_active_axis: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceReport {
    pub info: DeviceInfo,
    pub device_kind: String,
    /// Instants when at least one axis changed (any-axis), per moving phase.
    pub update_rate: BTreeMap<String, IntervalStats>,
    /// Per axis, during the "circles" phase.
    pub per_axis_circles: BTreeMap<u8, IntervalStats>,
    /// Device-clock spacing of gyro readings (one per report) during the sensors phase.
    pub sensor_device_clock: Option<IntervalStats>,
    /// SDL arrival-time spacing of the same gyro readings.
    pub sensor_arrival: Option<IntervalStats>,
    pub resolution: Vec<AxisResolution>,
    pub axes_that_moved: Vec<u8>,
    pub buttons_seen: Vec<u8>,
    pub activity: Vec<PhaseActivity>,
    pub rest_axis_changes: BTreeMap<u8, usize>,
    pub checks: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FramePhase {
    pub frames: u64,
    pub seconds: f64,
    pub max_frame_ms: f64,
    pub focused_frames: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunMeta {
    pub label: String,
    pub started_utc: String,
    pub os: String,
    pub quick: bool,
    pub bevy_gilrs_enabled: bool,
    pub launched_as_app_bundle: bool,
    pub input_monitoring_at_start: String,
    pub input_monitoring_at_end: String,
    pub duration_s: f64,
}

#[derive(Serialize)]
struct Stats<'a> {
    meta: &'a RunMeta,
    init: &'a InitInfo,
    poll_loop: BTreeMap<String, IntervalStats>,
    main_thread_frames: BTreeMap<String, FramePhase>,
    devices: Vec<DeviceReport>,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn histogram(ms: &[f64]) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for w in EDGES_MS.windows(2) {
        let c = ms.iter().filter(|&&x| x >= w[0] && x < w[1]).count();
        out.push((format!("{:.2}-{:.2} ms", w[0], w[1]), c));
    }
    out
}

/// `ts` are instants in nanoseconds, ascending. Duplicates are collapsed (one poll or report
/// produces several axis events with the same timestamp).
pub fn interval_stats(ts: &[u64], pause_ns: u64) -> Option<IntervalStats> {
    let mut uniq: Vec<u64> = ts.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    if uniq.len() < 3 {
        return None;
    }
    let mut ms = Vec::with_capacity(uniq.len());
    let mut pauses = 0;
    for w in uniq.windows(2) {
        let d = w[1] - w[0];
        if d > pause_ns {
            pauses += 1;
        } else {
            ms.push(d as f64 / 1e6);
        }
    }
    if ms.len() < 2 {
        return None;
    }
    let hist = histogram(&ms);
    let n = ms.len() as f64;
    let mean = ms.iter().sum::<f64>() / n;
    let var = ms.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let mut sorted = ms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = percentile(&sorted, 0.5);
    let mut peak = 0usize;
    let mut j = 0usize;
    for i in 0..uniq.len() {
        while uniq[i] - uniq[j] >= 100_000_000 {
            j += 1;
        }
        peak = peak.max(i - j + 1);
    }
    let on_grid_share = GRIDS_MS
        .iter()
        .map(|&g| {
            let on = ms.iter().filter(|&&x| (x / g).round() >= 1.0 && (x - (x / g).round() * g).abs() <= GRID_TOLERANCE_MS).count();
            (g, on as f64 / ms.len() as f64)
        })
        .collect();
    Some(IntervalStats {
        peak_100ms_hz: peak as f64 * 10.0,
        on_grid_share,
        samples: uniq.len(),
        intervals: ms.len(),
        pauses_excluded: pauses,
        mean_ms: mean,
        min_ms: sorted[0],
        p05_ms: percentile(&sorted, 0.05),
        median_ms: median,
        p95_ms: percentile(&sorted, 0.95),
        max_ms: *sorted.last().unwrap(),
        jitter_std_ms: var.sqrt(),
        rate_from_median_hz: 1000.0 / median,
        rate_from_mean_hz: 1000.0 / mean,
        histogram: hist,
    })
}

fn device_kind(info: &DeviceInfo) -> String {
    let (v, p) = info.vendor_product();
    if v == 0x1209 && p == 0x4F54 {
        "Radio (EdgeTX USB joystick, e.g. Radiomaster Pocket)".into()
    } else if v == 0x054C && (p == 0x0CE6 || p == 0x0DF2) {
        let how = if info.connection == "wireless" || info.bus == "Bluetooth" { "Bluetooth" } else { "USB" };
        format!("Gamepad (DualSense{}) over {how}", if p == 0x0DF2 { " Edge" } else { "" })
    } else {
        "Other Input Device".into()
    }
}

fn resolution(recs: &[&Rec], axis: u8) -> AxisResolution {
    let vals: BTreeSet<i32> = recs.iter().filter(|r| r.kind == Kind::Axis && r.idx == axis).map(|r| r.value).collect();
    let v: Vec<i32> = vals.iter().copied().collect();
    let mut gaps: BTreeMap<i32, usize> = BTreeMap::new();
    for w in v.windows(2) {
        *gaps.entry(w[1] - w[0]).or_default() += 1;
    }
    let step_min = gaps.keys().next().copied();
    let step_mode = gaps.iter().max_by_key(|(_, c)| **c).map(|(g, _)| *g);
    let levels = step_mode.map(|s| 65535.0 / s as f64 + 1.0);
    AxisResolution {
        axis,
        min: v.first().copied().unwrap_or(0),
        max: v.last().copied().unwrap_or(0),
        distinct_values: v.len(),
        step_min,
        step_mode,
        levels_full_range: levels,
        bits: levels.map(|l| l.log2()),
    }
}

fn analyse_device(info: &DeviceInfo, all: &[Rec]) -> DeviceReport {
    let recs: Vec<&Rec> = all.iter().filter(|r| r.dev == info.instance_id).collect();
    let kind = device_kind(info);

    let mut update_rate = BTreeMap::new();
    let all_ts: Vec<u64> = recs.iter().filter(|r| r.kind == Kind::Axis).map(|r| r.t_sdl).collect();
    if let Some(s) = interval_stats(&all_ts, PAUSE_NS) {
        update_rate.insert(ALL_STEPS.to_string(), s);
    }
    for ph in [Phase::Circles, Phase::Unfocused, Phase::Sensors, Phase::Extremes] {
        let ts: Vec<u64> =
            recs.iter().filter(|r| r.kind == Kind::Axis && r.phase == ph as u8).map(|r| r.t_sdl).collect();
        if let Some(s) = interval_stats(&ts, PAUSE_NS) {
            update_rate.insert(ph.key().to_string(), s);
        }
    }

    let mut per_axis_circles = BTreeMap::new();
    for a in 0..info.num_axes.max(0) as u8 {
        let ts: Vec<u64> = recs
            .iter()
            .filter(|r| r.kind == Kind::Axis && r.idx == a && r.phase == Phase::Circles as u8)
            .map(|r| r.t_sdl)
            .collect();
        if let Some(s) = interval_stats(&ts, PAUSE_NS) {
            per_axis_circles.insert(a, s);
        }
    }

    let gyro: Vec<&&Rec> = recs.iter().filter(|r| r.kind == Kind::Gyro).collect();
    let sensor_device_clock = interval_stats(&gyro.iter().map(|r| r.sensor_ts).collect::<Vec<_>>(), PAUSE_NS);
    let sensor_arrival = interval_stats(&gyro.iter().map(|r| r.t_sdl).collect::<Vec<_>>(), PAUSE_NS);

    let resolution_v: Vec<AxisResolution> = (0..info.num_axes.max(0) as u8).map(|a| resolution(&recs, a)).collect();
    let axes_that_moved: Vec<u8> = resolution_v.iter().filter(|r| r.distinct_values > 1).map(|r| r.axis).collect();
    let buttons_seen: Vec<u8> = recs
        .iter()
        .filter(|r| r.kind == Kind::Button && r.value != 0)
        .map(|r| r.idx)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut activity = Vec::new();
    for ph in Phase::ALL {
        if matches!(ph, Phase::Done | Phase::GetReady) {
            continue;
        }
        let mut axis_changes: BTreeMap<u8, usize> = BTreeMap::new();
        let mut buttons_pressed = BTreeSet::new();
        let mut hat_changes = 0;
        for r in recs.iter().filter(|r| r.phase == ph as u8) {
            match r.kind {
                Kind::Axis => *axis_changes.entry(r.idx).or_default() += 1,
                Kind::Button if r.value != 0 => {
                    buttons_pressed.insert(r.idx);
                }
                Kind::Hat => hat_changes += 1,
                _ => {}
            }
        }
        let most_active_axis = axis_changes.iter().max_by_key(|(_, c)| **c).map(|(a, _)| *a);
        activity.push(PhaseActivity {
            phase: ph.key().to_string(),
            axis_changes,
            buttons_pressed,
            hat_changes,
            most_active_axis,
        });
    }
    let rest_axis_changes = activity.iter().find(|a| a.phase == Phase::Rest.key()).map(|a| a.axis_changes.clone()).unwrap_or_default();

    let mut rep = DeviceReport {
        info: info.clone(),
        device_kind: kind,
        update_rate,
        per_axis_circles,
        sensor_device_clock,
        sensor_arrival,
        resolution: resolution_v,
        axes_that_moved,
        buttons_seen,
        activity,
        rest_axis_changes,
        checks: Vec::new(),
    };
    rep.checks = checks(&rep);
    rep.checks.extend(rest_and_disconnect(&rep, &recs));
    rep
}

fn verdict(ok: bool) -> &'static str {
    if ok { "MATCHES" } else { "DIFFERS" }
}

const ALL_STEPS: &str = "all steps";

/// One sentence on the report rate, built from the busiest 100 ms and the 1/2/4 ms grid test.
fn rate_line(what: &str, s: &IntervalStats, expect: &str, lo: f64, hi: f64, grid_ms: f64) -> String {
    let ok = (lo..hi).contains(&s.peak_100ms_hz) && s.grid(grid_ms) >= 0.9;
    format!(
        "{what}: expected {expect}. Busiest 100 ms: {:.0} updates per second. Gaps between updates on a 1 ms grid: {:.1}%, 2 ms: {:.1}%, 4 ms: {:.1}%. \
Average while moving: {:.0} per second (lower whenever a stick moves too slowly to change value on every report). {}",
        s.peak_100ms_hz,
        100.0 * s.grid(1.0),
        100.0 * s.grid(2.0),
        100.0 * s.grid(4.0),
        s.rate_from_mean_hz,
        verdict(ok)
    )
}

/// Extra lines every device gets: what happens at rest, and at disconnect.
fn rest_and_disconnect(rep: &DeviceReport, recs: &[&Rec]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(a) = rep.activity.iter().find(|a| a.phase == Phase::Rest.key()) {
        let n: usize = a.axis_changes.values().sum();
        out.push(format!(
            "Hands-off step: {n} axis changes. SDL only reports changes, so a still stick sends nothing; silence alone can't tell a resting stick from a stalled device."
        ));
    }
    if let Some(rm) = recs.iter().find(|r| r.kind == Kind::Removed) {
        let same: Vec<String> = recs
            .iter()
            .filter(|r| r.kind == Kind::Axis && r.t_rx == rm.t_rx)
            .map(|r| format!("axis {} -> {}", r.idx, r.value))
            .collect();
        out.push(format!(
            "Disconnected: SDL reported the removal {:.2} s into the run (the probe can't see when the cable was actually pulled).{}",
            rm.t_rx as f64 / 1e9,
            if same.is_empty() {
                String::new()
            } else {
                format!(" In the same poll it also delivered last-moment value changes that the pilot didn't make: {}.", same.join(", "))
            }
        ));
    }
    out
}

/// Plain-language comparison with the research's expectations (docs/research/input-devices.md).
fn checks(rep: &DeviceReport) -> Vec<String> {
    let mut out = Vec::new();
    let (v, p) = rep.info.vendor_product();
    let circles = rep.update_rate.get(Phase::Circles.key());
    let bits = rep
        .resolution
        .iter()
        .filter(|r| r.distinct_values > 50)
        .filter_map(|r| r.bits)
        .fold(f64::NAN, f64::max);
    let _ = circles;
    if v == 0x1209 && p == 0x4F54 {
        match rep.update_rate.get(ALL_STEPS) {
            Some(s) => out.push(rate_line("Report rate", s, "about 1000 Hz with RF off", 800.0, 1100.0, 1.0)),
            None => out.push("Report rate: no stick movement recorded.".into()),
        }
        let centred: Vec<usize> =
            rep.info.initial_axis_state.iter().enumerate().filter(|(_, v)| v.is_some_and(|v| (v as i32).abs() < 1000)).map(|(i, _)| i).collect();
        let bottom: Vec<usize> = rep.info.initial_axis_state.iter().take(4).enumerate().filter(|(_, v)| v.is_some_and(|v| v < -30000)).map(|(i, _)| i).collect();
        out.push(format!(
            "Stick axes at start: centred {:?}, at the bottom {:?}. EdgeTX's AETR order expects roll 0, pitch 1, throttle 2 (rests at the bottom), yaw 3.",
            centred.into_iter().filter(|i| *i < 4).collect::<Vec<_>>(),
            bottom
        ));
        out.push(format!(
            "Axes: expected 8 (CH1-CH8); SDL reports {}; {} of them moved during the run (axes {:?}). {}",
            rep.info.num_axes,
            rep.axes_that_moved.len(),
            rep.axes_that_moved,
            verdict(rep.info.num_axes == 8)
        ));
        out.push(format!(
            "Buttons: expected 24 (CH9-CH32); SDL reports {}; pressed during the run: {:?}.",
            rep.info.num_buttons, rep.buttons_seen
        ));
        out.push(format!(
            "Resolution: expected about 11 bits (0-2048); measured about {:.1} bits on the busiest axes. {}",
            bits,
            verdict((10.5..11.6).contains(&bits))
        ));
        if let Some(y) = rep.activity.iter().find(|a| a.phase == Phase::Yaw.key()) {
            let stick_changes: usize = y.axis_changes.iter().filter(|(a, _)| **a < 4).map(|(_, c)| c).sum();
            out.push(if stick_changes < 50 {
                "Yaw-only step: the sticks were not moved in this step, so it can't single out the yaw axis.".to_string()
            } else {
                format!(
                    "Yaw-only step: the most active axis was {:?} (EdgeTX AETR puts yaw/rudder on CH4, which is axis 3).",
                    y.most_active_axis
                )
            });
        }
        let ch58: Vec<u8> = rep.axes_that_moved.iter().copied().filter(|a| (4..8).contains(a)).collect();
        out.push(format!(
            "Channels 5-8 (axes 4-7): moved = {:?}. They only move if the radio model mixes switches or the pot onto CH5-CH8.",
            ch58
        ));
    } else if v == 0x054C {
        let bt = rep.info.connection == "wireless" || rep.info.bus == "Bluetooth";
        let (lo, hi, expect, grid) =
            if bt { (700.0, 1100.0, "about 800-1000 Hz over Bluetooth", 1.0) } else { (200.0, 300.0, "250 Hz over USB", 4.0) };
        for (key, what) in [
            (Phase::Circles.key(), "Stick report rate, sensors off (circles step)"),
            (Phase::Sensors.key(), "Stick report rate, sensors on (sensors step)"),
        ] {
            if let Some(c) = rep.update_rate.get(key) {
                out.push(rate_line(what, c, expect, lo, hi, grid));
            }
        }
        if let Some(s) = &rep.sensor_device_clock {
            out.push(format!(
                "Report rate from the controller's own clock (gyro timestamps): {:.0} Hz (mean {:.3} ms, median {:.3} ms). SDL claims {:?} Hz. {}",
                s.rate_from_mean_hz,
                s.mean_ms,
                s.median_ms,
                rep.info.sdl_sensor_rate_hz,
                verdict((lo..hi).contains(&s.rate_from_mean_hz))
            ));
        }
        out.push(format!(
            "Resolution: expected 8 bits (hardware limit); measured about {:.1} bits on the sticks. {}",
            bits,
            verdict((7.5..8.6).contains(&bits))
        ));
    }
    match rep.update_rate.get(Phase::Unfocused.key()) {
        Some(u) => out.push(format!(
            "With the probe window NOT focused: busiest 100 ms {:.0} updates per second, average {:.0}. Input keeps flowing without focus if this is close to the focused steps.",
            u.peak_100ms_hz, u.rate_from_mean_hz
        )),
        None => out.push("Window-not-focused step: no stick movement recorded, so focus independence is untested in this run.".into()),
    }
    out
}

/// Map entries keyed by step name, in the order the steps ran ("whole run" first).
fn in_step_order<V>(m: &BTreeMap<String, V>) -> Vec<(&String, &V)> {
    let order = |k: &str| {
        if k == "whole run" {
            return 0;
        }
        crate::STEPS
            .iter()
            .position(|p| p.key() == k)
            .map(|i| 2 * i + 2)
            .unwrap_or(1) // get-ready right after "whole run"
    };
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by_key(|(k, _)| order(k));
    v
}

fn fmt_stats_row(name: &str, s: &IntervalStats) -> String {
    format!(
        "| {name} | {} | {:.0} /s | {:.0} /s | {:.0}% / {:.0}% / {:.0}% | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
        s.samples,
        s.peak_100ms_hz,
        s.rate_from_mean_hz,
        100.0 * s.grid(1.0),
        100.0 * s.grid(2.0),
        100.0 * s.grid(4.0),
        s.mean_ms,
        s.min_ms,
        s.median_ms,
        s.p95_ms,
        s.max_ms,
        s.jitter_std_ms,
        s.pauses_excluded
    )
}

const STATS_HEADER: &str = "| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n";

fn histogram_md(s: &IntervalStats) -> String {
    let total: usize = s.histogram.iter().map(|(_, c)| c).sum();
    let mut out = String::from("| Interval | Count | Share |\n|---|---|---|\n");
    for (b, c) in &s.histogram {
        if *c > 0 {
            let _ = writeln!(out, "| {b} | {c} | {:.1}% |", 100.0 * *c as f64 / total.max(1) as f64);
        }
    }
    out
}

pub fn write_all(
    dir: &Path,
    meta: &RunMeta,
    result: &ThreadResult,
    frames: &BTreeMap<String, FramePhase>,
) -> std::io::Result<String> {
    std::fs::create_dir_all(dir)?;

    // Raw events.
    {
        let f = std::fs::File::create(dir.join("events.csv"))?;
        let mut w = std::io::BufWriter::new(f);
        writeln!(w, "t_sdl_ns,t_rx_ns,phase,device,kind,index,value,sensor_ts_ns")?;
        for r in &result.records {
            writeln!(
                w,
                "{},{},{},{},{},{},{},{}",
                r.t_sdl,
                r.t_rx,
                Phase::from_u8(r.phase).key(),
                r.dev,
                r.kind.as_str(),
                r.idx,
                r.value,
                r.sensor_ts
            )?;
        }
    }

    // Poll loop timing per phase.
    let mut poll_loop = BTreeMap::new();
    {
        let mut acc: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
        let mut t: BTreeMap<u8, u64> = BTreeMap::new();
        for (ph, us) in &result.poll_intervals {
            let e = t.entry(*ph).or_default();
            *e += *us as u64 * 1000;
            acc.entry(*ph).or_default().push(*e);
        }
        let mut all_t = 0u64;
        let mut all = Vec::with_capacity(result.poll_intervals.len());
        for (_, us) in &result.poll_intervals {
            all_t += *us as u64 * 1000;
            all.push(all_t);
        }
        if let Some(s) = interval_stats(&all, u64::MAX) {
            poll_loop.insert("whole run".to_string(), s);
        }
        for (ph, ts) in acc {
            if let Some(s) = interval_stats(&ts, u64::MAX) {
                poll_loop.insert(Phase::from_u8(ph).key().to_string(), s);
            }
        }
    }

    write_reports(dir, meta, &result.init, &result.devices, &result.records, &poll_loop, frames, None)
}

/// Writes `stats.json` and `summary.md` from the raw records. Used at the end of a run, and by
/// `--reanalyse` to rebuild them from a results folder's `events.csv` and `stats.json`.
#[allow(clippy::too_many_arguments)]
pub fn write_reports(
    dir: &Path,
    meta: &RunMeta,
    init: &InitInfo,
    devices_info: &[DeviceInfo],
    records: &[Rec],
    poll_loop: &BTreeMap<String, IntervalStats>,
    frames: &BTreeMap<String, FramePhase>,
    reanalysed_note: Option<&str>,
) -> std::io::Result<String> {
    let poll_loop = poll_loop.clone();
    let devices: Vec<DeviceReport> = devices_info.iter().map(|d| analyse_device(d, records)).collect();

    let stats = Stats { meta, init, poll_loop: poll_loop.clone(), main_thread_frames: frames.clone(), devices: devices.clone() };
    std::fs::write(dir.join("stats.json"), serde_json::to_string_pretty(&stats).unwrap())?;

    // Human summary.
    let mut md = String::new();
    let _ = writeln!(md, "# SDL3 input probe: {}\n", meta.label);
    if let Some(note) = reanalysed_note {
        let _ = writeln!(md, "> {note}\n");
    }
    let _ = writeln!(
        md,
        "PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.\n"
    );
    let _ = writeln!(md, "- Started: {} (UTC), lasted {:.1} s, {}", meta.started_utc, meta.duration_s, meta.os);
    let _ = writeln!(md, "- SDL {} ({}), built from source, statically linked, joystick + HIDAPI only", init.sdl_version, init.sdl_revision);
    let _ = writeln!(md, "- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) {}", if meta.bevy_gilrs_enabled { "ON (reading the same devices at the same time)" } else { "OFF" });
    let _ = writeln!(md, "- Launched as {}", if meta.launched_as_app_bundle { "its own .app bundle (macOS treats it as a separate app for permissions)" } else { "a plain binary from a terminal (macOS attributes permissions to the terminal app)" });
    if meta.quick {
        let _ = writeln!(md, "- QUICK mode (short steps, for a no-device smoke test)");
    }

    let _ = writeln!(md, "\n## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?\n");
    let i = init;
    let _ = writeln!(
        md,
        "- SDL_Init(GAMEPAD) on thread `{}`: {} in {:.1} ms{}",
        i.thread_name,
        if i.sdl_init_ok { "OK" } else { "FAILED" },
        i.sdl_init_ms,
        if i.sdl_error.is_empty() { String::new() } else { format!(" (error: {})", i.sdl_error) }
    );
    if let Some(m) = i.pthread_main_np {
        let _ = writeln!(md, "- `pthread_main_np()` on that thread = {m} ({}).", if m == 0 { "not the main thread" } else { "MAIN THREAD" });
    }
    if let Some(q) = i.qos_result {
        let _ = writeln!(md, "- Thread priority (QoS user-interactive) requested: result {q} ({}).", if q == 0 { "OK" } else { "failed" });
    }
    let _ = writeln!(md, "- The input thread slept {} µs between polls. Its loop timing (how often it actually ran):\n", i.poll_sleep_us);
    md.push_str(STATS_HEADER);
    for (k, s) in in_step_order(&poll_loop) {
        md.push_str(&fmt_stats_row(&format!("poll loop, {k}"), s));
    }
    let _ = writeln!(md, "\nMain thread (Bevy rendering the window) at the same time:\n");
    let _ = writeln!(md, "| Step | Frames | Avg fps | Longest frame ms | Window focused |\n|---|---|---|---|---|");
    for (k, f) in in_step_order(frames) {
        if f.frames == 0 {
            continue;
        }
        let _ = writeln!(
            md,
            "| {k} | {} | {:.0} | {:.1} | {:.0}% |",
            f.frames,
            f.frames as f64 / f.seconds.max(1e-9),
            f.max_frame_ms,
            100.0 * f.focused_frames as f64 / f.frames as f64
        );
    }

    let _ = writeln!(md, "\n## 2. Input Monitoring permission\n");
    let _ = writeln!(
        md,
        "- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **{}**, at end **{}**.",
        meta.input_monitoring_at_start, meta.input_monitoring_at_end
    );
    let meaning = if meta.input_monitoring_at_start.starts_with("Unknown") && meta.input_monitoring_at_end.starts_with("Unknown") {
        "Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app."
    } else if meta.input_monitoring_at_end.starts_with("Denied") {
        "The app macOS holds responsible (the terminal, unless launched with run.sh) is refused Input Monitoring. If the devices below still delivered input, Input Monitoring is not needed to read them."
    } else if meta.input_monitoring_at_end.starts_with("Granted") {
        "Already granted to the responsible app, so this run cannot show whether a prompt would appear. Use run.sh for a clean test."
    } else {
        "Unexpected status; check whether a macOS dialog appeared."
    };
    let _ = writeln!(md, "- What it means: {meaning}");
    let _ = writeln!(md, "- Also record whether any macOS permission dialog appeared during the run.");

    let _ = writeln!(md, "\n## 3. Devices\n");
    if devices.is_empty() {
        let _ = writeln!(md, "No Input Device was connected during this run.");
    }
    for d in &devices {
        let inf = &d.info;
        let _ = writeln!(md, "### {} — {}\n", inf.name, d.device_kind);
        let _ = writeln!(md, "**Compared with the research**\n");
        for c in &d.checks {
            let _ = writeln!(md, "- {c}");
        }
        let _ = writeln!(md, "\n**Identity**\n");
        let _ = writeln!(md, "| Field | Value |\n|---|---|");
        for (k, v) in [
            ("Name", inf.name.clone()),
            ("Vendor : Product", format!("{} : {}", inf.vendor_id, inf.product_id)),
            ("Product version / firmware", format!("0x{:04X} / 0x{:04X}", inf.product_version, inf.firmware_version)),
            ("Bus / connection", format!("{} / {}", inf.bus, inf.connection)),
            ("SDL driver", inf.sdl_driver.clone()),
            ("SDL joystick type", inf.joystick_type.clone()),
            ("Opened as SDL gamepad", format!("{} {}", inf.is_gamepad, inf.gamepad_type)),
            ("Axes / buttons / hats", format!("{} / {} / {}", inf.num_axes, inf.num_buttons, inf.num_hats)),
            ("Motion sensors", format!("gyro {}, accel {}, SDL rate {:?}", inf.has_gyro, inf.has_accel, inf.sdl_sensor_rate_hz)),
            ("Serial", inf.serial.clone()),
            ("GUID", inf.guid.clone()),
            ("Path", inf.path.clone()),
        ] {
            let _ = writeln!(md, "| {k} | {} |", v.replace('|', "/"));
        }

        let _ = writeln!(
            md,
            "\n**Update rate.** Counts the moments when any axis changed. A new value can only appear when the device sends a report, but a slow stick doesn't change on every report, so averages understate the report rate. Two measures don't depend on stick speed: the **busiest 100 ms**, and the **grid test** (a device reporting every N ms only produces gaps that are whole multiples of N). SDL stamps a change when the input thread polls (about every {:.2} ms here), so single gaps wobble by up to one poll period.\n",
            poll_loop.get("whole run").map(|s| s.mean_ms).unwrap_or(f64::NAN)
        );
        md.push_str(STATS_HEADER);
        if let Some(s) = d.update_rate.get(ALL_STEPS) {
            md.push_str(&fmt_stats_row(&format!("any axis, {ALL_STEPS}"), s));
        }
        for (k, s) in in_step_order(&d.update_rate) {
            if k != ALL_STEPS {
                md.push_str(&fmt_stats_row(&format!("any axis, {k}"), s));
            }
        }
        if let Some(s) = &d.sensor_device_clock {
            md.push_str(&fmt_stats_row("gyro, controller clock (= report rate)", s));
        }
        if let Some(s) = &d.sensor_arrival {
            md.push_str(&fmt_stats_row("gyro, arrival in SDL", s));
        }
        for (a, s) in &d.per_axis_circles {
            md.push_str(&fmt_stats_row(&format!("axis {a}, circles"), s));
        }
        if let Some(c) = d.update_rate.get(ALL_STEPS) {
            let _ = writeln!(md, "\nGaps between updates, any axis, all steps:\n");
            md.push_str(&histogram_md(c));
        }
        if let Some(s) = &d.sensor_device_clock {
            let _ = writeln!(md, "\nInterval histogram, gyro on the controller's clock:\n");
            md.push_str(&histogram_md(s));
        }

        let _ = writeln!(md, "\n**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)\n");
        let _ = writeln!(md, "| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |\n|---|---|---|---|---|---|---|---|");
        for r in &d.resolution {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                r.axis,
                r.min,
                r.max,
                r.distinct_values,
                r.step_min.map(|s| s.to_string()).unwrap_or("-".into()),
                r.step_mode.map(|s| s.to_string()).unwrap_or("-".into()),
                r.levels_full_range.map(|s| format!("{s:.0}")).unwrap_or("-".into()),
                r.bits.map(|s| format!("{s:.1}")).unwrap_or("-".into()),
            );
        }

        let _ = writeln!(md, "\n**What moved in each step** (axis: number of changes)\n");
        let _ = writeln!(md, "| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |\n|---|---|---|---|---|");
        for a in &d.activity {
            let _ = writeln!(
                md,
                "| {} | {:?} | {:?} | {} | {:?} |",
                a.phase, a.axis_changes, a.buttons_pressed, a.hat_changes, a.most_active_axis
            );
        }
        let _ = writeln!(md);
    }

    std::fs::write(dir.join("summary.md"), &md)?;

    let mut short = String::new();
    for d in &devices {
        let _ = writeln!(short, "{} — {}", d.info.name, d.device_kind);
        for c in &d.checks {
            let _ = writeln!(short, "  - {c}");
        }
    }
    Ok(short)
}
