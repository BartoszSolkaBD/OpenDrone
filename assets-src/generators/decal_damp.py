"""A decal of a damp patch (#321): where water stands or seeps on a floor, an
irregular patch with a soft, wandering edge and a few drops of damp apart from
it. Its material shows none of its own colour (`cover = 0`): it makes the floor
under it wet, darker and glossier, so the floor's own picture shows through.
The colour and relief are only what a dirty puddle's film would add, if a
material uses them."""

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 3500)
    X, Y = tex.grid(n)
    main = decalwork.blob(X, Y, s(), radius=0.33, softness=0.1, warp=0.3, stretch=(1.15, 0.9))
    # Drops and smaller patches round the edge.
    small = decalwork.blob(X, Y, s(), centre=(0.72, 0.7), radius=0.1, softness=0.05, warp=0.15)
    small2 = decalwork.blob(X, Y, s(), centre=(0.26, 0.3), radius=0.07, softness=0.04, warp=0.12)
    mask = tex.clamp(main + small * 0.8 + small2 * 0.7)
    # Wetter in the middle, uneven inside.
    mask = tex.clamp(mask * (0.8 + 0.4 * tex.fbm(X, Y, 8, 4, s()))) * decalwork.frame(X, Y)

    grime = tex.fbm(X, Y, 20, 4, s())
    albedo = tex.mix(tex.colour(54, 50, 44), tex.colour(78, 72, 62), grime)
    height = tex.clamp(0.5 + (tex.fbm(X, Y, 24, 3, s()) - 0.5) * 0.2)
    rough = 0.3 + grime * 0.1
    return decalwork.finish(albedo, height, rough, mask, 0.6, seed)
