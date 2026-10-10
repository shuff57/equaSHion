"""Fail when a PNG screenshot is blank: too small, one flat colour, or almost no contrast.

Standard library only (zlib), 8-bit RGB / RGBA PNGs as Chromium writes them.
usage: python3 -I check_png.py shot.png
"""
import struct
import sys
import zlib


def decode(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        (n,) = struct.unpack(">I", data[pos : pos + 4])
        kind, body = data[pos + 4 : pos + 8], data[pos + 8 : pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
            if depth != 8 or ctype not in (2, 6):
                raise ValueError(f"unsupported PNG: depth {depth}, colour type {ctype}")
            bpp = 3 if ctype == 2 else 4
        elif kind == b"IDAT":
            idat += body
    raw = zlib.decompress(idat)
    stride = w * bpp
    rows, prev = [], bytearray(stride)
    for y in range(h):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1 : (y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, bpp, rows


def main():
    path = sys.argv[1]
    w, h, bpp, rows = decode(path)
    colours, lum_min, lum_max = set(), 255, 0
    for y in range(0, h, 2):
        line = rows[y]
        for x in range(0, w, 2):
            r, g, b = line[x * bpp], line[x * bpp + 1], line[x * bpp + 2]
            colours.add((r, g, b))
            lum = (r * 299 + g * 587 + b * 114) // 1000
            lum_min, lum_max = min(lum_min, lum), max(lum_max, lum)
    print(f"check_png: {path}: {w}x{h}, {len(colours)} colours, luminance {lum_min}..{lum_max}")
    if w < 200 or h < 200 or len(colours) < 16 or lum_max - lum_min < 80:
        print("check_png: looks blank", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
