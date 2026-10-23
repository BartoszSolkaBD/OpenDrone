"""A JPEG writer that gives the same bytes on every computer (ADR-0024, #292).

The textures made from scratch are committed as JPG, and a check regenerates
them and demands the very same bytes, on a Mac and on CI's Linux. Blender's
own image saving goes through colour management and the processor's vector
units, which this repo can't vouch for, so the file is written here instead:

- every step is whole-number maths or the four basic operations on 64-bit
  floats (which every processor does exactly the same), in a fixed order:
  never `sin`, `exp`, a matrix product or anything else a maths library or a
  processor's vector unit may round differently;
- the DCT constants are written out below, not worked out with `cos` here;
- the Huffman tables are the standard ones of the JPEG standard (Annex K),
  so no table is built from counts.

It writes a baseline JPEG: greyscale, or colour with the chroma at full size
or halved in both directions. Any decoder reads it. Needs numpy, which
Blender's own Python has.
"""

import numpy as np

_LUMA_Q = (
     16,  11,  10,  16,  24,  40,  51,  61,
     12,  12,  14,  19,  26,  58,  60,  55,
     14,  13,  16,  24,  40,  57,  69,  56,
     14,  17,  22,  29,  51,  87,  80,  62,
     18,  22,  37,  56,  68, 109, 103,  77,
     24,  35,  55,  64,  81, 104, 113,  92,
     49,  64,  78,  87, 103, 121, 120, 101,
     72,  92,  95,  98, 112, 100, 103,  99,
)

_CHROMA_Q = (
     17,  18,  24,  47,  99,  99,  99,  99,
     18,  21,  26,  66,  99,  99,  99,  99,
     24,  26,  56,  99,  99,  99,  99,  99,
     47,  66,  99,  99,  99,  99,  99,  99,
     99,  99,  99,  99,  99,  99,  99,  99,
     99,  99,  99,  99,  99,  99,  99,  99,
     99,  99,  99,  99,  99,  99,  99,  99,
     99,  99,  99,  99,  99,  99,  99,  99,
)

_DC_LUMA_BITS = (
    0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
)

_DC_LUMA_VALS = (
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
)

_DC_CHROMA_BITS = (
    0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
)

_DC_CHROMA_VALS = (
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
)

_AC_LUMA_BITS = (
    0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7d,
)

_AC_LUMA_VALS = (
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
)

_AC_CHROMA_BITS = (
    0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77,
)

_AC_CHROMA_VALS = (
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
)

_ZIGZAG = (
      0,   1,   8,  16,   9,   2,   3,  10,
     17,  24,  32,  25,  18,  11,   4,   5,
     12,  19,  26,  33,  40,  48,  41,  34,
     27,  20,  13,   6,   7,  14,  21,  28,
     35,  42,  49,  56,  57,  50,  43,  36,
     29,  22,  15,  23,  30,  37,  44,  51,
     58,  59,  52,  45,  38,  31,  39,  46,
     53,  60,  61,  54,  47,  55,  62,  63,
)

# The 8-point DCT's matrix: row u, column n is c(u) * cos((2n + 1) u pi / 16),
# with c(0) = sqrt(1/8) and c(u) = 1/2. Written out as numbers (which Python
# reads the same everywhere), not worked out with `cos`.
_DCT = (
    (0.3535533905932738, 0.3535533905932738, 0.3535533905932738, 0.3535533905932738, 0.3535533905932738, 0.3535533905932738, 0.3535533905932738, 0.3535533905932738),
    (0.4903926402016152, 0.4157348061512726, 0.27778511650980114, 0.09754516100806417, -0.0975451610080641, -0.277785116509801, -0.4157348061512727, -0.4903926402016152),
    (0.46193976625564337, 0.19134171618254492, -0.19134171618254486, -0.46193976625564337, -0.4619397662556434, -0.19134171618254517, 0.191341716182545, 0.46193976625564326),
    (0.4157348061512726, -0.0975451610080641, -0.4903926402016152, -0.2777851165098011, 0.2777851165098009, 0.4903926402016152, 0.09754516100806439, -0.41573480615127256),
    (0.3535533905932738, -0.35355339059327373, -0.35355339059327384, 0.3535533905932737, 0.35355339059327384, -0.35355339059327334, -0.3535533905932736, 0.3535533905932733),
    (0.27778511650980114, -0.4903926402016152, 0.09754516100806415, 0.41573480615127273, -0.41573480615127256, -0.09754516100806401, 0.4903926402016153, -0.27778511650980076),
    (0.19134171618254492, -0.4619397662556434, 0.46193976625564326, -0.19134171618254495, -0.19134171618254528, 0.46193976625564337, -0.4619397662556432, 0.19134171618254478),
    (0.09754516100806417, -0.2777851165098011, 0.41573480615127273, -0.4903926402016153, 0.4903926402016152, -0.4157348061512725, 0.27778511650980076, -0.09754516100806429),
)


_ZIGZAG_ARRAY = np.array(_ZIGZAG, dtype=np.int64)


def _huffman_codes(bits, values):
    """The standard's way to turn a table's code lengths and values into
    codes: returns (code, length) arrays indexed by symbol value."""
    code = np.zeros(256, dtype=np.int64)
    length = np.zeros(256, dtype=np.int64)
    c = 0
    k = 0
    for size in range(1, 17):
        for _ in range(bits[size - 1]):
            code[values[k]] = c
            length[values[k]] = size
            c += 1
            k += 1
        c <<= 1
    return code, length


_TABLES = {
    # is the component a chroma one -> (DC codes, DC lengths, AC codes, AC lengths)
    False: _huffman_codes(_DC_LUMA_BITS, _DC_LUMA_VALS) + _huffman_codes(_AC_LUMA_BITS, _AC_LUMA_VALS),
    True: _huffman_codes(_DC_CHROMA_BITS, _DC_CHROMA_VALS) + _huffman_codes(_AC_CHROMA_BITS, _AC_CHROMA_VALS),
}


def _quant_table(base, quality):
    """libjpeg's way to scale a table for a quality from 1 to 100, in the
    order the table is written (row by row)."""
    scale = 5000 // quality if quality < 50 else 200 - quality * 2
    return [min(255, max(1, (b * scale + 50) // 100)) for b in base]


def _dct_blocks(plane):
    """The 8x8 DCT of every block of a plane (height and width multiples of
    8) as float64: shape (rows of blocks, columns of blocks, 8, 8), the
    vertical frequency first."""
    h, w = plane.shape
    x = plane.astype(np.float64).reshape(h // 8, 8, w // 8, 8).transpose(0, 2, 1, 3) - 128.0
    # Along the last axis: eight terms added in a fixed order.
    t = np.empty_like(x)
    for u in range(8):
        acc = x[..., 0] * _DCT[u][0]
        for n in range(1, 8):
            acc = acc + x[..., n] * _DCT[u][n]
        t[..., u] = acc
    # Along the one before it.
    y = np.empty_like(t)
    for v in range(8):
        acc = t[..., 0, :] * _DCT[v][0]
        for n in range(1, 8):
            acc = acc + t[..., n, :] * _DCT[v][n]
        y[..., v, :] = acc
    return y


def _bit_length(magnitude):
    """How many bits a whole number needs (0 for 0), for an array."""
    n = np.zeros(magnitude.shape, dtype=np.int64)
    for k in range(12):
        n += magnitude >= (1 << k)
    return n


def _component_items(plane, qbase, quality, chroma, blocks_per_mcu, slot):
    """The Huffman-coded pieces of one component, each with the key that puts
    it in the scan's order: arrays (key, code, length)."""
    q = np.array(_quant_table(qbase, quality), dtype=np.float64).reshape(8, 8)
    quant = np.rint(_dct_blocks(plane) / q).astype(np.int64)
    rows, cols = quant.shape[:2]
    # The scan visits the blocks MCU by MCU: a full-size component has one
    # block in each, luma under halved chroma a 2x2 square.
    if blocks_per_mcu == 4:
        quant = quant.reshape(rows // 2, 2, cols // 2, 2, 8, 8).transpose(0, 2, 1, 3, 4, 5)
    flat = quant.reshape(-1, 64)[:, _ZIGZAG_ARRAY]
    count = flat.shape[0]
    order = np.arange(count, dtype=np.int64)
    scan = (order // blocks_per_mcu) * 8 + slot + order % blocks_per_mcu
    dc_code, dc_len, ac_code, ac_len = _TABLES[chroma]

    # DC: the difference to the block before it in this component.
    dc = flat[:, 0]
    diff = np.empty_like(dc)
    diff[0] = dc[0]
    diff[1:] = dc[1:] - dc[:-1]
    cat = _bit_length(np.abs(diff))
    value = np.where(diff < 0, diff + (1 << cat) - 1, diff)
    keys = [scan * 1024]
    codes = [(dc_code[cat] << cat) | value]
    lens = [dc_len[cat] + cat]

    # AC: each non-zero one, after the run of zeros before it.
    block, pos = np.nonzero(flat[:, 1:])
    pos = pos + 1
    coef = flat[block, pos]
    first = np.ones(block.shape, dtype=bool)
    first[1:] = block[1:] != block[:-1]
    previous = np.zeros(block.shape, dtype=np.int64)
    previous[1:] = pos[:-1]
    previous[first] = 0
    run = pos - previous - 1
    zrl = run // 16
    run = run % 16
    cat = _bit_length(np.abs(coef))
    value = np.where(coef < 0, coef - 1 + (1 << cat), coef)
    symbol = (run << 4) | cat
    keys.append(scan[block] * 1024 + pos * 8 + 1)
    codes.append((ac_code[symbol] << cat) | value)
    lens.append(ac_len[symbol] + cat)
    # A run of 16 zeros or more is written as 0xF0 first, once for each 16.
    many = np.repeat(np.arange(block.shape[0]), zrl)
    keys.append(scan[block[many]] * 1024 + pos[many] * 8)
    codes.append(np.full(many.shape, ac_code[0xF0], dtype=np.int64))
    lens.append(np.full(many.shape, ac_len[0xF0], dtype=np.int64))
    # A block whose last coefficient is zero ends with the end-of-block code.
    last = np.zeros(count, dtype=np.int64)
    np.maximum.at(last, block, pos)
    ends = np.flatnonzero(last < 63)
    keys.append(scan[ends] * 1024 + 64 * 8)
    codes.append(np.full(ends.shape, ac_code[0x00], dtype=np.int64))
    lens.append(np.full(ends.shape, ac_len[0x00], dtype=np.int64))
    return np.concatenate(keys), np.concatenate(codes), np.concatenate(lens)


def _pack(codes, lens):
    """The pieces written one after the other as bits: bytes, the last one
    padded with 1s, and a 0 after every 0xFF as the standard asks."""
    ends = np.cumsum(lens)
    total = int(ends[-1])
    starts = ends - lens
    padded = (total + 7) // 8 * 8
    bits = np.zeros(padded, dtype=np.uint8)
    bits[total:] = 1
    for k in range(int(lens.max())):
        take = np.flatnonzero(lens > k)
        bits[starts[take] + k] = (codes[take] >> (lens[take] - 1 - k)) & 1
    data = np.packbits(bits)
    stuff = np.flatnonzero(data == 0xFF)
    if stuff.size:
        data = np.insert(data, stuff + 1, 0)
    return data.tobytes()


def _segment(marker, body):
    return bytes([0xFF, marker]) + (len(body) + 2).to_bytes(2, "big") + bytes(body)


def _table_segment(kind, ident, bits, values):
    return _segment(0xC4, bytes([(kind << 4) | ident]) + bytes(bits) + bytes(values))


def encode(picture, quality=85, halve_colour=False):
    """JPEG bytes of a picture: a `(height, width)` uint8 array for a grey
    one, `(height, width, 3)` for a colour one. The height and width must be
    multiples of 16. `halve_colour` keeps the colour at half the size, in
    both directions, which a colour picture hardly shows and which saves a
    third of the file; never use it for a normal map."""
    picture = np.asarray(picture)
    if picture.dtype != np.uint8:
        raise ValueError("the picture must be uint8")
    h, w = picture.shape[:2]
    if h % 16 or w % 16:
        raise ValueError("the picture's height and width must be multiples of 16")
    if not 1 <= quality <= 100:
        raise ValueError("the quality must be from 1 to 100")
    grey = picture.ndim == 2
    out = [b"\xff\xd8", _segment(0xE0, b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00")]
    out.append(_segment(0xDB, bytes([0]) + bytes(_quant_table(_LUMA_Q, quality)[i] for i in _ZIGZAG)))
    if grey:
        components = [(1, 0x11, 0)]
        planes = [picture.astype(np.int64)]
        per_mcu = [1]
        kinds = [(_LUMA_Q, False)]
    else:
        out.append(_segment(0xDB, bytes([1]) + bytes(_quant_table(_CHROMA_Q, quality)[i] for i in _ZIGZAG)))
        r = picture[..., 0].astype(np.int64)
        g = picture[..., 1].astype(np.int64)
        b = picture[..., 2].astype(np.int64)
        y = (19595 * r + 38470 * g + 7471 * b + 32768) >> 16
        cb = (-11059 * r - 21709 * g + 32768 * b + (128 << 16) + 32767) >> 16
        cr = (32768 * r - 27439 * g - 5329 * b + (128 << 16) + 32767) >> 16
        planes = [y, np.clip(cb, 0, 255), np.clip(cr, 0, 255)]
        if halve_colour:
            for i in (1, 2):
                p = planes[i]
                planes[i] = (p[0::2, 0::2] + p[0::2, 1::2] + p[1::2, 0::2] + p[1::2, 1::2] + 2) >> 2
            components = [(1, 0x22, 0), (2, 0x11, 1), (3, 0x11, 1)]
            per_mcu = [4, 1, 1]
        else:
            components = [(1, 0x11, 0), (2, 0x11, 1), (3, 0x11, 1)]
            per_mcu = [1, 1, 1]
        kinds = [(_LUMA_Q, False), (_CHROMA_Q, True), (_CHROMA_Q, True)]
    sof = bytes([8]) + h.to_bytes(2, "big") + w.to_bytes(2, "big") + bytes([len(components)])
    for ident, sampling, table in components:
        sof += bytes([ident, sampling, table])
    out.append(_segment(0xC0, sof))
    out.append(_table_segment(0, 0, _DC_LUMA_BITS, _DC_LUMA_VALS))
    out.append(_table_segment(1, 0, _AC_LUMA_BITS, _AC_LUMA_VALS))
    if not grey:
        out.append(_table_segment(0, 1, _DC_CHROMA_BITS, _DC_CHROMA_VALS))
        out.append(_table_segment(1, 1, _AC_CHROMA_BITS, _AC_CHROMA_VALS))
    sos = bytes([len(components)])
    for ident, _sampling, table in components:
        sos += bytes([ident, (table << 4) | table])
    sos += bytes([0, 63, 0])
    out.append(_segment(0xDA, sos))
    keys, codes, lens = [], [], []
    slot = 0
    for plane, (qbase, chroma), blocks in zip(planes, kinds, per_mcu):
        k, c, n = _component_items(plane, qbase, quality, chroma, blocks, slot)
        keys.append(k)
        codes.append(c)
        lens.append(n)
        slot += blocks
    order = np.argsort(np.concatenate(keys), kind="stable")
    out.append(_pack(np.concatenate(codes)[order], np.concatenate(lens)[order]))
    out.append(b"\xff\xd9")
    return b"".join(out)
