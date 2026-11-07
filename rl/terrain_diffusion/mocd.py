"""The `.mocd` weights container read by `crates/moc-diffusion/src/format.rs`.

Layout (little-endian):
    0   b"MOCD"
    4   u32 version (1)
    8   u32 header length in bytes
    12  header: UTF-8 JSON
        zero padding so the data block starts at a multiple of 16
    ... data block: tensors, each starting at a multiple of 16 (offsets are relative to it)

Header keys: "format", "arch" (UNetConfig), "norm", "sampling", "meta", and
"tensors": [{"name", "shape", "dtype": "f16" | "f32", "offset", "nbytes"}].
f16 halves the file; the runtime widens to f32 on load.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import numpy as np

MAGIC = b"MOCD"
VERSION = 1


def _align(n: int, a: int = 16) -> int:
    return (n + a - 1) // a * a


def write_mocd(path, header: dict, tensors: list, dtype: str = "f16") -> int:
    """`tensors` is a list of (name, ndarray). Returns the file size in bytes."""
    assert dtype in ("f16", "f32")
    np_dtype = np.float16 if dtype == "f16" else np.float32
    blobs, table, off = [], [], 0
    for name, arr in tensors:
        a = np.ascontiguousarray(np.asarray(arr, dtype=np.float32).astype(np_dtype))
        if dtype == "f16" and not np.all(np.isfinite(a)):
            raise ValueError(f"{name}: overflows float16")
        b = a.astype("<" + a.dtype.str[1:]).tobytes()
        table.append({"name": name, "shape": list(a.shape), "dtype": dtype, "offset": off, "nbytes": len(b)})
        blobs.append((off, b))
        off = _align(off + len(b))
    hdr = dict(header)
    hdr["tensors"] = table
    hj = json.dumps(hdr, separators=(",", ":")).encode("utf-8")
    start = _align(12 + len(hj))
    data = bytearray(off)
    for o, b in blobs:
        data[o : o + len(b)] = b
    out = MAGIC + struct.pack("<II", VERSION, len(hj)) + hj + b"\0" * (start - 12 - len(hj)) + bytes(data)
    Path(path).write_bytes(out)
    return len(out)


def read_mocd(path):
    """Returns (header, {name: float32 ndarray})."""
    raw = Path(path).read_bytes()
    if raw[:4] != MAGIC:
        raise ValueError("not a .mocd file")
    ver, hl = struct.unpack("<II", raw[4:12])
    if ver != VERSION:
        raise ValueError(f"unsupported version {ver}")
    hdr = json.loads(raw[12 : 12 + hl].decode("utf-8"))
    start = _align(12 + hl)
    out = {}
    for t in hdr["tensors"]:
        dt = np.dtype("<f2") if t["dtype"] == "f16" else np.dtype("<f4")
        o = start + t["offset"]
        a = np.frombuffer(raw[o : o + t["nbytes"]], dtype=dt).reshape(t["shape"]).astype(np.float32)
        out[t["name"]] = a
    return hdr, out


def state_tensors(model) -> list:
    """The model's parameters in state_dict order (names are what the runtime looks up)."""
    return [(k, v.detach().cpu().float().numpy()) for k, v in model.state_dict().items()]
