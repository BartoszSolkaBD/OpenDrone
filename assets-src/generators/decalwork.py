"""What every decal's pictures share (#321): a decal is one picture put once
on a surface, not a tile, so unlike a material's picture it doesn't repeat.
Its fourth picture, the **mask**, says how much of it covers the surface: 255
where it covers all, 0 where the surface shows. The mask is 0 along every edge
of the picture, so the decal's rectangle never shows, and the colour is filled
in everywhere, under the mask's 0 too, so a blurred edge doesn't darken.

The blocks are odtex's, so the rules at the top of `odtex.py` hold: whole
numbers and `+ - * /`, `sqrt` and `floor` only, the same bytes everywhere.
`X, Y` run from 0 to 1, `Y` down the picture: its top is the decal's `up`.
"""

import numpy as np

import odtex as tex

# How close to the edge the mask must be 0, and how far in it may be whole,
# as a share of the picture.
CLEAR_EDGE = 0.012
FRAME = 0.05


def frame(X, Y, bottom=None):
    """1 well inside the picture, falling to exactly 0 within `CLEAR_EDGE` of
    every edge. `bottom` lets the mask come closer to the bottom edge (for a
    decal that meets the floor at the foot of a wall)."""
    side = np.minimum(np.minimum(X, 1.0 - X), Y)
    out = tex.smoothstep(CLEAR_EDGE, FRAME, side)
    low = FRAME if bottom is None else bottom
    return out * tex.smoothstep(CLEAR_EDGE, max(low, CLEAR_EDGE * 1.5), 1.0 - Y)


def blob(X, Y, seed, centre=(0.5, 0.5), radius=0.36, softness=0.08, warp=0.18, stretch=(1.0, 1.0)):
    """An irregular patch: 1 inside, 0 outside, with a soft, wandering edge.
    `radius` is its size as a share of the picture, `warp` how far its edge
    wanders, `stretch` makes it wider or taller."""
    s = tex.Seeds(seed)
    wx = (tex.fbm(X, Y, 3, 4, s()) - 0.5) * warp
    wy = (tex.fbm(X, Y, 3, 4, s()) - 0.5) * warp
    dx = (X - centre[0] + wx) / stretch[0]
    dy = (Y - centre[1] + wy) / stretch[1]
    r = np.sqrt(dx * dx + dy * dy)
    edge = radius + (tex.fbm(X, Y, 7, 3, s()) - 0.5) * warp * 0.6
    return tex.smoothstep(edge, edge - softness, r)


def finish(albedo, height, roughness, mask, relief, seed):
    """The four pictures of a decal: colour, normal map and roughness as
    `odtex.finish` makes them, and the mask, forced to 0 along the edges."""
    n = albedo.shape[0]
    X, Y = tex.grid(n)
    side = np.minimum(np.minimum(X, 1.0 - X), np.minimum(Y, 1.0 - Y))
    mask = np.where(side < CLEAR_EDGE, 0.0, tex.clamp(mask))
    # A decal whose surface is one roughness all over may pass that number.
    roughness = np.zeros_like(mask) + roughness
    height = np.zeros_like(mask) + height
    diffuse, normal, rough = tex.finish(albedo, height, roughness, relief, seed=seed)
    return diffuse, normal, rough, tex.to_u8(mask)
