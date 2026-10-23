"""The brick wall under two materials: a running bond of old clinker bricks,
1.8 m of wall to the picture (24 courses of 75 mm, eight bricks of 225 mm
across), before any weather. `brick_red_weathered` weathers it in the open,
`brick_painted_peeling` paints and plasters over it.

Each brick is its own shade, from scorched purple-brown to pale orange, with
grain and speckle in its clay. The mortar is recessed and sandy; a few bricks
have lost their face and the edges are chipped."""

import numpy as np

import odtex as tex

COURSES = 24
PER_COURSE = 8


def fresh_brick(X, Y, s):
    """Returns a dict: `layout` (from `odtex.bricks`), `albedo`, `height` (0 to
    1; joints low), `rough`, and the masks `joint`, `edge`, `chip`, `spall`,
    `cloud`, `mortar_grain`."""
    layout = tex.bricks(X, Y, COURSES, PER_COURSE, 0.13, s())
    ident = layout["id"]
    joint = layout["joint"]
    edge = layout["edge"]
    zero = np.zeros(ident.shape, dtype=np.int64)

    base_a = tex.colour(150, 66, 46)
    base_b = tex.colour(178, 96, 64)
    dark = tex.colour(104, 50, 42)
    pick = tex.contrast(ident, 1.3)
    brick = tex.mix(base_a, base_b, pick)
    # Some bricks were fired too hot and come out dark, plum-brown.
    brick_no = (ident * 16777216.0).astype(np.int64)
    scorched = tex.cell_random(brick_no, zero, s())
    brick = tex.mix(brick, dark, tex.smoothstep(0.84, 0.98, scorched) * 0.6)
    nudge = tex.cell_random(brick_no, zero, s())
    brick = brick * (0.88 + 0.24 * nudge)[..., None]

    # The clay's own grain: soft cloudiness, and a little fine speckle.
    cloud = tex.fbm(X, Y, 12, 5, s())
    speckle = tex.fbm(X, Y, 150, 2, s())
    mottle = tex.fbm(X, Y, 40, 4, s())
    brick = brick * (0.82 + 0.36 * cloud)[..., None] * (0.86 + 0.28 * mottle)[..., None]
    brick = tex.mix(brick, tex.colour(70, 42, 36), tex.smoothstep(0.7, 0.9, speckle) * 0.22)
    brick = tex.mix(brick, tex.colour(204, 140, 100), tex.smoothstep(0.7, 0.86, tex.fbm(X, Y, 90, 2, s())) * 0.2)

    mortar_grain = tex.fbm(X, Y, 120, 3, s())
    mortar = tex.mix(tex.colour(112, 106, 96), tex.colour(172, 164, 148), mortar_grain)
    mortar = tex.mix(mortar, tex.colour(60, 56, 52), tex.smoothstep(0.62, 0.8, tex.fbm(X, Y, 200, 2, s())) * 0.5)

    # Chipped edges: the face breaks away along the arris, showing paler clay.
    chip = tex.edge_wear(edge, s(), X, Y, 0.05, 0.9) * (1.0 - joint)
    chip = chip * tex.smoothstep(0.45, 0.62, tex.fbm(X, Y, 30, 3, s()))
    brick = tex.mix(brick, tex.colour(196, 126, 94), chip * 0.7)

    # A few bricks have lost their face: a deeper, rougher, darker recess.
    spall = (ident > 0.955).astype(np.float64) * (1.0 - joint) * tex.smoothstep(0.0, 0.1, edge)
    brick = tex.mix(brick, tex.colour(120, 66, 52) * (0.7 + 0.4 * cloud)[..., None], spall)

    # The joint is recessed, so the brick's edge and the mortar are in shade.
    brick = brick * (0.78 + 0.22 * tex.smoothstep(0.0, 0.14, edge))[..., None]
    mortar = mortar * (0.8 + 0.2 * tex.smoothstep(-0.07, 0.0, edge))[..., None]
    albedo = tex.mix(brick, mortar, joint)

    # Height: faces stand proud of the joint, rounded at the arris, with the
    # clay's own pitting; chips and spalls sunk.
    arris = tex.smoothstep(-0.02, 0.07, edge)
    pit = tex.fbm(X, Y, 100, 4, s())
    face = 0.78 + 0.22 * pit - chip * 0.25 - spall * 0.4
    height = tex.mix(face, 0.25 + 0.1 * mortar_grain, 1.0 - arris)
    rough = tex.mix(0.9 - 0.12 * cloud, 0.96 - 0.04 * mortar_grain, joint)
    return {
        "layout": layout,
        "albedo": albedo,
        "height": tex.clamp(height),
        "rough": rough,
        "joint": joint,
        "edge": edge,
        "chip": chip,
        "spall": spall,
        "cloud": cloud,
        "mortar_grain": mortar_grain,
    }
