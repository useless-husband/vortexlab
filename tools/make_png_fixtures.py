#!/usr/bin/env python3
"""Writes the PNG test fixtures in tests/data/ with Python's zlib.

The Rust PNG decoder is tested against these files so that it is checked against an
independent encoder (zlib's dynamic-Huffman DEFLATE and all five row filters), not only
against the encoder in the same crate. Pixel values follow simple formulas that
tests/codecs.rs recomputes. Run from the repository root:  python3 tools/make_png_fixtures.py
"""
import struct
import zlib


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else (b if pb <= pc else c)


def png(width, height, depth, ctype, rows, extra=b""):
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    bpp = max(1, channels * depth // 8)
    raw = bytearray()
    prev = bytes(len(rows[0]))
    for y, row in enumerate(rows):
        f = y % 5  # cycle through None, Sub, Up, Average, Paeth
        raw.append(f)
        for x, v in enumerate(row):
            a = row[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            pred = [0, a, b, (a + b) // 2, paeth(a, b, c)][f]
            raw.append((v - pred) & 255)
        prev = row
    ihdr = struct.pack(">IIBBBBB", width, height, depth, ctype, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + extra + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")


def pack_bits(values, depth):
    out, acc, n = bytearray(), 0, 0
    for v in values:
        acc = (acc << depth) | v
        n += depth
        if n == 8:
            out.append(acc)
            acc, n = 0, 0
    if n:
        out.append(acc << (8 - n))
    return bytes(out)


W, H = 33, 21
files = {}
files["rgb8.png"] = png(W, H, 8, 2, [bytes((x * 7 + y * 13 + c * 29) % 256 for x in range(W) for c in range(3)) for y in range(H)])
files["rgba8.png"] = png(W, H, 8, 6, [bytes((x * 5 + y * 11 + c * 31) % 256 for x in range(W) for c in range(4)) for y in range(H)])
files["graya8.png"] = png(W, H, 8, 4, [bytes((x * 3 + y * 17 + c * 101) % 256 for x in range(W) for c in range(2)) for y in range(H)])
files["gray16.png"] = png(W, H, 16, 0, [b"".join(struct.pack(">H", (x * 1021 + y * 4099) % 65536) for x in range(W)) for y in range(H)])
files["gray1.png"] = png(19, H, 1, 0, [pack_bits([(x * x + y) % 3 == 0 for x in range(19)], 1) for y in range(H)])
palette = bytes(v for i in range(16) for v in (i * 16, 255 - i * 16, (i * 37) % 256))
trns = bytes((i * 17) % 256 for i in range(10))  # shorter than the palette: the rest is opaque
files["palette4.png"] = png(W, H, 4, 3, [pack_bits([(x + 2 * y) % 16 for x in range(W)], 4) for y in range(H)], chunk(b"PLTE", palette) + chunk(b"tRNS", trns))
# A larger smooth image, so zlib emits dynamic Huffman blocks with long matches.
files["smooth.png"] = png(200, 120, 8, 0, [bytes((x * x // 97 + y * 3) % 256 for x in range(200)) for y in range(120)])

for name, data in files.items():
    with open("tests/data/" + name, "wb") as fh:
        fh.write(data)
    print(name, len(data))
