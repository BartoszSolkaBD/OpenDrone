"""Rusted corrugated roof sheet: 1.2 m of sheet to the picture, twelve ridges
of 100 mm running up and down the picture, fixing screws in two rows with
rusty washers, and the lap with the next sheet along the bottom edge. The
galvanising has gone grey and chalky; rust spreads from the lap, the screws and
scratches, runs down the valleys, eats pinholes and a few real holes, and the
dirt of years sits in the valleys."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    ridges = 12

    wave = tex.corrugation(X, ridges)  # 1 on the crest, 0 in the valley
    # Dents: soft hollows pressed into the sheet by feet and hail.
    dents = tex.smoothstep(0.58, 0.8, tex.fbm(X, Y, (4, 4), 3, s()))

    # Fixing screws on the crests of every second ridge, in two rows.
    screws = np.zeros_like(X)
    washer = np.zeros_like(X)
    for row_y in (0.2, 0.8):
        along = X * (ridges / 2.0)
        cx = (along - np.floor(along)) - 0.5
        # The crest of every second ridge is at a fixed place; centre there.
        cy = (Y - row_y) * ridges / 2.0 * 1.0
        dist = np.sqrt(cx * cx + cy * cy)
        screws = np.maximum(screws, 1.0 - tex.smoothstep(0.07, 0.1, dist))
        washer = np.maximum(washer, 1.0 - tex.smoothstep(0.15, 0.18, dist))

    # The lap with the next sheet runs across the middle: a raised edge, and
    # the sheet under it in shadow.
    lap_d = Y - 0.5
    lap_edge = 1.0 - tex.smoothstep(0.0, 0.012, np.abs(lap_d))
    lap_shadow = (lap_d > 0.0) * (1.0 - tex.smoothstep(0.0, 0.07, lap_d))

    # Rust sources: lap, screws, and where the coating is scratched.
    scratch = tex.smoothstep(0.6, 0.7, tex.fbm(X, Y, (3, 60), 3, s())) * tex.smoothstep(0.4, 0.6, tex.fbm(X, Y, 5, 3, s()))
    failed = tex.smoothstep(0.64, 0.78, tex.fbm(X, Y, 5, 4, s()))
    source = tex.clamp(lap_edge * 1.0 + washer * 0.8 + scratch * 0.8 + failed * 0.7)
    rust, depth = tex.rust_spread(source, X, Y, s(), 0.12, 0.75)
    # Water runs down the valleys: rust gathers there more than on crests.
    valley = 1.0 - wave
    runoff = tex.streaks(source * 0.8 + rust * 0.4, 160, s(), X, Y, 44, 0.99)
    rust = tex.clamp(np.maximum(rust, runoff * valley * 0.6) + 0.15 * valley * rust)

    # Coating: dull galvanised grey, chalky, with spangle blotches.
    spangle = tex.fbm(X, Y, 20, 4, s())
    sheet = tex.mix(tex.colour(124, 128, 126), tex.colour(176, 180, 178), spangle)
    sheet = sheet * (0.92 + 0.16 * tex.fbm(X, Y, 120, 2, s()))[..., None]
    albedo = sheet * (0.85 + 0.25 * wave)[..., None]

    bloom = tex.mix(tex.colour(176, 90, 40), tex.colour(140, 66, 30), tex.fbm(X, Y, 70, 3, s()))
    scale = tex.mix(tex.colour(100, 50, 30), tex.colour(66, 36, 26), tex.fbm(X, Y, 100, 3, s()))
    layers = tex.mix(bloom, scale, tex.smoothstep(0.3, 0.7, depth))
    albedo = tex.mix(albedo, layers * (0.8 + 0.3 * wave)[..., None], tex.smoothstep(0.12, 0.5, rust))

    # Holes: where the rust has eaten right through, dark with a bright rim.
    hole_n = tex.fbm(X, Y, 18, 4, s())
    holes = tex.smoothstep(0.58, 0.62, hole_n + (depth - 0.5) * 0.4) * tex.smoothstep(0.45, 0.75, depth)
    rim = tex.clamp(tex.blur(holes, 3, 2) * 3.0 - holes * 0.6)
    albedo = tex.mix(albedo, tex.colour(24, 22, 22), holes)
    albedo = tex.mix(albedo, tex.colour(90, 44, 26), rim * 0.4)
    # Screws and washers.
    albedo = tex.mix(albedo, tex.mix(tex.colour(96, 94, 90), tex.colour(130, 70, 36), rust), washer * 0.8)
    albedo = tex.mix(albedo, tex.colour(52, 50, 48), screws * 0.8)
    # Dirt in the valleys, and long dark runs of old grime down the sheet.
    dirt = valley * tex.fbm(X, Y, 14, 4, s()) * 0.5
    dirt = tex.clamp(dirt + 0.5 * tex.streaks(tex.smoothstep(0.55, 0.8, tex.fbm(X, Y, 8, 3, s())) * 0.8, 300, s(), X, Y, 30, 0.996))
    albedo = albedo * (1.0 - 0.45 * dirt)[..., None]
    albedo = albedo * (1.0 - 0.3 * lap_shadow)[..., None]

    # Height: the corrugation (strongest), dents, pitted rust, the lap step.
    fine = tex.fbm(X, Y, 130, 4, s())
    height = 0.2 + 0.6 * wave - 0.12 * dents + 0.08 * lap_edge
    height = height - 0.12 * rust * (0.5 + fine) + 0.05 * washer + 0.07 * screws
    height = tex.clamp(height) * (1.0 - holes) + holes * 0.0

    rough = tex.mix(0.55 + 0.2 * (1.0 - spangle), 0.93, tex.smoothstep(0.1, 0.5, rust))
    rough = tex.mix(rough, 0.6, washer * 0.4)
    return tex.finish(albedo, height, rough, 14.0, seed=seed)
