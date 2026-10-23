"""Rusted steel beam: 1 m of steel plate to the picture, two plates joined by
a welded seam across the middle, a row of rivet heads along each side of it,
and the picture's top and bottom edges as plate edges. Blue-grey paint
survives in islands; rust spreads from the seam, the rivets and the edges in
layers (orange bloom, brown scale, black pitted crust), starting as specks
under the paint, running down in streaks, and in a few places the plate has
been scratched to bright steel."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # Where the plates meet: the seam across the middle, the plate edges at
    # the top and bottom of the tile, and rows of rivets beside the seam.
    seam_d = tex.wrap_distance(Y, 0.5)
    seam = 1.0 - tex.smoothstep(0.006, 0.02, seam_d)
    ragged = tex.fbm(X, Y, (60, 4), 2, s())
    bead = (1.0 - tex.smoothstep(0.0, 0.014 + 0.006 * ragged, seam_d)) * (0.7 + 0.3 * ragged)
    edge_d = tex.wrap_distance(Y, 0.0)
    edge = 1.0 - tex.smoothstep(0.004, 0.035, edge_d)

    rivets = np.zeros_like(X)
    rivet_ring = np.zeros_like(X)
    rivet_top = np.zeros_like(X)
    count = 14
    for row_y in (0.5 - 0.075, 0.5 + 0.075):
        along = X * count
        cx = (along - np.floor(along)) - 0.5
        cy = (Y - row_y) * count
        dist = np.sqrt(cx * cx + cy * cy)
        rivets = np.maximum(rivets, 1.0 - tex.smoothstep(0.17, 0.2, dist))
        rivet_ring = np.maximum(rivet_ring, 1.0 - tex.smoothstep(0.0, 0.05, np.abs(dist - 0.27)))
        rivet_top = np.maximum(rivet_top, tex.clamp(1.0 - dist / 0.2))

    # Rust grows from the seam, rivets and edges, and from anywhere the paint
    # has failed.
    failed = tex.smoothstep(0.6, 0.74, tex.fbm(X, Y, 5, 4, s()))
    source = tex.clamp(seam * 0.8 + edge * 0.9 + tex.blur(rivets, 4, 2) * 2.5 + failed * 0.7)
    rust, depth = tex.rust_spread(source, X, Y, s(), 0.09, 0.68)
    # Streaks run down from the seam and the rivets, and from every patch.
    run = tex.streaks(tex.clamp(seam * 0.6 + rivets * 0.9 + rust * 0.25), 220, s(), X, Y, 70, 0.992)
    rust = tex.clamp(np.maximum(rust, run * 0.5))

    # Paint: blue-grey, scuffed and dull; bare steel where it was scratched.
    wear = tex.fbm(X, Y, 8, 5, s())
    paint = tex.mix(tex.colour(62, 78, 90), tex.colour(98, 112, 122), wear)
    paint = paint * (0.9 + 0.2 * tex.fbm(X, Y, 120, 2, s()))[..., None]
    chalk = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, 20, 4, s()))
    paint = tex.mix(paint, tex.colour(138, 148, 152), chalk * 0.35)
    scratch_n = tex.fbm(X + tex.perlin(X, Y, 6, s()) * 0.03, Y, (9, 120), 3, s())
    scratch = tex.smoothstep(0.64, 0.7, scratch_n) * tex.smoothstep(0.4, 0.6, tex.fbm(X, Y, 6, 3, s()))
    albedo = tex.mix(paint, tex.colour(150, 150, 146), scratch * 0.5 * (1.0 - rust))

    # Rust in layers: the thin orange bloom, brown scale, black crust.
    bloom = tex.mix(tex.colour(190, 100, 40), tex.colour(150, 70, 28), tex.fbm(X, Y, 60, 3, s()))
    scale = tex.mix(tex.colour(112, 54, 30), tex.colour(78, 40, 28), tex.fbm(X, Y, 90, 3, s()))
    crust = tex.colour(40, 30, 26)
    layers = tex.mix(bloom, scale, tex.smoothstep(0.25, 0.6, depth))
    layers = tex.mix(layers, crust, tex.smoothstep(0.78, 0.98, depth) * 0.7)
    grain = tex.fbm(X, Y, 200, 2, s())
    layers = layers * (0.8 + 0.4 * grain)[..., None]
    albedo = tex.mix(albedo, layers, tex.smoothstep(0.08, 0.4, rust))
    # A darker line where paint meets rust: the paint's thickness.
    boundary = tex.smoothstep(0.08, 0.22, rust) * (1.0 - tex.smoothstep(0.3, 0.5, rust))
    albedo = albedo * (1.0 - 0.25 * boundary)[..., None]
    # Weld bead and rivet heads: dark steel, rusted at the foot.
    albedo = tex.mix(albedo, tex.colour(70, 66, 62) * (0.8 + 0.4 * grain)[..., None], bead * 0.5)
    albedo = tex.mix(albedo, albedo * 0.5, rivet_ring * 0.4)
    head = tex.mix(tex.colour(70, 66, 62), tex.colour(120, 76, 46), rust) * (0.55 + 0.7 * rivet_top)[..., None]
    albedo = tex.mix(albedo, head, rivets * 0.85)
    # Dirt and old grease low in the pits.
    pits = tex.cavity(0.6 + 0.3 * depth, 3)
    albedo = albedo * (1.0 - 0.3 * pits)[..., None]

    # Height: smooth paint, pitted rust, raised weld, domed rivets.
    fine = tex.fbm(X, Y, 110, 4, s())
    plate = 0.62 + 0.04 * wear
    pitted = 0.45 + 0.35 * fine - 0.1 * depth
    height = tex.mix(plate, pitted, tex.smoothstep(0.12, 0.45, rust))
    height = height + 0.18 * bead + 0.22 * rivet_top - 0.05 * rivet_ring + 0.04 * scratch
    height = tex.clamp(height - 0.1 * edge)

    rough = tex.mix(0.5 - 0.1 * wear, 0.92, tex.smoothstep(0.1, 0.45, rust))
    rough = tex.mix(rough, 0.3, scratch * (1.0 - rust))
    rough = tex.mix(rough, 0.55, rivets * 0.5)
    return tex.finish(albedo, height, rough, 8.0, seed=seed)
