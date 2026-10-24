"""A decal of rust bleeding from a steel fixing, a bolt or a seam (#323): the
iron has rusted at a point near the top of the picture and the rain has
carried the colour down, so there is a small dark crusty bloom at the source
and orange-brown streaks running from it, fanning a little, fading out at
different lengths, with a thin brighter fringe of fresh rust at their edges.
The top of the picture is up; put the source over the fixing."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4500)
    X, Y = tex.grid(n)
    flat = Y * 0.0

    # The fixing: a small ragged bloom near the top, and a short seam along it.
    bloom = decalwork.blob(X, Y, s(), centre=(0.5, 0.12), radius=0.09, softness=0.06, warp=0.3, stretch=(1.0, 0.9))
    seam = tex.smoothstep(0.012, 0.0, np.abs(Y - 0.115)) * tex.smoothstep(0.3, 0.12, np.abs(X - 0.5))
    source = np.maximum(bloom, seam * 0.8)
    # Streaks run down from it, in a cone that opens with the distance.
    fan = 1.0 + 0.8 * Y
    Xf = 0.5 + (X - 0.5) / fan
    runs = tex.streaks(source, int(n * 0.8), s(), Xf, Y, 30, 0.9965)
    thin = tex.streaks(source * 0.8, int(n * 0.45), s(), Xf, Y, 60, 0.993)
    fade = tex.smoothstep(0.98, 0.25, Y + (tex.fbm(Xf, Y, (11, 3), 3, s()) - 0.5) * 0.4)
    cone = tex.smoothstep(0.1 + 0.28 * Y + 0.06, 0.1 + 0.28 * Y - 0.08, np.abs(X - 0.5))
    streak = tex.clamp((runs + thin * 0.7) * 1.8) * fade * cone
    # The rust spreads in specks and blooms round the source.
    rust, depth = tex.rust_spread(tex.clamp(bloom * 1.2), X, Y, s(), 0.09, 0.9)
    mask = tex.clamp(np.maximum(streak * 1.1, rust * 0.9) + bloom)
    mask = mask * (0.8 + 0.2 * tex.fbm(X, Y, 50, 3, s())) * decalwork.frame(X, Y)

    fresh = tex.colour(150, 70, 28)
    old = tex.colour(84, 42, 22)
    dark = tex.colour(40, 24, 16)
    albedo = tex.mix(fresh, old, tex.smoothstep(0.1, 0.7, Y + (tex.fbm(X, Y, 12, 3, s()) - 0.5) * 0.3))
    albedo = tex.mix(albedo, dark, tex.clamp(bloom * 1.2 + depth * 0.4))
    albedo = albedo * (0.85 + 0.3 * tex.fbm(X, Y, 45, 3, s()))[..., None]

    height = tex.clamp(0.4 + depth * 0.5 + bloom * 0.3)
    rough = 0.86 - streak * 0.1
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 3.0, seed)
