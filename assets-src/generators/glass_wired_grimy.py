"""Wired glass in an old factory window, a metre to the picture, after decades
without a wash: a grey-green pane with a square wire mesh cast into it every
25 mm, filmed over with grime that is thickest in blotches and runs down in
streaks, with a bloom of lichen-green here and there. The game draws glass as
an opaque surface, and glass this dirty reads as one."""

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)

    # The glass itself: a dark grey-green that changes a little across it.
    tone = tex.fbm(X, Y, 3, 4, s())
    albedo = tex.mix(tex.colour(58, 70, 66), tex.colour(84, 96, 90), tone)

    # The wire mesh: 40 wires each way across the metre.
    def wire(a):
        f = a * 40.0
        return 1.0 - tex.smoothstep(0.03, 0.09, tex.wrap_distance(f - tex.np.floor(f), 0.0))

    mesh = tex.clamp(wire(X) + wire(Y))
    albedo = tex.mix(albedo, tex.colour(46, 44, 40), mesh * 0.7)

    # Grime: blotches, and streaks running down from them.
    blotch = tex.smoothstep(0.4, 0.75, tex.fbm(X, Y, 4, 5, s()))
    run = tex.streaks(blotch * 0.8, 160, s(), X, Y, 36, 0.985)
    grime = tex.clamp(blotch * 0.7 + run * 0.6 + tex.fbm(X, Y, 24, 3, s()) * 0.25)
    dirt = tex.mix(tex.colour(96, 92, 78), tex.colour(124, 116, 94), tex.fbm(X, Y, 50, 3, s()))
    albedo = tex.mix(albedo, dirt, grime * 0.75)

    # Lichen in the dampest places.
    lichen = tex.smoothstep(0.66, 0.8, tex.fbm(X, Y, (5, 5), 4, s())) * blotch
    albedo = tex.mix(albedo, tex.colour(104, 120, 70), lichen * 0.8)

    height = tex.clamp(0.5 + grime * 0.3 + lichen * 0.3 - mesh * 0.3 + (tone - 0.5) * 0.2)
    rough = tex.clamp(0.18 + grime * 0.6 + lichen * 0.3)
    return tex.finish(albedo, height, rough, 3.0, seed=seed)
