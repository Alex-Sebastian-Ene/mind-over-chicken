"""Cosine VP schedule, v-prediction loss and the few-step DDIM sampler.

Mirrored by `crates/moc-diffusion/src/sampler.rs`:
    alpha(t) = cos(pi/2 t), sigma(t) = sin(pi/2 t), t in [0, 1], x_1 = pure noise.
    x_t = alpha x0 + sigma eps,   v = alpha eps - sigma x0.
DDIM (eta = 0) on the uniform grid t_k = 1 - k/K, k = 0..K:
    x0^ = alpha_k x - sigma_k v,  eps^ = sigma_k x + alpha_k v,  x <- alpha_{k+1} x0^ + sigma_{k+1} eps^
The final x (t = 0) is the generated, normalised residual.
"""

from __future__ import annotations

import math

import torch


def alpha_sigma(t: torch.Tensor):
    a = torch.cos(0.5 * math.pi * t)
    s = torch.sin(0.5 * math.pi * t)
    return a, s


def v_loss(model, x0: torch.Tensor, cond: torch.Tensor, weight: torch.Tensor | None = None):
    """Standard v-prediction MSE with t ~ U(0, 1). `weight` is an optional per-pixel mask."""
    b = x0.shape[0]
    t = torch.rand(b, device=x0.device)
    eps = torch.randn_like(x0)
    a, s = alpha_sigma(t)
    a = a[:, None, None, None]
    s = s[:, None, None, None]
    x_t = a * x0 + s * eps
    v = a * eps - s * x0
    pred = model(x_t, cond, t)
    err = (pred.float() - v) ** 2
    if weight is not None:
        return (err * weight).sum() / weight.sum().clamp_min(1.0)
    return err.mean()


def ddim_times(steps: int) -> list[float]:
    return [1.0 - k / steps for k in range(steps + 1)]


@torch.no_grad()
def ddim_sample(model, cond: torch.Tensor, steps: int = 6, noise: torch.Tensor | None = None,
                generator: torch.Generator | None = None) -> torch.Tensor:
    b, _, h, w = cond.shape
    x = noise if noise is not None else torch.randn(b, 1, h, w, device=cond.device, generator=generator)
    ts = ddim_times(steps)
    for k in range(steps):
        t = torch.full((b,), ts[k], device=cond.device)
        v = model(x, cond, t).float()
        a, s = alpha_sigma(torch.tensor(ts[k], dtype=torch.float32))
        a2, s2 = alpha_sigma(torch.tensor(ts[k + 1], dtype=torch.float32))
        x0 = a * x - s * v
        eps = s * x + a * v
        x = a2 * x0 + s2 * eps
    return x
