"""Bark of the self-sown trees in the yard, a metre to the picture: grey-brown
ridges split by furrows that run up the trunk, broken across now and then,
with green algae and pale lichen in patches. The picture's up is up the
trunk."""

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # Furrows: noise stretched up the trunk, warped so they wander and join.
    warp = tex.perlin(X, Y, (4, 2), s()) * 0.04
    ridge = tex.fbm(X + warp, Y, (18, 2), 4, s())
    furrow = tex.smoothstep(0.52, 0.36, ridge)
    # Breaks across the ridges.
    _f1, _f2, ident, _dx, dy = tex.voronoi(X + warp, Y, (14), s(), 0.8)
    breaks = 1.0 - tex.smoothstep(0.0, 0.08, tex.np.abs(dy) - 0.38)
    furrow = tex.clamp(furrow + breaks * 0.5 * (1.0 - furrow))

    tone = tex.fbm(X, Y, (6, 3), 4, s())
    albedo = tex.mix(tex.colour(92, 84, 74), tex.colour(130, 120, 106), tone)
    albedo = tex.mix(albedo, albedo * 0.85 + tex.colour(10, 6, 2), tex.unit(ident) * 0.5)
    albedo = tex.mix(albedo, tex.colour(36, 30, 26), furrow * 0.85)

    algae = tex.smoothstep(0.55, 0.72, tex.fbm(X, Y, (3, 4), 4, s())) * (0.4 + 0.6 * furrow)
    albedo = tex.mix(albedo, tex.colour(70, 90, 44), algae * 0.6)
    lichen = tex.smoothstep(0.7, 0.8, tex.fbm(X, Y, 20, 3, s())) * (1.0 - furrow)
    albedo = tex.mix(albedo, tex.colour(176, 180, 160), lichen * 0.7)

    height = tex.clamp(0.8 - furrow * 0.7 + (ridge - 0.5) * 0.2 + lichen * 0.05)
    rough = tex.clamp(0.8 + furrow * 0.15 - algae * 0.1)
    return tex.finish(albedo, height, rough, 7.0, seed=seed)
