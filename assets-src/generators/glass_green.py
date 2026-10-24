"""Green bottle glass (#324): dark bottle green, glossy, with dust and dried
rain spots on it. Drawn opaque, as every surface of a Map is."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (18, 58, 22), (34, 78, 36), (96, 92, 74), 0.5, 0.08,
                          fade=0.3, dirt=0.35, relief=1.0)
