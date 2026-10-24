"""Black plastic and rubber (#324): a bin bag's glossy skin, a tyre, the carbon
and plastic of a crashed quad. Dust greys it in patches."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (18, 18, 19), (40, 40, 40), (72, 68, 60), 0.82, 0.38,
                          scratch=(52, 52, 52), fade=0.4, dirt=0.4)
