"""White plastic, grimy (#324): a garden chair that has been out for years,
grey in its hollows."""

import plainwork


def make(seed, n):
    return plainwork.make(seed, n, (200, 198, 188), (214, 212, 204), (96, 90, 78), 0.7, 0.45,
                          scratch=(150, 146, 138), fade=0.3, dirt=0.65)
