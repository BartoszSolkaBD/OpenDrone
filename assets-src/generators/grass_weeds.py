"""Grass and weeds: 2 m of rough, unmown ground to the picture, seen from
above and slightly aslant. Thousands of blades in several layers over bare
earth, tall dry stems, patches gone straw-yellow, and broad-leaved weeds with
a few small yellow flowers."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    px = n / 1024.0  # blade sizes are written for a 1K picture

    # Bare earth showing between the blades.
    cloud = tex.fbm(X, Y, 6, 5, s())
    earth = tex.mix(tex.colour(48, 36, 26), tex.colour(86, 68, 48), cloud)

    layers = []
    # Undergrowth: short, dark, dense.
    layers.append(tex.blades(n, int(16000 * 1), s(), 24 * px, 2.0 * px, 0.0) + (tex.colour(34, 62, 22), tex.colour(76, 110, 40)))
    # The main sward: longer and brighter.
    layers.append(tex.blades(n, int(11000), s(), 40 * px, 2.0 * px, 0.0) + (tex.colour(60, 98, 30), tex.colour(132, 168, 58)))
    # Tall, dry, pale stems.
    layers.append(tex.blades(n, int(1800), s(), 72 * px, 1.5 * px, 0.3) + (tex.colour(120, 112, 64), tex.colour(186, 170, 100)))
    # Broad-leaved weeds: fewer, wide leaves.
    layers.append(tex.blades(n, int(500), s(), 64 * px, 5.0 * px, 0.0) + (tex.colour(46, 96, 36), tex.colour(100, 150, 56)))

    albedo = earth
    height = 0.12 + 0.05 * cloud
    top = np.zeros_like(X)
    patch = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, 5, 4, s()))
    for h, tone, mask, low, high in layers:
        shade = 0.55 + 0.45 * h  # the lower parts of a blade are darker
        blade = tex.mix(low, high, tone) * shade[..., None]
        # Patches of the whole sward gone straw-yellow.
        blade = tex.mix(blade, tex.colour(150, 140, 70) * shade[..., None], patch * 0.5)
        albedo = tex.mix(albedo, blade, mask)
        height = np.where(mask > 0.0, np.maximum(height, 0.25 + 0.7 * h), height)
        top = np.maximum(top, h)

    # A few small yellow flowers among the weeds.
    f1, _f2, ident, _dx, _dy = tex.voronoi(X, Y, 46, s())
    has = tex.unit(ident) > 0.93
    flower = tex.smoothstep(0.1, 0.05, f1) * has
    albedo = tex.mix(albedo, tex.colour(236, 200, 50), flower * 0.9)
    height = np.maximum(height, 0.3 + 0.5 * flower)

    # Shade between the blades, darkest where they are thickest and lowest.
    albedo = albedo * (0.72 + 0.28 * tex.clamp(height * 1.4))[..., None]
    rough = 0.8 + 0.12 * (1.0 - height)
    # Softened a little, or every blade's tip would be a speck in the normal map.
    return tex.finish(albedo, tex.blur(tex.clamp(height), 1, 1), rough, 7.0, seed=seed)
