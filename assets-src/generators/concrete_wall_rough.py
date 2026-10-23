"""Rough concrete wall: 2 m of board-formed concrete to the picture, ten
boards of 20 cm with the wood's grain printed into the surface, a panel seam
down the middle, form-tie holes on a 1 m grid with rust running down from
each, air holes and hairline cracks, and the foot of the wall is dark with
damp. No spalled patch with a rebar and no stencilled block: a mark that
appears once to the picture would repeat every 2 m, so those are separate
geometry or decals (#304)."""

import numpy as np

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    boards = tex.planks(X, Y, 10, s(), 2, 0.012)
    board_id = boards["id"]
    zero = np.zeros(X.shape, dtype=np.int64)

    # Each board is poured against its own plank: its own tone and grain.
    tone = 0.94 + 0.12 * board_id
    cloud = tex.fbm(X, Y, 4, 6, s())
    base = tex.mix(tex.colour(118, 116, 112), tex.colour(164, 160, 152), cloud)
    base = base * tone[..., None]
    board_no = np.floor(Y * 10.0).astype(np.int64)
    shift_y = tex.cell_random(board_no, zero, s()) * 40.0
    grain = tex.fbm(X, Y * 0.1 + shift_y * 0.0, (3, 160), 3, s())
    grain2 = tex.fbm(X + tex.cell_random(board_no, zero, s()), Y, (5, 120), 3, s())
    grain = tex.mix(grain, grain2, 0.5)
    albedo = base * (0.9 + 0.2 * grain)[..., None]
    # Fine sand and stray darker flecks.
    flecks = tex.fbm(X, Y, 180, 2, s())
    albedo = tex.mix(albedo, tex.colour(70, 68, 66), tex.smoothstep(0.66, 0.8, flecks) * 0.3)
    albedo = tex.mix(albedo, tex.colour(206, 202, 190), tex.smoothstep(0.7, 0.84, tex.fbm(X, Y, 110, 2, s())) * 0.2)

    # The gap between boards leaks a thin fin of cement: a pale raised line.
    fin = boards["gap"]
    albedo = tex.mix(albedo, tex.colour(176, 172, 164), fin * 0.35)

    # A panel seam down the middle (and at the tile's edge), the two panels
    # at slightly different levels, darkened with dirt.
    seam_x = np.minimum(np.abs(X - 0.0), np.abs(X - 1.0))
    seam_x = np.minimum(seam_x, np.abs(X - 0.5))
    seam = 1.0 - tex.smoothstep(0.0015, 0.005, seam_x)
    seam_dirt = tex.streaks(seam, 120, s(), X, Y, 30, 0.99)
    albedo = tex.mix(albedo, albedo * 0.45, seam * 0.8)
    albedo = tex.mix(albedo, albedo * 0.6, seam_dirt * 0.5)

    # Form-tie holes: dark cones on a 1 m grid; rust runs down from each.
    # Each half-tile square holds one, at its middle.
    dx = (X * 2.0 - np.floor(X * 2.0)) - 0.5
    dy = (Y * 2.0 - np.floor(Y * 2.0)) - 0.5
    dist = np.sqrt(dx * dx + dy * dy) / 2.0  # tile units
    hole = 1.0 - tex.smoothstep(0.012, 0.0145, dist)
    rim = tex.clamp(1.0 - np.abs(dist - 0.016) / 0.005)
    rust_source = tex.clamp(hole + 0.6 * rim)
    rust_run = tex.streaks(tex.blur(rust_source, 3, 2) * 2.0, 200, s(), X, Y, 90, 0.994)
    rust_run = rust_run * tex.smoothstep(0.0, 0.05, 1.0 - hole)
    rust_col = tex.mix(tex.colour(150, 82, 40), tex.colour(112, 58, 34), tex.fbm(X, Y, 60, 3, s()))
    albedo = tex.mix(albedo, rust_col, tex.clamp(rust_run * 1.2) * 0.65)
    albedo = tex.mix(albedo, tex.colour(36, 32, 30), hole)
    albedo = tex.mix(albedo, tex.colour(120, 66, 38), rim * 0.4)

    # Air holes: pockets in the face where concrete didn't fill.
    f1, _f2, ident, _dx, _dy = tex.voronoi(X, Y, 130, s())
    zero_i = np.zeros(ident.shape, dtype=np.int64)
    has = tex.unit(ident) > 0.88
    radius = 0.06 + 0.2 * tex.unit(tex.hash_u32(ident.astype(np.int64), zero_i, s()))
    pit = tex.smoothstep(radius, radius * 0.3, f1) * has
    albedo = tex.mix(albedo, tex.colour(60, 58, 55), pit * 0.7)
    # Long runs of rain-dirt from the seams and fins, over the whole wall.
    grime = tex.streaks(tex.clamp(seam + fin * 0.5), 260, s(), X, Y, 40, 0.995)
    albedo = albedo * (1.0 - 0.3 * grime)[..., None]

    # Hairline cracks.
    crack = tex.cracks(X, Y, 3, s(), 0.0018, 0.05, 0.0) * tex.smoothstep(0.5, 0.68, tex.fbm(X, Y, (2, 2), 3, s()))
    albedo = tex.mix(albedo, tex.colour(40, 38, 36), crack * 0.8)

    # Damp patches: dark, with a greenish bloom in the wettest.
    foot = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, (3, 2), 4, s()))
    albedo = tex.mix(albedo, albedo * 0.55, foot * 0.55)
    albedo = tex.mix(albedo, tex.colour(70, 86, 56), tex.smoothstep(0.6, 0.85, foot * tex.fbm(X, Y, 20, 3, s()) * 1.6) * 0.4)
    stain = tex.stains(X, Y, s(), 0.3, 3)
    albedo = albedo * (1.0 - 0.18 * stain)[..., None]

    # Height: board grain, fins, seams, holes and pits.
    height = 0.62 + 0.14 * grain + 0.06 * tone - 0.5 * hole + 0.08 * rim + 0.12 * fin
    height = height - 0.3 * pit - 0.12 * seam - 0.25 * crack
    height = tex.clamp(height + 0.05 * flecks)

    rough = 0.88 - 0.08 * grain + 0.06 * stain
    return tex.finish(albedo, height, rough, 6.0, seed=seed)
