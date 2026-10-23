"""Worn concrete floor: 3 m of an industrial floor to the picture, four slabs
of 1.5 m with their saw-cut joints. Trowelled grey with soft cloudy patches
and swirls, fine aggregate showing in the worn lanes where forklifts and feet
go, black rubber scuffs, stray hairline cracks, oil stains with a dark core,
the pale rings left where water stood, and pits where air was trapped."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # Slabs: each is a little lighter or darker than the next.
    slab = tex.cell_random(np.floor(X * 2.0).astype(np.int64), np.floor(Y * 2.0).astype(np.int64), s())
    tone = 0.88 + 0.24 * slab
    cloud = tex.fbm(X, Y, 5, 6, s())
    mid = tex.fbm(X, Y, 16, 4, s())
    # Trowel swirls: noise bent round on itself.
    bend_x = tex.perlin(X, Y, 3, s()) * 0.12
    bend_y = tex.perlin(X, Y, 3, s()) * 0.12
    trowel = tex.fbm(X + bend_x, Y + bend_y, (3, 12), 3, s())
    base = tex.mix(tex.colour(96, 94, 90), tex.colour(152, 150, 143), cloud)
    base = base * tone[..., None] * (0.86 + 0.28 * mid)[..., None] * (0.94 + 0.12 * trowel)[..., None]

    # Fine aggregate: tan, grey-blue and pink grains, more of them where worn.
    wear_lane = tex.smoothstep(0.42, 0.68, tex.fbm(X, Y, (4, 3), 4, s()))
    grains_a = tex.smoothstep(0.58, 0.74, tex.fbm(X, Y, 260, 2, s()))
    grains_b = tex.smoothstep(0.58, 0.74, tex.fbm(X, Y, 240, 2, s()))
    grains_c = tex.smoothstep(0.3, 0.16, tex.fbm(X, Y, 220, 2, s()))
    exposed = 0.12 + 0.55 * wear_lane
    albedo = tex.mix(base, tex.colour(178, 164, 138), grains_a * exposed)
    albedo = tex.mix(albedo, tex.colour(130, 138, 148), grains_b * exposed * 0.8)
    albedo = tex.mix(albedo, tex.colour(54, 52, 50), grains_c * 0.28)

    # Saw-cut joints between the slabs: a dark line with a ragged shoulder.
    across = np.abs(X * 2.0 - np.floor(X * 2.0 + 0.5)) / 2.0
    down = np.abs(Y * 2.0 - np.floor(Y * 2.0 + 0.5)) / 2.0
    jd = np.minimum(across, down)
    ragged = tex.fbm(X, Y, 90, 3, s())
    groove = 1.0 - tex.smoothstep(0.0012, 0.0032 + 0.003 * ragged, jd)
    shoulder = (1.0 - tex.smoothstep(0.0, 0.014, jd)) * tex.smoothstep(0.4, 0.7, ragged)
    dirt_in_joint = (1.0 - tex.smoothstep(0.0, 0.006, jd))
    albedo = tex.mix(albedo, albedo * 0.8, shoulder * 0.7)
    albedo = tex.mix(albedo, tex.colour(30, 28, 26), tex.clamp(groove + dirt_in_joint * 0.4) * 0.85)

    # Cracks: a few long hairlines that wander across a slab and fork.
    crack = tex.cracks(X, Y, 3, s(), 0.0022, 0.07, 0.0)
    crack_fine = tex.cracks(X, Y, 8, s(), 0.0012, 0.05, 0.0) * tex.smoothstep(0.45, 0.65, tex.fbm(X, Y, 6, 3, s()))
    crack_all = np.maximum(crack, crack_fine * 0.8)
    lip = tex.blur(crack_all, 3, 2)
    albedo = tex.mix(albedo, albedo * 0.7, tex.clamp(lip * 2.0) * 0.45)
    albedo = tex.mix(albedo, tex.colour(26, 24, 24), tex.clamp(crack_all * 1.3) * 0.9)

    # Rubber: black scuffs where tyres turned, in short curved streaks.
    scuff_n = tex.fbm(X + tex.perlin(X, Y, 4, s()) * 0.1, Y, (5, 22), 3, s())
    scuff = tex.smoothstep(0.62, 0.72, scuff_n) * tex.smoothstep(0.4, 0.62, tex.fbm(X, Y, 4, 3, s()))
    albedo = tex.mix(albedo, tex.colour(34, 33, 32), scuff * 0.6)

    # Oil: dark, soft, wet-looking at its core.
    oil_n = tex.fbm(X, Y, 4, 5, s())
    where = tex.smoothstep(0.38, 0.62, tex.fbm(X, Y, 3, 2, s()))
    oil = tex.smoothstep(0.62, 0.7, oil_n) * where
    halo = tex.smoothstep(0.55, 0.64, oil_n) * where
    albedo = tex.mix(albedo, albedo * 0.74, halo * 0.55)
    albedo = tex.mix(albedo, albedo * 0.35 + tex.colour(20, 16, 12) * 0.5, oil * 0.8)

    # Tide marks where water stood: a pale, soft rim and a darker inside.
    puddle_n = tex.fbm(X, Y, 5, 4, s())
    rim = tex.blur(tex.smoothstep(0.014, 0.0, np.abs(puddle_n - 0.6)), 3, 2)
    inside = tex.smoothstep(0.58, 0.66, puddle_n)
    albedo = tex.mix(albedo, tex.colour(182, 176, 164), tex.clamp(rim * 2.0) * 0.14)
    albedo = albedo * (1.0 - 0.1 * inside)[..., None]

    # Pits: air bubbles, small dark holes.
    f1, _f2, ident, _dx, _dy = tex.voronoi(X, Y, 200, s())
    has_pit = tex.unit(ident) > 0.9
    pit = tex.smoothstep(0.16, 0.05, f1) * has_pit
    albedo = tex.mix(albedo, tex.colour(46, 44, 42), pit * 0.7)

    # Height: a flat floor with its fine unevenness, joints and cracks cut in.
    micro = tex.fbm(X, Y, 130, 4, s())
    height = 0.7 + 0.18 * micro + 0.08 * cloud - groove * 0.4 - shoulder * 0.05 - crack_all * 0.4 - pit * 0.3
    height = tex.clamp(height + (grains_a + grains_b) * 0.03 * exposed)

    rough = 0.84 - 0.24 * wear_lane * (0.3 + 0.7 * tex.fbm(X, Y, 30, 3, s())) + 0.08 * (1.0 - tone)
    rough = tex.mix(rough, 0.42, oil * 0.7)
    rough = tex.mix(rough, 0.55, inside * 0.25)
    rough = tex.mix(rough, 0.7, scuff * 0.5)
    rough = rough + groove * 0.1
    return tex.finish(albedo, height, rough, 5.0, seed=seed)
