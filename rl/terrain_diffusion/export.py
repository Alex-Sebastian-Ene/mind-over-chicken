"""Export a checkpoint's EMA weights to the `.mocd` file the Rust runtime loads.

    python export.py --ckpt RUN/ckpt.pt --out crates/moc-diffusion/weights/relief.mocd

The header carries the architecture, the normalisation constants (so the runtime maps game
heights in and residuals out exactly as in training), the default step count, provenance and
a self-check: the file is re-read and one forward pass compared against the checkpoint.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import subprocess
from pathlib import Path

import torch

from mocd import read_mocd, state_tensors, write_mocd
from model import COND_NAMES, ReliefUNet, UNetConfig

ETOPO_CITATION = (
    "NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global Relief Model. "
    "https://doi.org/10.25921/fd45-gt74 (CC0-1.0)"
)


def git_sha() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()
    except Exception:
        return "unknown"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--steps", type=int, help="default DDIM steps (default: distilled count, else 8)")
    ap.add_argument("--dtype", default="f16", choices=["f16", "f32"])
    ap.add_argument("--dataset", default=ETOPO_CITATION)
    args = ap.parse_args()

    ck = torch.load(args.ckpt, map_location="cpu", weights_only=False)
    cfg = UNetConfig.from_json(ck["cfg"])
    net = ReliefUNet(cfg)
    net.load_state_dict(ck["ema"])
    net.eval()
    stats = ck["stats"]
    steps = args.steps or ck.get("distilled_steps") or 8
    header = {
        "format": "moc-relief-unet/1",
        "arch": cfg.to_json(),
        "cond": list(COND_NAMES),
        "norm": {
            "coarse_mean": float(stats["coarse_mean"]),
            "coarse_std": float(stats["coarse_std"]),
            "resid_std": float(stats["resid_std"]),
        },
        "sampling": {"steps": int(steps)},
        "meta": {
            "trained_steps": int(ck["step"]),
            "distilled_steps": ck.get("distilled_steps"),
            "train_stats": {k: float(v) for k, v in stats.items()},
            "units": "game heights: land ln(1+m/500)/ln(13), sea -ln(1+|m|/1500)/ln(5)",
            "dataset": args.dataset,
            "git": git_sha(),
            "exported": _dt.datetime.now(_dt.timezone.utc).isoformat(timespec="seconds"),
        },
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    n = write_mocd(args.out, header, state_tensors(net), dtype=args.dtype)

    # Self-check: reload and compare one forward pass (circular padding, as the runtime runs).
    hdr, tensors = read_mocd(args.out)
    back = ReliefUNet(UNetConfig.from_json(hdr["arch"]))
    back.load_state_dict({k: torch.from_numpy(v) for k, v in tensors.items()})
    net.set_padding("circular")
    back.set_padding("circular").eval()
    g = torch.Generator().manual_seed(0)
    x = torch.randn(1, 1, 32, 48, generator=g)
    c = torch.rand(1, cfg.n_cond, 32, 48, generator=g) * 2 - 1
    t = torch.tensor([0.5])
    with torch.no_grad():
        err = (net(x, c, t) - back(x, c, t)).abs().max().item()
    print(f"wrote {args.out} ({n / 1024:.0f} KiB, {args.dtype}), steps {steps}, reload max |diff| {err:.2e}")
    if err > 5e-2:
        raise SystemExit("export self-check failed")


if __name__ == "__main__":
    main()
