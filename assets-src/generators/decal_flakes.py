"""A decal of flakes of peeling paint (#323): chips of old paint that have
curled off the wall or ceiling and lie where they fell, scattered thickly at
the middle of the picture and thinning out, each an angular flake of its own
size, in two or three faded paint colours (a pale green, a cream, a dull
blue-grey), brighter at the curled edge and darker where the underside shows,
with a dusting of tiny crumbs round them. It has no up: it can be turned any
way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 5200)
    X, Y = tex.grid(n)

    where = decalwork.blob(X, Y, s(), radius=0.42, softness=0.3, warp=0.35)
    height_a, ident_a, mask_a, edge_a = tex.stones(X, Y, 7, s(), fill=0.9, angular=0.85)
    height_b, ident_b, mask_b, edge_b = tex.stones(X, Y, 16, s(), fill=0.7, angular=0.9)
    # Cells decide where flakes are; the patch decides which of them survive.
    zero_a = np.zeros(ident_a.shape, dtype=np.int64)
    zero_b = np.zeros(ident_b.shape, dtype=np.int64)
    keep_a = tex.unit(tex.hash_u32(ident_a.astype(np.int64), zero_a, 31)) < tex.smoothstep(0.0, 0.5, where)
    keep_b = tex.unit(tex.hash_u32(ident_b.astype(np.int64), zero_b, 32)) < tex.smoothstep(0.0, 0.4, where) * 0.8
    big = mask_a * keep_a
    small = mask_b * keep_b * (1.0 - big)
    flake = np.maximum(big, small)
    crumbs = tex.smoothstep(0.7, 0.8, tex.fbm(X, Y, 130, 2, s())) * tex.smoothstep(0.05, 0.4, where) * (1.0 - flake) * 0.8
    mask = tex.clamp(flake + crumbs) * decalwork.frame(X, Y)

    ident = np.where(big > 0.0, ident_a, ident_b)
    which = tex.unit(tex.hash_u32(ident.astype(np.int64), np.zeros(ident.shape, dtype=np.int64), 33))
    green = tex.colour(128, 150, 120)
    cream = tex.colour(188, 178, 146)
    blue = tex.colour(112, 130, 138)
    paint = tex.mix(green, cream, tex.smoothstep(0.3, 0.45, which))
    paint = tex.mix(paint, blue, tex.smoothstep(0.7, 0.8, which))
    edge = np.where(big > 0.0, edge_a, edge_b)
    # A curled flake: bright rim, a darker, older inside.
    albedo = paint * (0.8 + 0.5 * tex.clamp(1.0 - edge * 7.0) - 0.2 * tex.clamp(edge * 5.0))[..., None]
    albedo = tex.mix(albedo, tex.colour(70, 62, 48), crumbs * 0.5)
    albedo = albedo * (0.9 + 0.2 * tex.fbm(X, Y, 60, 3, s()))[..., None]

    height = tex.clamp(np.where(big > 0.0, height_a, height_b) * 0.8 * flake + crumbs * 0.3)
    rough = 0.55 + 0.1 * tex.fbm(X, Y, 30, 3, s())
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 5.0, seed)
