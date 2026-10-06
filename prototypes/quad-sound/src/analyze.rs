//! PROTOTYPE (#34). `analyze FILE.wav [t1 t2 ...]`: the strongest spectral peaks at given times,
//! plus level, clipping and click checks. A sanity check only; the sound is judged by ear.

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

pub fn analyze(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("usage: analyze FILE.wav [seconds ...]");
        return;
    };
    let mut r = hound::WavReader::open(path).expect("open wav");
    let spec = r.spec();
    let ch = spec.channels as usize;
    let sr = spec.sample_rate as f32;
    let s: Vec<f32> = r.samples::<i16>().map(|x| x.unwrap() as f32 / 32768.0).collect();
    let mono: Vec<f32> = s.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    let peak = mono.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    let rms = (mono.iter().map(|x| x * x).sum::<f32>() / mono.len() as f32).sqrt();
    let clipped = s.iter().filter(|x| x.abs() > 0.999).count();
    let mut jumps = 0;
    for w in mono.windows(3) {
        // A click: a big step that isn't part of a smooth waveform.
        let d2 = (w[2] - 2.0 * w[1] + w[0]).abs();
        if d2 > 0.5 {
            jumps += 1;
        }
    }
    let dc = mono.iter().sum::<f32>() / mono.len() as f32;
    println!(
        "{path}: {:.2} s, peak {:.3}, rms {:.4} ({:.1} dBFS), clipped samples {clipped}, sharp steps {jumps}, DC {dc:.5}",
        mono.len() as f32 / sr,
        peak,
        rms,
        20.0 * rms.max(1e-9).log10()
    );
    let times: Vec<f32> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
    let n = 8192;
    for t in times {
        let start = ((t * sr) as usize).min(mono.len().saturating_sub(n));
        let mut re: Vec<f32> = (0..n)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos();
                mono.get(start + i).copied().unwrap_or(0.0) * w
            })
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mag: Vec<f32> = (0..n / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let mut peaks: Vec<(f32, f32)> = Vec::new();
        for k in 2..n / 2 - 2 {
            if mag[k] > mag[k - 1] && mag[k] >= mag[k + 1] && mag[k] > mag[k - 2] && mag[k] >= mag[k + 2] {
                peaks.push((k as f32 * sr / n as f32, mag[k]));
            }
        }
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top = peaks.first().map(|p| p.1).unwrap_or(1.0).max(1e-9);
        let lrms = (mono[start..(start + n).min(mono.len())].iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
        // Spectral centroid, as a rough "brightness".
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for (k, m) in mag.iter().enumerate() {
            num += k as f32 * sr / n as f32 * m;
            den += m;
        }
        let list: Vec<String> = peaks.iter().take(10).map(|(f, m)| format!("{:.0}Hz({:.0}dB)", f, 20.0 * (m / top).log10())).collect();
        println!("  t={t:5.2}s rms {:.4} centroid {:5.0} Hz | {}", lrms, num / den.max(1e-9), list.join(" "));
    }
}
