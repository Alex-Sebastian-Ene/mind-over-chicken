"""Turn raw RGBA terrain dumps into PNGs (stdlib only), for docs/terrain/ (#109).

    python scripts/terrain_images.py IN.rgba OUT.png [--size W H] [--scale K] [--tile NX NY]
                                     [--crop X Y W H] [--shrink K]

The size comes from IN.txt (written next to the dump by `cargo run -p moc-terrain --example
render`) unless --size is given. --scale K draws each tile as K×K pixels (nearest neighbour),
--shrink K keeps every K-th pixel, --tile repeats the map (2 2 shows the seamless torus wrap),
--crop cuts a window first (wrapping across the seams).
"""

import argparse
import struct
import zlib
from pathlib import Path


def write_png(path, w, h, rows):
    """rows: list of bytes, each w*3 long (RGB)."""
    raw = b"".join(b"\x00" + r for r in rows)

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    Path(path).write_bytes(png)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--size", nargs=2, type=int)
    ap.add_argument("--scale", type=int, default=1)
    ap.add_argument("--shrink", type=int, default=1)
    ap.add_argument("--tile", nargs=2, type=int, default=[1, 1])
    ap.add_argument("--crop", nargs=4, type=int)
    a = ap.parse_args()
    src = Path(a.src)
    if a.size:
        w, h = a.size
    else:
        w, h = map(int, src.with_suffix(".txt").read_text().split())
    data = src.read_bytes()
    assert len(data) == w * h * 4, f"{src}: expected {w}x{h} RGBA"
    rgb = [bytes(data[y * w * 4:(y + 1) * w * 4][k] for k in range(w * 4) if k % 4 != 3) for y in range(h)]
    if a.crop:
        cx, cy, cw, ch = a.crop
        rgb = [b"".join(rgb[(cy + y) % h][((cx + x) % w) * 3:((cx + x) % w) * 3 + 3] for x in range(cw)) for y in range(ch)]
        w, h = cw, ch
    if a.shrink > 1:
        k = a.shrink
        rgb = [b"".join(r[x * 3:x * 3 + 3] for x in range(0, w, k)) for r in rgb[::k]]
        w, h = len(rgb[0]) // 3, len(rgb)
    nx, ny = a.tile
    rgb = [r * nx for r in rgb] * ny
    w, h = w * nx, h * ny
    if a.scale > 1:
        k = a.scale
        rgb = [b"".join(r[x * 3:x * 3 + 3] * k for x in range(w)) for r in rgb for _ in range(k)]
        w, h = w * k, h * k
    write_png(a.dst, w, h, rgb)
    print(f"{a.dst}: {w}x{h}")


if __name__ == "__main__":
    main()
