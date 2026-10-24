"""A decal of scuffs on a floor (#323): where people walk, boots and dragged
things have rubbed the dirt and the paint into pale, smeared streaks, most
of them running one way (along the picture's `up`, which is the way people
walk), with grit dragged out in short fine lines and a few darker heel marks
across them. The streaks are thick in the middle of the picture and thin out
to its sides, so a path of them has a worn middle and ragged edges.
The top of the picture is the way the path runs."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4600)
    X, Y = tex.grid(n)
    flat = Y * 0.0

    # Smears: long lanes along the path, broken up, strongest in the middle.
    lanes = tex.fbm(X, Y, (30, 3), 3, s())
    wide = tex.fbm(X, Y, (9, 2), 2, s())
    along = tex.fbm(X, Y, (30, 6), 2, s())
    smear = tex.smoothstep(0.42, 0.65, lanes) * (0.35 + 0.8 * wide) * tex.smoothstep(0.3, 0.6, along)
    # The path's own width: wavering sides, a worn middle.
    wander = (tex.fbm(X, flat + Y, (2, 3), 2, s()) - 0.5) * 0.14
    middle = tex.smoothstep(0.46, 0.08, np.abs(X - 0.5 + wander))
    # Thin grit lines, dragged along the path.
    grit = tex.smoothstep(0.64, 0.8, tex.fbm(X, Y, (90, 10), 2, s())) * tex.smoothstep(0.5, 0.15, np.abs(X - 0.5 + wander))
    # A few heel marks across it: short dark crescents' worth of smudge.
    heel = tex.smoothstep(0.7, 0.85, tex.fbm(X, Y, (12, 18), 2, s())) * middle
    mask = tex.clamp(smear * middle * 1.3 + grit * 0.9)
    mask = tex.clamp(mask + heel * 0.4)
    mask = mask * (0.75 + 0.3 * tex.fbm(X, Y, 60, 3, s())) * decalwork.frame(X, Y)

    # Pale where grit and dust were dragged; darker in the heel marks.
    # Dirt and rubber, dark; the dragged grit is paler.
    dirt = tex.colour(70, 66, 58)
    pale = tex.colour(176, 170, 156)
    albedo = tex.mix(dirt, pale, tex.clamp(grit * 1.5))
    albedo = tex.mix(albedo, tex.colour(34, 32, 30), heel * 0.8)
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 50, 3, s()))[..., None]

    height = tex.clamp(0.5 + grit * 0.15 - smear * 0.05)
    rough = 0.62 + 0.2 * smear
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.5, seed)
