"""Write the Rust parity fixtures: a tiny ReliefUNet with random (synthetic) weights and the
outputs PyTorch produces for it.

    python rl/terrain_diffusion/make_fixtures.py

Writes crates/moc-diffusion/tests/fixtures/{tiny.mocd, tiny_ref.mocd}:
- tiny.mocd: the weights as float16, exactly the format a trained model ships in;
- tiny_ref.mocd (float32): x_t, cond, t -> v from one forward pass with circular padding, and
  a full DDIM run (noise -> x0). PyTorch runs on the float16-rounded weights, so the Rust
  runtime (which widens f16 on load) must match to float32 rounding.
"""

from __future__ import annotations

import math
from pathlib import Path

import numpy as np
import torch

from diffusion import ddim_sample
from mocd import state_tensors, write_mocd
from model import ReliefUNet, UNetConfig

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "crates" / "moc-diffusion" / "tests" / "fixtures"

TINY = UNetConfig(widths=(8, 12, 16), mid_blocks=2, time_freqs=8, time_dim=16)
DDIM_STEPS = 3


def tiny_header(cfg: UNetConfig) -> dict:
    return {
        "format": "moc-relief-unet/1",
        "arch": cfg.to_json(),
        "norm": {"coarse_mean": 0.0, "coarse_std": 0.35, "resid_std": 0.05},
        "sampling": {"steps": 4},
        "meta": {"note": "synthetic random weights for parity tests; not a trained model"},
    }


def main():
    torch.manual_seed(1234)
    model = ReliefUNet(TINY)
    with torch.no_grad():
        for name, p in model.named_parameters():
            if p.dim() > 1:
                p.normal_(0.0, 1.0 / math.sqrt(p[0].numel()))
            else:
                p.normal_(0.0, 0.1)
            p.copy_(p.half().float())  # what the runtime sees after loading f16
    model.set_padding("circular").eval()

    OUT.mkdir(parents=True, exist_ok=True)
    n = write_mocd(OUT / "tiny.mocd", tiny_header(TINY), state_tensors(model), dtype="f16")

    g = torch.Generator().manual_seed(99)
    h, w = 24, 40  # not square, so a transposed index shows up
    x_t = torch.randn(1, 1, h, w, generator=g)
    cond = torch.rand(1, TINY.n_cond, h, w, generator=g) * 2 - 1
    t = torch.tensor([0.63])
    noise = torch.randn(1, 1, h, w, generator=g)
    with torch.no_grad():
        v = model(x_t, cond, t)
        x0 = ddim_sample(model, cond, steps=DDIM_STEPS, noise=noise)
    ref = [
        ("x_t", x_t[0, 0].numpy()),
        ("cond", cond[0].numpy()),
        ("t", t.numpy()),
        ("v", v[0, 0].numpy()),
        ("noise", noise[0, 0].numpy()),
        ("ddim_x0", x0[0, 0].numpy()),
    ]
    m = write_mocd(OUT / "tiny_ref.mocd", {"format": "moc-relief-ref/1", "ddim_steps": DDIM_STEPS}, ref, dtype="f32")
    print(f"tiny.mocd {n} B, tiny_ref.mocd {m} B, |v| mean {v.abs().mean():.4f}, |x0| mean {x0.abs().mean():.4f}")


if __name__ == "__main__":
    main()
