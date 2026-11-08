"""Training data: height units, conditioning, augmentation, and a synthetic stand-in.

Units. Real elevations (metres) map to game height units (sea 0, peaks ~1, floor -1) with a
log curve, so lowland relief is not crushed next to the Himalaya:
    land: T(m) = ln(1 + m/500) / ln(1 + 6000/500)       100 m -> 0.07, 1000 m -> 0.43, 6000 m -> 1
    sea:  T(m) = -ln(1 + |m|/1500) / ln(1 + 6000/1500)   -200 m -> -0.08, -4000 m -> -0.81

Training pairs. For a real patch h = T(metres) at the model grid:
    coarse = cubic_up2(avg_pool2(blur(h, s)))        s ~ U(0.6, 3.0) px: like the game's coarse map
                                                     (2 model px per coarse cell), smooth to blocky
    land   = same pipeline on (h > 0)
    uplift = local relief of h (max - min over ~9 px), blurred, / UPLIFT_REF, clipped to [0, 1]
    rough  = local std of (h - coarse), blurred, / ROUGH_REF, clipped to [0, 1]
    target = (h - coarse) / RESID_STD                (what the diffusion model generates)
Everything here runs batched on the training device.
"""

from __future__ import annotations

import math

import numpy as np
import torch
import torch.nn.functional as F

LAND_H, LAND_TOP = 500.0, 6000.0
SEA_H, SEA_FLOOR = 1500.0, 6000.0

# Normalisation constants. prep_etopo.py measures them on the real data and writes them to
# <data>/prep_stats.json; train.py reads that file and records them in the exported header.
DEFAULT_STATS = {"resid_std": 0.05, "uplift_ref": 0.35, "rough_ref": 0.06, "coarse_mean": 0.0, "coarse_std": 0.35}


def metres_to_game(m):
    """numpy or torch, elementwise."""
    if isinstance(m, torch.Tensor):
        land = torch.log1p(m.clamp_min(0) / LAND_H) / math.log1p(LAND_TOP / LAND_H)
        sea = -torch.log1p((-m).clamp_min(0) / SEA_H) / math.log1p(SEA_FLOOR / SEA_H)
        return torch.where(m >= 0, land, sea)
    m = np.asarray(m, dtype=np.float32)
    land = np.log1p(np.clip(m, 0, None) / LAND_H) / math.log1p(LAND_TOP / LAND_H)
    sea = -np.log1p(np.clip(-m, 0, None) / SEA_H) / math.log1p(SEA_FLOOR / SEA_H)
    return np.where(m >= 0, land, sea).astype(np.float32)


def _gauss_kernels(sigma: torch.Tensor, radius: int) -> torch.Tensor:
    x = torch.arange(-radius, radius + 1, device=sigma.device, dtype=torch.float32)
    k = torch.exp(-0.5 * (x[None, :] / sigma[:, None].clamp_min(1e-3)) ** 2)
    return k / k.sum(dim=1, keepdim=True)


def blur(x: torch.Tensor, sigma) -> torch.Tensor:
    """Separable Gaussian blur of [B, 1, H, W] with a per-sample (or scalar) sigma, reflect padding."""
    b, c, h, w = x.shape
    assert c == 1
    if not isinstance(sigma, torch.Tensor):
        sigma = torch.full((b,), float(sigma), device=x.device)
    radius = int(math.ceil(3 * float(sigma.max())))
    radius = max(1, min(radius, min(h, w) // 2 - 1))
    k = _gauss_kernels(sigma.float(), radius)  # [B, K]
    y = x.reshape(1, b, h, w)
    y = F.pad(y, (radius, radius, 0, 0), mode="reflect")
    y = F.conv2d(y, k[:, None, None, :], groups=b)
    y = F.pad(y, (0, 0, radius, radius), mode="reflect")
    y = F.conv2d(y, k[:, None, :, None], groups=b)
    return y.reshape(b, 1, h, w)


def coarse_like(x: torch.Tensor, sigma: torch.Tensor) -> torch.Tensor:
    """What the game hands the model: a coarse grid (2 model px per cell) upsampled smoothly."""
    y = F.avg_pool2d(blur(x, sigma), 2)
    return F.interpolate(y, scale_factor=2, mode="bicubic", align_corners=False)


def local_relief(h: torch.Tensor, r: int = 4) -> torch.Tensor:
    k = 2 * r + 1
    hi = F.max_pool2d(F.pad(h, (r, r, r, r), mode="replicate"), k, stride=1)
    lo = -F.max_pool2d(F.pad(-h, (r, r, r, r), mode="replicate"), k, stride=1)
    return hi - lo


def make_pair(metres: torch.Tensor, stats: dict, gen: torch.Generator | None = None, augment: bool = True):
    """metres [B, 1, H, W] -> (x0 target [B,1,H,W], cond [B,4,H,W], parts dict)."""
    b = metres.shape[0]
    dev = metres.device
    h = metres_to_game(metres.float())
    u = lambda lo, hi: lo + (hi - lo) * torch.rand(b, device=dev, generator=gen)  # noqa: E731
    if augment:
        # The game's coarse maps are not blurred real DEMs: vary the relief amplitude and add a
        # gentle low-frequency wobble. Applied to the terrain itself (not just the condition),
        # so the target residual stays pure detail and never asks the model to undo something
        # it cannot see.
        gain = u(0.85, 1.15)[:, None, None, None]
        wobble = blur(torch.randn(h.shape, device=dev, generator=gen), 6.0) * 6.0 * u(0.0, 0.02)[:, None, None, None]
        h = h * gain + wobble
    sigma = u(0.6, 3.0) if augment else torch.full((b,), 1.5, device=dev)
    coarse = coarse_like(h, sigma)
    land = coarse_like((h > 0).float(), sigma).clamp(0, 1)
    resid = h - coarse
    uplift = (blur(local_relief(h), 3.0) / stats["uplift_ref"]).clamp(0, 1)
    uplift = coarse_like(uplift, sigma).clamp(0, 1)
    rough = (blur(resid**2, 3.0).clamp_min(0).sqrt() / stats["rough_ref"]).clamp(0, 1)
    rough = coarse_like(rough, sigma).clamp(0, 1)
    c_in = coarse
    if augment:
        uplift = (uplift + 0.05 * (torch.rand(b, 1, 1, 1, device=dev, generator=gen) - 0.5)).clamp(0, 1)
        rough = (rough + 0.05 * (torch.rand(b, 1, 1, 1, device=dev, generator=gen) - 0.5)).clamp(0, 1)
    cond = torch.cat(
        [
            (c_in - stats["coarse_mean"]) / stats["coarse_std"],
            uplift * 2 - 1,
            land * 2 - 1,
            rough * 2 - 1,
        ],
        dim=1,
    )
    x0 = (h - c_in) / stats["resid_std"]
    return x0, cond, {"h": h, "coarse": c_in, "uplift": uplift, "land": land, "rough": rough}


def dihedral(x: torch.Tensor, k: int) -> torch.Tensor:
    """One of the 8 rotations/flips of the last two dims."""
    if k & 4:
        x = x.flip(-1)
    return torch.rot90(x, k & 3, dims=(-2, -1))


# ---------------------------------------------------------------------------------------
# Synthetic terrain (smoke tests only): fBm continents + ridged ranges, in metres.


def _fbm(rng: np.random.Generator, n: int, octaves: int, base: int, ridged: bool = False) -> np.ndarray:
    out = np.zeros((n, n), np.float32)
    amp, tot = 1.0, 0.0
    for o in range(octaves):
        cells = base * 2**o
        lat = rng.standard_normal((cells + 3, cells + 3)).astype(np.float32)
        t = torch.from_numpy(lat)[None, None]
        up = F.interpolate(t, size=(n + 2 * n // cells, n + 2 * n // cells), mode="bicubic", align_corners=False)[0, 0].numpy()
        layer = up[n // cells : n // cells + n, n // cells : n // cells + n]
        if ridged:
            layer = 1.0 - np.abs(layer)
        out += amp * layer
        tot += amp
        amp *= 0.5
    return out / tot


def synthetic_patches(n: int, size: int = 128, seed: int = 0) -> np.ndarray:
    """n fake DEM patches in metres: continents, coasts, ridged mountains, plains."""
    rng = np.random.default_rng(seed)
    out = np.zeros((n, size, size), np.float32)
    for i in range(n):
        base = _fbm(rng, size, 5, 2)
        ridges = _fbm(rng, size, 4, 3, ridged=True)
        mask = np.clip((_fbm(rng, size, 2, 1) + 0.3) * 2, 0, 1)
        sea = rng.uniform(-0.6, 0.4)
        h = (base - sea * 0.5) * 2500 + mask * np.clip(ridges - 0.4, 0, None) * 6000 * rng.uniform(0.2, 1.0)
        out[i] = np.where(h < 0, h * 1.8, h)
    return out
