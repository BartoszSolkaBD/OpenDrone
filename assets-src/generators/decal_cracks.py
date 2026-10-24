"""A decal of cracks (#323): a network of dark hairline cracks that fork and
join, wider and darker where they are deepest, with a stain of damp or dust
along the widest ones and a few crumbs of broken edge. It reads on a wall, a
floor, a plinth or a roof. The cracks start at a point near the middle and
die out toward the picture's edges, so no two ends line up with a frame.
It can be turned any way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4800)
    X, Y = tex.grid(n)

    # Cracks follow the edges of cells, bent by noise, and are kept near a
    # wandering centre so they die out toward the edges.
    reach = decalwork.blob(X, Y, s(), radius=0.4, softness=0.28, warp=0.5, stretch=(1.0, 1.0))
    c1 = tex.cracks(X, Y, 5, s(), 0.0045, warp=0.07, branch=0.0)
    c2 = tex.cracks(X, Y, 11, s(), 0.0028, warp=0.05, branch=0.0)
    crack = tex.clamp(np.maximum(c1, c2 * 0.85) * 1.5) * tex.smoothstep(0.1, 0.5, reach)
    # A stain along the cracks, as dust and damp collect in them.
    halo = tex.clamp(tex.blur(crack, max(2, n // 40), 3) * 3.0)
    mask = tex.clamp(crack * 1.2 + halo * 0.3 * tex.fbm(X, Y, 14, 3, s()))
    mask = tex.smoothstep(0.04, 0.7, mask) * decalwork.frame(X, Y)

    albedo = tex.mix(tex.colour(52, 48, 42), tex.colour(14, 12, 10), crack)
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 40, 3, s()))[..., None]
    height = tex.clamp(0.55 - crack * 0.45 + halo * 0.05)
    rough = 0.88 + 0.06 * tex.fbm(X, Y, 16, 3, s())
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 4.0, seed)
