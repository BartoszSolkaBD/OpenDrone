"""A decal of lichen (#323): round crusty rosettes of every size, grey-green,
sulphur yellow and orange, scattered unevenly over the picture and growing
into one another where they meet, each with a rim a shade brighter than its
middle and rings of growth. They grow on still, old stone and on roofs more
than on a wall's brick, mostly in clumps. It has no up: it can be turned any
way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4300)
    X, Y = tex.grid(n)

    # Rosettes: each cell of a cellular noise may hold one, of its own size
    # and colour; a second, finer set fills in between.
    mask = np.zeros_like(X)
    tone = np.zeros_like(X)
    rings = np.zeros_like(X)
    hue = np.zeros_like(X)
    clump = tex.fbm(X, Y, 3, 3, s())
    for cells, fill, size in ((4, 0.9, 0.36), (8, 0.8, 0.32), (16, 0.55, 0.26)):
        f1, _f2, ident, dx, dy = tex.voronoi(X, Y, cells, s(), 0.9)
        zero = np.zeros(ident.shape, dtype=np.int64)
        r1 = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 11))
        r2 = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 12))
        r3 = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 13))
        # Clumps: more rosettes where the slow noise is high.
        has = r1 < fill * tex.clamp(0.05 + 1.5 * clump - 0.2)
        radius = 0.1 + size * (0.4 + 0.6 * r2)
        wobble = (tex.fbm(X, Y, cells * 3, 4, s()) - 0.5) * 0.7
        d = f1 * (1.0 + wobble)
        disc = tex.smoothstep(radius, radius - 0.06, d) * has
        take = disc > mask
        mask = np.where(take, disc, mask)
        # How far into the rosette: 0 at its rim, 1 in the middle.
        depth = tex.clamp(1.0 - d / np.maximum(radius, 1e-3))
        tone = np.where(take, depth, tone)
        hue = np.where(take, r3, hue)
        rings = np.where(take, tex.smoothstep(0.35, 0.65, tex.fbm(X, Y, cells * 6, 2, s())), rings)
    fine = tex.fbm(X, Y, 150, 3, s())
    mask = tex.clamp(mask * (0.55 + 0.7 * fine) * 1.25)
    mask = tex.smoothstep(0.15, 0.6, mask) * decalwork.frame(X, Y)

    grey = tex.colour(118, 124, 102)
    yellow = tex.colour(146, 144, 92)
    orange = tex.colour(146, 108, 66)
    base = tex.mix(grey, yellow, tex.smoothstep(0.45, 0.75, hue))
    base = tex.mix(base, orange, tex.smoothstep(0.85, 0.95, hue))
    # A rim a shade brighter, the middle duller and rougher with rings.
    albedo = base * (1.1 - 0.35 * tone * (0.7 + 0.3 * rings))[..., None]
    albedo = albedo * (0.85 + 0.3 * fine)[..., None]

    height = tex.clamp(mask * (0.4 + 0.3 * fine + 0.2 * rings))
    rough = 0.9 - 0.05 * tone
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 3.0, seed)
