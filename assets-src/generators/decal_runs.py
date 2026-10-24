"""A decal of water runs below a sill, a roof edge or a broken gutter (#323):
dirty water has trickled down from a line along the top of the picture for
years, and left thin dark streaks that fan wider as they go, a few darker than
the rest, each fading out at its own length. Unlike `decal_stain` there is no
blotch: the runs start at the very top, so the top of the picture is meant to
sit just under the edge the water came over. The top of the picture is up."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4100)
    X, Y = tex.grid(n)
    flat = Y * 0.0

    # The runs spread: lanes are drawn in a coordinate that stretches with the
    # distance down, so they diverge from the middle.
    fan = 1.0 + 0.9 * Y
    Xf = 0.5 + (X - 0.5) / fan
    ragged = (tex.fbm(X, flat, (5, 1), 3, s()) - 0.5) * 0.18
    # Where the water came over the edge: a ragged line along the top, widest
    # in the middle, and the cone it spreads into.
    half = 0.16 + 0.3 * Y + (tex.fbm(X, Y, 4, 3, s()) - 0.5) * 0.2
    cone = tex.smoothstep(half + 0.06, half - 0.1, np.abs(X - 0.5) + ragged * 0.5)
    source = (Y < 0.08) * tex.smoothstep(0.34, 0.1, np.abs(X - 0.5) + ragged)
    runs = tex.streaks(source, int(n * 0.95), s(), Xf, Y, 34, 0.9975)
    thin = tex.streaks(source * 0.7, int(n * 0.6), s(), Xf, Y, 70, 0.995)
    # Each run fades out at its own length, raggedly.
    fade = tex.smoothstep(1.0, 0.25, Y + (tex.fbm(Xf, Y, (14, 3), 3, s()) - 0.5) * 0.45)
    streak = tex.clamp((runs * 1.25 + thin * 0.6) * 1.9) * fade * cone
    # A few runs carry more dirt than the rest.
    heavy = tex.smoothstep(0.5, 0.75, tex.fbm(Xf, Y, (9, 2), 2, s()))
    mask = tex.clamp(streak * (0.8 + 0.5 * heavy)) * (0.8 + 0.2 * tex.fbm(X, Y, 50, 3, s()))
    mask = mask * decalwork.frame(X, Y)

    # Grey-brown dirt, a little rust-coloured where the water ran heaviest.
    dirt = tex.colour(34, 31, 27)
    rust = tex.colour(66, 44, 30)
    albedo = tex.mix(dirt, rust, heavy * 0.5)
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 40, 3, s()))[..., None]

    height = tex.clamp(0.5 - streak * 0.06)
    rough = 0.8 - streak * 0.12
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 2.0, seed)
