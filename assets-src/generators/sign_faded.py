"""A faded painted sign on a steel board, 4 m to the picture, for one board on
a wall: the Map's script puts the board over the part of one repeat that holds
the sign (from 0.10 to 0.90 across and 0.30 to 0.70 down), so it shows once.
Cream paint, chalky and peeling to rust, carries a band of blocky lettering
that spells nothing and a strip of hazard chevrons below; rust runs down from
the bolts. No real brand, word or company."""

import numpy as np

import odtex as tex

# Where the board is in the picture: left, top, right, bottom in tile units.
BOARD = (0.10, 0.30, 0.90, 0.70)


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    left, top, right, bottom = BOARD

    # Steel showing through, then the cream paint over it, chalky and blotched.
    steel = tex.mix(tex.colour(70, 48, 36), tex.colour(120, 78, 50), tex.fbm(X, Y, 30, 3, s()))
    cream = tex.mix(tex.colour(176, 168, 140), tex.colour(206, 198, 170), tex.fbm(X, Y, 8, 4, s()))
    gone, lip = tex.peel(X, Y, s(), 0.22)
    albedo = tex.mix(cream, steel, gone)

    # The lettering: a band of blocky shapes, faded and patchy.
    letters = tex.sign_marks(X, Y, seed + 7, (0.16, 0.36, 0.84, 0.52), kind="bar")
    fade = 0.45 + 0.4 * tex.fbm(X, Y, 10, 3, s())
    albedo = tex.mix(albedo, tex.colour(120, 36, 30), letters * fade * (1.0 - gone))
    # Hazard chevrons in a strip along the bottom of the board.
    chev = tex.sign_marks(X, Y, seed, (0.13, 0.58, 0.87, 0.65), kind="stripes")
    strip = ((X >= 0.13) & (X < 0.87) & (Y >= 0.58) & (Y < 0.65)).astype(np.float64)
    yellow = tex.colour(196, 160, 60)
    albedo = tex.mix(albedo, yellow, strip * (1.0 - chev) * fade * (1.0 - gone))
    albedo = tex.mix(albedo, tex.colour(30, 28, 26), chev * fade * (1.0 - gone))

    # Bolts at the board's corners, rust streaking down from each, and from
    # the board's top edge.
    bolts = np.zeros_like(X)
    for bx in (left + 0.02, right - 0.02):
        for by in (top + 0.03, bottom - 0.03):
            d = np.sqrt((X - bx) * (X - bx) + (Y - by) * (Y - by))
            bolts = np.maximum(bolts, 1.0 - tex.smoothstep(0.004, 0.007, d))
    top_edge = tex.band(Y, top + 0.004, 0.004, 0.006) * ((X > left) & (X < right))
    rust_run = tex.streaks(tex.clamp(bolts + top_edge * 0.6 + lip * 0.3), 140, s(), X, Y, 40, 0.988)
    albedo = tex.mix(albedo, tex.colour(110, 58, 30), tex.clamp(rust_run * 0.8))
    albedo = tex.mix(albedo, tex.colour(40, 34, 30), bolts)
    grime = tex.smoothstep(0.5, 0.8, tex.fbm(X, Y, 5, 4, s()))
    albedo = tex.mix(albedo, albedo * 0.6, grime * 0.5)

    height = tex.clamp(0.4 + (1.0 - gone) * 0.25 + lip * 0.3 + bolts * 0.5 + (tex.fbm(X, Y, 40, 3, s()) - 0.5) * 0.2)
    rough = tex.clamp(0.7 + gone * 0.15 - letters * 0.05 + grime * 0.1)
    return tex.finish(tex.clamp(albedo), height, rough, 6.0, seed=seed)
