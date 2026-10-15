"""Draws the app icon: a butterfly made of an open book. The spine is the
body, the pages with lines of text are the upper wings. Dithered to the two
tones of the app. Writes assets/icon.ico, assets/icon.png and, on macOS,
assets/icon.icns (on a rounded tile).

    python3 scripts/icon.py
"""
import math
import shutil
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

INK = (0x1B, 0x1B, 0x1A)
PAPER = (0xDD, 0xDB, 0xD4)
BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
# Each size is drawn on its own grid of dots, so small sizes stay sharp.
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
ICNS_SIZES = [16, 32, 64, 128, 256, 512, 1024]
ASSETS = Path(__file__).resolve().parent.parent / "assets"
# The macOS tile: 824 of 1024 pixels, with corners like a superellipse.
TILE = 824 / 1024
TILE_POWER = 5


def ellipse(u, v, cx, cy, rx, ry, angle):
    """Local coordinates and the normalized radius (1 at the edge)."""
    du, dv = u - cx, v - cy
    lx = du * math.cos(angle) + dv * math.sin(angle)
    ly = -du * math.sin(angle) + dv * math.cos(angle)
    return lx, ly, math.hypot(lx / rx, ly / ry)


def near_segment(u, v, a, b, width):
    (ax, ay), (bx, by) = a, b
    t = ((u - ax) * (bx - ax) + (v - ay) * (by - ay)) / ((bx - ax) ** 2 + (by - ay) ** 2)
    t = min(max(t, 0), 1)
    return math.hypot(u - ax - t * (bx - ax), v - ay - t * (by - ay)) <= width


def antenna(x, v, line):
    """A curve from the head, out and up, with a knob at the end."""
    points = [(0.012 + 0.12 * t ** 1.4, 0.19 - 0.13 * t) for t in (i / 12 for i in range(13))]
    if math.hypot(x - points[-1][0], v - points[-1][1]) <= 0.026:
        return True
    return any(near_segment(x, v, a, b, line / 2) for a, b in zip(points, points[1:]))


def page(x, v):
    """The right page of the open book, which is the upper wing. Its top
    rises from the spine to the outer edge, as the pages of an open book do."""
    reach = x / 0.45
    top = 0.25 - 0.12 * math.sin(reach * math.pi / 2)
    bottom = 0.58 - 0.10 * reach**2
    rounded = (x / 0.45) ** 4 + ((v - 0.36) / 0.27) ** 4 <= 1
    return 0 <= x and top <= v <= bottom and rounded


def lower_wing(x, v):
    return ellipse(x, v, 0.16, 0.66, 0.14, 0.15, 0.5)[2] <= 1


def outline(inside, x, v, line):
    """Inside, but a neighbor at `line` distance is outside."""
    return inside(x, v) and not all(
        inside(x + line * math.cos(a), v + line * math.sin(a))
        for a in (i * math.pi / 4 for i in range(8))
    )


def shape(u, v, dot):
    """Ink, paper, a darkness from 0 to 1 to dither, or None for transparent.
    `u` and `v` go from 0 to 1."""
    small = dot > 1 / 24
    line = dot if small else max(1.5 * dot, 0.026)
    x = abs(u - 0.5)
    v -= 0.06

    if near_segment(x, v, (0, 0.27), (0, 0.80), 0.034) or math.hypot(x, v - 0.22) <= 0.04:
        return "ink"
    if dot < 1 / 16 and antenna(x, v, line):
        return "ink"

    if page(x, v):
        if outline(page, x, v, line):
            return "ink"
        if small:
            return "paper"
        # Lines of text, broken into words, with margins.
        spacing = max(0.06, 3 * dot)
        stroke = max(dot, 0.013)
        row = math.floor(v / spacing)
        word = ((x + 0.29 * row) / 0.085) % 1
        margin = 2.2 * line
        text = (
            v % spacing < stroke
            and x > 0.034 + margin
            and word < 0.8
            and all(page(x + dx, v + dy) for dx, dy in ((margin, 0), (0, -margin), (0, margin)))
        )
        return "ink" if text else "paper"

    if lower_wing(x, v):
        if small or outline(lower_wing, x, v, line):
            return "ink"
        return 0.3 + 0.4 * (v - 0.55) / 0.25
    return None


# At 16 pixels the computed drawing is not clear, so it is drawn by hand.
# "o" is paper inside the pages.
TINY = [
    ".#....#.",
    "..#..#..",
    "########",
    "#oo##oo#",
    "#oo##oo#",
    "########",
    ".#.##.#.",
    "...##...",
]


def in_tile(u, v):
    """`u` and `v` in tile coordinates, from -1 to 1."""
    return abs(u) ** TILE_POWER + abs(v) ** TILE_POWER <= 1


def render(grid, scale, tile):
    """A grid of dots, each `scale` pixels. On a tile, the drawing takes the
    middle of the tile."""
    inset = (0.09 if grid > 32 else 0.03) if tile else 0
    cells = []
    for y in range(grid):
        row = []
        for x in range(grid):
            u, v = (x + 0.5) / grid, (y + 0.5) / grid
            if tile:
                tu, tv = (u - 0.5) * 2 / TILE, (v - 0.5) * 2 / TILE
                if not in_tile(tu, tv):
                    row.append(None)
                    continue
                edge = not all(
                    in_tile(tu + du * 2 / TILE / grid, tv + dv * 2 / TILE / grid)
                    for du, dv in ((1, 0), (-1, 0), (0, 1), (0, -1))
                )
                if edge:
                    row.append(INK)
                    continue
                span = TILE * (1 - 2 * inset)
                u, v = (u - 0.5) / span + 0.5, (v - 0.5) / span + 0.5
                dot = 1 / grid / span
            else:
                dot = 1 / grid
            if not tile and grid == 16:
                row.append({"#": INK, "o": PAPER}.get(TINY[y // 2][x // 2]))
                continue
            if tile and grid == 16:
                tx, ty = x - 4, y - 4
                inside = 0 <= tx < 8 and 0 <= ty < 8
                row.append(INK if inside and TINY[ty][tx] == "#" else PAPER)
                continue
            t = shape(u, v, dot)
            if t is None:
                row.append(PAPER if tile else None)
            elif t == "ink" or (t != "paper" and t > (BAYER[y % 4][x % 4] + 0.5) / 16):
                row.append(INK)
            else:
                row.append(PAPER)
        cells.append(row)
    if not tile:
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


def png(size, tile=False):
    grid = min(size, 128 if tile else 64)
    pixels = render(grid, size // grid, tile)
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


def icns():
    """Uses iconutil, so only on macOS."""
    if not shutil.which("iconutil"):
        print("iconutil not found: icon.icns not written")
        return
    images = {size: png(size, tile=True) for size in ICNS_SIZES}
    with tempfile.TemporaryDirectory() as temp:
        iconset = Path(temp) / "icon.iconset"
        iconset.mkdir()
        for size in ICNS_SIZES[:-1]:
            (iconset / f"icon_{size}x{size}.png").write_bytes(images[size])
            (iconset / f"icon_{size}x{size}@2x.png").write_bytes(images[size * 2])
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(ASSETS / "icon.icns")], check=True)


images = [(size, png(size)) for size in ICO_SIZES]
(ASSETS / "icon.ico").write_bytes(ico(images))
(ASSETS / "icon.png").write_bytes(dict(images)[256])
icns()
