"""Burnt timber, a metre to the picture: wood charred to charcoal in a fire
that went out long ago. The char has split into the blocky "alligator" cracks
of burnt wood, deep and black between the blocks; grey ash lies in the cracks
and over some blocks, and in places the char has worn through to brown,
scorched grain. It has no grain direction to speak of, so it may be turned."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # The blocks: cellular noise, warped so the cracks wander.
    warp = tex.perlin(X, Y, 5, s()) * 0.03
    f1, f2, ident, _dx, _dy = tex.voronoi(X + warp, Y + warp, 14, s(), 0.9)
    crack = 1.0 - tex.smoothstep(0.02, 0.09, f2 - f1)
    fine = tex.cracks(X, Y, 30, s(), 0.0015, 0.08, 0.4)
    crack = tex.clamp(crack + fine * 0.7)

    # Each block its own shade of charcoal, and a gloss to fresh char.
    shade = tex.unit(ident)
    char = tex.mix(tex.colour(18, 17, 16), tex.colour(44, 40, 37), shade)
    albedo = tex.mix(char, tex.colour(6, 6, 6), crack)
    # Worn through to scorched wood: brown grain that runs along the board.
    grain = tex.fbm(X, Y, (2, 40), 3, s())
    worn = tex.smoothstep(0.62, 0.78, tex.fbm(X, Y, 3, 5, s()))
    wood = tex.mix(tex.colour(70, 42, 24), tex.colour(112, 70, 40), grain)
    albedo = tex.mix(albedo, wood, worn * (1.0 - crack) * 0.9)
    # Ash: grey in the cracks and over blotches.
    ash_patch = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, 6, 4, s()))
    ash = tex.clamp(crack * 0.5 * tex.fbm(X, Y, 40, 2, s()) + ash_patch * 0.6)
    albedo = tex.mix(albedo, tex.colour(128, 124, 118), ash * 0.7)

    height = tex.clamp(0.75 - crack * 0.7 + (shade - 0.5) * 0.1 - worn * 0.15)
    rough = tex.clamp(0.72 + ash * 0.2 - (1.0 - crack) * (1.0 - worn) * shade * 0.2)
    return tex.finish(albedo, height, rough, 5.0, seed=seed)
