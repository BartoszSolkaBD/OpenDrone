"""A decal of soot and fire blackening (#323): the smoke of a fire has risen
up a wall and left a plume, black just above the flame at the bottom of the
picture, fanning wider as it climbs, thinning into streaky tongues and a
faint brown haze at its fringe, with the lowest part scorched dark brown
where the flame touched. Put its bottom just above the fire, or over a
burnt door's opening. The top of the picture is up; on a ceiling it can be
turned any way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4900)
    X, Y = tex.grid(n)
    flat = Y * 0.0
    up = 1.0 - Y  # 0 at the flame, 1 at the top

    # The plume: a narrow neck at the bottom that fans out as it climbs,
    # its sides torn by noise.
    torn = (tex.fbm(X, Y, 6, 5, s()) - 0.5) * 0.7
    half = 0.1 + 0.3 * up
    side = np.abs(X - 0.5) + torn * (0.25 + 0.75 * up)
    body = tex.smoothstep(half + 0.1, half - 0.08, side)
    # Dense low down, thinning with height, with a clear top.
    height_fade = tex.smoothstep(1.0, 0.1, up + (tex.fbm(X, Y, 5, 3, s()) - 0.5) * 0.35)
    # The flame's own edge at the bottom is ragged too.
    base = tex.smoothstep(0.0, 0.1 + 0.12 * tex.fbm(X, Y, 12, 3, s()), up)
    density = body * height_fade * (0.35 + 0.65 * base)
    # Tongues of smoke climb higher in streaks; the source is flipped so they
    # run up the picture.
    source = tex.smoothstep(0.5, 0.2, side) * (up < 0.18)
    tongues = tex.streaks(source[::-1], int(n * 0.85), s(), X, Y, 22, 0.996)[::-1]
    streak = tex.clamp(tongues * 1.3) * tex.smoothstep(0.1 + 0.3 * up, 0.0, np.abs(X - 0.5))
    mask = tex.clamp(density * 0.95 + streak * 0.7)
    mask = mask * (0.82 + 0.25 * tex.fbm(X, Y, 45, 3, s()))
    mask = tex.smoothstep(0.04, 0.8, mask) * decalwork.frame(X, Y, bottom=0.03) * (0.15 + 0.85 * base)

    black = tex.colour(10, 10, 9)
    brown = tex.colour(48, 38, 30)
    haze = tex.colour(70, 62, 54)
    albedo = tex.mix(black, brown, tex.smoothstep(0.55, 1.0, 1.0 - density))
    albedo = tex.mix(albedo, haze, tex.smoothstep(0.7, 1.0, 1.0 - mask) * 0.6)
    albedo = albedo * (0.8 + 0.4 * tex.fbm(X, Y, 50, 3, s()))[..., None]

    height = tex.clamp(0.5 + tex.fbm(X, Y, 60, 2, s()) * 0.1)
    rough = 0.9 + 0.06 * tex.fbm(X, Y, 18, 3, s())
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.5, seed)
