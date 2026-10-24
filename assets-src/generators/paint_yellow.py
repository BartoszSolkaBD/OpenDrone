"""Yellow floor paint, worn (#324): the walkway lines of a works and the mark
on a launch pad. Thick yellow paint chipped and worn through to grey concrete
in patches, scuffed black by tyres and boots, dirty in its pits."""

import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    paint = tex.mix(tex.colour(196, 156, 40), tex.colour(170, 132, 36), tex.fbm(X, Y, 10, 3, s()))
    concrete = tex.mix(tex.colour(104, 102, 96), tex.colour(140, 138, 130), tex.fbm(X, Y, 30, 3, s()))
    # Worn through: big patches and fine chips.
    worn = tex.clamp(tex.smoothstep(0.55, 0.62, tex.fbm(X, Y, 4, 4, s()))
                     + tex.smoothstep(0.7, 0.74, tex.fbm(X, Y, 40, 3, s())))
    albedo = tex.mix(paint, concrete, worn)
    scuff = tex.smoothstep(0.62, 0.74, tex.fbm(X, Y, (3, 12), 3, s()))
    albedo = tex.mix(albedo, tex.colour(46, 42, 36), scuff * 0.5)
    grit = tex.smoothstep(0.7, 0.78, tex.fbm(X, Y, 120, 2, s()))
    albedo = tex.mix(albedo, tex.colour(60, 56, 50), grit * 0.5)
    height = tex.clamp(0.6 + 0.08 * (1.0 - worn) - grit * 0.1)
    rough = tex.clamp(0.58 + 0.25 * worn + 0.1 * scuff)
    return tex.finish(tex.clamp(albedo), height, rough, 4.0, seed=seed)
