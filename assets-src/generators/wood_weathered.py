"""Weathered softwood, a metre to the picture (#324): pallet boards and a
cable drum left out for years. Silver-grey where sun and rain have worked on
it, brown where it is fresher, the grain raised and split along the board,
dark knots, black mould in the splits and rust round the nails. The grain runs
along the picture's width; the boards are separate pieces, so the picture has
none of their edges."""

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # Grain: long streaks along X, bent a little.
    bend = tex.perlin(X, Y, 3, s()) * 0.05
    grain = tex.fbm(X, Y + bend, (2, 64), 3, s())
    fine = tex.fbm(X, Y + bend, (6, 220), 2, s())
    grey = tex.mix(tex.colour(78, 74, 66), tex.colour(118, 112, 100), grain)
    brown = tex.mix(tex.colour(92, 66, 40), tex.colour(126, 94, 60), grain)
    fresh = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, (3, 6), 4, s()))
    albedo = tex.mix(grey, brown, fresh * 0.8)
    albedo = albedo * (0.86 + 0.24 * fine)[..., None]

    # Splits along the grain, with black mould in them.
    split = tex.smoothstep(0.83, 0.9, tex.fbm(X, Y + bend, (4, 90), 2, s())) * tex.smoothstep(0.45, 0.6, tex.fbm(X, Y, 3, 3, s()))
    albedo = tex.mix(albedo, tex.colour(28, 26, 22), split * 0.85)
    mould = tex.smoothstep(0.6, 0.78, tex.fbm(X, Y, 8, 4, s()))
    albedo = tex.mix(albedo, tex.colour(46, 44, 38), mould * 0.45)

    # Knots: small dark rings.
    f1, _f2, ident, _dx, _dy = tex.voronoi(X, Y, 6, s(), 0.8)
    knot = tex.smoothstep(0.08, 0.03, f1) * (tex.unit(ident) > 0.6)
    albedo = tex.mix(albedo, tex.colour(58, 40, 26), knot * 0.8)
    # Nails: a dark head with rust bleeding round it.
    g1, _g2, gid, _gx, _gy = tex.voronoi(X, Y, 9, s(), 0.9)
    nail = tex.smoothstep(0.06, 0.0, g1) * (tex.unit(gid) > 0.7)
    stain = tex.blur(nail, max(2, n // 100), 2)
    albedo = tex.mix(albedo, tex.colour(90, 48, 24), tex.clamp(stain * 3.0) * 0.6)
    albedo = tex.mix(albedo, tex.colour(30, 24, 20), nail * 0.9)

    height = tex.clamp(0.6 + 0.2 * grain * (1.0 - fresh * 0.5) + 0.06 * fine - split * 0.5 - knot * 0.1)
    rough = tex.clamp(0.82 + 0.08 * (1.0 - fresh) - 0.06 * mould)
    return tex.finish(tex.clamp(albedo), height, rough, 4.0, seed=seed)
