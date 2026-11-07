"""Train the Stone Age AI god with PPO, evaluate it against baselines, export it for the browser.

    python rl/train_ppo.py --timesteps 300000 --n-envs 8

Outputs
  rl/models/stone_ppo.zip          the SB3 model (actor + critic)
  rl/models/eval_results.json      random vs scripted vs PPO on held-out worlds
  rl/runs/<name>/progress.csv      training curve (gitignored)
  src/ai/stone_policy.json         actor weights for src/ai/policy.js
"""

from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
import time
from collections import deque
from pathlib import Path

import numpy as np
import torch
from stable_baselines3 import PPO
from stable_baselines3.common.callbacks import BaseCallback
from stable_baselines3.common.vec_env import VecMonitor

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ_env import REPO, RL_DIR, StoneAgeVecEnv  # noqa: E402
from evaluate import compare, table  # noqa: E402
from export_policy import DEFAULT_OUT, export_policy  # noqa: E402


class GodLogger(BaseCallback):
    """Prints and records rolling win rate / return over the last `window` episodes."""

    def __init__(self, csv_path: Path, window: int = 100, every: int = 1):
        super().__init__()
        self.csv_path, self.every = csv_path, every
        self.eps = deque(maxlen=window)
        self.n_eps = 0
        self.rollouts = 0
        self.t0 = time.time()
        self.rows = []

    def _on_step(self) -> bool:
        for done, info in zip(self.locals["dones"], self.locals["infos"]):
            if done:
                self.eps.append(info)
                self.n_eps += 1
        return True

    def _on_rollout_end(self) -> None:
        self.rollouts += 1
        if not self.eps or self.rollouts % self.every:
            return
        eps = list(self.eps)
        wins = [e for e in eps if e.get("outcome") == "win"]
        row = {
            "timesteps": self.num_timesteps,
            "minutes": round((time.time() - self.t0) / 60, 2),
            "episodes": self.n_eps,
            "mean_return": round(float(np.mean([e["ret"] for e in eps])), 3),
            "win_rate": round(len(wins) / len(eps), 3),
            "lose_rate": round(sum(e.get("outcome") == "lose" for e in eps) / len(eps), 3),
            "mean_win_seconds": round(float(np.mean([e["t"] for e in wins])), 2) if wins else "",
            "mean_holds": round(float(np.mean([e["holds"] for e in eps])), 2),
            "steps_per_sec": round(self.num_timesteps / max(1e-6, time.time() - self.t0)),
        }
        self.rows.append(row)
        self.logger.record("god/win_rate", row["win_rate"])
        self.logger.record("god/mean_return", row["mean_return"])
        print(f"[{row['minutes']:5.1f} min] {row['timesteps']:>7} steps  eps {row['episodes']:>5}  "
              f"return {row['mean_return']:7.2f}  win {row['win_rate']:.0%}  lose {row['lose_rate']:.0%}  "
              f"win@ {row['mean_win_seconds'] or '-':>5}s  holds {row['mean_holds']:.2f}  {row['steps_per_sec']} sps",
              flush=True)

    def _on_training_end(self) -> None:
        if not self.rows:
            return
        with open(self.csv_path, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(self.rows[0]))
            w.writeheader()
            w.writerows(self.rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--timesteps", type=int, default=300_000)
    ap.add_argument("--n-envs", type=int, default=8)
    ap.add_argument("--brain", default="sim", choices=["sim", "proxy"],
                    help="sim = the real Brain + emulated headset (default), proxy = fast stand-in")
    ap.add_argument("--n-steps", type=int, default=128, help="rollout length per env")
    ap.add_argument("--batch-size", type=int, default=256)
    ap.add_argument("--epochs", type=int, default=10)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--ent-coef", type=float, default=0.01)
    ap.add_argument("--gamma", type=float, default=0.99)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--name", default="stone_ppo")
    ap.add_argument("--eval-episodes", type=int, default=60)
    ap.add_argument("--no-eval", action="store_true")
    ap.add_argument("--export", default=str(DEFAULT_OUT))
    args = ap.parse_args()

    torch.set_num_threads(1)  # leave the cores to the Node simulations
    run_dir = RL_DIR / "runs" / args.name
    run_dir.mkdir(parents=True, exist_ok=True)
    model_dir = RL_DIR / "models"
    model_dir.mkdir(parents=True, exist_ok=True)

    venv = VecMonitor(StoneAgeVecEnv(n_envs=args.n_envs, brain=args.brain, seed=args.seed))
    model = PPO(
        "MlpPolicy", venv,
        n_steps=args.n_steps, batch_size=args.batch_size, n_epochs=args.epochs,
        learning_rate=args.lr, ent_coef=args.ent_coef, gamma=args.gamma, gae_lambda=0.95, clip_range=0.2,
        policy_kwargs=dict(net_arch=dict(pi=[64, 64], vf=[64, 64]), activation_fn=torch.nn.Tanh),
        seed=args.seed, device="cpu", verbose=0,
    )
    logger = GodLogger(run_dir / "progress.csv")
    t0 = time.time()
    print(f"training {args.timesteps} steps on {args.n_envs} envs (brain={args.brain})", flush=True)
    model.learn(total_timesteps=args.timesteps, callback=logger)
    train_minutes = (time.time() - t0) / 60
    venv.close()
    model_path = model_dir / f"{args.name}.zip"
    model.save(str(model_path))
    print(f"saved {model_path} after {train_minutes:.1f} min ({model.num_timesteps} steps)")

    meta = {"timesteps": int(model.num_timesteps), "train_minutes": round(train_minutes, 1), "brain": args.brain,
            "n_envs": args.n_envs, "algo": "PPO (stable-baselines3)", "net": [64, 64]}
    results = None
    if not args.no_eval:
        print(f"evaluating on {args.eval_episodes} held-out worlds per god (brain=sim)...", flush=True)
        results = compare(model, args.eval_episodes, brain="sim", n_procs=4, seed=args.seed)
        print(table(results))
        (model_dir / "eval_results.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
        ppo = next(r for r in results if r["policy"] == "ppo")
        meta["eval"] = {k: ppo[k] for k in ("episodes", "win_rate", "mean_return", "mean_win_seconds")}

    out = export_policy(model, Path(args.export), meta=meta)
    print(f"exported actor to {out} ({out.stat().st_size / 1024:.1f} KB)")
    # Re-check the export through the browser runtime (src/ai/policy.js) inside the real sim.
    chk = subprocess.run(["node", str(RL_DIR / "env_server.mjs"), "--selftest", "--policy", "json", "--policy-file", str(out),
                          "--episodes", "10", "--seed", "1000000", "--quiet"], cwd=str(REPO), capture_output=True, text=True)
    print("browser runtime check:", chk.stdout.strip().replace("\n", " ") if chk.returncode == 0 else chk.stderr)


if __name__ == "__main__":
    main()
