"""Cut a balanced, diverse training set of terrain patches out of a global DEM.

    python prep_etopo.py --src ETOPO_2022_v1_60s_N90W180_surface.tif --out /scratch/.../data
    python prep_etopo.py --synthetic --out /tmp/td_data          # smoke test, no download

Pipeline
1. Load the global grid (GeoTIFF via tifffile, or netCDF via h5py), north at row 0, metres.
2. Area-average it to several scales (default 1, 2, 4, 8 arc-min per pixel; ETOPO 60s is 1').
   Real relief is roughly self-similar, so mixing scales gives the model ranges, valleys and
   coasts at every size the game asks for.
3. Candidate patches (128 x 128) on a grid over latitudes -78..78 with 50% overlap. Pixels are
   made square on the ground (longitude spacing / cos(lat)), with a box prefilter along x.
4. Each candidate gets features (land fraction, relief, mean elevation, basin-ness, |lat|)
   and a landform class: ocean, coast, polar_coast, glaciated, plains, plateau, basin, hills,
   low_mountains, high_mountains. Relief thresholds are quantiles per scale, so every scale
   feeds every class.
5. Open ocean is capped; everything else is kept. train.py samples classes evenly (and
   scales evenly within a class), so flat plains and ocean cannot dominate.
Outputs: patches.npy (float16 metres, N x 128 x 128), index.npz, prep_stats.json,
classes.png (a few patches per class).
"""

from __future__ import annotations

import argparse
import json
import math
import time
from pathlib import Path

import numpy as np
import torch

from data import DEFAULT_STATS, blur, local_relief, make_pair, metres_to_game, synthetic_patches
from png import save_rgb, terrain_rgb

CLASSES = ["ocean", "coast", "polar_coast", "glaciated", "plains", "plateau", "basin", "hills", "low_mountains", "high_mountains"]
PATCH = 128


def load_grid(path: Path) -> np.ndarray:
    if path.suffix.lower() in (".tif", ".tiff"):
        import tifffile

        a = tifffile.imread(str(path))
    else:
        import h5py

        with h5py.File(path, "r") as f:
            name = "z" if "z" in f else [k for k in f if f[k].ndim == 2][0]
            a = f[name][...]
            lat = f["lat"][...] if "lat" in f else None
        if lat is not None and lat[0] < lat[-1]:
            a = a[::-1]
    a = np.asarray(a, dtype=np.float32)
    a[~np.isfinite(a)] = 0.0
    return a


def synthetic_grid(h: int = 900, w: int = 1800, seed: int = 0) -> np.ndarray:
    """A fake 12' world built from synthetic patches (smoke test only)."""
    tiles = synthetic_patches((h // PATCH + 1) * (w // PATCH + 1), PATCH, seed)
    g = np.zeros(((h // PATCH + 1) * PATCH, (w // PATCH + 1) * PATCH), np.float32)
    k = 0
    for r in range(h // PATCH + 1):
        for c in range(w // PATCH + 1):
            g[r * PATCH : (r + 1) * PATCH, c * PATCH : (c + 1) * PATCH] = tiles[k]
            k += 1
    return g[:h, :w]


def area_mean(a: np.ndarray, k: int) -> np.ndarray:
    if k == 1:
        return a
    h, w = a.shape[0] // k * k, a.shape[1] // k * k
    return a[:h, :w].reshape(h // k, k, w // k, k).mean(axis=(1, 3), dtype=np.float64).astype(np.float32)


def extract(g: np.ndarray, lat: float, lon: float, size: int = PATCH) -> np.ndarray:
    """Square-pixel patch centred at (lat, lon) from a global equirectangular grid g (row 0 =
    90N, col 0 = 180W), pixel = one grid step north-south."""
    gh, gw = g.shape
    step = 180.0 / gh  # degrees per pixel
    coslat = max(math.cos(math.radians(lat)), 0.05)
    xstep = 1.0 / coslat  # grid columns per output pixel
    rows = (90.0 - lat) / step - 0.5 + (np.arange(size) - (size - 1) / 2.0)
    cols = (lon + 180.0) / step - 0.5 + (np.arange(size) - (size - 1) / 2.0) * xstep
    r0 = int(np.floor(rows[0]))
    r1 = int(np.floor(rows[-1])) + 2
    rr = np.clip(np.arange(r0, r1), 0, gh - 1)
    box = max(1, int(round(xstep)))
    c0 = int(np.floor(cols[0])) - box
    c1 = int(np.floor(cols[-1])) + 2 + box
    cc = np.arange(c0, c1) % gw
    strip = g[np.ix_(rr, cc)]
    if box > 1:  # anti-alias along x before resampling: centred running mean of `box` columns
        cs = np.concatenate([np.zeros((strip.shape[0], 1), np.float64), np.cumsum(strip, axis=1, dtype=np.float64)], axis=1)
        lo = np.clip(np.arange(strip.shape[1]) - box // 2, 0, strip.shape[1] - box)
        strip = ((cs[:, lo + box] - cs[:, lo]) / box).astype(np.float32)
    fr = rows - r0
    fc = cols - c0
    i0 = np.clip(np.floor(fr).astype(int), 0, strip.shape[0] - 2)
    j0 = np.clip(np.floor(fc).astype(int), 0, strip.shape[1] - 2)
    ty = (fr - i0)[:, None]
    tx = (fc - j0)[None, :]
    a = strip[np.ix_(i0, j0)]
    b = strip[np.ix_(i0, j0 + 1)]
    c = strip[np.ix_(i0 + 1, j0)]
    d = strip[np.ix_(i0 + 1, j0 + 1)]
    return ((a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty).astype(np.float32)


def features(p: np.ndarray, lat: float) -> dict:
    land = p > 0
    lf = float(land.mean())
    q2, q98 = np.percentile(p, [2, 98])
    n = p.shape[0]
    inner = p[n // 4 : 3 * n // 4, n // 4 : 3 * n // 4]
    ring = np.concatenate([p[: n // 8].ravel(), p[-n // 8 :].ravel(), p[:, : n // 8].ravel(), p[:, -n // 8 :].ravel()])
    return {
        "land_frac": lf,
        "relief": float(q98 - q2),
        "mean": float(p[land].mean()) if land.any() else float(p.mean()),
        "basin": float(np.median(ring) - np.median(inner)),
        "abs_lat": abs(lat),
    }


def classify(feats: list[dict], scales: np.ndarray) -> np.ndarray:
    cls = np.zeros(len(feats), np.int64)
    lf = np.array([f["land_frac"] for f in feats])
    rel = np.array([f["relief"] for f in feats])
    mean = np.array([f["mean"] for f in feats])
    basin = np.array([f["basin"] for f in feats])
    alat = np.array([f["abs_lat"] for f in feats])
    C = {n: i for i, n in enumerate(CLASSES)}
    cls[:] = C["ocean"]
    coast = (lf >= 0.02) & (lf < 0.9)
    cls[coast] = np.where(alat[coast] >= 55, C["polar_coast"], C["coast"])
    landp = lf >= 0.9
    for s in np.unique(scales):
        m = landp & (scales == s) & (alat < 60)
        if not m.any():
            continue
        q30, q60, q85 = np.quantile(rel[m], [0.30, 0.60, 0.85])
        sub = np.full(m.sum(), C["plains"])
        r, mu, bs = rel[m], mean[m], basin[m]
        sub[r >= q30] = C["hills"]
        sub[r >= q60] = C["low_mountains"]
        sub[r >= q85] = C["high_mountains"]
        sub[(mu > 1200) & (r < q60)] = C["plateau"]
        sub[(bs > 0.15 * r) & (r >= q30) & (r < q85)] = C["basin"]
        cls[m] = sub
    cls[landp & (alat >= 60)] = C["glaciated"]
    return cls


def candidates(scale_deg: float, max_lat: float, overlap: float):
    """Patch centres for a patch extent of `scale_deg` degrees N-S."""
    step = scale_deg * (1 - overlap)
    lat = -max_lat + scale_deg / 2
    while lat <= max_lat - scale_deg / 2 + 1e-9:
        lstep = step / max(math.cos(math.radians(lat)), 0.2)
        n = max(1, int(360.0 / lstep))
        for i in range(n):
            yield lat, -180.0 + (i + 0.5) * 360.0 / n
        lat += step


def measure_stats(patches: np.ndarray, n: int = 2048, seed: int = 0) -> dict:
    """Normalisation constants for make_pair, measured on a random subset."""
    rng = np.random.default_rng(seed)
    idx = rng.choice(len(patches), size=min(n, len(patches)), replace=False)
    m = torch.from_numpy(patches[np.sort(idx)].astype(np.float32))[:, None]
    stats = dict(DEFAULT_STATS)
    h = metres_to_game(m)
    rel = blur(local_relief(h), 3.0)
    land = h > 0
    stats["uplift_ref"] = float(torch.quantile(rel[land].flatten()[:2_000_000], 0.95)) if land.any() else stats["uplift_ref"]
    # Residual scale with the default blur, then rough_ref from the same residuals.
    x0, _, parts = make_pair(m, {**stats, "resid_std": 1.0}, augment=False)
    resid = parts["h"] - parts["coarse"]
    stats["resid_std"] = float(resid.std())
    loc = blur(resid**2, 3.0).clamp_min(0).sqrt()
    stats["rough_ref"] = float(torch.quantile(loc.flatten()[:2_000_000], 0.95))
    stats["coarse_mean"] = 0.0
    stats["coarse_std"] = float(parts["coarse"].std())
    return stats


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", type=Path, help="ETOPO GeoTIFF (.tif) or netCDF (.nc)")
    ap.add_argument("--synthetic", action="store_true", help="fake world instead of --src (smoke test)")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--scales", default="1,2,4,8", help="arc-minutes per pixel (multiples of the source)")
    ap.add_argument("--overlap", type=float, default=0.5)
    ap.add_argument("--max-lat", type=float, default=78.0)
    ap.add_argument("--ocean-cap", type=float, default=0.06, help="max share of open-ocean patches")
    ap.add_argument("--seed", type=int, default=0)
    args = ap.parse_args()
    t0 = time.time()
    args.out.mkdir(parents=True, exist_ok=True)

    if args.synthetic:
        g = synthetic_grid()
        scales = [12, 24]
    else:
        g = load_grid(args.src)
        scales = [int(s) for s in args.scales.split(",")]
    src_arcmin = 180.0 * 60.0 / g.shape[0]
    print(f"grid {g.shape}, {src_arcmin:.2f}' per px, range {g.min():.0f}..{g.max():.0f} m  ({time.time() - t0:.0f}s)")

    patches, meta, feats = [], [], []
    for s in scales:
        k = int(round(s / src_arcmin))
        gs = area_mean(g, max(k, 1))
        extent = PATCH * s / 60.0
        n0 = len(patches)
        for lat, lon in candidates(extent, args.max_lat, args.overlap):
            p = extract(gs, lat, lon)
            patches.append(p.astype(np.float16))
            meta.append((s, lat, lon))
            feats.append(features(p, lat))
        print(f"scale {s}': patch {extent:.1f} deg, {len(patches) - n0} candidates  ({time.time() - t0:.0f}s)")

    meta_a = np.array(meta, np.float32)
    cls = classify(feats, meta_a[:, 0])
    rng = np.random.default_rng(args.seed)
    keep = np.ones(len(cls), bool)
    ocean = np.flatnonzero(cls == 0)
    cap = int(args.ocean_cap * (len(cls) - len(ocean)) / (1 - args.ocean_cap))
    if len(ocean) > cap:
        keep[rng.choice(ocean, size=len(ocean) - cap, replace=False)] = False
    sel = np.flatnonzero(keep)
    arr = np.stack([patches[i] for i in sel])
    np.save(args.out / "patches.npy", arr)
    f = lambda k: np.array([feats[i][k] for i in sel], np.float32)  # noqa: E731
    # Hold out a longitude band for validation (no overlap leakage).
    val = (meta_a[sel, 2] >= 150.0)
    np.savez(
        args.out / "index.npz",
        cls=cls[sel], scale=meta_a[sel, 0], lat=meta_a[sel, 1], lon=meta_a[sel, 2], val=val,
        land_frac=f("land_frac"), relief=f("relief"), mean=f("mean"), basin=f("basin"),
    )
    stats = measure_stats(arr[~val])
    counts = {c: int((cls[sel] == i).sum()) for i, c in enumerate(CLASSES)}
    by_scale = {str(s): {c: int(((cls[sel] == i) & (meta_a[sel, 0] == s)).sum()) for i, c in enumerate(CLASSES)} for s in scales}
    report = {
        "source": "synthetic" if args.synthetic else str(args.src.name),
        "patch": PATCH,
        "scales_arcmin": scales,
        "patches": int(len(sel)),
        "val_patches": int(val.sum()),
        "dropped_open_ocean": int((~keep).sum()),
        "classes": CLASSES,
        "counts": counts,
        "counts_by_scale": by_scale,
        "stats": stats,
        "seconds": round(time.time() - t0, 1),
    }
    (args.out / "prep_stats.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))

    # A look at each class: up to 6 patches per row.
    rows = []
    for i, c in enumerate(CLASSES):
        ids = np.flatnonzero(cls[sel] == i)[:6]
        tiles = [terrain_rgb(metres_to_game(arr[j].astype(np.float32))) for j in ids]
        while len(tiles) < 6:
            tiles.append(np.zeros((PATCH, PATCH, 3), np.uint8))
        rows.append(np.concatenate(tiles, axis=1))
    save_rgb(args.out / "classes.png", np.concatenate(rows, axis=0))


if __name__ == "__main__":
    main()
