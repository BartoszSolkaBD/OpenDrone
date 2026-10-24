"""Blue plastic, faded and grimy (#324): a bottle crate that has stood out in
the sun for years, chalky where it faded, scratched to a paler blue."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (34, 72, 140), (96, 128, 168), (60, 56, 48), 0.72, 0.42,
                          scratch=(120, 150, 186), fade=0.7, dirt=0.45)
