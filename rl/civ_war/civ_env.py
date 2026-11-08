"""Python side of the war councils' training environment (#100).

The environment itself is Rust (`crates/moc-civ/src/gym.rs`, served by the `civ_gym` example
as newline-delimited JSON on stdin/stdout); this file only drives several server processes as
one stable-baselines3 VecEnv. Each process runs one world with `learners` slots (civilisations
the learner drives); the VecEnv's envs are all the slots of all the processes. Action masks
(the physics the policy can't break) come from Rust and are handed to sb3-contrib's
MaskablePPO through `env_method("action_masks")`.

This is offline training tooling only; nothing here ships in the game.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

import numpy as np
from gymnasium import spaces
from stable_baselines3.common.vec_env.base_vec_env import VecEnv

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
EXE_NAME = "civ_gym.exe" if sys.platform == "win32" else "civ_gym"

# Worlds for training come from [0, TRAIN_SEEDS); evaluation uses EVAL_SEED_BASE + i (never seen).
TRAIN_SEEDS = (0, 500_000)
EVAL_SEED_BASE = 1_000_000


def gym_exe() -> Path:
    """The release build of the env server (CARGO_TARGET_DIR or target/)."""
    if os.environ.get("CIV_GYM"):
        return Path(os.environ["CIV_GYM"])
    tgt = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target"))
    exe = tgt / "release" / "examples" / EXE_NAME
    if not exe.exists():
        raise FileNotFoundError(f"{exe} not found: cargo build --release -p moc-civ --example civ_gym")
    return exe


class GymClient:
    """One `civ_gym` process."""

    def __init__(self, exe: Path | None = None):
        self.proc = subprocess.Popen([str(exe or gym_exe())], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=None, bufsize=0)

    def send(self, msg: dict[str, Any]) -> None:
        self.proc.stdin.write((json.dumps(msg) + "\n").encode())
        self.proc.stdin.flush()

    def recv(self) -> dict[str, Any]:
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError(f"civ_gym exited (code {self.proc.poll()})")
        out = json.loads(line)
        if "error" in out:
            raise RuntimeError(f"civ_gym: {out['error']}")
        return out

    def call(self, msg: dict[str, Any]) -> dict[str, Any]:
        self.send(msg)
        return self.recv()

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                self.send({"cmd": "close"})
                self.proc.wait(timeout=5)
            except Exception:
                self.proc.kill()


def spec() -> dict[str, Any]:
    c = GymClient()
    s = c.call({"cmd": "spec"})
    c.close()
    return s


class CivWarVecEnv(VecEnv):
    """`n_procs` worlds × `learners` slots, stepped in parallel (all writes, then all reads)."""

    def __init__(self, n_procs: int, config: dict[str, Any], seed: int = 0, league: dict | None = None):
        self.mode = config.get("mode", "council")
        self.config = dict(config)
        self.learners = 1 if self.mode == "commander" else int(config.get("learners", 1))
        self.clients = [GymClient() for _ in range(n_procs)]
        self.spec_info = self.clients[0].call({"cmd": "spec"})
        sp = self.spec_info[self.mode]
        self.nvec = list(sp["nvec"])
        obs_space = spaces.Box(low=-2.0, high=2.0, shape=(sp["obs_size"],), dtype=np.float32)
        act_space = spaces.MultiDiscrete(self.nvec)
        super().__init__(n_procs * self.learners, obs_space, act_space)
        self.rng = np.random.default_rng(seed)
        self._masks = np.ones((self.num_envs, sum(self.nvec)), dtype=bool)
        self._actions = None
        if league:
            self.set_league(league)

    def set_league(self, league: dict) -> None:
        for c in self.clients:
            c.send({"cmd": "league", **league})
        for c in self.clients:
            c.recv()

    def _slots(self, i: int) -> slice:
        return slice(i * self.learners, (i + 1) * self.learners)

    def reset(self):
        for c in self.clients:
            s = int(self.rng.integers(*TRAIN_SEEDS))
            c.send({"cmd": "reset", **self.config, "seed": s, "seeds": list(TRAIN_SEEDS)})
        obs = np.zeros((self.num_envs, self.observation_space.shape[0]), dtype=np.float32)
        for i, c in enumerate(self.clients):
            o = c.recv()
            obs[self._slots(i)] = np.asarray(o["obs"], dtype=np.float32)
            self._masks[self._slots(i)] = np.asarray(o["masks"], dtype=bool)
        self._reset_seeds()
        self._reset_options()
        return obs

    def step_async(self, actions):
        self._actions = np.asarray(actions)
        for i, c in enumerate(self.clients):
            c.send({"cmd": "step", "actions": self._actions[self._slots(i)].astype(int).tolist()})

    def step_wait(self):
        obs = np.zeros((self.num_envs, self.observation_space.shape[0]), dtype=np.float32)
        rews = np.zeros(self.num_envs, dtype=np.float32)
        dones = np.zeros(self.num_envs, dtype=bool)
        infos: list[dict] = [{} for _ in range(self.num_envs)]
        for i, c in enumerate(self.clients):
            o = c.recv()
            sl = self._slots(i)
            obs[sl] = np.asarray(o["obs"], dtype=np.float32)
            self._masks[sl] = np.asarray(o["masks"], dtype=bool)
            rews[sl] = o["rewards"]
            dones[sl] = o["dones"]
            for k in range(self.learners):
                j = i * self.learners + k
                info = dict(o["infos"][k])
                if o["dones"][k]:
                    info["terminal_observation"] = np.asarray(o["terminal_obs"][k], dtype=np.float32)
                    info["TimeLimit.truncated"] = bool(o["truncated"][k])
                infos[j] = info
            if o["episodes"]:
                infos[sl.start]["episodes"] = o["episodes"]
        return obs, rews, dones, infos

    def action_masks(self) -> np.ndarray:
        return self._masks.copy()

    def close(self):
        for c in self.clients:
            c.close()

    # VecEnv plumbing: the per-env objects live in Rust.
    def _idx(self, indices):
        return list(self._get_indices(indices))

    def has_attr(self, attr_name: str) -> bool:
        return attr_name == "action_masks" or hasattr(self, attr_name)

    def get_attr(self, attr_name, indices=None):
        if attr_name == "render_mode":
            return [None for _ in self._idx(indices)]
        return [getattr(self, attr_name) for _ in self._idx(indices)]

    def set_attr(self, attr_name, value, indices=None):
        setattr(self, attr_name, value)

    def env_method(self, method_name, *method_args, indices=None, **method_kwargs):
        if method_name == "action_masks":
            return [self._masks[i].copy() for i in self._idx(indices)]
        raise NotImplementedError(method_name)

    def env_is_wrapped(self, wrapper_class, indices=None):
        return [False for _ in self._idx(indices)]
