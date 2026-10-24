"""A decal of moss at the foot of a wall (#321): a mat of moss, thick and
joined along the bottom, where the damp stays, thinning upward into separate
cushions, islands and ragged tongues. Deep green in the shade of each cushion,
yellow-green on the tops, with a few dry brown patches. The top of the picture
is up the wall; its bottom is meant to sit on, or just over, the floor, so the
mask reaches nearly to the bottom edge."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 3100)
    X, Y = tex.grid(n)
    up = 1.0 - Y  # 0 at the foot, 1 at the top
    flat = Y * 0.0

    # How high the moss reaches along the wall: a line that rises and falls,
    # with tongues climbing higher here and there.
    swell = tex.fbm(X, flat, (4, 1), 4, s())
    reach = 0.36 + 0.3 * tex.clamp((swell - 0.3) / 0.4)
    tongues = tex.smoothstep(0.55, 0.8, tex.fbm(X, flat, (6, 1), 3, s()))
    reach = reach + tongues * 0.1
    # The edge is torn up by finer noise, and above it islands are left.
    torn = (tex.fbm(X, Y, 9, 5, s()) - 0.5) * 0.26
    cover = tex.smoothstep(reach + 0.04, reach - 0.12, up + torn)
    islands = tex.smoothstep(0.6, 0.72, tex.fbm(X, Y, 14, 4, s())) * tex.smoothstep(reach + 0.3, reach, up)
    cover = tex.clamp(cover + islands * 0.9)
    # The two ends thin out, unevenly.
    ends = np.minimum(X, 1.0 - X) + (tex.fbm(X, Y, 6, 3, s()) - 0.5) * 0.12
    cover = cover * tex.smoothstep(0.02, 0.14, ends)

    # Cushions of every size: two cellular noises, so they don't line up.
    f1a, f2a, ida, _x, _y = tex.voronoi(X, Y, 44, s(), 1.0)
    f1b, _f2b, _idb, _x, _y = tex.voronoi(X, Y, 19, s(), 1.0)
    dome = tex.clamp(np.maximum(1.0 - f1a * 1.25, (1.0 - f1b * 1.15) * 0.9))
    crease = tex.smoothstep(0.0, 0.1, f2a - f1a)
    fine = tex.fbm(X, Y, 120, 3, s())
    # Thick where it is well inside; a cushion's own shape at the fringe.
    body = tex.clamp(cover * 1.6 - 0.25)
    mask = tex.clamp(body * (0.75 + 0.25 * dome) + (cover - body) * dome * 1.4)
    mask = mask * (0.82 + 0.25 * fine) * (0.85 + 0.15 * crease)
    # The bottom of the picture lies out on the ground: there the moss thins
    # out raggedly, so no straight edge shows.
    low = 0.03 + 0.12 * tex.fbm(X, Y, (7, 5), 4, s())
    mask = mask * tex.smoothstep(low, low + 0.12, up + (fine - 0.5) * 0.08)
    mask = tex.smoothstep(0.08, 0.5, mask) * decalwork.frame(X, Y, bottom=0.02)

    # Colour, filled in everywhere: deep green in the creases and shade,
    # yellow-green on the tops, brown where a patch has dried.
    deep = tex.colour(20, 28, 13)
    lit = tex.colour(72, 84, 34)
    tone = tex.clamp(dome * 0.75 * crease + fine * 0.45 - 0.12)
    albedo = tex.mix(deep, lit, tone)
    # Patches of darker, older moss and of fresher, lighter growth.
    age = tex.fbm(X, Y, 5, 4, s())
    albedo = albedo * (0.75 + 0.5 * age)[..., None]
    dry = tex.smoothstep(0.66, 0.78, tex.fbm(X, Y, 7, 4, s())) * tex.smoothstep(0.1, 0.5, up)
    albedo = tex.mix(albedo, tex.colour(96, 80, 44), dry * 0.75)
    tips = tex.smoothstep(0.68, 0.82, tex.fbm(X, Y, 220, 2, s()))
    albedo = tex.mix(albedo, tex.colour(118, 124, 58), tips * dome * 0.35)

    height = tex.clamp(dome * 0.7 * crease + fine * 0.12)
    rough = 0.88 - dome * 0.05 + dry * 0.05
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 5.0, seed)
