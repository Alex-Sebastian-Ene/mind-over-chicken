"""Train the ReliefUNet (teacher), or distil it to fewer sampling steps (student).

    # teacher: v-prediction DDPM on balanced real-terrain patches
    python train.py --data DATA --out RUN --steps 150000
    # progressive distillation (Salimans & Ho 2022): halve the DDIM steps per round
    python train.py --data DATA --out RUN_d8 --distill-from RUN/ckpt.pt --student-steps 8 --steps 20000
    python train.py --data DATA --out RUN_d4 --distill-from RUN_d8/ckpt.pt --student-steps 4 --steps 20000

Checkpoints (`RUN/ckpt.pt`, atomic) every --ckpt-every steps and on SIGTERM/SIGUSR1 (Slurm
sends one before the time limit), and the run resumes from it automatically. Metrics go to
RUN/metrics.jsonl, previews (coarse | real | sampled) to RUN/preview_XXXXXX.png.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import os
import signal
import time
from pathlib import Path

import numpy as np
import torch

from data import DEFAULT_STATS, make_pair, synthetic_patches
from diffusion import alpha_sigma, ddim_sample, v_loss
from model import ReliefUNet, UNetConfig, param_count
from png import save_rgb, terrain_rgb


class Patches:
    """Class-balanced sampler over prep_etopo.py output: class uniformly (weights optional),
    then scale uniformly within the class, then a patch; random crop and dihedral flip."""

    def __init__(self, data: Path | None, split: str, class_weights: dict | None = None, synthetic: int = 0):
        if data is None:
            self.x = synthetic_patches(synthetic, 128, seed=0 if split == "train" else 1).astype(np.float16)
            cls = np.zeros(len(self.x), np.int64)
            scale = np.zeros(len(self.x), np.float32)
            names = ["synthetic"]
            self.stats = dict(DEFAULT_STATS)
        else:
            self.x = np.load(data / "patches.npy", mmap_mode="r")
            ix = np.load(data / "index.npz")
            keep = ix["val"] if split == "val" else ~ix["val"]
            sel = np.flatnonzero(keep)
            self.x_index = sel
            cls, scale = ix["cls"][sel], ix["scale"][sel]
            prep = json.loads((data / "prep_stats.json").read_text())
            names = prep["classes"]
            self.stats = {**DEFAULT_STATS, **prep["stats"]}
        if not hasattr(self, "x_index"):
            self.x_index = np.arange(len(self.x))
        self.groups = []  # per class: list of index arrays per scale
        weights = []
        for c in np.unique(cls):
            m = cls == c
            self.groups.append([np.flatnonzero(m & (scale == s)) for s in np.unique(scale[m])])
            weights.append((class_weights or {}).get(names[c], 1.0))
        w = np.asarray(weights, np.float64)
        self.class_p = w / w.sum()
        self.names = [names[c] for c in np.unique(cls)]

    def __len__(self):
        return len(self.x_index)

    def batch(self, n: int, crop: int, rng: np.random.Generator) -> torch.Tensor:
        out = np.empty((n, 1, crop, crop), np.float32)
        cs = rng.choice(len(self.groups), size=n, p=self.class_p)
        for i, c in enumerate(cs):
            g = self.groups[c][rng.integers(len(self.groups[c]))]
            j = self.x_index[g[rng.integers(len(g))]]
            p = self.x[j]
            y0, x0 = rng.integers(0, p.shape[0] - crop + 1, size=2)
            k = int(rng.integers(8))  # one of the 8 rotations/flips, per sample
            c = np.rot90(np.asarray(p[y0 : y0 + crop, x0 : x0 + crop], np.float32), k & 3)
            out[i, 0] = c[:, ::-1] if k & 4 else c
        return torch.from_numpy(out)


def save_ckpt(path: Path, state: dict):
    tmp = path.with_suffix(".tmp")
    torch.save(state, tmp)
    os.replace(tmp, path)


def preview(net, batch_m: torch.Tensor, stats: dict, steps: int, path: Path, device):
    net.eval()
    with torch.no_grad():
        g = torch.Generator(device="cpu").manual_seed(0)
        x0, cond, parts = make_pair(batch_m.to(device), stats, augment=False)
        noise = torch.randn(x0.shape, generator=g).to(device)
        xs = ddim_sample(net, cond, steps=steps, noise=noise)
        sample = parts["coarse"] + xs * stats["resid_std"]
    rows = []
    for i in range(min(8, batch_m.shape[0])):
        tiles = [terrain_rgb(parts["coarse"][i, 0].cpu().numpy()), terrain_rgb(parts["h"][i, 0].cpu().numpy()), terrain_rgb(sample[i, 0].float().cpu().numpy())]
        rows.append(np.concatenate([np.concatenate(tiles, axis=1)], axis=1))
    save_rgb(path, np.concatenate(rows, axis=0))
    net.train()


def distill_loss(student, teacher, x0, cond, n_student: int):
    """One progressive-distillation step: the student's single DDIM step from t to t - 1/N
    must land where two teacher steps (t -> t - 1/2N -> t - 1/N) land."""
    b = x0.shape[0]
    dev = x0.device
    i = torch.randint(1, n_student + 1, (b,), device=dev)
    t = i.float() / n_student
    t_mid = t - 0.5 / n_student
    t_end = t - 1.0 / n_student
    a, s = alpha_sigma(t)
    am, sm = alpha_sigma(t_mid)
    ae, se = alpha_sigma(t_end)
    sh = lambda v: v[:, None, None, None]  # noqa: E731
    eps = torch.randn_like(x0)
    z = sh(a) * x0 + sh(s) * eps
    with torch.no_grad():
        v1 = teacher(z, cond, t).float()
        x1 = sh(a) * z - sh(s) * v1
        e1 = sh(s) * z + sh(a) * v1
        zm = sh(am) * x1 + sh(sm) * e1
        v2 = teacher(zm, cond, t_mid).float()
        x2 = sh(am) * zm - sh(sm) * v2
        e2 = sh(sm) * zm + sh(am) * v2
        ze = sh(ae) * x2 + sh(se) * e2
        # The x-prediction a single DDIM step t -> t_end needs to reach ze.
        ratio = sh(se / s)
        x_tgt = (ze - ratio * z) / (sh(ae) - ratio * sh(a))
    v = student(z, cond, t).float()
    x_hat = sh(a) * z - sh(s) * v
    w = sh(torch.clamp(a**2 / s.clamp_min(1e-4) ** 2, min=1.0, max=10.0))  # truncated SNR weighting
    return (w * (x_hat - x_tgt) ** 2).mean()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", type=Path, help="prep_etopo.py output directory")
    ap.add_argument("--synthetic", type=int, default=0, help="train on N synthetic patches instead (smoke test)")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--steps", type=int, default=150_000)
    ap.add_argument("--batch", type=int, default=96)
    ap.add_argument("--crop", type=int, default=96)
    ap.add_argument("--widths", default="24,32,48")
    ap.add_argument("--mid-blocks", type=int, default=2)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--warmup", type=int, default=2000)
    ap.add_argument("--ema", type=float, default=0.999)
    ap.add_argument("--ckpt-every", type=int, default=2000)
    ap.add_argument("--log-every", type=int, default=100)
    ap.add_argument("--preview-every", type=int, default=5000)
    ap.add_argument("--preview-steps", type=int, default=8)
    ap.add_argument("--ocean-weight", type=float, default=0.5)
    ap.add_argument("--distill-from", type=Path, help="teacher checkpoint: run progressive distillation")
    ap.add_argument("--student-steps", type=int, default=8)
    ap.add_argument("--max-minutes", type=float, default=0, help="stop cleanly after this long (0 = no limit)")
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--seed", type=int, default=0)
    args = ap.parse_args()
    t_start = time.time()
    args.out.mkdir(parents=True, exist_ok=True)
    dev = torch.device(args.device)
    torch.manual_seed(args.seed)
    if dev.type == "cuda":
        torch.backends.cudnn.benchmark = True
        torch.backends.cuda.matmul.allow_tf32 = True
        torch.backends.cudnn.allow_tf32 = True

    cw = {"ocean": args.ocean_weight}
    train_set = Patches(args.data, "train", cw, synthetic=args.synthetic)
    val_set = Patches(args.data, "val", cw, synthetic=max(16, args.synthetic // 8))
    stats = train_set.stats
    print(f"train patches {len(train_set)}, val {len(val_set)}, classes {train_set.names}, stats {stats}")

    teacher = None
    if args.distill_from:
        tk = torch.load(args.distill_from, map_location="cpu", weights_only=False)
        cfg = UNetConfig.from_json(tk["cfg"])
        teacher = ReliefUNet(cfg)
        teacher.load_state_dict(tk["ema"])
        teacher.to(dev).eval().requires_grad_(False)
        net = ReliefUNet(cfg)
        net.load_state_dict(tk["ema"])
    else:
        cfg = UNetConfig(widths=tuple(int(c) for c in args.widths.split(",")), mid_blocks=args.mid_blocks)
        net = ReliefUNet(cfg)
    net.to(dev)
    ema = copy.deepcopy(net).eval().requires_grad_(False)
    opt = torch.optim.AdamW(net.parameters(), lr=args.lr, weight_decay=0.01, betas=(0.9, 0.99))
    print(f"params {param_count(net)}, MACs/px/step {net.macs_per_pixel():.0f}, device {dev}")

    step = 0
    ck = args.out / "ckpt.pt"
    if ck.exists():
        st = torch.load(ck, map_location="cpu", weights_only=False)
        net.load_state_dict(st["net"])
        ema.load_state_dict(st["ema"])
        opt.load_state_dict(st["opt"])
        step = st["step"]
        print(f"resumed at step {step}")

    stop = {"flag": False}

    def on_signal(signum, _frame):
        print(f"signal {signum}: checkpointing and exiting")
        stop["flag"] = True

    for sig in (signal.SIGTERM, getattr(signal, "SIGUSR1", None)):
        if sig is not None:
            signal.signal(sig, on_signal)

    def state():
        return {
            "net": net.state_dict(), "ema": ema.state_dict(), "opt": opt.state_dict(), "step": step,
            "cfg": cfg.to_json(), "stats": stats, "args": {k: str(v) for k, v in vars(args).items()},
            "distilled_steps": args.student_steps if teacher is not None else None,
        }

    rng = np.random.default_rng(args.seed + step)
    vrng = np.random.default_rng(1234)
    val_batch = val_set.batch(min(64, args.batch), args.crop, vrng)
    prev_batch = val_set.batch(8, args.crop, np.random.default_rng(7))
    use_bf16 = dev.type == "cuda" and torch.cuda.is_bf16_supported()
    metrics = open(args.out / "metrics.jsonl", "a")
    t_log, loss_acc, n_acc = time.time(), 0.0, 0
    net.train()
    while step < args.steps and not stop["flag"]:
        lr = args.lr * min(1.0, (step + 1) / args.warmup) * (0.5 * (1 + math.cos(math.pi * min(1.0, step / args.steps))) * 0.9 + 0.1)
        for gp in opt.param_groups:
            gp["lr"] = lr
        m = train_set.batch(args.batch, args.crop, rng).to(dev, non_blocking=True)
        with torch.no_grad():
            x0, cond, _ = make_pair(m, stats, augment=True)
        with torch.autocast(device_type=dev.type, dtype=torch.bfloat16, enabled=use_bf16):
            if teacher is None:
                loss = v_loss(net, x0, cond)
            else:
                loss = distill_loss(net, teacher, x0, cond, args.student_steps)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
        opt.step()
        with torch.no_grad():
            d = min(args.ema, (1 + step) / (10 + step))
            for pe, pn in zip(ema.parameters(), net.parameters()):
                pe.lerp_(pn, 1 - d)
        step += 1
        loss_acc += float(loss)
        n_acc += 1
        if step % args.log_every == 0:
            with torch.no_grad(), torch.autocast(device_type=dev.type, dtype=torch.bfloat16, enabled=use_bf16):
                torch.manual_seed(0)  # same noise and t for every validation pass
                vx0, vcond, _ = make_pair(val_batch.to(dev), stats, gen=None, augment=False)
                vl = float(v_loss(ema, vx0, vcond)) if teacher is None else float(distill_loss(ema, teacher, vx0, vcond, args.student_steps))
                torch.manual_seed(args.seed + step)
            rec = {"step": step, "loss": loss_acc / n_acc, "val": vl, "lr": lr, "sec_per_step": (time.time() - t_log) / n_acc, "elapsed_min": (time.time() - t_start) / 60}
            print(json.dumps(rec), flush=True)
            metrics.write(json.dumps(rec) + "\n")
            metrics.flush()
            t_log, loss_acc, n_acc = time.time(), 0.0, 0
        if step % args.ckpt_every == 0:
            save_ckpt(ck, state())
        if args.preview_every and step % args.preview_every == 0:
            ps = args.student_steps if teacher is not None else args.preview_steps
            preview(ema, prev_batch, stats, ps, args.out / f"preview_{step:06d}.png", dev)
        if args.max_minutes and (time.time() - t_start) / 60 > args.max_minutes:
            print("time budget reached")
            break
    save_ckpt(ck, state())
    print(f"saved step {step} to {ck}")


if __name__ == "__main__":
    main()
