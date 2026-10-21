#!/usr/bin/env python3
"""Crop (and integer-zoom) a PNG with the standard library only.

Usage: pngcrop.py in.png out.png x y w h [zoom]
Used to inspect the help screenshots (extracted/chm/clip*.png, local only).
"""
import struct
import sys
import zlib


def read_png(path):
    data = open(path, 'rb').read()
    assert data[:8] == b'\x89PNG\r\n\x1a\n', 'not a PNG'
    pos, idat, plte = 8, b'', None
    while pos < len(data):
        n, kind = struct.unpack('>I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b'IHDR':
            w, h, depth, ctype, _, _, interlace = struct.unpack('>IIBBBBB', body)
            assert depth == 8 and interlace == 0, 'only 8-bit non-interlaced'
        elif kind == b'PLTE':
            plte = [body[i:i + 3] for i in range(0, len(body), 3)]
        elif kind == b'IDAT':
            idat += body
        pos += 12 + n
    bpp = {2: 3, 6: 4, 3: 1, 0: 1, 4: 2}[ctype]
    raw = zlib.decompress(idat)
    stride = w * bpp
    rows, prev = [], bytearray(stride)
    for y in range(h):
        ft = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if ft == 1:
                line[i] = (line[i] + a) & 255
            elif ft == 2:
                line[i] = (line[i] + b) & 255
            elif ft == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    rgb = []
    for line in rows:
        px = []
        for x in range(w):
            if ctype == 3:
                px.append(plte[line[x]])
            elif ctype in (0, 4):
                v = line[x * bpp]
                px.append(bytes((v, v, v)))
            else:
                px.append(line[x * bpp:x * bpp + 3])
        rgb.append(px)
    return w, h, rgb


def write_png(path, rows):
    h, w = len(rows), len(rows[0])
    raw = b''.join(b'\0' + b''.join(r) for r in rows)
    chunk = lambda k, b: struct.pack('>I', len(b)) + k + b + struct.pack('>I', zlib.crc32(k + b) & 0xffffffff)
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


if __name__ == '__main__':
    src, dst, x, y, w, h = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
    zoom = int(sys.argv[7]) if len(sys.argv) > 7 else 1
    _, _, img = read_png(src)
    crop = [row[x:x + w] for row in img[y:y + h]]
    out = [[p for p in row for _ in range(zoom)] for row in crop for _ in range(zoom)]
    write_png(dst, out)
