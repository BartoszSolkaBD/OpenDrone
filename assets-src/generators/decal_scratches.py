"""A decal of scratches (#323): a patch of fine scrapes on a wall or a door, as
a bag, a key, a belt or a tool dragged against it leaves: many hairlines,
mostly one way with a few crossing, bright where they cut through the paint
or dirt to the lighter stuff under it, and a few deeper gouges that are dark
in the groove. Strongest in the middle of the patch. It can be turned any
way."""

import numpy as np

import decalwork
import odtex as tex


def _lines(X, Y, seed, count, width, slope, region):
    """`count` straight scratches, each a thin line of its own length and
    place: the distance to the nearest is tested pixel by pixel against each.
    `slope` is the lean of the main direction (0 across, 1 up the picture)
    and `region` where their middles may be (left, top, right, bottom)."""
    left, top, right, bottom = region
    index = np.arange(count, dtype=np.int64)
    zero = np.zeros(count, dtype=np.int64)
    cx = left + (right - left) * tex.cell_random(index, zero, seed)
    cy = top + (bottom - top) * tex.cell_random(index, zero, seed + 1)
    half = 0.03 + 0.12 * tex.cell_random(index, zero, seed + 2)
    # A direction is a random pair divided by its length, as odtex asks.
    dx = 1.0 - slope + (tex.cell_random(index, zero, seed + 3) - 0.5) * 0.5
    dy = slope + (tex.cell_random(index, zero, seed + 4) - 0.5) * 0.5
    norm = np.sqrt(dx * dx + dy * dy)
    dx, dy = dx / norm, dy / norm
    out = np.zeros_like(X)
    for k in range(count):
        px, py = X - cx[k], Y - cy[k]
        t = tex.clamp((px * dx[k] + py * dy[k]) / (2.0 * half[k]) + 0.5)
        qx = px - dx[k] * (t - 0.5) * 2.0 * half[k]
        qy = py - dy[k] * (t - 0.5) * 2.0 * half[k]
        d = np.sqrt(qx * qx + qy * qy)
        taper = 1.0 - np.abs(t - 0.5) * 2.0
        out = np.maximum(out, (1.0 - tex.smoothstep(width * 0.4, width, d)) * tex.clamp(taper * 3.0))
    return out


def make(seed, n):
    s = tex.Seeds(seed + 4700)
    X, Y = tex.grid(n)
    fine = _lines(X, Y, s(), 90, 1.0 / n * 1.5, 0.8, (0.3, 0.25, 0.7, 0.75))
    cross = _lines(X, Y, s(), 10, 1.0 / n * 1.5, -0.5, (0.3, 0.25, 0.7, 0.75))
    gouge = _lines(X, Y, s(), 3, 1.0 / n * 3.2, 0.9, (0.4, 0.35, 0.6, 0.65))
    patch = decalwork.blob(X, Y, s(), radius=0.3, softness=0.2, warp=0.3, stretch=(0.8, 1.0))
    bright = tex.clamp(fine * 0.8 + cross * 0.6) * (0.4 + 0.6 * patch)
    # A thin line is lost to a low-resolution mask, so it is drawn at full
    # strength where it is and the patch only thins how many show.
    mask = tex.clamp(np.maximum(bright, gouge * 0.9) * 1.1) * decalwork.frame(X, Y)

    light = tex.colour(188, 150, 128)
    dark = tex.colour(34, 30, 26)
    albedo = tex.mix(light, dark, tex.clamp(gouge * 1.5))
    height = tex.clamp(0.5 - gouge * 0.35 - bright * 0.08)
    rough = 0.55 + 0.2 * gouge
    return decalwork.finish(tex.clamp(albedo), height, rough, mask, 2.5, seed)
