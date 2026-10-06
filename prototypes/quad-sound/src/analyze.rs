//! PROTOTYPE (#34). Spectral analysis, to compare the synth with real recordings (reference
//! recordings are analysed only, never committed).
//!
//! `analyze FILE [t0 t1]...`: for each window [t0, t1] (seconds), an averaged spectrum (Welch,
//! 16384-point Hann frames, half overlap) and from it:
//! - the strongest peaks (dB relative to the strongest)
//! - level in octave bands, relative to the loudest band
//! - how tonal it is: the share of energy within ±3 bins of the top 40 peaks
//! - a harmonic-series estimate: the f0 between 40 and 1500 Hz whose multiples carry the most
//!   energy, and how strong its first 12 multiples are
//! Without windows, it reports whole-file level and clipping.

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (ar, ai) = (re[i + k], im[i + k]);
                let (br, bi) = (re[i + k + len / 2], im[i + k + len / 2]);
                let (tr, ti) = (br * cr - bi * ci, br * ci + bi * cr);
                re[i + k] = ar + tr;
                im[i + k] = ai + ti;
                re[i + k + len / 2] = ar - tr;
                im[i + k + len / 2] = ai - ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

pub fn load_mono(path: &str) -> (Vec<f32>, f32) {
    let probed = symphonium::probe_from_file(path, None).expect("probe");
    let d = symphonium::decode_f32(probed, &Default::default(), None, None, None).expect("decode");
    let sr = d.sample_rate.get() as f32;
    let n = d.data[0].len();
    let ch = d.data.len();
    let mono: Vec<f32> = (0..n).map(|i| d.data.iter().map(|c| c[i]).sum::<f32>() / ch as f32).collect();
    (mono, sr)
}

/// Welch power spectrum of mono[a..b].
pub fn power_spectrum(mono: &[f32], sr: f32, t0: f32, t1: f32) -> (Vec<f32>, f32) {
    let n = 16384usize;
    let a = ((t0 * sr) as usize).min(mono.len());
    let b = ((t1 * sr) as usize).min(mono.len());
    let mut acc = vec![0.0f32; n / 2];
    let mut frames = 0;
    let mut start = a;
    while start + n <= b.max(a + n).min(mono.len()) {
        let mut re: Vec<f32> = (0..n)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos();
                mono[start + i] * w
            })
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        for k in 0..n / 2 {
            acc[k] += re[k] * re[k] + im[k] * im[k];
        }
        frames += 1;
        start += n / 2;
    }
    if frames > 0 {
        acc.iter_mut().for_each(|x| *x /= frames as f32);
    }
    (acc, sr / n as f32)
}

pub fn describe(mono: &[f32], sr: f32, t0: f32, t1: f32) -> String {
    let (p, df) = power_spectrum(mono, sr, t0, t1);
    let n2 = p.len();
    let total: f32 = p[2..].iter().sum::<f32>().max(1e-20);
    let db = |x: f32| 10.0 * x.max(1e-20).log10();
    // Peaks: local maxima over ±3 bins, at least 6 dB above the median of ±60 bins.
    let mut peaks: Vec<(usize, f32)> = Vec::new();
    for k in 4..n2 - 4 {
        if (k as f32 * df) < 40.0 {
            continue;
        }
        if (1..=3).all(|d| p[k] >= p[k - d] && p[k] >= p[k + d]) {
            let lo = k.saturating_sub(60);
            let hi = (k + 60).min(n2);
            let mut v: Vec<f32> = p[lo..hi].to_vec();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = v[v.len() / 2];
            if p[k] > med * 4.0 {
                peaks.push((k, p[k]));
            }
        }
    }
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let top = peaks.first().map(|x| x.1).unwrap_or(1e-20);
    let mut tonal = 0.0f32;
    for (k, _) in peaks.iter().take(40) {
        for j in k.saturating_sub(3)..(k + 4).min(n2) {
            tonal += p[j];
        }
    }
    // Octave bands.
    let edges = [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
    let mut bands = Vec::new();
    for w in edges.windows(2) {
        let (a, b) = ((w[0] / df) as usize, ((w[1] / df) as usize).min(n2));
        bands.push((w[0], p[a..b].iter().sum::<f32>()));
    }
    let bmax = bands.iter().map(|b| b.1).fold(1e-20, f32::max);
    let centroid = p.iter().enumerate().map(|(k, x)| k as f32 * df * x).sum::<f32>() / total;
    // Harmonic series: f0 whose first 12 multiples (±1.5 % windows) hold the most energy,
    // weighting each multiple equally in dB above the local floor.
    let mut best = (0.0f32, f32::MIN);
    let mut f0 = 40.0f32;
    while f0 < 1500.0 {
        let mut score = 0.0;
        for h in 1..=12 {
            let f = f0 * h as f32;
            if f > sr * 0.45 {
                break;
            }
            let c = (f / df) as usize;
            let w = ((f * 0.015) / df).max(1.0) as usize;
            let m = p[c.saturating_sub(w)..(c + w + 1).min(n2)].iter().cloned().fold(0.0, f32::max);
            score += db(m);
        }
        if score > best.1 {
            best = (f0, score);
        }
        f0 *= 1.002;
    }
    let f0 = best.0;
    let mut harm = Vec::new();
    for h in 1..=12 {
        let f = f0 * h as f32;
        if f > sr * 0.45 {
            break;
        }
        let c = (f / df) as usize;
        let w = ((f * 0.015) / df).max(1.0) as usize;
        let (mut mk, mut mv) = (c, 0.0f32);
        for j in c.saturating_sub(w)..(c + w + 1).min(n2) {
            if p[j] > mv {
                mv = p[j];
                mk = j;
            }
        }
        harm.push(format!("{}:{:.0}Hz {:.0}", h, mk as f32 * df, db(mv / top)));
    }
    let list: Vec<String> = peaks.iter().take(14).map(|(k, v)| format!("{:.0}({:.0})", *k as f32 * df, db(v / top))).collect();
    let bl: Vec<String> = bands.iter().map(|(f, e)| format!("{}:{:.0}", *f as u32, db(e / bmax))).collect();
    format!(
        "  [{t0:.1}-{t1:.1} s] level {:.1} dBFS, centroid {:.0} Hz, tonal share {:.0}%\n    peaks Hz(dB): {}\n    octave bands from Hz (dB): {}\n    harmonic series f0 ≈ {:.1} Hz: {}",
        db(total * 2.0 / (16384.0 * 16384.0 * 0.375) * 2.0),
        centroid,
        tonal / total * 100.0,
        list.join(" "),
        bl.join(" "),
        f0,
        harm.join("  ")
    )
}

pub fn analyze(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("usage: analyze FILE [t0 t1]...");
        return;
    };
    let (mono, sr) = load_mono(path);
    let peak = mono.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    let rms = (mono.iter().map(|x| x * x).sum::<f32>() / mono.len().max(1) as f32).sqrt();
    println!(
        "{path}: {:.2} s at {} Hz, peak {:.3}, rms {:.1} dBFS, samples at full scale {}",
        mono.len() as f32 / sr,
        sr,
        peak,
        20.0 * rms.max(1e-9).log10(),
        mono.iter().filter(|x| x.abs() > 0.999).count()
    );
    let ts: Vec<f32> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
    for w in ts.chunks(2) {
        if w.len() == 2 {
            println!("{}", describe(&mono, sr, w[0], w[1]));
        }
    }
}
