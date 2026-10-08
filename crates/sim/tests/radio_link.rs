//! Readable checks for the Radio Link on its own (ADR-0007, ADR-0020, #27):
//! every Packet Rate, the lock to an Input Device's report beat, a late
//! report, and the silence rule. The recorded DualSense and Pocket replayed
//! through the whole Simulation are the Scenarios in `scenarios/recorded/`.
//!
//! Times are physics steps at 8 kHz: one step is 0.125 ms, a DualSense's
//! 4 ms report period is 32 steps, a Pocket's 1 ms is 8, and the lock's
//! 0.75 ms margin is 6.

use opendrone_sim::{
    Channel, Channels, Frame, InputDeviceFacts, PacketRate, PhysicsRate, RadioLink, ReportRate,
    SimulationTime,
};

const PHYSICS_HZ: u32 = 8000;

fn link(packet_hz: u32, device: Option<InputDeviceFacts>) -> RadioLink {
    RadioLink::new(
        PacketRate::from_hz(packet_hz).unwrap(),
        PhysicsRate::from_hz(PHYSICS_HZ).unwrap(),
        device,
    )
}

/// An Input Device reporting at `report_hz`, at rest or only on change.
fn device(report_hz: u32, reports_at_rest: bool) -> Option<InputDeviceFacts> {
    Some(InputDeviceFacts {
        report_rate: ReportRate::from_hz(report_hz),
        reports_at_rest,
    })
}

/// Roll at a receiver step, the rest at rest.
fn rolled(step: u16) -> Channels {
    Channels {
        roll: Channel::from_step(step).unwrap(),
        ..Channels::RESTING
    }
}

/// Runs a link for `ticks` steps, handing it each report at its step (in
/// order) before the step's frame. Gives back every frame with its step.
fn run(link: &mut RadioLink, reports: &[(u64, Channels)], ticks: u64) -> Vec<(u64, Frame)> {
    let mut frames = Vec::new();
    let mut next = 0;
    for tick in 0..ticks {
        while next < reports.len() && reports[next].0 == tick {
            link.hear(SimulationTime::from_ticks(tick), reports[next].1);
            next += 1;
        }
        if let Some(frame) = link.frame(SimulationTime::from_ticks(tick)) {
            frames.push((tick, frame));
        }
    }
    frames
}

/// A device's reports: one every `period` steps from step `first`, each
/// arriving the given number of steps late, in turn, its roll moving a step
/// with every report so each one is told apart.
fn reports(first: u64, period: u64, late: &[u64], count: u64) -> Vec<(u64, Channels)> {
    (0..count)
        .map(|k| {
            let at = first + k * period + late[(k as usize) % late.len()];
            (at, rolled(992 + (k % 800) as u16))
        })
        .collect()
}

/// A DualSense's arrival wobble, 0 to 0.375 ms (#27 measured 0–0.48 ms).
const WOBBLE: [u64; 7] = [0, 2, 1, 3, 0, 2, 3];

#[test]
fn every_packet_rate_a_pilot_can_pick_sends_that_many_frames_a_second_on_its_own_clock() {
    // Basis: Rule (ADR-0007: 50, 100, 150, 250, 333, 500 or 1000 Hz; with
    // scripted sticks there is no device, so frame k leaves on the first step
    // at or after k ÷ Packet Rate seconds).
    for hz in PacketRate::ALL {
        let mut link = link(hz, None);
        assert!(!link.is_locked());
        let frames = run(&mut link, &[(0, rolled(992))], u64::from(PHYSICS_HZ));
        assert_eq!(frames.len(), hz as usize, "{hz} Hz");
        for (k, (tick, _)) in frames.iter().enumerate() {
            assert_eq!(*tick, (k as u64 * 8000).div_ceil(u64::from(hz)), "{hz} Hz");
        }
    }
}

#[test]
fn the_packet_rates_that_lock_are_those_that_divide_the_devices_report_rate() {
    // Basis: Source (ADR-0020, "Which rates lock": the DualSense over USB at
    // 250 Hz locks at 250 and 50 Hz, the Pocket at 1 kHz at 1000, 500, 333,
    // 250, 100 and 50 Hz; 333 Hz counts as a frame every 3 ms).
    let spans = |report_hz: u32| -> Vec<(u32, Option<u32>)> {
        PacketRate::ALL
            .iter()
            .map(|&hz| {
                let rate = PacketRate::from_hz(hz).unwrap();
                (
                    hz,
                    rate.reports_per_frame(ReportRate::from_hz(report_hz).unwrap()),
                )
            })
            .collect()
    };
    assert_eq!(
        spans(250),
        [
            (50, Some(5)),
            (100, None),
            (150, None),
            (250, Some(1)),
            (333, None),
            (500, None),
            (1000, None)
        ]
    );
    assert_eq!(
        spans(1000),
        [
            (50, Some(20)),
            (100, Some(10)),
            (150, None),
            (250, Some(4)),
            (333, Some(3)),
            (500, Some(2)),
            (1000, Some(1))
        ]
    );
    // A Radio with RF on reports once per ELRS packet, so at its own rate.
    assert_eq!(
        PacketRate::from_hz(333)
            .unwrap()
            .reports_per_frame(ReportRate::from_hz(333).unwrap()),
        Some(1)
    );
    for (hz, device) in [
        (250, device(250, true)),
        (50, device(250, true)),
        (333, device(1000, false)),
    ] {
        assert!(link(hz, device).is_locked(), "{hz} Hz");
    }
    for (hz, device) in [
        (150, device(250, true)),
        (500, device(250, true)),
        (250, None),
        (
            250,
            Some(InputDeviceFacts {
                report_rate: None,
                reports_at_rest: true,
            }),
        ),
    ] {
        assert!(!link(hz, device).is_locked(), "{hz} Hz");
    }
}

#[test]
fn locked_each_frame_leaves_0_75_ms_after_a_report_is_due_and_carries_exactly_one_fresh_report() {
    // Basis: Rule (ADR-0020: the beat is learnt from the reports' stamps; a
    // report is due where the earliest arrives; each frame leaves 0.75 ms
    // after, so it carries exactly one fresh report, always about the same
    // age). Reports due every 32 steps from step 5, each 0 to 3 steps late:
    // frames leave 6 steps after each is due, and carry it 3 to 6 steps old.
    let reports = reports(5, 32, &WOBBLE, 500);
    let mut link = link(250, device(250, true));
    let frames = run(&mut link, &reports, 16_000);
    assert_eq!(frames.len(), 500);
    for (k, (tick, frame)) in frames.iter().enumerate() {
        assert_eq!(*tick, 5 + 32 * k as u64 + 6, "frame {k}");
        assert_eq!(frame.reports, 1, "frame {k}");
        assert_eq!(frame.channels, reports[k].1, "frame {k}");
        assert!(
            (3..=6).contains(&frame.age),
            "frame {k}: {} steps",
            frame.age
        );
    }
}

#[test]
fn at_50_hz_a_locked_frame_carries_five_dualsense_reports_and_leaves_after_the_fifth() {
    // Basis: Rule (ADR-0020: 50 Hz divides 250 Hz, five reports a frame).
    let reports = reports(5, 32, &WOBBLE, 500);
    let mut link = link(50, device(250, true));
    let frames = run(&mut link, &reports, 16_000);
    // The first frame carries the first report; each after it the next five.
    assert_eq!(frames[0].0, 11);
    assert_eq!(frames[0].1.reports, 1);
    for (k, (tick, frame)) in frames.iter().enumerate().skip(1) {
        assert_eq!(*tick, 5 + 160 * k as u64 + 6, "frame {k}");
        assert_eq!(frame.reports, 5, "frame {k}");
        assert_eq!(frame.channels, reports[5 * k].1, "frame {k}");
    }
}

#[test]
fn on_its_own_clock_a_frame_that_lands_in_the_wobble_catches_some_reports_and_misses_others() {
    // Basis: Rule (ADR-0020, why it locks: a free-running 250 Hz clock that
    // lands within the arrival wobble gives frames with no fresh report and
    // frames with two). The same reports, due 2 steps before each frame of
    // the link's own clock, on a Packet Rate that can't lock (no Report
    // Rate known).
    let reports = reports(30, 32, &WOBBLE, 500);
    let unknown = Some(InputDeviceFacts {
        report_rate: None,
        reports_at_rest: true,
    });
    let mut link = link(250, unknown);
    let frames = run(&mut link, &reports, 16_000);
    let carrying = |n: u32| frames.iter().filter(|(_, f)| f.reports == n).count();
    assert!(carrying(0) > 50, "{} frames with none", carrying(0));
    assert!(carrying(2) > 50, "{} frames with two", carrying(2));
}

#[test]
fn a_late_report_costs_one_repeated_frame() {
    // Basis: Rule (ADR-0020: a report that arrives after its frame has left
    // costs one repeated frame, like one lost ELRS packet). Report 100 is
    // due at step 3205 and its frame leaves at 3211, but it arrives at 3213.
    let mut reports = reports(5, 32, &[0, 2], 200);
    reports[100].0 = 3213;
    let mut link = link(250, device(250, true));
    let frames = run(&mut link, &reports, 6_400);
    let (tick, repeated) = frames[100];
    assert_eq!(tick, 3211);
    assert_eq!(repeated.reports, 0);
    assert_eq!(repeated.channels, reports[99].1);
    let (tick, next) = frames[101];
    assert_eq!(tick, 3243);
    assert_eq!(next.reports, 2);
    assert_eq!(next.channels, reports[101].1);
    assert!(frames[102..].iter().all(|(_, f)| f.reports == 1));
}

#[test]
fn the_lock_follows_a_device_whose_clock_runs_slow() {
    // Basis: Rule (ADR-0020: a DualSense reports 4.5–6 ppm slower than the
    // Mac's clock; the beat is learnt again from every second's reports).
    // Over two minutes at 6 ppm slow, its reports drift 0.72 ms (5.8 steps)
    // later than a 4 ms beat; every frame still carries exactly one.
    let count = 30_000u64;
    let reports: Vec<(u64, Channels)> = (0..count)
        .map(|k| {
            let due = 5.0 + 32.0 * k as f64 * (1.0 + 6e-6);
            let late = WOBBLE[(k as usize) % WOBBLE.len()] as f64;
            ((due + late).ceil() as u64, rolled(992 + (k % 800) as u16))
        })
        .collect();
    let mut link = link(250, device(250, true));
    let frames = run(&mut link, &reports, 32 * count);
    assert!(frames.len() >= count as usize - 1);
    for (k, (_, frame)) in frames.iter().enumerate() {
        assert_eq!(frame.reports, 1, "frame {k}");
        assert_eq!(frame.channels, reports[k].1, "frame {k}");
    }
}

#[test]
fn a_pocket_whose_first_report_came_late_still_locks_within_a_second() {
    // Basis: Rule (ADR-0020). A Pocket reports every 1 ms, 8 steps, and
    // #30's recordings put its arrivals over 0.44–0.55 ms: up to 4 steps
    // late, half its period. Its first report here comes the latest; the
    // link starts from it and settles on the earliest within a second, so
    // that every frame at 1000 Hz carries exactly one report.
    let late = [4, 0, 2, 4, 1, 3, 0, 4, 2];
    let reports = reports(3, 8, &late, 4_000);
    let mut link = link(1000, device(1000, false));
    let frames = run(&mut link, &reports, 32_000);
    let settled: Vec<&(u64, Frame)> = frames.iter().filter(|(t, _)| *t >= 8_000).collect();
    assert!(settled.len() > 2_900);
    for (tick, frame) in settled {
        assert_eq!(frame.reports, 1, "frame at step {tick}");
        assert_eq!((tick - 3) % 8, 6, "frame at step {tick}");
    }
}

#[test]
fn a_device_that_reports_at_rest_is_lost_after_1_s_of_silence_and_back_with_its_next_report() {
    // Basis: Source (#27: a device that reports at rest counts as lost after
    // 1 s of silence, exactly as if unplugged; the DualSense's motion sensors
    // are its heartbeat). Reports every 4 ms until step 7973, then nothing
    // until step 24 005: lost from step 15 973, 1 s after the last report,
    // with no frames, then back with the next report.
    let mut heard = reports(5, 32, &[0], 250);
    heard.extend(reports(24_005, 32, &[0], 10));
    let mut link = link(250, device(250, true));
    let mut lost_at = None;
    let mut back_at = None;
    let mut frames = Vec::new();
    let mut next = 0;
    for tick in 0..25_000u64 {
        let was_lost = link.is_lost();
        while next < heard.len() && heard[next].0 == tick {
            link.hear(SimulationTime::from_ticks(tick), heard[next].1);
            next += 1;
        }
        if let Some(frame) = link.frame(SimulationTime::from_ticks(tick)) {
            frames.push((tick, frame));
        }
        match (was_lost, link.is_lost()) {
            (false, true) => lost_at = Some(tick),
            (true, false) => back_at = Some(tick),
            _ => {}
        }
    }
    assert_eq!(lost_at, Some(7_973 + 8_000));
    assert_eq!(back_at, Some(24_005));
    assert!(
        frames.iter().all(|(t, _)| !(15_973..24_005).contains(t)),
        "no frame leaves while it is lost"
    );
    // Up to the loss, the beat carried on with repeated frames: one every
    // 32 steps from step 11, the last at 15 947.
    assert_eq!(frames.iter().filter(|(t, _)| *t < 15_973).count(), 499);
    let (tick, first_back) = frames.iter().find(|(t, _)| *t >= 24_005).copied().unwrap();
    assert_eq!(tick, 24_011);
    assert_eq!(first_back.channels, heard[250].1);
}

#[test]
fn a_device_that_reports_only_changes_and_scripted_sticks_are_never_lost_for_silence() {
    // Basis: Source (#27, #18: a resting Pocket sends nothing at all, so a
    // device that reports only changes is lost only when unplugged).
    for device in [device(1000, false), device(250, false), None] {
        let mut link = link(250, device);
        let frames = run(&mut link, &[(5, rolled(992))], 80_000);
        assert!(!link.is_lost());
        assert!(frames.len() > 2_400, "{device:?}");
    }
}

#[test]
fn an_unplugged_device_is_lost_until_it_is_back_and_then_has_a_second_to_report() {
    // Basis: Rule (#37: an Input Device lost or back is a Flight Input; a
    // device that reports at rest and stays silent for 1 s is lost).
    let mut link = link(250, device(250, true));
    link.hear(SimulationTime::from_ticks(0), rolled(992));
    link.lose(SimulationTime::from_ticks(100));
    link.hear(SimulationTime::from_ticks(200), rolled(1000));
    for tick in 0..20_000 {
        let frame = link.frame(SimulationTime::from_ticks(tick));
        if tick >= 100 {
            assert!(frame.is_none() && link.is_lost(), "step {tick}");
        }
    }
    link.back(SimulationTime::from_ticks(20_000));
    let frames = run_from(&mut link, 20_000, 28_100);
    assert!(!frames.is_empty());
    assert!(frames.iter().all(|(t, _)| *t < 28_000));
    assert!(link.is_lost());
}

/// Steps a link on from step `from` to `to`, with no reports.
fn run_from(link: &mut RadioLink, from: u64, to: u64) -> Vec<(u64, Frame)> {
    (from..to)
        .filter_map(|tick| {
            link.frame(SimulationTime::from_ticks(tick))
                .map(|frame| (tick, frame))
        })
        .collect()
}

#[test]
fn values_read_in_two_polls_within_one_report_count_as_one_report() {
    // Basis: Rule (ADR-0020; #30's Pocket recordings show one report's
    // values reaching the input thread in two polls, 0.35 ms apart). A 1 kHz
    // device whose every third report arrives in two parts, as it is due and
    // 3 steps later: locked at 1000 Hz, each frame carries one report, with
    // the second part's values. On the link's own clock each part counts.
    let mut heard = Vec::new();
    for k in 0..2_000u64 {
        let due = 3 + 8 * k;
        let values = rolled(992 + (k % 800) as u16);
        if k % 3 == 0 {
            heard.push((due, rolled(172)));
            heard.push((due + 3, values));
        } else {
            heard.push((due + 1, values));
        }
    }
    let mut locked = link(1000, device(1000, false));
    let frames = run(&mut locked, &heard, 16_002);
    assert_eq!(frames.len(), 2_000);
    for (k, (tick, frame)) in frames.iter().enumerate() {
        assert_eq!(*tick, 3 + 8 * k as u64 + 6, "frame {k}");
        assert_eq!(frame.reports, 1, "frame {k}");
        assert_eq!(frame.channels.roll.step(), 992 + (k % 800) as u16);
    }
    let mut own = link(1000, None);
    let frames = run(&mut own, &heard, 16_000);
    assert!(frames.iter().any(|(_, frame)| frame.reports == 2));
}
