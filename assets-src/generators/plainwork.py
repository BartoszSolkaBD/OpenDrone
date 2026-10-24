"""What the plain-coloured materials of a Map's props share (#324): a surface
of one colour (a plastic crate, a bin bag, a bottle's glass, a can's paint)
that has been out in the weather. It is the colour, faded in soft patches by
the sun, with grime settled in blotches and specks, fine scratches that show a
lighter or darker colour under it, and a roughness that is glossier where it
is clean.

A prop is small, so its picture is seen across a few centimetres to a metre:
the marks are fine, and there is no one-off mark that would show again with
every repeat. The blocks are odtex's, so the rules at the top of `odtex.py`
hold: whole numbers and `+ - * /`, `sqrt` and `floor`, the same bytes
everywhere.
"""

import odtex as tex


def make(seed, n, base, faded, grime, rough, gloss, scratch=None, relief=3.0, fade=0.5, dirt=0.5):
    """The three pictures of a weathered plain surface.

    - `base`, `faded`, `grime`: 0-255 colours: the surface's own colour, what
      the sun has faded it to, and the dirt that settles on it.
    - `rough`, `gloss`: its roughness where it is dirty, and where it is
      clean.
    - `scratch`: the colour that shows in a scratch (None for no scratches).
    - `fade` and `dirt` (0 to 1): how much of each.
    """
    s = tex.Seeds(seed)
    X, Y = tex.grid(n)
    base, faded, grime = tex.colour(*base), tex.colour(*faded), tex.colour(*grime)

    # Sun-faded patches, large and soft, and a fine mottle in the material.
    sun = tex.smoothstep(0.35, 0.75, tex.fbm(X, Y, 3, 4, s())) * fade
    mottle = tex.fbm(X, Y, 24, 3, s())
    albedo = tex.mix(base, faded, sun)
    albedo = albedo * (0.94 + 0.12 * mottle)[..., None]

    # Grime: blotches, and specks of grit.
    blotch = tex.smoothstep(0.55, 0.8, tex.fbm(X, Y, 6, 4, s())) * dirt
    specks = tex.smoothstep(0.72, 0.8, tex.fbm(X, Y, 90, 2, s())) * dirt
    albedo = tex.mix(albedo, grime, tex.clamp(blotch * 0.7 + specks * 0.5))

    height = 0.6 + 0.08 * mottle - 0.05 * blotch
    roughness = tex.mix(gloss, rough, tex.clamp(blotch + specks * 0.5 + sun * 0.4))
    if scratch is not None:
        lines = tex.cracks(X, Y, 12, s(), 0.0016, 0.02, 0.1) * tex.smoothstep(0.4, 0.6, tex.fbm(X, Y, 4, 3, s()))
        albedo = tex.mix(albedo, tex.colour(*scratch), tex.clamp(lines * 1.4) * 0.7)
        height = height - lines * 0.2
        roughness = tex.mix(roughness, rough, lines * 0.5)
    return tex.finish(tex.clamp(albedo), tex.clamp(height), tex.clamp(roughness), relief, seed=seed)
