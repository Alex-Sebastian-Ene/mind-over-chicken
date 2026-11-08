"""Tiny dependency-free PNG writer and a terrain colouring for previews."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

import numpy as np


def save_rgb(path, img: np.ndarray) -> None:
    """img: H x W x 3 uint8."""
    img = np.ascontiguousarray(img, dtype=np.uint8)
    h, w, _ = img.shape
    raw = b"".join(b"\x00" + img[y].tobytes() for y in range(h))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")
    Path(path).write_bytes(png)


_STOPS = [  # game height -> colour
    (-1.0, (8, 24, 68)),
    (-0.3, (22, 64, 128)),
    (-0.02, (64, 132, 186)),
    (0.0, (196, 186, 140)),
    (0.08, (92, 148, 72)),
    (0.3, (138, 160, 84)),
    (0.5, (150, 126, 88)),
    (0.75, (128, 112, 104)),
    (1.0, (240, 240, 244)),
]


def terrain_rgb(h: np.ndarray, shade: bool = True) -> np.ndarray:
    """Game-unit heights (H x W) -> RGB with a hypsometric tint and a simple hillshade."""
    h = np.asarray(h, np.float32)
    xs = np.array([s[0] for s in _STOPS], np.float32)
    cols = np.array([s[1] for s in _STOPS], np.float32)
    rgb = np.stack([np.interp(h, xs, cols[:, c]) for c in range(3)], axis=-1)
    if shade:
        gy, gx = np.gradient(np.where(h > 0, h, h * 0.2))
        light = np.clip(1.0 + (-(gx + gy) * 12.0), 0.55, 1.35)
        rgb = rgb * light[..., None]
    return np.clip(rgb, 0, 255).astype(np.uint8)
