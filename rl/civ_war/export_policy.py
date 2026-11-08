"""Export the actor of a (Maskable)PPO model to the `MOCP` binary format that
`crates/moc-civ/src/policy_rt.rs` runs (in the env server's league, the Rust evaluation and the
browser's wasm).

    python rl/civ_war/export_policy.py --model rl/civ_war/models/council.zip --kind council \
        --out crates/moc-civ/policy/council.mocp

Only the policy half (mlp_extractor.policy_net + action_net) is exported. The file carries a few
probe observations with PyTorch's logits; the Rust loader recomputes them and rejects the file
if they differ by more than 1e-3 (then the game falls back to the rules).
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import numpy as np
import torch
from torch import nn

KINDS = {"council": 0, "commander": 1}
ACTS = {nn.Tanh: 0, nn.ReLU: 1}


def _load(model):
    if hasattr(model, "policy"):
        return model
    from sb3_contrib import MaskablePPO

    return MaskablePPO.load(str(model), device="cpu")


def export_policy(model, out: Path, kind: str, obs_version: int, n_checks: int = 6) -> Path:
    model = _load(model)
    pol = model.policy.to("cpu").eval()
    layers, act = [], None
    for m in pol.mlp_extractor.policy_net:
        if isinstance(m, nn.Linear):
            layers.append(m)
        elif type(m) in ACTS:
            if act not in (None, ACTS[type(m)]):
                raise ValueError("mixed activations are not supported")
            act = ACTS[type(m)]
        else:
            raise ValueError(f"unsupported layer {m}")
    layers.append(pol.action_net)
    nvec = [int(n) for n in model.action_space.nvec]
    n_obs = layers[0].in_features
    if layers[-1].out_features != sum(nvec):
        raise ValueError("action head size does not match the action space")

    rng = np.random.default_rng(0)
    probes = [np.zeros(n_obs), np.full(n_obs, 0.5)] + [rng.uniform(0, 1, n_obs) for _ in range(n_checks - 2)]
    with torch.no_grad():
        x = torch.as_tensor(np.asarray(probes), dtype=torch.float32)
        logits = pol.action_net(pol.mlp_extractor.forward_actor(pol.extract_features(x, pol.pi_features_extractor))).numpy()

    b = bytearray()
    u32 = lambda v: b.extend(struct.pack("<I", int(v)))  # noqa: E731
    f32s = lambda a: b.extend(np.asarray(a, dtype="<f4").tobytes())  # noqa: E731
    b.extend(b"MOCP")
    u32(1)
    u32(KINDS[kind])
    u32(obs_version)
    u32(n_obs)
    u32(len(nvec))
    for n in nvec:
        u32(n)
    u32(act or 0)
    u32(len(layers))
    for l in layers:
        u32(l.in_features)
        u32(l.out_features)
    for l in layers:
        f32s(l.weight.detach().numpy().reshape(-1))
        f32s(l.bias.detach().numpy())
    u32(len(probes))
    for p, z in zip(probes, logits):
        f32s(p)
        f32s(z)
    b.extend(b"END!")
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(bytes(b))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", required=True)
    ap.add_argument("--kind", choices=list(KINDS), required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--obs-version", type=int, default=1)
    a = ap.parse_args()
    out = export_policy(a.model, Path(a.out), a.kind, a.obs_version)
    print(f"wrote {out} ({out.stat().st_size / 1024:.1f} KB)")


if __name__ == "__main__":
    main()
