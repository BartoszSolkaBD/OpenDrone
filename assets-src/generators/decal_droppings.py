"""A decal of bird droppings on a floor (#323): where birds roost on a truss
or a ledge, a loose scatter of splats falls below it, each one a white splash
with a dark brown-grey middle, ragged at the edge, of every size from a coin
down to specks, some with a flick of splash thrown out sideways. The splats
gather in the middle of the picture. It has no up: it can be turned any way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 5100)
    X, Y = tex.grid(n)

    # Where the droppings fell: a wandering patch, thick at its middle.
    where = decalwork.blob(X, Y, s(), radius=0.34, softness=0.3, warp=0.3, stretch=(1.0, 1.0))
    splat = np.zeros_like(X)
    core = np.zeros_like(X)
    for cells, fill, size in ((3, 0.9, 0.5), (6, 0.8, 0.4), (14, 0.6, 0.3)):
        f1, _f2, ident, dx, dy = tex.voronoi(X, Y, cells, s(), 0.9)
        zero = np.zeros(ident.shape, dtype=np.int64)
        r1 = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 21))
        r2 = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 22))
        has = r1 < fill * tex.smoothstep(0.1, 0.5, where)
        radius = 0.08 + size * r2 * r2
        # A ragged edge, and a flick of splash thrown out along the cell's
        # own direction.
        ragged = (tex.fbm(X, Y, cells * 5, 3, s()) - 0.5) * 0.5
        flick = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, cells * 9, 2, s())) * 0.12
        d = f1 * (1.0 + ragged) - flick
        disc = tex.smoothstep(radius, radius - 0.07, d) * has
        splat = np.maximum(splat, disc)
        inner = tex.smoothstep(radius * 0.7, radius * 0.3, d) * has
        core = np.maximum(core, inner)
    mask = tex.smoothstep(0.1, 0.7, splat) * decalwork.frame(X, Y)

    white = tex.colour(226, 224, 214)
    dark = tex.colour(72, 64, 48)
    albedo = tex.mix(white, dark, core * 0.85)
    albedo = albedo * (0.9 + 0.2 * tex.fbm(X, Y, 70, 3, s()))[..., None]

    height = tex.clamp(0.5 + splat * 0.15 + core * 0.1)
    rough = 0.5 + 0.2 * (1.0 - core)
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 2.0, seed)
