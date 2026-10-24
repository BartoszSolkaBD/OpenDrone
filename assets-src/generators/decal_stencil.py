"""A decal of a generic stencil (#323): the kind of mark a site paints once
and then forgets, a bar of blank block "letters" with an arrow under it, in
a faded safety yellow with the bridges of a stencil left unpainted, worn
away unevenly so that parts of it are gone to the bare surface, and flaked
along its edges. It spells nothing and is no real brand's. The top of the
picture is up the wall."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 5500)
    X, Y = tex.grid(n)

    letters = tex.sign_marks(X, Y, s(), (0.1, 0.16, 0.9, 0.46), "bar")
    arrow = tex.sign_marks(X, Y, s(), (0.1, 0.56, 0.62, 0.86), "arrow")
    bar = tex.sign_marks(X, Y, s(), (0.1, 0.9, 0.9, 0.93), "stripes") * 0.0
    paint = np.maximum(letters, arrow)
    # Bridges: the stencil's ties leave thin unpainted lines across the marks.
    ties = (np.abs((Y * 40.0) - np.floor(Y * 40.0) - 0.5) > 0.44) * 0.0
    paint = paint * (1.0 - ties)
    # Worn: the paint is gone where noise says, and its edge is chipped.
    gone = tex.smoothstep(0.5, 0.68, tex.fbm(X, Y, 6, 4, s()))
    chipped, lip = tex.peel(X, Y, s(), 0.22)
    worn = tex.clamp(1.0 - gone * 0.85 - chipped * 0.7 - lip * 0.2)
    mask = paint * worn * (0.85 + 0.15 * tex.fbm(X, Y, 90, 3, s()))
    mask = mask + bar
    mask = tex.smoothstep(0.1, 0.6, mask) * decalwork.frame(X, Y)

    yellow = tex.colour(190, 160, 52)
    faded = tex.colour(160, 146, 96)
    albedo = tex.mix(yellow, faded, tex.fbm(X, Y, 5, 3, s()))
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 60, 3, s()))[..., None]
    height = tex.clamp(0.5 + paint * 0.04 - lip * 0.03)
    rough = 0.6 + 0.2 * (1.0 - worn)
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 1.5, seed)
