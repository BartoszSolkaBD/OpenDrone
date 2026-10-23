"""Dirt and rubble ground: 2 m of ground to the picture. Packed brown earth
with dry cracks and trampled lanes, gravel, and chunks of broken concrete,
brick, tile and slag lying in it, each dusted with the earth's own colour and
each with a dark contact shadow where it sits. A few weeds have sprouted in
the cracks and between the stones."""

import numpy as np

import odtex as tex


def stone_colour(ident, zero):
    """A colour for each stone from its number: mostly broken concrete, some
    brick, a little tile and slag."""
    pick = tex.unit(ident)
    vary = tex.unit(tex.hash_u32(ident.astype(np.int64), zero, 1))
    concrete = tex.mix(tex.colour(118, 116, 110), tex.colour(164, 160, 150), vary)
    brick = tex.mix(tex.colour(128, 62, 46), tex.colour(160, 88, 64), vary)
    tile = tex.mix(tex.colour(188, 180, 160), tex.colour(220, 214, 198), vary)
    slag = tex.mix(tex.colour(46, 42, 40), tex.colour(84, 78, 74), vary)
    flint = tex.mix(tex.colour(120, 108, 92), tex.colour(170, 154, 128), vary)
    out = tex.mix(concrete, flint, tex.smoothstep(0.5, 0.52, pick))
    out = tex.mix(out, brick, tex.smoothstep(0.7, 0.72, pick))
    out = tex.mix(out, tile, tex.smoothstep(0.86, 0.88, pick))
    out = tex.mix(out, slag, tex.smoothstep(0.93, 0.95, pick))
    return out


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    zero = np.zeros(X.shape, dtype=np.int64)

    # The earth: three browns, cloud on cloud, with grit.
    cloud = tex.fbm(X, Y, 4, 6, s())
    patches = tex.fbm(X, Y, 7, 3, s())
    earth = tex.mix(tex.colour(78, 58, 42), tex.colour(124, 98, 72), cloud)
    earth = tex.mix(earth, tex.colour(150, 126, 96), tex.smoothstep(0.55, 0.8, patches) * 0.6)
    grit = tex.fbm(X, Y, 200, 2, s())
    earth = earth * (0.8 + 0.4 * grit)[..., None]
    clods = tex.fbm(X, Y, 40, 4, s())
    earth = earth * (0.85 + 0.3 * clods)[..., None]
    # Trampled, compacted lanes: darker, smoother.
    lane = tex.smoothstep(0.55, 0.72, tex.fbm(X, Y, (3, 9), 4, s()))
    earth = earth * (1.0 - 0.2 * lane)[..., None]

    # Dry cracks in the earth, in the drier patches.
    crack = tex.cracks(X, Y, 5, s(), 0.003, 0.08, 0.3) * tex.smoothstep(0.35, 0.6, patches)
    earth = tex.mix(earth, tex.colour(36, 27, 20), crack * 0.8)

    # Gravel (many small, round) and chunks (fewer, bigger, angular).
    g_h, g_id, g_mask, g_in = tex.stones(X, Y, 80, s(), 0.3, 0.3)
    c_h, c_id, c_mask, c_in = tex.stones(X, Y, 13, s(), 0.4, 0.8)
    m_h, m_id, m_mask, m_in = tex.stones(X, Y, 30, s(), 0.25, 0.6)
    solid = np.maximum(g_mask, np.maximum(c_mask, m_mask))

    grain = tex.fbm(X, Y, 120, 3, s())
    def lit(colour, h):
        return colour * ((0.5 + 0.7 * h) * (0.82 + 0.36 * grain))[..., None]

    # Contact shadow: the earth darkens round each stone.
    shadow = tex.clamp(tex.blur(solid, 5, 2) * 2.4 - solid)
    earth = earth * (1.0 - 0.6 * shadow)[..., None]

    albedo = earth
    for colours, h, mask in ((stone_colour(g_id, zero), g_h, g_mask), (stone_colour(m_id, zero), m_h, m_mask), (stone_colour(c_id, zero), c_h, c_mask)):
        albedo = tex.mix(albedo, lit(colours, h), mask)
    # Cracks and chips in the big pieces, and a film of dust over everything.
    chip = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, 60, 3, s())) * c_mask
    albedo = tex.mix(albedo, albedo * 0.7, chip * 0.5)
    film = tex.fbm(X, Y, 30, 3, s())
    albedo = tex.mix(albedo, tex.colour(128, 104, 80) * 0.95, (0.12 + 0.28 * film) * solid * (1.0 - 0.5 * np.maximum(c_h, m_h)))

    # A few weeds in the cracks and round the stones.
    sprout = tex.smoothstep(0.7, 0.8, tex.fbm(X, Y, 24, 4, s())) * (1.0 - solid)
    sprout = np.maximum(sprout, tex.clamp(shadow * 1.5) * tex.smoothstep(0.55, 0.7, tex.fbm(X, Y, 50, 3, s())) * (1.0 - solid))
    leaf = tex.mix(tex.colour(62, 86, 36), tex.colour(110, 130, 56), tex.fbm(X, Y, 100, 2, s()))
    albedo = tex.mix(albedo, leaf, sprout * 0.75)

    height = 0.25 + 0.12 * cloud + 0.06 * grit + 0.06 * clods - 0.1 * crack
    height = height + 0.3 * g_h + 0.6 * m_h + 0.8 * c_h + 0.06 * sprout
    height = tex.clamp(height)
    rough = tex.mix(0.97 - 0.05 * lane, 0.8 + 0.12 * grain, solid)
    return tex.finish(albedo, height, rough, 12.0, seed=seed)
