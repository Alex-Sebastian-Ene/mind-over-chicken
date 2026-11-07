"""Compare gods on held-out worlds: random, the scripted baseline, and a trained PPO model.

    python rl/evaluate.py --model rl/models/stone_ppo.zip --episodes 60

Every policy plays the same seeds (EVAL_SEED_BASE + i), none of which appear in training.
Besides win rate and return it reports behaviour statistics (what each god does while the
calm challenge runs, when the fire is out, ...) so the learned policy can be described.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np

from civ_env import EVAL_SEED_BASE, NodeEnvClient, RL_DIR

OBS = None  # index lookup filled from the env spec


def run_episodes(policy, n_episodes: int, brain: str = "sim", n_procs: int = 4, seed_base: int = EVAL_SEED_BASE,
                 max_seconds: float = 120.0):
    """`policy(obs_batch) -> list of actions` (ints, or "scripted"). Returns per-episode dicts."""
    global OBS
    n_procs = max(1, min(n_procs, n_episodes))
    clients = [NodeEnvClient(brain=brain, max_seconds=max_seconds) for _ in range(n_procs)]
    spec = clients[0].call({"cmd": "spec"})
    for c in clients[1:]:
        c.call({"cmd": "spec"})
    OBS = {n: i for i, n in enumerate(spec["obs_names"])}
    queue = list(range(seed_base, seed_base + n_episodes))
    obs, beh, results = {}, {}, []

    def start(i):
        clients[i].send({"cmd": "reset", "seed": queue.pop(0)})
        out = clients[i].recv()
        obs[i] = out["obs"]
        beh[i] = np.zeros((2, 2, len(spec["actions"])))  # [fire lit][hold active][action]

    for i in range(n_procs):
        start(i)
    try:
        while obs:
            idx = list(obs)
            acts = policy(np.asarray([obs[i] for i in idx], dtype=np.float32))
            for i, a in zip(idx, acts):
                clients[i].send({"cmd": "step", "action": a if a == "scripted" else int(a)})
            for i in idx:
                out = clients[i].recv()
                o = obs[i]
                beh[i][int(o[OBS["fire_lit"]] > 0.5), int(o[OBS["hold_active"]] > 0.5), out["action"]] += 1
                obs[i] = out["obs"]
                if out["terminated"] or out["truncated"]:
                    info = out["info"]
                    info["behaviour"] = beh[i]
                    results.append(info)
                    if queue:
                        start(i)
                    else:
                        del obs[i]
    finally:
        for c in clients:
            c.close()
    return results, spec


def summarise(name: str, eps: list[dict], actions: list[str]) -> dict:
    wins = [e for e in eps if e["outcome"] == "win"]
    beh = sum(e["behaviour"] for e in eps)
    def share(mask_lit, mask_hold, a):
        n = beh[mask_lit, mask_hold].sum()
        return float(beh[mask_lit, mask_hold, a] / n) if n else None
    total = beh.sum()
    return {
        "policy": name,
        "episodes": len(eps),
        "win_rate": len(wins) / len(eps),
        "lose_rate": sum(e["outcome"] == "lose" for e in eps) / len(eps),
        "mean_return": float(np.mean([e["ret"] for e in eps])),
        "mean_win_seconds": float(np.mean([e["t"] for e in wins])) if wins else None,
        "mean_alive_end": float(np.mean([e["alive"] for e in eps])),
        "mean_deaths": float(np.mean([e["deaths"] for e in eps])),
        "mean_holds": float(np.mean([e["holds"] for e in eps])),
        "mean_bolts": float(np.mean([e["bolts"] for e in eps])),
        "mean_smitten": float(np.mean([e["smitten"] for e in eps])),
        "action_share": {a: float(beh[..., k].sum() / total) for k, a in enumerate(actions)},
        "calm_share_during_hold": share(1, 1, actions.index("calm")),
        "calm_share_outside_hold": (float((beh[0, 0, 3] + beh[1, 0, 3]) / max(1, beh[:, 0].sum()))),
        "fire_strike_share_when_out": share(0, 0, actions.index("strike_fire")),
        "fire_strike_share_when_lit": float((beh[1, :, 1].sum()) / max(1, beh[1].sum())),
        "random_strike_share": float(beh[..., actions.index("strike_random")].sum() / total),
    }


def table(rows: list[dict]) -> str:
    head = "| god | win rate | lose rate | mean return | mean time to win (s) | alive at end | deaths | calm holds | bolts | smitten |"
    sep = "|---|---|---|---|---|---|---|---|---|---|"
    lines = [head, sep]
    for r in rows:
        t = f"{r['mean_win_seconds']:.1f}" if r["mean_win_seconds"] else "-"
        lines.append(f"| {r['policy']} | {r['win_rate']:.0%} | {r['lose_rate']:.0%} | {r['mean_return']:.2f} | {t} | "
                     f"{r['mean_alive_end']:.1f} | {r['mean_deaths']:.2f} | {r['mean_holds']:.2f} | {r['mean_bolts']:.1f} | {r['mean_smitten']:.1f} |")
    return "\n".join(lines)


def compare(model=None, episodes: int = 60, brain: str = "sim", n_procs: int = 4, seed: int = 0):
    rng = np.random.default_rng(seed)
    gods = [("random", lambda o: rng.integers(0, 5, size=len(o)).tolist()),
            ("scripted", lambda o: ["scripted"] * len(o))]
    if model is not None:
        if not hasattr(model, "predict"):
            from stable_baselines3 import PPO

            model = PPO.load(str(model), device="cpu")
        gods.append(("ppo", lambda o: model.predict(o, deterministic=True)[0].tolist()))
    rows = []
    for name, fn in gods:
        eps, spec = run_episodes(fn, episodes, brain=brain, n_procs=n_procs)
        rows.append(summarise(name, eps, spec["actions"]))
        print(f"  evaluated {name}: win {rows[-1]['win_rate']:.0%}, return {rows[-1]['mean_return']:.2f}", flush=True)
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", default=str(RL_DIR / "models" / "stone_ppo.zip"))
    ap.add_argument("--episodes", type=int, default=60)
    ap.add_argument("--brain", default="sim", choices=["sim", "proxy"])
    ap.add_argument("--procs", type=int, default=4)
    ap.add_argument("--out", default=str(RL_DIR / "models" / "eval_results.json"))
    args = ap.parse_args()
    model = args.model if Path(args.model).is_file() else None
    rows = compare(model, args.episodes, args.brain, args.procs)
    print(table(rows))
    Path(args.out).write_text(json.dumps(rows, indent=1), encoding="utf-8")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
