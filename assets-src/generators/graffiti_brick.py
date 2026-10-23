"""Generic graffiti on weathered brick, 3.6 m to the picture (two repeats of
`brick_red_weathered`'s bricks each way, so its courses line up with a wall of
that material), for one patch of wall: the Map's script lays a thin skin over
the part of one repeat that holds the paint (from 0.08 to 0.92 across and 0.25
to 0.78 down), so it shows once. Loose looping strokes in two colours with a
dark outline and a few drips, faded and flaking with the brick: a scrawl that
is nobody's tag, name or work."""

import numpy as np

import brickwork
import odtex as tex

PATCH = (0.08, 0.25, 0.92, 0.78)


def make(seed, n):
    s = tex.Seeds(seed + 500)
    X, Y = tex.grid(n)
    # Two repeats of the brick layout each way.
    X2 = X * 2.0 - np.floor(X * 2.0)
    Y2 = Y * 2.0 - np.floor(Y * 2.0)
    fresh = brickwork.fresh_brick(X2, Y2, tex.Seeds(seed))
    joint = fresh["joint"]
    albedo = fresh["albedo"]
    low = tex.cavity(1.0 - joint * 0.6, 3)
    dirt = tex.clamp(tex.smoothstep(0.42, 0.7, tex.fbm(X, Y, (6, 4), 4, s())) * 0.6 + low * 0.7)
    soot = tex.mix(tex.colour(52, 44, 42), tex.colour(78, 66, 60), tex.fbm(X, Y, 60, 3, s()))
    albedo = tex.mix(albedo, albedo * 0.38 + soot * 0.3, dirt * 0.8)

    # The scrawl: a fat fill, a dark outline round it, a thinner second
    # colour over it.
    region = (0.14, 0.34, 0.86, 0.66)
    fill = tex.scribbles(X, Y, seed + 11, 3, 0.022, region)
    outline = tex.clamp(tex.scribbles(X, Y, seed + 11, 3, 0.03, region) - fill)
    accent = tex.scribbles(X, Y, seed + 29, 2, 0.008, (0.2, 0.38, 0.8, 0.62))
    drips = tex.streaks(fill * tex.smoothstep(0.6, 0.8, tex.fbm(X, Y, 30, 2, s())), 90, s(), X, Y, 60, 0.975)
    paint = tex.clamp(fill + drips * 0.7)
    # Weather: the paint is patchy, misses the joints and has flaked.
    wear = tex.clamp(0.55 + 0.45 * tex.fbm(X, Y, 16, 3, s()) - joint * 0.6)
    gone, _lip = tex.peel(X, Y, s(), 0.15)
    keep = wear * (1.0 - gone)
    blue = tex.mix(tex.colour(40, 96, 150), tex.colour(70, 130, 176), tex.fbm(X, Y, 8, 3, s()))
    albedo = tex.mix(albedo, tex.colour(24, 22, 24), outline * keep)
    albedo = tex.mix(albedo, blue, paint * keep)
    albedo = tex.mix(albedo, tex.colour(214, 206, 196), accent * keep * 0.9)

    height = tex.clamp(fresh["height"] + (paint + outline) * keep * 0.03)
    rough = tex.clamp(fresh["rough"] - (paint + outline) * keep * 0.25 - dirt * 0.03)
    return tex.finish(tex.clamp(albedo * 1.15), height, rough, 5.0, seed=seed)
