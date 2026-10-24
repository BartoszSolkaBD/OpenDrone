"""Black plastic and rubber (#324): a bin bag's glossy skin, a tyre, the carbon
and plastic of a crashed quad. Dust greys it in patches."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (18, 18, 19), (62, 62, 60), (86, 80, 70), 0.82, 0.38,
                          scratch=(60, 60, 60), fade=0.5, dirt=0.55)
