"""The large-scale picture that breaks up a big surface (#304): 80 m to the
picture, not a material of its own. The game lays it over the world, by where
a surface is, and multiplies the colour of every material that asks for it
with it, so a long floor or wall has cleaner and dirtier areas, bleached and
damp zones and stained patches that carry on from one mesh to the next.

How to read it: the colour picture is a multiplier, not a colour. A value of
128 (half of 255) is a multiplier of 1, so nothing changes where the picture
is mid-grey; 77 is about 0.6 (darker) and 166 about 1.3 (lighter). A bright
value also nudges a surface's roughness up and a dark one down: the game
reads that from how light the multiplier is. The normal map is a faint grain
and the roughness picture is not used by the game: they are here because
every generated texture has all three.

The picture is 80 m wide with soft features of 10 to 40 m, as a texture
that is never looked at one metre at a time."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # The multiplier starts at 1 and each layer of grime or wear scales it.
    multiplier = np.ones(X.shape + (3,), dtype=np.float64)

    # Dirtier and cleaner areas: broad, soft, the biggest feature (40 m).
    cloud = tex.fbm(X, Y, 2, 5, s())
    dirty = tex.smoothstep(0.42, 0.72, cloud)
    multiplier = multiplier * tex.mix(np.ones(3), np.array([0.72, 0.68, 0.60]), dirty)

    # Bleached zones: sun-faded and dry, lighter and a little warmer.
    sun = tex.smoothstep(0.58, 0.8, tex.fbm(X, Y, (3, 2), 4, s()))
    multiplier = multiplier * tex.mix(np.ones(3), np.array([1.32, 1.28, 1.18]), sun)

    # Damp zones: darker, a little cooler and green.
    damp = tex.smoothstep(0.55, 0.78, tex.fbm(X, Y, (2, 3), 4, s()))
    multiplier = multiplier * tex.mix(np.ones(3), np.array([0.78, 0.86, 0.80]), damp)

    # Stained patches of 5 to 15 m, soft-edged, a shade either way.
    patches = tex.fbm(X, Y, 8, 4, s())
    multiplier = multiplier * (0.86 + 0.28 * patches)[..., None]

    # The colour picture holds the multiplier at half scale, 1 at 128.
    albedo = tex.clamp(multiplier * 0.5, 0.0, 1.0)

    # A faint grain for the normal map and a roughness picture, which the game
    # doesn't read.
    height = tex.fbm(X, Y, 24, 3, s())
    rough = 0.5 + 0.2 * (patches - 0.5)
    return tex.finish(albedo, height, rough, 12.0, grain=0.0, seed=seed)
