"""ReliefUNet: the small conditional UNet behind the learned relief detail (#110).

Everything in this file is mirrored op-for-op by `crates/moc-diffusion/src/unet.rs`; change
one, change the other and re-run `make_fixtures.py` (the Rust parity test will tell you).

Design rules, all for the Rust/wasm runtime:
- Purely local and translation-equivariant: no attention, no GroupNorm/BatchNorm (their
  statistics would depend on the map size). The runtime then runs the net on the whole
  torus with circular padding, so there are no patches and no seams on either axis.
- Cheap at full resolution: the input is space-to-depth'd by 2 straight away, so the widest
  3x3 convs run at a quarter of the pixels. Downsampling is space-to-depth + 1x1 conv, and
  upsampling is a 1x1 conv at low resolution then nearest x2 + skip add.
- Conditioning (coarse heights, uplift, land, roughness) is concatenated at the input. Its
  share of `conv_in` does not depend on the step, so the runtime computes it once per map.
"""

from __future__ import annotations

import math
from dataclasses import asdict, dataclass, field

import torch
import torch.nn.functional as F
from torch import nn

COND_NAMES = ("coarse", "uplift", "land", "rough")


@dataclass
class UNetConfig:
    n_cond: int = len(COND_NAMES)
    s2d: int = 2  # fixed: the runtime implements factor 2 only
    widths: tuple = (24, 32, 48)
    mid_blocks: int = 2
    time_freqs: int = 16
    time_dim: int = 64

    def to_json(self) -> dict:
        d = asdict(self)
        d["widths"] = list(self.widths)
        return d

    @staticmethod
    def from_json(d: dict) -> "UNetConfig":
        d = dict(d)
        d["widths"] = tuple(d["widths"])
        return UNetConfig(**d)

    @property
    def stride(self) -> int:
        """Map sides must be multiples of this (space-to-depth, then two downsamples)."""
        return self.s2d * 4


def time_features(t: torch.Tensor, freqs: int) -> torch.Tensor:
    """Sinusoidal features of t in [0, 1]. Rust: `unet::time_features`."""
    k = torch.arange(freqs, dtype=torch.float32, device=t.device)
    f = torch.exp(-math.log(1000.0) * k / freqs)
    a = (t.float()[:, None] * 1000.0) * f[None, :]
    return torch.cat([torch.sin(a), torch.cos(a)], dim=1)


class ResBlock(nn.Module):
    """h + conv2(silu(film(conv1(silu(h)), temb))), FiLM = a * (1 + scale) + shift."""

    def __init__(self, c: int, tdim: int):
        super().__init__()
        self.conv1 = nn.Conv2d(c, c, 3, padding=1)
        self.film = nn.Linear(tdim, 2 * c)
        self.conv2 = nn.Conv2d(c, c, 3, padding=1)
        # Start as the identity: stable without normalisation layers.
        nn.init.zeros_(self.conv2.weight)
        nn.init.zeros_(self.conv2.bias)
        nn.init.zeros_(self.film.weight)
        nn.init.zeros_(self.film.bias)

    def forward(self, h: torch.Tensor, temb: torch.Tensor) -> torch.Tensor:
        a = self.conv1(F.silu(h))
        scale, shift = self.film(temb).chunk(2, dim=1)
        a = a * (1.0 + scale[:, :, None, None]) + shift[:, :, None, None]
        return h + self.conv2(F.silu(a))


class ReliefUNet(nn.Module):
    """v-prediction network: (x_t, cond, t) -> v, all at the model grid (H, W multiples of 8)."""

    def __init__(self, cfg: UNetConfig | None = None):
        super().__init__()
        cfg = cfg or UNetConfig()
        assert cfg.s2d == 2, "the runtime implements space-to-depth 2 only"
        assert len(cfg.widths) == 3, "the runtime implements three levels"
        self.cfg = cfg
        c0, c1, c2 = cfg.widths
        r2 = cfg.s2d * cfg.s2d
        td = cfg.time_dim
        self.time_l1 = nn.Linear(2 * cfg.time_freqs, td)
        self.time_l2 = nn.Linear(td, td)
        self.conv_in = nn.Conv2d(r2 * (1 + cfg.n_cond), c0, 3, padding=1)
        self.enc0 = ResBlock(c0, td)
        self.down0 = nn.Conv2d(4 * c0, c1, 1)
        self.enc1 = ResBlock(c1, td)
        self.down1 = nn.Conv2d(4 * c1, c2, 1)
        self.mid = nn.ModuleList([ResBlock(c2, td) for _ in range(cfg.mid_blocks)])
        self.up1 = nn.Conv2d(c2, c1, 1)
        self.dec1 = ResBlock(c1, td)
        self.up0 = nn.Conv2d(c1, c0, 1)
        self.dec0 = ResBlock(c0, td)
        self.conv_out = nn.Conv2d(c0, r2, 3, padding=1)
        nn.init.zeros_(self.conv_out.weight)
        nn.init.zeros_(self.conv_out.bias)

    def set_padding(self, mode: str) -> "ReliefUNet":
        """'zeros' for training on patches, 'circular' to match the runtime's torus."""
        for m in self.modules():
            if isinstance(m, nn.Conv2d) and m.kernel_size == (3, 3):
                m.padding_mode = mode
        return self

    def forward(self, x_t: torch.Tensor, cond: torch.Tensor, t: torch.Tensor) -> torch.Tensor:
        cfg = self.cfg
        temb = F.silu(self.time_l2(F.silu(self.time_l1(time_features(t, cfg.time_freqs)))))
        x = torch.cat([x_t, cond], dim=1)
        h = self.conv_in(F.pixel_unshuffle(x, cfg.s2d))
        e0 = self.enc0(h, temb)
        h = self.down0(F.pixel_unshuffle(e0, 2))
        e1 = self.enc1(h, temb)
        h = self.down1(F.pixel_unshuffle(e1, 2))
        for blk in self.mid:
            h = blk(h, temb)
        h = F.interpolate(self.up1(h), scale_factor=2, mode="nearest") + e1
        h = self.dec1(h, temb)
        h = F.interpolate(self.up0(h), scale_factor=2, mode="nearest") + e0
        h = self.dec0(h, temb)
        return F.pixel_shuffle(self.conv_out(F.silu(h)), cfg.s2d)

    def macs_per_pixel(self) -> float:
        """Multiply-adds per model-grid pixel per denoising step (what the runtime pays)."""
        cfg = self.cfg
        c0, c1, c2 = cfg.widths
        q = 1 / (cfg.s2d**2)  # level-0 pixels per model pixel
        n = 0.0
        n += 9 * 4 * c0 * q  # conv_in, x_t share only (the cond share is computed once)
        res = lambda c, a: 2 * 9 * c * c * a  # noqa: E731
        n += res(c0, q) + 4 * c0 * c1 * q / 4 + res(c1, q / 4) + 4 * c1 * c2 * q / 16
        n += cfg.mid_blocks * res(c2, q / 16)
        n += c2 * c1 * q / 16 + res(c1, q / 4) + c1 * c0 * q / 4 + res(c0, q)
        n += 9 * c0 * 4 * q
        return n


def param_count(m: nn.Module) -> int:
    return sum(p.numel() for p in m.parameters())


if __name__ == "__main__":
    for w in [(16, 24, 32), (24, 32, 48), (32, 48, 64)]:
        m = ReliefUNet(UNetConfig(widths=w))
        print(w, "params", param_count(m), "MACs/px/step", round(m.macs_per_pixel()))
