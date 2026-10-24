"""A drinks can's painted aluminium (#324), plain colours and no brand: bare
silver metal with bands of red paint worn through, scratched bright. Also the
bright steel of a new chain and padlock."""

import numpy as np

import odtex as tex
import plainwork


def make(seed, n):
    diffuse, normal, rough = plainwork.make(seed, n, (168, 170, 172), (184, 184, 182), (80, 72, 60), 0.5, 0.2,
                                            scratch=(220, 220, 222), fade=0.2, dirt=0.3, relief=3.0)
    # Bands of red paint across the picture, worn at their edges.
    X, Y = tex.grid(n)
    s = tex.Seeds(seed + 77)
    band = tex.smoothstep(0.42, 0.5, tex.fbm(X * 0.0, Y, (1, 4), 2, s()))
    worn = tex.smoothstep(0.62, 0.7, tex.fbm(X, Y, 20, 3, s()))
    paint = (band * (1.0 - worn))[..., None]
    red = np.array([150.0, 28.0, 30.0])
    out = diffuse.astype(np.float64) * (1.0 - paint * 0.85) + red * paint * 0.85
    return np.floor(out + 0.5).astype(np.uint8), normal, rough
