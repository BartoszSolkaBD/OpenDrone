"""A decal of a water stain down a wall (#321): water that has run for years
from a leak near the top (a broken gutter, a crack in a sill) has left a dark
blotch where it comes out and dark streaks running down from it, rust-brown
near the top where it carried iron, sooty lower down, each streak fading out
unevenly. Tide lines ring the blotch where it dried. The top of the picture
is up the wall."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 3300)
    X, Y = tex.grid(n)

    # Where the water comes out: a soft blotch near the top.
    source = decalwork.blob(X, Y, s(), centre=(0.5, 0.16), radius=0.14, softness=0.07, warp=0.12, stretch=(1.6, 0.8))
    # Streaks run down from the blotch, and from its lower edge most.
    lower = source * tex.smoothstep(0.08, 0.2, Y)
    runs = tex.streaks(lower, int(n * 0.72), s(), X, Y, 38, 0.997)
    runs2 = tex.streaks(lower * 0.8, int(n * 0.5), s(), X, Y, 64, 0.994)
    # Streaks fade with distance down the wall, raggedly.
    fade = tex.smoothstep(0.98, 0.3, Y + (tex.fbm(X, Y, (12, 3), 3, s()) - 0.5) * 0.3)
    streak = tex.clamp((runs * 1.1 + runs2 * 0.8) * 1.4) * fade
    # A tide line, faint and broken, where the blotch once dried.
    broken = tex.smoothstep(0.35, 0.65, tex.fbm(X, Y, 16, 3, s()))
    rings = (tex.smoothstep(0.3, 0.4, source) - tex.smoothstep(0.42, 0.6, source)) * broken
    mask = tex.clamp(source * 0.8 + streak + rings * 0.25)
    mask = mask * (0.8 + 0.2 * tex.fbm(X, Y, 40, 3, s())) * decalwork.frame(X, Y)

    # Dark rust-brown near the top, sooty grey-green lower down: a stain reads
    # by being darker than the wall, whatever the wall is.
    rust = tex.colour(46, 32, 24)
    soot = tex.colour(30, 33, 28)
    albedo = tex.mix(rust, soot, tex.smoothstep(0.15, 0.6, Y))
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 30, 3, s()))[..., None]

    height = tex.clamp(0.5 + streak * 0.05 + rings * 0.05)
    rough = 0.78 - streak * 0.08
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.5, seed)
