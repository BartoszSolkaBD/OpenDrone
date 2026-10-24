"""A decal of a generic tag (#323): a spray-painted scrawl, loose looping
strokes that read as a hand's signature and spell nothing, in one saturated
paint colour, with a darker core where the can lingered, a soft overspray
halo along the strokes and a few drips running down from the lowest ones.
Nothing here copies a real tag, a person or an artist. The top of the picture
is up the wall."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 5400)
    X, Y = tex.grid(n)

    strokes = tex.scribbles(X, Y, s(), 3, 0.014, region=(0.16, 0.2, 0.84, 0.62))
    underline = tex.scribbles(X, Y, s(), 1, 0.01, region=(0.14, 0.62, 0.86, 0.72))
    paint = np.maximum(strokes, underline)
    # Rounded: the strokes are polylines, so their corners are softened.
    paint = tex.smoothstep(0.3, 0.7, tex.blur(paint, max(1, n // 200), 2) * 1.6)
    # A soft halo of overspray round the lines.
    halo = tex.clamp(tex.blur(paint, max(2, n // 90), 3) * 3.5)
    # Drips: the paint ran where a stroke was heaviest, below it.
    heavy = paint * tex.smoothstep(0.58, 0.72, tex.fbm(X, Y, 22, 3, s()))
    drips = tex.streaks(heavy, int(n * 0.22), s(), X, Y, 40, 0.992) * tex.smoothstep(0.7, 0.2, Y)
    drips = drips * (1.0 - paint) * 0.9
    mask = tex.clamp(paint + halo * 0.3 + drips)
    mask = mask * (0.9 + 0.1 * tex.fbm(X, Y, 80, 2, s())) * decalwork.frame(X, Y)

    bright = tex.colour(24, 150, 150)
    deep = tex.colour(10, 92, 100)
    albedo = tex.mix(bright, deep, tex.clamp(halo * 0.0 + paint * 0.5 + drips * 0.5))
    height = tex.clamp(0.5 + paint * 0.05)
    rough = 0.45 + 0.2 * (1.0 - paint)
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.0, seed)
