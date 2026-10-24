"""A decal of efflorescence, the white salt that damp brings out of brick and
concrete (#323): a crust and a powder along the bottom of the picture, where
water rises up the wall and dries, thinning upward into separate frosty
patches and fine dendritic fingers. Warm white, a little yellow in the thick
crust, with the dry crystals standing a hair proud of the wall. The top of the
picture is up the wall; the bottom may sit on, or just over, the floor."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4200)
    X, Y = tex.grid(n)
    flat = Y * 0.0
    up = 1.0 - Y

    # How high the salt reaches: a wavering line, with fingers climbing.
    swell = tex.fbm(X, flat, (5, 1), 3, s())
    reach = 0.22 + 0.28 * swell
    fingers = tex.smoothstep(0.6, 0.85, tex.fbm(X, flat, (7, 1), 2, s())) * 0.14
    torn = (tex.fbm(X, Y, 11, 5, s()) - 0.5) * 0.3
    cover = tex.smoothstep(reach + fingers + 0.05, reach + fingers - 0.15, up + torn)
    ends = np.minimum(X, 1.0 - X) + (tex.fbm(X, Y, 6, 3, s()) - 0.5) * 0.12
    cover = cover * tex.smoothstep(0.02, 0.14, ends)

    # A powder of fine specks that thins out, and a few thicker crusts.
    specks = tex.smoothstep(0.42, 0.58, tex.fbm(X, Y, 90, 3, s()))
    frost = tex.smoothstep(0.5, 0.7, tex.fbm(X, Y, 24, 4, s()))
    crust = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, 14, 4, s())) * tex.smoothstep(0.1, 0.35, cover)
    body = tex.clamp(cover * 1.2 - 0.3)
    mask = tex.clamp(body * (0.3 + 0.3 * frost) + cover * specks * 0.7 + crust * 0.4)
    mask = tex.smoothstep(0.1, 0.6, mask * cover * 1.1) * decalwork.frame(X, Y, bottom=0.02)

    pale = tex.colour(204, 198, 182)
    warm = tex.colour(184, 172, 138)
    albedo = tex.mix(pale, warm, crust * 0.6 + tex.fbm(X, Y, 8, 3, s()) * 0.2)
    albedo = albedo * (0.9 + 0.2 * tex.fbm(X, Y, 60, 3, s()))[..., None]

    height = tex.clamp(tex.blur(crust * 0.6 + specks * 0.3 + frost * 0.2, 1, 1))
    rough = 0.92 - crust * 0.05
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.5, seed)
