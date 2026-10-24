"""A decal of oil stains on a floor (#323): where an engine, a drum or a can
has leaked, dark glossy blots of every size, overlapping, with a thin
soaked ring round each that is darker than the concrete but not black, a few
drips that trail away from the biggest, and a faint blue-green sheen where
the oil is thin. Roughness is low in the blots, so they catch the sky. It
has no up: it can be turned any way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 5000)
    X, Y = tex.grid(n)

    # One big blot, a smaller one beside it and a few spots, each with its
    # own wandering edge; the soaked ring is the same shapes, wider.
    core = np.zeros_like(X)
    wide = np.zeros_like(X)
    for centre, radius, stretch in (((0.46, 0.48), 0.2, (1.1, 0.9)), ((0.7, 0.62), 0.1, (1.0, 1.0)),
                                    ((0.3, 0.7), 0.07, (1.0, 1.2)), ((0.62, 0.3), 0.045, (1.0, 1.0)),
                                    ((0.2, 0.35), 0.03, (1.0, 1.0))):
        seed_k = s()
        core = np.maximum(core, decalwork.blob(X, Y, seed_k, centre, radius, softness=0.05, warp=0.22, stretch=stretch))
        wide = np.maximum(wide, decalwork.blob(X, Y, seed_k, centre, radius * 1.55, softness=0.12, warp=0.3, stretch=stretch))
    # Drips: short thin lines trailing down and sideways from the big blot.
    drips = tex.streaks(tex.smoothstep(0.1, 0.0, np.abs(core - 0.5) - 0.4) * (tex.fbm(X, Y, 18, 2, s()) > 0.55), int(n * 0.16), s(), X, Y, 30, 0.985)
    sheen = tex.fbm(X, Y, 12, 4, s())
    mask = tex.clamp(core + wide * 0.45 * (0.5 + 0.5 * sheen) + drips * 0.5)
    mask = tex.smoothstep(0.04, 0.9, mask) * decalwork.frame(X, Y)

    black = tex.colour(14, 12, 10)
    brown = tex.colour(48, 38, 28)
    sheen_colour = tex.colour(36, 52, 56)
    albedo = tex.mix(brown, black, core)
    albedo = tex.mix(albedo, sheen_colour, tex.smoothstep(0.55, 0.8, sheen) * (1.0 - core) * 0.6)

    height = tex.clamp(0.5 + core * 0.03)
    rough = 0.55 - core * 0.32 + (1.0 - core) * 0.05
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.0, seed)
