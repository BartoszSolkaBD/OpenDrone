"""A decal of a cobweb in a corner (#323): a fan of fine strands from the
corner at the top middle of the picture, with sagging threads across them
at growing distances, hung with a few clumps of dust, grey-white and so
faint that only its threads and clumps show. Put the corner where two walls
meet under the ceiling, facing out of it (the decal lies across both walls
and bends round the corner). The top of the picture is up, and the corner
is the line down its middle."""

import numpy as np

import decalwork
import odtex as tex


def _strand(dx, dy, slope, width):
    """1 on a line from the corner at (0, 0) down the picture, going `slope`
    across for each unit down, `width` wide, in tile units."""
    norm = np.sqrt(slope * slope + 1.0)
    d = np.abs(dx * (1.0 / norm) - dy * (slope / norm))
    return 1.0 - tex.smoothstep(width * 0.5, width, d)


def make(seed, n):
    s = tex.Seeds(seed + 5300)
    X, Y = tex.grid(n)
    dx = X - 0.5
    dy = Y - 0.015
    px = 1.0 / n

    reach = 0.95
    wobble = (tex.fbm(X, Y, 8, 3, s()) - 0.5) * 0.04
    d = np.sqrt(dx * dx + dy * dy) + wobble
    web = np.zeros_like(X)
    # Strands out of the corner, at slightly uneven slopes, ending at their
    # own lengths.
    slopes = (-3.1, -1.8, -1.0, -0.45, 0.0, 0.5, 1.05, 1.9, 3.0)
    for k, slope in enumerate(slopes):
        end = reach * (0.55 + 0.45 * tex.cell_random(np.array([k]), np.array([0]), s())[0])
        web = np.maximum(web, _strand(dx, dy, slope, px * 1.3) * tex.smoothstep(end, end - 0.05, d))
    # Threads across, between the first and last strand, each sagging in the
    # middle of its span. Sag is a parabola of the position across the span.
    across = tex.clamp((dx / np.maximum(dy, 0.02) + 3.1) / 6.2)
    sag = 0.05 * (1.0 - (2.0 * across - 1.0) * (2.0 * across - 1.0))
    inside = tex.smoothstep(3.3, 3.0, np.abs(dx / np.maximum(dy, 0.02)))
    for k in range(7):
        r = 0.1 + k * 0.12 + 0.02 * tex.cell_random(np.array([k]), np.array([1]), s())[0]
        ring = 1.0 - tex.smoothstep(px * 0.6, px * 1.3, np.abs(d - r - sag * (0.4 + r)))
        web = np.maximum(web, ring * inside * tex.smoothstep(reach, reach - 0.1, r + sag))
    web = web * tex.smoothstep(0.0, 0.03, Y) * tex.smoothstep(reach + 0.04, reach - 0.04, d)
    # Dust gathered into clumps along the threads.
    dust = tex.smoothstep(0.7, 0.85, tex.fbm(X, Y, 28, 3, s())) * tex.clamp(tex.blur(web, 3, 2) * 4.0)
    mask = tex.clamp(web * 0.85 + dust * 0.9) * decalwork.frame(X, Y)

    albedo = tex.mix(tex.colour(200, 200, 194), tex.colour(120, 116, 106), dust)
    height = tex.clamp(0.5 + web * 0.1 + dust * 0.3)
    rough = 0.72 - 0.1 * tex.fbm(X, Y, 20, 3, s())
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.0, seed)
