"""Building blocks for textures made from scratch (ADR-0024, #292).

A texture is a square picture that repeats, so every block here repeats too:
a field made with `period = 8` runs the same at the left and right edge, and
at the top and bottom, and a `blur` or a `streaks` wraps round. Fields are
float64 arrays of shape (n, n), usually between 0 and 1, and a colour is
(n, n, 3) with red, green and blue between 0 and 1 as they should look on the
screen (not linear light).

The same bytes on every computer: like `odjpeg.py`, everything here is whole
numbers or `+ - * /`, `sqrt`, `floor` and comparisons on 64-bit floats, which
every processor does exactly alike, in a fixed order. Never `sin`, `cos`,
`exp`, `**` with a fraction, `np.dot` or a random generator of numpy's: the
noise is a hash of the cell numbers and a seed (`hash_u32`), and a direction
is a random (x, y) divided by its length, not an angle.

Coordinates are `X, Y` from `grid(n)`: the pixel centres in tile units, from
0 to 1, with `Y` growing down the picture, as the picture's rows do. A
generator lays its marks out in tile units and the `size` the material
manifest gives says how many metres one tile is.
"""

import numpy as np

# ---------------------------------------------------------------- numbers

_U32 = np.uint32


def grid(n):
    """`X, Y`: the pixel centres of an n x n picture, in tile units."""
    axis = (np.arange(n, dtype=np.float64) + 0.5) / n
    return np.meshgrid(axis, axis)


def clamp(a, lo=0.0, hi=1.0):
    return np.minimum(np.maximum(a, lo), hi)


def smoothstep(edge0, edge1, x):
    """0 below `edge0`, 1 above `edge1`, a smooth curve between. `edge0` may
    be above `edge1`, which turns it round."""
    t = clamp((x - edge0) / (edge1 - edge0))
    return t * t * (3.0 - 2.0 * t)


def mix(a, b, t):
    """`a` where `t` is 0, `b` where it is 1. `t` of shape (n, n) works with
    colours of shape (n, n, 3)."""
    if np.ndim(t) == 2 and (np.ndim(a) in (1, 3) or np.ndim(b) in (1, 3)):
        t = t[..., None]
    return a + (b - a) * t


def colour(r, g, b):
    """A colour given as 0-255 numbers, as 0-1 floats."""
    return np.array([r, g, b], dtype=np.float64) / 255.0


def to_u8(a):
    """Floats from 0 to 1 as the nearest whole numbers from 0 to 255."""
    return np.floor(clamp(a) * 255.0 + 0.5).astype(np.uint8)


# ------------------------------------------------------------------ seeds


class Seeds:
    """Hands out one new number each time it is called, from a seed, in a
    fixed order, so a generator can ask for a fresh noise at every step
    without the steps' numbers meeting."""

    def __init__(self, seed):
        self.base = int(seed) & 0xFFFFFF
        self.count = 0

    def __call__(self):
        self.count += 1
        return (self.base * 7919 + self.count * 104729) & 0x7FFFFFFF


def hash_u32(a, b, seed):
    """A well-mixed unsigned 32-bit number for each pair of whole numbers
    (arrays) and a seed. Wraps round, as the processor does."""
    h = a.astype(_U32) * _U32(0x9E3779B1)
    h = h ^ (b.astype(_U32) * _U32(0x85EBCA77))
    h = h + _U32((int(seed) * 0xC2B2AE3D) & 0xFFFFFFFF)
    h = h ^ (h >> _U32(15))
    h = h * _U32(0x2C1B3C6D)
    h = h ^ (h >> _U32(12))
    h = h * _U32(0x297A2D39)
    h = h ^ (h >> _U32(15))
    return h


def unit(h):
    """A hash as a number in [0, 1), from its top 24 bits."""
    return (h >> _U32(8)).astype(np.float64) / 16777216.0


def cell_random(ix, iy, seed):
    """A number in [0, 1) for each pair of whole numbers and a seed."""
    return unit(hash_u32(ix, iy, seed))


# ------------------------------------------------------------------ noise

_GRADIENTS_X = np.array([1.0, -1.0, 0.0, 0.0, 0.7071067811865476, -0.7071067811865476, 0.7071067811865476, -0.7071067811865476])
_GRADIENTS_Y = np.array([0.0, 0.0, 1.0, -1.0, 0.7071067811865476, 0.7071067811865476, -0.7071067811865476, -0.7071067811865476])


def _periods(period):
    if isinstance(period, tuple):
        return int(period[0]), int(period[1])
    return int(period), int(period)


def perlin(X, Y, period, seed):
    """Smooth gradient noise between about -1 and 1 that repeats every
    `period` cells across the tile (a whole number, or `(across, down)`)."""
    px, py = _periods(period)
    x = X * px
    y = Y * py
    x0 = np.floor(x)
    y0 = np.floor(y)
    fx = x - x0
    fy = y - y0
    ix = x0.astype(np.int64) % px
    iy = y0.astype(np.int64) % py
    jx = (ix + 1) % px
    jy = (iy + 1) % py

    def corner(cx, cy, dx, dy):
        g = (hash_u32(cx, cy, seed) & _U32(7)).astype(np.int64)
        return _GRADIENTS_X[g] * dx + _GRADIENTS_Y[g] * dy

    n00 = corner(ix, iy, fx, fy)
    n10 = corner(jx, iy, fx - 1.0, fy)
    n01 = corner(ix, jy, fx, fy - 1.0)
    n11 = corner(jx, jy, fx - 1.0, fy - 1.0)
    u = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0)
    v = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0)
    top = n00 + u * (n10 - n00)
    bottom = n01 + u * (n11 - n01)
    return (top + v * (bottom - top)) * 1.4142135623730951


def fbm(X, Y, period, octaves, seed, gain=0.5):
    """Noise of several sizes added up ("multi-scale"): each octave has twice
    the period and `gain` times the weight of the one before. Between about 0
    and 1, centred on 0.5."""
    px, py = _periods(period)
    total = np.zeros_like(X)
    weight = 1.0
    norm = 0.0
    for octave in range(octaves):
        total = total + weight * perlin(X, Y, (px << octave, py << octave), seed + octave * 31)
        norm += weight
        weight *= gain
    return clamp(0.5 + 0.5 * total / norm * 1.15)


def wrap_distance(a, centre):
    """How far `a` (tile units, 0 to 1) is from `centre` going the short way
    round the tile, so a mark placed with it repeats like the picture."""
    d = np.abs(a - centre)
    return np.minimum(d, 1.0 - d)


def band(a, centre, half_width, softness):
    """1 within `half_width` of `centre` and falling smoothly to 0 over the
    next `softness`, on the tile's wrap-round axis `a` (0 to 1)."""
    return 1.0 - smoothstep(half_width, half_width + softness, wrap_distance(a, centre))


def contrast(a, amount, centre=0.5):
    """Pushes a field away from `centre` (amount > 1) or towards it."""
    return clamp((a - centre) * amount + centre)


def voronoi(X, Y, cells, seed, jitter=1.0):
    """Cellular noise with `cells` cells across the tile: returns
    `(f1, f2, ident, dx, dy)`: the distance to the nearest and the second
    nearest cell point in cell units, a random 32-bit number that names the
    nearest cell (the same for every pixel of it), and the vector from the
    pixel to that point in cell units."""
    x = X * cells
    y = Y * cells
    x0 = np.floor(x)
    y0 = np.floor(y)
    fx = x - x0
    fy = y - y0
    ix = x0.astype(np.int64)
    iy = y0.astype(np.int64)
    best1 = np.full(X.shape, 1.0e9)
    best2 = np.full(X.shape, 1.0e9)
    ident = np.zeros(X.shape, dtype=_U32)
    bdx = np.zeros(X.shape)
    bdy = np.zeros(X.shape)
    for oy in (-1, 0, 1):
        for ox in (-1, 0, 1):
            cx = (ix + ox) % cells
            cy = (iy + oy) % cells
            hx = cell_random(cx, cy, seed)
            hy = cell_random(cx, cy, seed + 7919)
            dx = ox + 0.5 + (hx - 0.5) * jitter - fx
            dy = oy + 0.5 + (hy - 0.5) * jitter - fy
            d = dx * dx + dy * dy
            closer = d < best1
            best2 = np.where(closer, best1, np.minimum(best2, d))
            best1 = np.where(closer, d, best1)
            ident = np.where(closer, hash_u32(cx, cy, seed + 104729), ident)
            bdx = np.where(closer, dx, bdx)
            bdy = np.where(closer, dy, bdy)
    return np.sqrt(best1), np.sqrt(best2), ident, bdx, bdy


# ----------------------------------------------------- blur and slopes


def _box_blur_axis(a, radius, axis):
    n = a.shape[axis]
    size = 2 * radius + 1
    pad = [(0, 0)] * a.ndim
    pad[axis] = (radius + 1, radius)
    padded = np.pad(a, pad, mode="wrap")
    total = np.cumsum(padded, axis=axis)
    hi = np.take(total, np.arange(size, size + n), axis=axis)
    lo = np.take(total, np.arange(0, n), axis=axis)
    return (hi - lo) / size


def blur(a, radius, passes=2):
    """A soft blur that wraps round the tile: box blurs of `radius` pixels,
    `passes` times over, across and down."""
    radius = int(radius)
    if radius < 1:
        return a
    for _ in range(passes):
        a = _box_blur_axis(a, radius, 0)
        a = _box_blur_axis(a, radius, 1)
    return a


def shift(a, dy, dx):
    """The picture moved `dy` rows down and `dx` columns right, wrapping."""
    return np.roll(np.roll(a, dy, axis=0), dx, axis=1)


def normal_map(height, relief):
    """A tangent-space normal map in the OpenGL convention the textures from
    Poly Haven use (red leans right, green leans up the picture) for a height
    field between 0 and 1 whose whole range stands `relief` pixels tall.
    Returns uint8 of shape (n, n, 3)."""
    # Height gained going one pixel right, and one row down, over two pixels.
    dx = (shift(height, 0, -1) - shift(height, 0, 1)) * 0.5 * relief
    down = (shift(height, -1, 0) - shift(height, 1, 0)) * 0.5 * relief
    # The normal leans away from rising ground: left where it rises to the
    # right, and, because "up the picture" is against the rows, towards where
    # it rises going down.
    nx = -dx
    ny = down
    length = np.sqrt(nx * nx + ny * ny + 1.0)
    out = np.stack([nx / length, ny / length, 1.0 / length], axis=-1)
    return to_u8(out * 0.5 + 0.5)


def cavity(height, radius):
    """How much lower than its surroundings each place is, between 0 (on a
    high place) and 1 (deep): where dirt, moss and water gather."""
    return clamp((blur(height, radius, 2) - height) * 4.0)


# ----------------------------------------------------------- layouts


def bricks(X, Y, courses, per_course, mortar, seed, stagger=0.5):
    """A running bond of bricks that repeats over the tile: `courses` rows
    (an even number, for the stagger to repeat) of `per_course` bricks, with a
    joint of `mortar` (as a fraction of a course's height) between them.

    Returns a dict:
    - `id`, `row`: the brick's own random number in [0, 1) and its course;
    - `u`, `v`: where the pixel is in its brick, 0 to 1 across and up;
    - `edge`: how far from the brick's edge, in courses' height units (0 at
      the edge, up to about 0.5 in the middle of a brick; negative in the
      joint, where the mortar is);
    - `joint`: 1 in the mortar, 0 in a brick, soft at the border."""
    rows = np.floor(Y * courses)
    v = Y * courses - rows
    odd = (rows.astype(np.int64) % 2).astype(np.float64)
    along = X * per_course + odd * stagger
    cols = np.floor(along)
    u = along - cols
    cx = cols.astype(np.int64) % per_course
    ident = cell_random(cx, rows.astype(np.int64), seed)
    # Distances in units of one course's height: a brick is `aspect` wide.
    aspect = (1.0 / per_course) / (1.0 / courses)
    du = np.minimum(u, 1.0 - u) * aspect
    dv = np.minimum(v, 1.0 - v)
    edge = np.minimum(du, dv) - mortar * 0.5
    joint = 1.0 - smoothstep(0.0, 0.03, edge)
    return {"id": ident, "row": rows, "u": u, "v": v, "edge": edge, "joint": joint}


def planks(X, Y, boards, seed, joints_per_board=2, gap=0.01):
    """Boards laid side by side across the tile (a board is a band of the
    picture's height), each cut into pieces by joints at its own places:
    `boards` bands, `joints_per_board` pieces in each. Returns `id`, `u`
    (along the board, 0 to 1 over a piece), `v` (across it) and `edge`, the
    distance to the nearest edge or joint in board-widths (negative in the
    gap), and `gap`: 1 in the gap between boards."""
    band = np.floor(Y * boards)
    v = Y * boards - band
    b = band.astype(np.int64)
    # Each board's joints sit at their own offset, so they don't line up.
    offset = cell_random(b, np.zeros_like(b), seed)
    along = X * joints_per_board + offset
    piece = np.floor(along)
    u = along - piece
    ident = cell_random(piece.astype(np.int64) % joints_per_board, b, seed + 1)
    aspect = (1.0 / joints_per_board) / (1.0 / boards)
    du = np.minimum(u, 1.0 - u) * aspect
    dv = np.minimum(v, 1.0 - v)
    edge = np.minimum(du, dv) - gap * 0.5
    return {"id": ident, "u": u, "v": v, "edge": edge, "gap": 1.0 - smoothstep(0.0, 0.03, edge)}


def corrugation(X, count):
    """The height of corrugated sheet, 0 to 1 (1 on the crests), with `count`
    ridges across the tile: a rounded triangle wave, which is a polynomial, so
    exact everywhere."""
    t = X * count
    f = t - np.floor(t)
    tri = np.abs(f * 2.0 - 1.0)  # 1 at the valley, 0 at the crest
    s = 1.0 - tri
    return s * s * (3.0 - 2.0 * s)


# --------------------------------------------------------- weathering


def streaks(source, length, seed, X, Y, narrowness=48, decay=0.97):
    """Streaks running down the picture from where `source` (0 to 1) is: rust
    from a bolt, dirt from a ledge, a drip from a crack. `length` is how far
    they run, in pixels (about); `narrowness` how many separate streaks fit
    across the tile. The streaks wander a little, vary in width, fade out
    unevenly and break up on the way."""
    out = source
    step = 1
    while step < length:
        out = np.maximum(out, shift(out, step, 0) * _powi(decay, step))
        step *= 2
    meander = perlin(X, Y, (3, 3), seed + 5) * 0.012
    lanes = fbm(X + meander, Y, (narrowness, 5), 3, seed)
    wide = fbm(X + meander, Y, (max(2, narrowness // 4), 3), 2, seed + 1)
    breaks = fbm(X, Y, (narrowness // 2 + 1, 24), 2, seed + 2)
    lane = smoothstep(0.38, 0.66, lanes) * (0.45 + 0.7 * wide) * (0.5 + 0.8 * breaks)
    return clamp(out * lane * 1.6)


def _powi(base, exponent):
    result = 1.0
    for _ in range(exponent):
        result *= base
    return result


def cracks(X, Y, cells, seed, width, warp=0.05, branch=0.0):
    """Thin cracks that wander over the tile: 1 on a crack, 0 away from one.
    They follow the edges of cells of a cellular noise (`cells` across), bent
    by noise, which makes them join and fork like real ones; `width` is the
    widest a crack gets, in tile units (0.0015 is a hairline at 1K, 0.004 a
    gap you could slip a coin in). Each crack pinches to nothing and opens
    again along its length, and `branch` (0 to 1) keeps more of them whole."""
    w1 = perlin(X, Y, 6, seed) * warp
    w2 = perlin(X, Y, 6, seed + 1) * warp
    f1, f2, _ident, _dx, _dy = voronoi(X + w1, Y + w2, cells, seed + 2)
    pinch = fbm(X, Y, 9, 2, seed + 3)
    # Half of f2 - f1 is how far a pixel is from the line between two cells'
    # points, in cell units; turned into tile units.
    d = (f2 - f1) * 0.5 / cells
    open_ = np.maximum(smoothstep(0.38, 0.7, pinch), branch)
    reach = width * open_
    return 1.0 - smoothstep(0.0, 1.0, d / np.maximum(reach, 1e-9))


def stains(X, Y, seed, coverage, scale=4, softness=0.12):
    """Blotches of wet-looking staining: 1 inside, 0 outside, covering about
    `coverage` (0 to 1) of the tile with soft edges."""
    n = fbm(X, Y, scale, 4, seed)
    return smoothstep(1.0 - coverage - softness, 1.0 - coverage + softness, n)


def edge_wear(edge, seed, X, Y, width, roughness=0.5):
    """Chipped wear along edges: `edge` is the distance from an edge (0 on
    it, as `bricks` and `planks` return); gives 1 where the edge is chipped
    or rubbed away and 0 farther in, ragged because noise decides how far in
    the wear reaches (`width`, in the same units as `edge`)."""
    ragged = fbm(X, Y, 48, 3, seed)
    reach = width * (0.25 + 1.5 * ragged * roughness + 0.5 * (1.0 - roughness))
    return 1.0 - smoothstep(0.0, 1.0, edge / np.maximum(reach, 1e-6))


def rust_spread(source, X, Y, seed, reach, amount):
    """Rust that creeps out from the edges and seams in `source` (1 on one, 0
    away from it), as far as `reach` in tile units, farther where noise lets
    it, and starting as a scatter of specks in front of the solid patches the
    way it does under paint. Returns `(rust, depth)`: how rusty, 0 to 1, and a
    0 to 1 field that is deepest where it is thickest, for pitting."""
    spread = clamp(blur(source, max(2, int(reach * X.shape[0])), 3) * 3.0)
    far = clamp(blur(source, max(3, int(reach * 2.4 * X.shape[0])), 3) * 3.0)
    n = fbm(X, Y, 8, 5, seed)
    patchy = spread * 0.75 + n * 0.65 - (1.0 - amount) * 0.75
    rust = smoothstep(0.35, 0.5, patchy)
    # Specks: pinpricks and little blooms that come before the patches.
    speck_n = fbm(X, Y, 90, 3, seed + 50)
    specks = smoothstep(0.58, 0.68, speck_n + far * 0.35 - 0.22) * far
    blooms = smoothstep(0.55, 0.7, fbm(X, Y, 30, 3, seed + 51) + far * 0.3 - 0.2) * far * 0.8
    rust = np.maximum(rust, np.maximum(specks, blooms * 0.7))
    fine = fbm(X, Y, 40, 3, seed + 99)
    depth = clamp(rust * (0.4 + fine) * (0.6 + 0.6 * patchy))
    return clamp(rust), depth


def peel(X, Y, seed, amount):
    """Flakes of paint that have come away: returns `(gone, lip)`: 1 where
    the paint is off, 0 where it is on, and a thin 1 line just inside the
    paint along the break, which is the paint's thickness catching the light.
    `amount` is the share of the paint that has come away, 0 to 1."""
    n = fbm(X + perlin(X, Y, 5, seed) * 0.04, Y + perlin(X, Y, 5, seed + 1) * 0.04, 6, 5, seed + 2)
    cut = 1.0 - amount
    gone = smoothstep(cut - 0.005, cut + 0.005, n)
    lip = smoothstep(cut - 0.045, cut - 0.005, n) * (1.0 - gone)
    return gone, lip


def stones(X, Y, cells, seed, fill=0.7, angular=0.6):
    """Loose stones, chunks and pebbles on the ground: cellular noise whose
    cells hold a stone each (about `fill` of them have one), shrunk away from
    the cell's edges by their own random amount, so they have all sizes and
    are many-sided, with rounded tops. `angular` (0 to 1) is how sharp the
    corners are; 0 gives round pebbles. Returns `(height, ident, mask, edge)`:
    a dome-shaped height 0 to 1 on each stone, the stone's own 32-bit number,
    1 on the stone, and how far inside it each pixel is (0 at its rim)."""
    f1, f2, ident, dx, dy = voronoi(X, Y, cells, seed, jitter=0.9)
    r = unit(ident)
    z = np.zeros(ident.shape, dtype=np.int64)
    has = unit(hash_u32(ident.astype(np.int64), z, seed + 5)) < fill
    size = unit(hash_u32(ident.astype(np.int64), z, seed + 6))
    # How far the stone's rim stands from the cell's edge, in cell units.
    gap = 0.06 + 0.3 * size * size * size
    polygon = (f2 - f1) * 0.5 - gap
    disc = 0.5 - f1 - gap * 0.4
    inside = polygon * angular + disc * (1.0 - angular)
    inside = np.where(has, inside, -1.0)
    mask = smoothstep(0.0, 0.04, inside)
    top = clamp(inside / 0.22)
    height = np.sqrt(top) * (0.4 + 0.6 * r)
    return height * mask, ident, mask, np.maximum(inside, 0.0)


def blades(n, count, seed, length, width, wind=0.0):
    """Blades of grass or weeds seen from above, scattered over the tile and
    wrapping at its edges: each is a curved stroke from a root outward that
    thins to a tip. Returns `(height, tone, mask)` arrays of shape (n, n): how
    high the topmost blade at each pixel stands (0 to 1), a 0 to 1 number for
    each blade's colour, and 1 where a blade is. `length` and `width` are in
    pixels."""
    index = np.arange(count, dtype=np.int64)
    zero = np.zeros(count, dtype=np.int64)
    root_x = cell_random(index, zero, seed) * n
    root_y = cell_random(index, zero, seed + 1) * n
    dx = cell_random(index, zero, seed + 2) * 2.0 - 1.0 + wind
    dy = cell_random(index, zero, seed + 3) * 2.0 - 1.0
    norm = np.sqrt(dx * dx + dy * dy) + 1e-9
    dx = dx / norm
    dy = dy / norm
    bend = (cell_random(index, zero, seed + 4) - 0.5) * 1.4
    size = length * (0.55 + 0.9 * cell_random(index, zero, seed + 5))
    tone = cell_random(index, zero, seed + 6)
    stand = cell_random(index, zero, seed + 7)
    samples = max(6, int(length * 1.6 / max(width, 1.0)))
    px, py, ph, pt, pw = [], [], [], [], []
    for k in range(samples):
        t = (k + 0.5) / samples
        x = root_x + dx * size * t - dy * bend * size * t * t
        y = root_y + dy * size * t + dx * bend * size * t * t
        px.append(x)
        py.append(y)
        # Higher towards the tip, and thinner there.
        ph.append(stand * 0.5 + 0.5 * t)
        pt.append(tone)
        pw.append(np.full(count, 1.0 - 0.7 * t))
    px = np.concatenate(px)
    py = np.concatenate(py)
    ph = np.concatenate(ph)
    pt = np.concatenate(pt)
    pw = np.concatenate(pw)
    reach = max(1, int(width))
    height = np.zeros((n, n))
    toneimg = np.zeros((n, n))
    # Draw from the lowest up, so that the highest sample is the last write.
    order = np.argsort(ph, kind="stable")
    px, py, ph, pt, pw = px[order], py[order], ph[order], pt[order], pw[order]
    ix = np.floor(px).astype(np.int64)
    iy = np.floor(py).astype(np.int64)
    for oy in range(-reach + 1, reach + 1):
        for ox in range(-reach + 1, reach + 1):
            falloff = 1.0 - (ox * ox + oy * oy) / (float(reach * reach) * pw * pw + 1.0)
            keep_it = falloff > 0.0
            xs = (ix + ox) % n
            ys = (iy + oy) % n
            keep = ph * falloff
            win = keep_it & (keep >= height[ys, xs])
            height[ys[win], xs[win]] = keep[win]
            toneimg[ys[win], xs[win]] = pt[win]
    mask = (height > 0.0).astype(np.float64)
    return height, toneimg, mask


def scribbles(X, Y, seed, strokes, thickness, region=(0.0, 0.0, 1.0, 1.0)):
    """Generic graffiti: loose looping strokes, as a tag or a scrawl, made of
    nothing a person drew. 1 on the paint, 0 off it, inside the rectangle
    `region` (left, top, right, bottom in tile units). `thickness` is in
    tile units."""
    n = X.shape[0]
    left, top, right, bottom = region
    out = np.zeros_like(X)
    per = 90
    index = np.arange(per, dtype=np.int64)
    zero = np.zeros(per, dtype=np.int64)
    for s in range(strokes):
        base = seed + s * 977
        x0 = left + (right - left) * cell_random(np.array([0]), np.array([0]), base)[0]
        y0 = top + (bottom - top) * cell_random(np.array([0]), np.array([1]), base)[0]
        # A random walk whose direction turns a little at each step.
        hx = cell_random(index, zero, base + 1)
        hy = cell_random(index, zero, base + 2)
        px = np.empty(per)
        py = np.empty(per)
        dirx, diry = 1.0, 0.0
        x, y = x0, y0
        step = (right - left) * 0.016
        for i in range(per):
            turn = (hx[i] - 0.5) * 1.1
            dirx, diry = dirx - diry * turn, diry + dirx * turn
            length = np.sqrt(dirx * dirx + diry * diry)
            dirx, diry = dirx / length, diry / length
            x += dirx * step
            y += diry * step * 1.3
            # Keep it inside the region by turning back.
            if x < left or x > right:
                dirx = -dirx
                x = min(max(x, left), right)
            if y < top or y > bottom:
                diry = -diry
                y = min(max(y, top), bottom)
            px[i] = x
            py[i] = y
        # The distance of each pixel to the walk's segments.
        best = np.full(X.shape, 1.0e9)
        for i in range(per - 1):
            ax, ay, bx, by = px[i], py[i], px[i + 1], py[i + 1]
            ex, ey = bx - ax, by - ay
            ll = ex * ex + ey * ey + 1e-12
            t = clamp(((X - ax) * ex + (Y - ay) * ey) / ll)
            qx = X - (ax + ex * t)
            qy = Y - (ay + ey * t)
            best = np.minimum(best, qx * qx + qy * qy)
        out = np.maximum(out, 1.0 - smoothstep(thickness * 0.6, thickness, np.sqrt(best)))
    return out


def sign_marks(X, Y, seed, region, kind="stripes"):
    """Faded signs and markings that belong to no real brand: `stripes` are
    hazard chevrons over a bar, `bar` a plain painted band with a block of
    blank "letters", `arrow` an arrow. 1 on the paint, 0 off it, inside the
    rectangle `region` (left, top, right, bottom in tile units)."""
    left, top, right, bottom = region
    inside = (X >= left) & (X < right) & (Y >= top) & (Y < bottom)
    u = (X - left) / (right - left)
    v = (Y - top) / (bottom - top)
    if kind == "stripes":
        # Slanting bars as wide as the band is tall, at 45 degrees on the
        # wall, whatever shape the rectangle has.
        slant = (u * (right - left) + v * (bottom - top)) / (bottom - top) * 2.0
        paint = ((slant - np.floor(slant)) < 0.5).astype(np.float64)
    elif kind == "arrow":
        shaft = (np.abs(v - 0.5) < 0.12) & (u < 0.6)
        head = (np.abs(v - 0.5) < (1.0 - u) * 0.9) & (u >= 0.55)
        paint = (shaft | head).astype(np.float64)
    else:
        # Blocks of "letters": solid bars of different widths, with gaps.
        cols = 14
        c = np.floor(u * cols).astype(np.int64)
        on = cell_random(c, np.zeros_like(c), seed) > 0.28
        within = u * cols - np.floor(u * cols)
        paint = ((v > 0.2) & (v < 0.8) & on & (within > 0.12) & (within < 0.88)).astype(np.float64)
    return paint * inside


# ---------------------------------------------------------- finishing


def finish(albedo, height, roughness, relief, grain=0.012, seed=1):
    """The three pictures of a material, ready to write: `albedo` (n, n, 3)
    0 to 1, `height` 0 to 1 that stands `relief` pixels tall, `roughness`
    0 to 1. A little fine grain is added to the colour so that large flat
    areas don't show bands. Returns `(diffuse, normal, roughness)` as uint8."""
    n = albedo.shape[0]
    ix, iy = np.meshgrid(np.arange(n, dtype=np.int64), np.arange(n, dtype=np.int64))
    dither = cell_random(ix, iy, seed + 4242) - 0.5
    diffuse = to_u8(albedo + dither[..., None] * grain)
    return diffuse, normal_map(height, relief), to_u8(roughness)
