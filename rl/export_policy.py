"""Export the actor half of a stable-baselines3 PPO MlpPolicy to JSON for src/ai/policy.js.

    python rl/export_policy.py --model rl/models/stone_ppo.zip --out src/ai/stone_policy.json

Only the policy network (mlp_extractor.policy_net + action_net) is exported; the value
head is not needed to act. The JSON also carries the observation/action names (from the
env server, so they always match src/ai/observe.js) and a few probe observations with the
probabilities PyTorch produced, which MLPPolicy.selfCheck() compares against.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
from pathlib import Path

import numpy as np
import torch
from torch import nn

RL_DIR = Path(__file__).resolve().parent
REPO = RL_DIR.parent
DEFAULT_OUT = REPO / "src" / "ai" / "stone_policy.json"
DEFAULT_MODEL = RL_DIR / "models" / "stone_ppo.zip"

_ACT_NAMES = {nn.Tanh: "tanh", nn.ReLU: "relu", nn.Identity: "identity"}


def _round(a: np.ndarray, digits: int = 6):
    return np.round(a.astype(np.float64), digits).tolist()


def export_policy(model, out: Path = DEFAULT_OUT, spec: dict | None = None, meta: dict | None = None) -> Path:
    """`model` is a loaded PPO model or a path to its .zip."""
    if not hasattr(model, "policy"):
        from stable_baselines3 import PPO

        model = PPO.load(str(model), device="cpu")
    pol = model.policy.to("cpu").eval()

    layers, act_name = [], None
    for m in pol.mlp_extractor.policy_net:
        if isinstance(m, nn.Linear):
            layers.append(m)
        elif type(m) in _ACT_NAMES:
            name = _ACT_NAMES[type(m)]
            if act_name not in (None, name):
                raise ValueError("mixed activations are not supported")
            act_name = name
        else:
            raise ValueError(f"unsupported layer {m}")
    layers.append(pol.action_net)
    out_layers = [{"w": _round(l.weight.detach().numpy()), "b": _round(l.bias.detach().numpy())} for l in layers]

    if spec is None:
        from civ_env import NodeEnvClient

        c = NodeEnvClient(brain="proxy")
        spec = c.call({"cmd": "spec"})
        c.close()

    # Probe observations: a cold start, a fading fire, and random states.
    n_in = layers[0].in_features
    rng = np.random.default_rng(0)
    probes = [np.zeros(n_in), np.full(n_in, 0.5)] + [rng.uniform(0, 1, n_in) for _ in range(4)]
    with torch.no_grad():
        x = torch.as_tensor(np.asarray(probes), dtype=torch.float32)
        dist = pol.get_distribution(x)
        probs = dist.distribution.probs.numpy()

    doc = {
        "format": "mlp-policy/1",
        "game": "mind-over-chicken",
        "scene": "stone",
        "obs_version": spec.get("obs_version"),
        "obs_names": spec["obs_names"],
        "actions": spec["actions"],
        "decision_dt": spec["decision_dt"],
        "activation": act_name or "tanh",
        "layers": out_layers,
        "meta": {"exported": _dt.datetime.now().isoformat(timespec="seconds"), **(meta or {})},
        "check": {"obs": _round(np.asarray(probes), 6), "probs": _round(probs, 6)},
    }
    if len(spec["obs_names"]) != n_in:
        raise ValueError(f"model expects {n_in} inputs but the env has {len(spec['obs_names'])}")
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, separators=(",", ":")), encoding="utf-8")
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", default=str(DEFAULT_MODEL))
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    args = ap.parse_args()
    out = export_policy(args.model, Path(args.out))
    print(f"wrote {out} ({out.stat().st_size / 1024:.1f} KB)")


if __name__ == "__main__":
    main()
