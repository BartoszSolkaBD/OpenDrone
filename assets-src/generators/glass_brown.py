"""Brown bottle glass (#324): dark amber, glossy, with dust and dried rain
spots on it. Drawn opaque, as every surface of a Map is."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (52, 26, 8), (70, 40, 16), (96, 88, 74), 0.5, 0.08, scratch=(96, 66, 36),
                          fade=0.3, dirt=0.35, relief=3.0)
