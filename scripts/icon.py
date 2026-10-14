"""Draws the app icon: two book spines, one standing and one leaning on it,
dithered to the two tones of the app. Writes assets/icon.ico and
assets/icon.png.

    python scripts/icon.py
"""
import math
import struct
import zlib
from pathlib import Path

INK = (0x1B, 0x1B, 0x1A)
PAPER = (0xDD, 0xDB, 0xD4)
BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
# Each size is drawn on its own grid of dots, so small sizes stay sharp.
SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
ASSETS = Path(__file__).resolve().parent.parent / "assets"


def spine(lx, ly, width, height, line, small, fill):
    """A spine in its own coordinates, `ly` up from the bottom."""
    if lx < line or lx > width - line or ly < line or ly > height - line:
        return "ink"
    if small:
        return fill
    band = height * 0.1
    for bottom in (height * 0.12, height * 0.78):
        if bottom <= ly <= bottom + band:
            return "ink" if abs(ly - (bottom + band / 2)) < line / 2 else "paper"
    return fill


def shape(u, v, dot):
    """Ink, paper, a darkness from 0 to 1 to dither, or None for transparent.
    `u` and `v` go from 0 to 1."""
    line = max(1.5 * dot, 0.035)
    small = dot > 1 / 24
    x0, x1, y0, y1 = 0.48, 0.76, 0.06, 0.94
    if x0 <= u <= x1 and y0 <= v <= y1:
        return spine(u - x0, y1 - v, x1 - x0, y1 - y0, line, small, 0.3 + 0.5 * (v - y0))
    # The leaning spine stands on its bottom right corner, and its top right
    # corner touches the standing spine.
    angle, width, height = 0.36, 0.20, 0.80
    px = x0 - height * math.sin(angle)
    du, dv = u - px, v - y1
    lx = -(du * math.cos(angle) + dv * math.sin(angle))
    ly = du * math.sin(angle) - dv * math.cos(angle)
    if 0 <= lx <= width and 0 <= ly <= height:
        return "ink" if small else spine(lx, ly, width, height, line, small, 0.55)
    return None


def render(grid, scale):
    cells = []
    for y in range(grid):
        row = []
        for x in range(grid):
            # A dot takes the value at its center, so edges stay crisp.
            t = shape((x + 0.5) / grid, (y + 0.5) / grid, 1 / grid)
            if t is None:
                row.append(None)
            elif t == "ink" or (t != "paper" and t > (BAYER[y % 4][x % 4] + 0.5) / 16):
                row.append(INK)
            else:
                row.append(PAPER)
        cells.append(row)
    # A paper edge around the ink keeps the icon clear on a dark background.
    for y in range(grid):
        for x in range(grid):
            if cells[y][x] is None and any(
                cells[j][i] == INK
                for j in range(max(y - 1, 0), min(y + 2, grid))
                for i in range(max(x - 1, 0), min(x + 2, grid))
            ):
                cells[y][x] = PAPER
    pixels = []
    for row in cells:
        line = [c + (255,) if c else (0, 0, 0, 0) for c in row for _ in range(scale)]
        pixels += [line] * scale
    return pixels


def png(size):
    grid = min(size, 64)
    pixels = render(grid, size // grid)
    raw = b"".join(b"\0" + bytes(c for p in row for c in p) for row in pixels)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    """An ICO file with PNG entries."""
    offset = 6 + 16 * len(images)
    entries, data = b"", b""
    for size, image in images:
        side = size % 256  # 0 means 256
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(image), offset + len(data))
        data += image
    return struct.pack("<HHH", 0, 1, len(images)) + entries + data


images = [(size, png(size)) for size in SIZES]
(ASSETS / "icon.ico").write_bytes(ico(images))
(ASSETS / "icon.png").write_bytes(dict(images)[256])
