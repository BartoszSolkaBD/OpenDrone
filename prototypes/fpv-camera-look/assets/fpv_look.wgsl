// PROTOTYPE (#28). The one merged camera pass: fisheye warp (ADR-0009) + Video Look + Breakup.
//
// The Map was drawn once by a normal camera into `src_tex` (wider than the picture, with mips).
// For every screen pixel we find which direction the fisheye Lens sees there, look that direction
// up in the flat render, and then make the Analog or Digital picture from it.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct FpvParams {
    picture: vec4<f32>,   // x0, y0, w, h (target px)
    lens: vec4<f32>,      // R (frame half-diagonal px), theta_max, k, g(theta_max)
    source: vec4<f32>,    // tan half-extent x, y; source w, h (px)
    mode: vec4<f32>,      // look (0 analog, 1 digital), time s, frame, reduce motion
    analog: vec4<f32>,    // output px per analog px, softness, grain, colour bleed (analog px)
    analog2: vec4<f32>,   // contrast, saturation, brightness, flicker allowed
    breakup_a: vec4<f32>, // static, colour loss, tearing, roll
    breakup_d: vec4<f32>, // smear, blocks, -, black
    digital: vec4<f32>,   // sharpen, contrast, saturation, brightness
    misc: vec4<f32>,
    cam: vec4<f32>,       // dynamic-range factor, tone curve, px per analog line, px per digital pixel
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> P: FpvParams;
// Plain trilinear (no anisotropy): Analog's soft taps want blur, not crisp edges.
@group(0) @binding(3) var blur_samp: sampler;

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn hash3(x: i32, y: i32, z: u32) -> f32 {
    let h = pcg(bitcast<u32>(x) ^ pcg(bitcast<u32>(y) ^ pcg(z)));
    return f32(h) * (1.0 / 4294967295.0);
}

fn vnoise(p: vec2<f32>, seed: u32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let ix = i32(i.x);
    let iy = i32(i.y);
    let a = hash3(ix, iy, seed);
    let b = hash3(ix + 1, iy, seed);
    let c = hash3(ix, iy + 1, seed);
    let d = hash3(ix + 1, iy + 1, seed);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn noise1(x: f32, seed: u32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash3(i32(i), 0, seed), hash3(i32(i) + 1, 0, seed), u);
}

// Screen pixel -> source uv. The Lens: r / R = g(theta) / g(theta_max), with
// g(theta) = theta (k = 0, the stock equidistant fisheye) or tan(k theta) / k (gentler, k > 0).
fn warp(pix: vec2<f32>) -> vec2<f32> {
    let c = P.picture.xy + 0.5 * P.picture.zw;
    let q = (pix - c) / P.lens.x;
    let r = max(length(q), 1e-6);
    let theta_max = P.lens.y;
    let k = P.lens.z;
    let gmax = P.lens.w;
    var theta = r * theta_max;
    if (k > 1e-4) {
        theta = atan(r * gmax * k) / k;
    }
    theta = min(theta, 1.5607);
    let s = tan(theta) * q / r;
    return vec2<f32>(s.x / P.source.x, s.y / P.source.y) * 0.5 + 0.5;
}

// The camera's response (round 3). The Map render stores Reinhard-compressed light
// (y = x / (1 + x), after auto-exposure), so we undo that, squeeze or stretch the stops around
// the exposure's middle grey to this camera's dynamic range, and apply the tone curve.
const PIVOT: f32 = 0.263;  // scene light that ACES shows as 18% grey

fn rrt_odt(v: vec3<f32>) -> vec3<f32> {
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return a / b;
}

fn aces(color: vec3<f32>) -> vec3<f32> {
    let rgb_to_rrt = mat3x3<f32>(
        vec3<f32>(0.59719, 0.35458, 0.04823),
        vec3<f32>(0.07600, 0.90834, 0.01566),
        vec3<f32>(0.02840, 0.13383, 0.83777),
    );
    let odt_to_rgb = mat3x3<f32>(
        vec3<f32>(1.60475, -0.53108, -0.07367),
        vec3<f32>(-0.10208, 1.10813, -0.00605),
        vec3<f32>(-0.00327, -0.07276, 1.07602),
    );
    var c = color * rgb_to_rrt;
    c = rrt_odt(c);
    c = c * odt_to_rgb;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn cam_curve(y_in: vec3<f32>) -> vec3<f32> {
    let y = clamp(y_in, vec3<f32>(0.0), vec3<f32>(0.992));
    let x = max(y / (vec3<f32>(1.0) - y), vec3<f32>(1e-6));
    let xs = PIVOT * exp2(log2(x / PIVOT) * P.cam.x);
    if (P.cam.y < 0.5) {
        return aces(xs);
    } else if (P.cam.y < 1.5) {
        return xs / (xs + vec3<f32>(1.198));
    }
    return clamp(xs * (0.18 / PIVOT), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

fn rgb2yiq(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(0.299, 0.587, 0.114)),
        dot(c, vec3<f32>(0.596, -0.274, -0.322)),
        dot(c, vec3<f32>(0.211, -0.523, 0.312)),
    );
}

fn yiq2rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        c.x + 0.956 * c.y + 0.621 * c.z,
        c.x - 0.272 * c.y - 0.647 * c.z,
        c.x - 1.106 * c.y + 1.703 * c.z,
    );
}

fn sampb(uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>) -> vec3<f32> {
    return textureSampleGrad(src_tex, blur_samp, uv, gx, gy).rgb;
}

fn samp(uv: vec2<f32>, gx: vec2<f32>, gy: vec2<f32>) -> vec3<f32> {
    return textureSampleGrad(src_tex, src_samp, uv, gx, gy).rgb;
}

fn analog(pix_in: vec2<f32>) -> vec3<f32> {
    let time = P.mode.y;
    let frame = u32(P.mode.z);
    let apx = P.analog.x;     // output px per analog pixel across (from the camera's TVL)
    let apy = P.cam.z;        // output px per analog line (from the camera's lines)
    let soft = P.analog.y;
    let grain = P.analog.z;
    let bleed = P.analog.w;
    let stat = P.breakup_a.x;
    let closs = P.breakup_a.y;
    let tear = P.breakup_a.z;
    let roll = P.breakup_a.w;
    let flicker = P.analog2.w;

    // Sync loss: the picture slips vertically, and lines tear sideways.
    var rel = pix_in - P.picture.xy;
    rel.y = rel.y + roll * P.picture.w * fract(time * 0.37);
    rel.y = rel.y - P.picture.w * floor(rel.y / P.picture.w);
    let line = floor(rel.y / apy);
    let tear_wave = (noise1(line * 0.045 + time * 2.3, 11u) - 0.5) * 2.0;
    let jitter = (hash3(i32(line), 3, frame * flicker_u(flicker)) - 0.5) * 2.0;
    rel.x = rel.x + tear * (tear_wave * 0.05 * P.picture.z + jitter * 2.0 * apx);
    let pix = P.picture.xy + rel;

    let uv = warp(pix);
    let gx = warp(pix + vec2<f32>(1.0, 0.0)) - uv;
    let gy = warp(pix + vec2<f32>(0.0, 1.0)) - uv;

    // Luma: a soft picture of `lines` lines, four taps spread over the analog pixel.
    let fx = gx * (apx * soft);
    let fy = gy * (apy * soft / 0.7);  // Kell factor: interlaced video resolves ~70% of its lines
    let g0 = fx * 0.75;
    let g1 = fy * 0.75;
    // (The camera curve runs once on each blurred result, not per tap: it costs ~0.3 ms per tap set.)
    let t0 = sampb(uv + 0.30 * fx + 0.10 * fy, g0, g1);
    let t1 = sampb(uv - 0.30 * fx - 0.10 * fy, g0, g1);
    let t2 = sampb(uv + 0.10 * fx - 0.30 * fy, g0, g1);
    let t3 = sampb(uv - 0.10 * fx + 0.30 * fy, g0, g1);
    let lum = rgb2yiq(to_gamma(cam_curve((t0 + t1 + t2 + t3) * 0.25))).x;

    // Chroma: much less bandwidth, and it trails to the right (colour bleed).
    let bx = gx * (apx * max(bleed, 0.01));
    let cg = bx * 0.5;
    let c0 = sampb(uv - bx * 0.15, cg, fy);
    let c1 = sampb(uv - bx * 0.50, cg, fy);
    let c2 = sampb(uv - bx * 0.85, cg, fy);
    let chroma = rgb2yiq(to_gamma(cam_curve((c0 + c1 + c2) / 3.0))).yz;
    var yiq = vec3<f32>(lum, chroma);

    // Grade: contrast and saturation in gamma space; colour fades out as the signal weakens.
    yiq.x = (yiq.x - 0.5) * P.analog2.x + 0.5;
    let sat = P.analog2.y * (1.0 - closs);
    yiq = vec3<f32>(yiq.x, yiq.yz * sat);

    // Faint grain on the analog grid, new every frame.
    let a = rel / vec2<f32>(apx, apy);
    let gn = vnoise(a * vec2<f32>(1.0, 1.0), frame * 3u + 1u) - 0.5;
    // The noise floor rises as the signal weakens: the first sign, before any sparkles.
    yiq.x = yiq.x + gn * (grain * 2.0 + stat * 0.45);

    if (stat > 0.0) {
        // Sparkles: short white/black streaks along the lines.
        let seg = 2.0 + 10.0 * hash3(i32(line), 7, frame);
        let cell = floor(a.x / seg);
        let h = hash3(i32(cell), i32(line), frame ^ 40503u);
        let sp = smoothstep(0.2, 1.0, stat);
        let p_sp = sp * sp * 0.3;
        if (h < p_sp) {
            let s = select(-1.0, 1.0, hash3(i32(cell), i32(line), frame + 5u) > 0.35);
            yiq.x = mix(yiq.x, 0.5 + 0.55 * s, 0.85);
            yiq = vec3<f32>(yiq.x, yiq.yz * 0.3);
        }
        // Snow: the picture drowns in noise, up to full static.
        let sn = vnoise(a * vec2<f32>(1.0, 1.0), frame * 7u + 3u);
        let sn2 = vnoise(a * vec2<f32>(0.5, 1.0), frame * 7u + 4u);
        let snow = clamp(sn * 0.75 + sn2 * 0.5 - 0.15, 0.0, 1.0);
        let m = smoothstep(0.55, 1.0, stat);
        // Brightness pumps with the static (off with Reduce motion).
        let pump = 1.0 + flicker * (hash3(0, 0, frame) - 0.5) * 0.25 * stat;
        yiq = mix(yiq, vec3<f32>(snow * pump, 0.0, 0.0), m);
    }

    var rgb = yiq2rgb(yiq);
    return to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))) * P.analog2.z;
}

fn flicker_u(f: f32) -> u32 {
    return select(0u, 1u, f > 0.5);
}

fn digital(pix: vec2<f32>) -> vec3<f32> {
    let smear = P.breakup_d.x;
    let blocks = P.breakup_d.y;
    let black = P.breakup_d.w;
    let uv = warp(pix);
    let gx = warp(pix + vec2<f32>(1.0, 0.0)) - uv;
    let gy = warp(pix + vec2<f32>(0.0, 1.0)) - uv;
    // The transmitted picture's resolution (1080 lines = a little softer than this screen).
    let blur = max(P.cam.w, 1.0) * (1.0 + smear * 10.0);
    var c = cam_curve(samp(uv, gx * blur, gy * blur));
    if (P.digital.x > 0.0) {
        let b = cam_curve(samp(uv, gx * 2.5 * blur, gy * 2.5 * blur));
        c = max(c + (c - b) * P.digital.x, vec3<f32>(0.0));
    }
    if (blocks > 0.0) {
        // Macroblocks: some blocks lose their detail, a new pattern 8 times a second.
        let bs = max(8.0, P.picture.w / 60.0);
        let rel = pix - P.picture.xy;
        let cell = floor(rel / bs);
        let tslot = u32(floor(P.mode.y * 8.0));
        let h = hash3(i32(cell.x), i32(cell.y), tslot);
        if (h < blocks) {
            let cuv = warp(P.picture.xy + (cell + 0.5) * bs);
            let bc = cam_curve(samp(cuv, gx * bs, gy * bs));
            c = mix(c, bc, 0.85);
        }
    }
    var g = to_gamma(c);
    let l = dot(g, vec3<f32>(0.299, 0.587, 0.114));
    g = mix(vec3<f32>(l), g, P.digital.z);
    g = (g - 0.5) * P.digital.y + 0.5;
    return to_linear(clamp(g, vec3<f32>(0.0), vec3<f32>(1.0))) * P.digital.w * (1.0 - black);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pix = in.position.xy;
    if (P.misc.x > 1.5) {
        return vec4<f32>(textureSampleLevel(src_tex, src_samp, in.uv, 0.0).rgb, 1.0);
    }
    if (P.misc.x > 0.5) {
        return vec4<f32>(fract(warp(pix) * 4.0), 0.25, 1.0);
    }
    let p0 = P.picture.xy;
    let p1 = P.picture.xy + P.picture.zw;
    let inside = pix.x >= p0.x && pix.x < p1.x && pix.y >= p0.y && pix.y < p1.y;
    if (!inside) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    var col: vec3<f32>;
    if (P.mode.x < 0.5) {
        col = analog(pix);
    } else {
        col = digital(pix);
    }
    return vec4<f32>(col, 1.0);
}
