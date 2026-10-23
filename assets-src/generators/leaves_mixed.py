"""Leaves, 60 cm to the picture, for the leaf cards of the yard's trees and the
blades of the weeds: each leaf-sized patch is its own green, from deep shade
green to a young yellow-green, with a few gone yellow or brown, a paler
midrib and veins, and a little dust. A leaf of the Map is cut from somewhere
in the picture, so neighbouring leaves differ."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # Leaf-sized patches, each with its own tone.
    warp = tex.perlin(X, Y, 6, s()) * 0.02
    f1, f2, ident, dx, dy = tex.voronoi(X + warp, Y + warp, 5, s(), 0.9)
    pick = tex.unit(ident)
    pick2 = tex.unit(ident * np.uint32(2654435761) + np.uint32(seed))
    deep = tex.colour(30, 58, 22)
    mid = tex.colour(62, 104, 34)
    young = tex.colour(118, 150, 52)
    green = tex.mix(tex.mix(deep, mid, tex.clamp(pick * 2.0)), young, tex.clamp(pick * 2.0 - 1.0))
    autumn = tex.mix(tex.colour(150, 128, 44), tex.colour(110, 70, 34), pick2)
    turned = tex.smoothstep(0.86, 0.9, pick2)
    albedo = tex.mix(green, autumn, turned)

    # Within a leaf: a midrib, veins either side, and a darker rim.
    along = dx * 0.8 + dy * 0.6
    across = dy * 0.8 - dx * 0.6
    midrib = 1.0 - tex.smoothstep(0.0, 0.03, np.abs(across))
    vein_f = (along + np.abs(across)) * 9.0
    veins = 1.0 - tex.smoothstep(0.0, 0.12, tex.wrap_distance(vein_f - np.floor(vein_f), 0.5) - 0.38)
    rim = tex.smoothstep(0.03, 0.0, f2 - f1)
    albedo = albedo * (1.0 + midrib[..., None] * 0.35 + veins[..., None] * 0.12)
    albedo = tex.mix(albedo, albedo * 0.55, rim)
    # Mottling and dust.
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 20, 3, s()))[..., None]
    dust = tex.smoothstep(0.6, 0.8, tex.fbm(X, Y, 4, 4, s()))
    albedo = tex.mix(albedo, tex.colour(120, 116, 96), dust * 0.25)

    height = tex.clamp(0.6 + midrib * 0.2 + veins * 0.08 - rim * 0.3)
    rough = tex.clamp(0.6 - midrib * 0.1 + dust * 0.25 + turned * 0.15)
    return tex.finish(tex.clamp(albedo * 0.85), height, rough, 2.0, seed=seed)
