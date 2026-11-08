"""Train the PPO war councils (#100): the council (strategy) through a curriculum, then the army
commander (operations), with self-play against a league of past checkpoints.

    python rl/civ_war/train.py --config local            # ~70 min on 8 CPU cores
    python rl/civ_war/train.py --config full --out $SCRATCH/civ_war   # the cluster run

Each stage trains sb3-contrib MaskablePPO (64x64 tanh MLPs, CPU) against the real Rust sim
(`civ_gym` processes, see civ_env.py) for a wall-clock budget. Every `league.every_minutes` the
current policy is saved, exported to MOCP and added to the league the env servers draw
opponents from (newest half the time, a random past one otherwise, the rules with p_rules).
Council stages run the curriculum in the config (2 civs on a small planet up to many civs on a
big one); the commander stage trains army orders under the finished council.

Outputs in --out (default rl/civ_war/runs/<config>):
  <stage>/progress.csv, <stage>/sb3/progress.csv (+ TensorBoard if installed)
  <stage>/checkpoints/*.zip, league/*.mocp, state.json (resume point)
  council.zip / council.mocp, commander.zip / commander.mocp (the finished policies)
With --install the finished .mocp files are also copied to crates/moc-civ/policy/.
"""

from __future__ import annotations

import argparse
import csv
import json
import shutil
import time
from collections import deque
from pathlib import Path

import numpy as np
import torch

from civ_env import HERE, REPO, CivWarVecEnv
from export_policy import export_policy
from sb3_contrib import MaskablePPO
from stable_baselines3.common.callbacks import BaseCallback
from stable_baselines3.common.logger import configure


class StageLogger(BaseCallback):
    """CSV curve per rollout, league snapshots on a clock, stop at the stage's budget."""

    def __init__(self, stage: dict, out: Path, budget_s: float, league_every_s: float, on_snapshot, start_minutes: float = 0.0):
        super().__init__()
        self.stage, self.out, self.budget_s, self.every = stage, out, budget_s, league_every_s
        self.on_snapshot = on_snapshot
        self.t0 = time.time()
        self.offset = start_minutes * 60
        self.last_snap = time.time()
        self.eps = deque(maxlen=200)
        self.rew = []
        self.rows = []
        self.csv_path = out / "progress.csv"

    def _on_step(self) -> bool:
        self.rew.append(float(np.mean(self.locals["rewards"])))
        for info in self.locals["infos"]:
            for e in info.get("episodes", ()):
                self.eps.append(e)
        if time.time() - self.last_snap >= self.every:
            self.last_snap = time.time()
            self.on_snapshot(self.model)
        return time.time() - self.t0 < self.budget_s

    def _on_rollout_end(self) -> None:
        eps = list(self.eps)
        row = {
            "timesteps": self.num_timesteps,
            "minutes": round((time.time() - self.t0 + self.offset) / 60, 2),
            "mean_step_reward": round(float(np.mean(self.rew)) if self.rew else 0.0, 4),
            "episodes": len(eps),
            "mean_return": round(float(np.mean([e["ret"] for e in eps])), 3) if eps else "",
            "mean_len": round(float(np.mean([e["len"] for e in eps])), 1) if eps else "",
            "survival": round(float(np.mean([e["alive"] for e in eps])), 3) if eps else "",
            "mean_age": round(float(np.mean([e["age"] for e in eps])), 2) if eps else "",
            "mean_pop": round(float(np.mean([e["pop"] for e in eps])), 1) if eps else "",
            "mean_villages": round(float(np.mean([e["villages"] for e in eps])), 1) if eps else "",
            "sps": round(self.num_timesteps / max(1e-6, time.time() - self.t0)),
        }
        self.rew = []
        self.rows.append(row)
        for k in ("mean_step_reward", "mean_return", "survival", "mean_age"):
            if row[k] != "":
                self.logger.record(f"civ/{k}", row[k])
        print(f"[{self.stage['name']} {row['minutes']:5.1f} min] {row['timesteps']:>8} steps  r/step {row['mean_step_reward']:+.4f}  "
              f"return {row['mean_return']}  survive {row['survival']}  age {row['mean_age']}  pop {row['mean_pop']}  {row['sps']} sps", flush=True)
        write_header = not self.csv_path.exists()
        with open(self.csv_path, "a", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(row))
            if write_header:
                w.writeheader()
            w.writerow(row)


def make_model(env, cfg: dict, prev: MaskablePPO | None, seed: int):
    p = cfg["ppo"]
    n_steps = max(16, p["rollout"] // env.num_envs)
    model = MaskablePPO(
        "MlpPolicy", env, n_steps=n_steps, batch_size=p["batch_size"], n_epochs=p["n_epochs"], learning_rate=p["lr"],
        gamma=p["gamma"], gae_lambda=p["gae_lambda"], ent_coef=p["ent_coef"], clip_range=p["clip_range"], vf_coef=0.5,
        max_grad_norm=0.5, policy_kwargs=dict(net_arch=dict(pi=cfg["net"], vf=cfg["net"]), activation_fn=torch.nn.Tanh),
        seed=seed, device="cpu", verbose=0,
    )
    if prev is not None:
        model.policy.load_state_dict(prev.policy.state_dict())
    return model


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--config", default="local", help="local | full | path to a config JSON")
    ap.add_argument("--out", default=None)
    ap.add_argument("--n-procs", type=int, default=None, help="env processes (default: config)")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--minutes-scale", type=float, default=1.0, help="scale every stage's budget")
    ap.add_argument("--install", action="store_true", help="copy the finished .mocp files to crates/moc-civ/policy/")
    ap.add_argument("--only", default=None, help="comma-separated stage names to run")
    a = ap.parse_args()

    cfg_path = Path(a.config) if a.config.endswith(".json") else HERE / "configs" / f"{a.config}.json"
    cfg = json.loads(cfg_path.read_text())
    out = Path(a.out or HERE / "runs" / f"{cfg['name']}-s{a.seed}")
    out.mkdir(parents=True, exist_ok=True)
    (out / "league").mkdir(exist_ok=True)
    n_procs = a.n_procs or cfg["n_procs"]
    torch.set_num_threads(max(1, cfg.get("torch_threads", 1)))
    state_path = out / "state.json"
    state = json.loads(state_path.read_text()) if state_path.exists() else {"done": [], "league": {"council": [], "commander": []}, "elapsed": {}}
    save_state = lambda: state_path.write_text(json.dumps(state, indent=1))  # noqa: E731
    obs_version = 1
    t_all = time.time()
    prev = {"council": None, "commander": None}
    # Resuming: the latest finished policy of each kind seeds the next stage.
    for kind in ("council", "commander"):
        z = out / f"{kind}.zip"
        if z.exists():
            prev[kind] = MaskablePPO.load(str(z), device="cpu")

    for st in cfg["stages"]:
        name, mode = st["name"], st["mode"]
        if name in state["done"] or (a.only and name not in a.only.split(",")):
            continue
        sdir = out / name
        (sdir / "checkpoints").mkdir(parents=True, exist_ok=True)
        env_cfg = {k: st[k] for k in ("mode", "n_civs", "size", "learners", "max_seconds", "warmup", "p_rules", "mood") if k in st}
        league_cfg = cfg["league"]
        lg = state["league"]

        def league_msg():
            if mode == "council":
                return {"council": list(reversed(lg["council"][-league_cfg["max_size"]:])), "commander": []}
            fin = [str(out / "council.mocp")] if (out / "council.mocp").exists() else []
            return {"council": fin, "commander": list(reversed(lg["commander"][-league_cfg["max_size"]:]))}

        env = CivWarVecEnv(n_procs, env_cfg, seed=a.seed + len(state["done"]) * 1000, league=league_msg())
        model = make_model(env, cfg, prev[mode], a.seed)
        ckpts = sorted((sdir / "checkpoints").glob("*.zip"), key=lambda p: p.stat().st_mtime)
        if ckpts:
            model = MaskablePPO.load(str(ckpts[-1]), env=env, device="cpu")
            print(f"resumed {name} from {ckpts[-1].name}", flush=True)
        fmts = ["csv"]
        try:
            import tensorboard  # noqa: F401

            fmts.append("tensorboard")
        except ImportError:
            pass
        model.set_logger(configure(str(sdir / "sb3"), fmts))

        def snapshot(m, final=False):
            k = len(lg[mode])
            tag = "final" if final else f"{k:03d}"
            z = sdir / "checkpoints" / f"{name}-{tag}.zip"
            m.save(str(z))
            mp = out / "league" / f"{mode}-{name}-{tag}.mocp"
            export_policy(m, mp, mode, obs_version)
            lg[mode].append(str(mp))
            state["elapsed"][name] = state["elapsed"].get(name, 0.0) + (time.time() - snap_clock[0]) / 60
            snap_clock[0] = time.time()
            save_state()
            env.set_league(league_msg())
            print(f"  league += {mp.name} ({len(lg[mode])} {mode} snapshots)", flush=True)

        snap_clock = [time.time()]
        done_min = state["elapsed"].get(name, 0.0)
        budget = max(0.0, st["minutes"] * a.minutes_scale - done_min) * 60
        print(f"== stage {name}: {mode}, {st.get('n_civs')} civs on {st.get('size')}, {env.num_envs} envs "
              f"({n_procs} procs), {budget / 60:.1f} min ==", flush=True)
        cb = StageLogger(st, sdir, budget, league_cfg["every_minutes"] * 60, snapshot, done_min)
        model.learn(total_timesteps=10**10, callback=cb, reset_num_timesteps=not ckpts)
        snapshot(model, final=True)
        model.save(str(out / f"{mode}.zip"))
        export_policy(model, out / f"{mode}.mocp", mode, obs_version)
        prev[mode] = model
        env.close()
        state["done"].append(name)
        save_state()

    total = (time.time() - t_all) / 60
    print(f"training finished in {total:.1f} min (this session)", flush=True)
    meta = {"config": cfg["name"], "minutes_this_session": round(total, 1), "stages": state["done"], "elapsed": state["elapsed"]}
    (out / "train_meta.json").write_text(json.dumps(meta, indent=1))
    if a.install:
        for kind in ("council", "commander"):
            src = out / f"{kind}.mocp"
            if src.exists():
                shutil.copy(src, REPO / "crates" / "moc-civ" / "policy" / f"{kind}.mocp")
                print(f"installed {src} -> crates/moc-civ/policy/{kind}.mocp")


if __name__ == "__main__":
    main()
