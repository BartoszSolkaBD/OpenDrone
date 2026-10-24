"""Red plastic, faded and grimy (#324): a bottle crate gone pink and chalky in
the sun, scratched paler; also a ratchet strap's webbing."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (150, 32, 28), (176, 104, 96), (62, 54, 46), 0.72, 0.42,
                          scratch=(196, 130, 120), fade=0.7, dirt=0.45)
