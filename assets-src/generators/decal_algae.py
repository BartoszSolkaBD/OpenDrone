"""A decal of algae and a wet film on a floor or the foot of a wall (#323): a
thin, slimy skin of dark green and brown in the lowest, dampest places,
broken into islands and tongues and shot through with bare wet patches, and
darker and glossier than the surface it lies on. The colour is faint, since
the film is thin; the wet is the main thing, so it asks the library for a
high `wetness`. It has no up: it can be turned any way."""

import numpy as np

import decalwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 4400)
    X, Y = tex.grid(n)

    # A wide patch whose edge wanders, with islands round it and holes in it.
    body = decalwork.blob(X, Y, s(), radius=0.34, softness=0.1, warp=0.3, stretch=(1.2, 0.9))
    islands = tex.smoothstep(0.62, 0.74, tex.fbm(X, Y, 8, 4, s())) * tex.smoothstep(0.5, 0.3, body)
    cover = tex.clamp(body + islands * 0.8)
    holes = tex.smoothstep(0.64, 0.76, tex.fbm(X, Y, 10, 4, s()))
    cover = cover * (1.0 - holes * 0.7)
    # Slime runs a little downhill, in short soft streaks.
    run = tex.streaks(tex.smoothstep(0.4, 0.8, cover) * (tex.fbm(X, Y, 30, 2, s()) > 0.62), int(n * 0.2), s(), X, Y, 28, 0.98)
    cover = tex.clamp(cover + run * 0.4 * (1.0 - cover))
    fine = tex.fbm(X, Y, 90, 3, s())
    mask = tex.clamp(cover * (0.7 + 0.4 * fine))
    mask = tex.smoothstep(0.05, 0.6, mask) * decalwork.frame(X, Y)

    green = tex.colour(34, 52, 24)
    brown = tex.colour(58, 52, 30)
    black = tex.colour(18, 24, 16)
    tone = tex.fbm(X, Y, 6, 4, s())
    albedo = tex.mix(green, brown, tex.smoothstep(0.45, 0.7, tone))
    albedo = tex.mix(albedo, black, tex.smoothstep(0.4, 0.8, cover) * 0.5)
    albedo = albedo * (0.8 + 0.4 * fine)[..., None]

    height = tex.clamp(0.4 + fine * 0.2 - holes * 0.2)
    rough = 0.45 + 0.2 * fine
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 2.0, seed)
