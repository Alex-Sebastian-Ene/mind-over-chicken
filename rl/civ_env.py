"""Gymnasium wrapper around the headless Stone Age (rl/env_server.mjs).

Each environment owns one Node process running the real game modules; they talk
newline-delimited JSON over stdin/stdout. `StoneAgeEnv` is a plain gymnasium.Env.
`StoneAgeVecEnv` is a stable-baselines3 VecEnv that drives N Node processes in
parallel (it writes every action first, then reads every reply), which is much
faster than wrapping single envs in DummyVecEnv and avoids SubprocVecEnv's extra
Python processes on Windows.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces

RL_DIR = Path(__file__).resolve().parent
REPO = RL_DIR.parent
SERVER = RL_DIR / "env_server.mjs"

# Training worlds are drawn from [0, TRAIN_SEED_SPACE); evaluation uses EVAL_SEED_BASE + i,
# so the reported results are on worlds the policy never trained on.
TRAIN_SEED_SPACE = 500_000
EVAL_SEED_BASE = 1_000_000


class NodeEnvClient:
    """One `node rl/env_server.mjs` process."""

    def __init__(self, brain: str = "sim", max_seconds: float = 120.0, node: str | None = None):
        node = node or shutil.which("node")
        if not node:
            raise RuntimeError("node was not found on PATH (Node 18+ is required)")
        self.proc = subprocess.Popen(
            [node, str(SERVER), "--brain", brain, "--max-seconds", str(max_seconds)],
            cwd=str(REPO),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None,  # inherit: warnings and crashes show up in the console
            bufsize=0,
        )

    def send(self, msg: dict[str, Any]) -> None:
        self.proc.stdin.write((json.dumps(msg) + "\n").encode())
        self.proc.stdin.flush()

    def recv(self) -> dict[str, Any]:
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError(f"env_server exited (code {self.proc.poll()})")
        out = json.loads(line)
        if "error" in out:
            raise RuntimeError(f"env_server: {out['error']}")
        return out

    def call(self, msg: dict[str, Any]) -> dict[str, Any]:
        self.send(msg)
        return self.recv()

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                self.send({"cmd": "close"})
                self.proc.wait(timeout=3)
            except Exception:
                self.proc.kill()


def _spaces(spec: dict[str, Any]):
    obs_space = spaces.Box(low=-1.0, high=2.0, shape=(spec["obs_size"],), dtype=np.float32)
    return obs_space, spaces.Discrete(spec["n_actions"])


class StoneAgeEnv(gym.Env):
    """The Stone Age as a gymnasium environment.

    Actions: 0 wait, 1 lightning on the fire pit, 2 lightning on the nearest mammoth,
    3 be calm this step, 4 lightning on a random spot. One decision = 0.25 s of game time.
    Pass action="scripted" to `step` to let the baseline god choose (useful for baselines).
    """

    metadata = {"render_modes": []}

    def __init__(self, brain: str = "sim", max_seconds: float = 120.0):
        super().__init__()
        self.client = NodeEnvClient(brain=brain, max_seconds=max_seconds)
        self.spec_info = self.client.call({"cmd": "spec"})
        self.observation_space, self.action_space = _spaces(self.spec_info)
        self.obs_names = self.spec_info["obs_names"]
        self.action_names = self.spec_info["actions"]

    def reset(self, *, seed: int | None = None, options: dict | None = None):
        super().reset(seed=seed)
        ep_seed = seed if seed is not None else int(self.np_random.integers(TRAIN_SEED_SPACE))
        out = self.client.call({"cmd": "reset", "seed": int(ep_seed)})
        return np.asarray(out["obs"], dtype=np.float32), out["info"]

    def step(self, action):
        a = action if action == "scripted" else int(action)
        out = self.client.call({"cmd": "step", "action": a})
        info = out["info"]
        info["action"] = out["action"]
        return (np.asarray(out["obs"], dtype=np.float32), float(out["reward"]),
                bool(out["terminated"]), bool(out["truncated"]), info)

    def close(self):
        self.client.close()


try:
    from stable_baselines3.common.vec_env.base_vec_env import VecEnv
except ImportError:  # gymnasium-only installs can still use StoneAgeEnv
    VecEnv = None

if VecEnv is not None:

    class StoneAgeVecEnv(VecEnv):
        """N Node processes stepped in parallel, with SB3's auto-reset semantics."""

        def __init__(self, n_envs: int = 4, brain: str = "sim", max_seconds: float = 120.0, seed: int = 0):
            self.clients = [NodeEnvClient(brain=brain, max_seconds=max_seconds) for _ in range(n_envs)]
            spec = self.clients[0].call({"cmd": "spec"})
            for c in self.clients[1:]:
                c.call({"cmd": "spec"})
            self.spec_info = spec
            obs_space, act_space = _spaces(spec)
            super().__init__(n_envs, obs_space, act_space)
            self.rng = np.random.default_rng(seed)
            self.actions = None

        def _episode_seed(self, i: int) -> int:
            s = self._seeds[i]
            if s is not None:
                self._seeds[i] = None  # explicit seeds apply to the next reset only
                return int(s)
            return int(self.rng.integers(TRAIN_SEED_SPACE))

        def reset(self):
            for i, c in enumerate(self.clients):
                c.send({"cmd": "reset", "seed": self._episode_seed(i)})
            outs = [c.recv() for c in self.clients]
            self.reset_infos = [o["info"] for o in outs]
            self._reset_seeds()
            self._reset_options()
            return np.asarray([o["obs"] for o in outs], dtype=np.float32)

        def step_async(self, actions):
            self.actions = actions
            for c, a in zip(self.clients, actions):
                c.send({"cmd": "step", "action": a if a == "scripted" else int(a)})

        def step_wait(self):
            outs = [c.recv() for c in self.clients]
            obs = np.asarray([o["obs"] for o in outs], dtype=np.float32)
            rews = np.asarray([o["reward"] for o in outs], dtype=np.float32)
            dones = np.zeros(self.num_envs, dtype=bool)
            infos = []
            resets = []
            for i, o in enumerate(outs):
                info = o["info"]
                info["action"] = o["action"]
                if o["terminated"] or o["truncated"]:
                    dones[i] = True
                    info["terminal_observation"] = obs[i].copy()
                    info["TimeLimit.truncated"] = bool(o["truncated"] and not o["terminated"])
                    self.clients[i].send({"cmd": "reset", "seed": self._episode_seed(i)})
                    resets.append(i)
                infos.append(info)
            for i in resets:
                r = self.clients[i].recv()
                obs[i] = np.asarray(r["obs"], dtype=np.float32)
                self.reset_infos[i] = r["info"]
            return obs, rews, dones, infos

        def close(self):
            for c in self.clients:
                c.close()

        def _idx(self, indices):
            return list(self._get_indices(indices))

        def get_attr(self, attr_name, indices=None):
            if attr_name == "render_mode":
                return [None for _ in self._idx(indices)]
            return [getattr(self, attr_name) for _ in self._idx(indices)]

        def set_attr(self, attr_name, value, indices=None):
            setattr(self, attr_name, value)

        def env_method(self, method_name, *method_args, indices=None, **method_kwargs):
            raise NotImplementedError("StoneAgeVecEnv has no per-env Python objects")

        def env_is_wrapped(self, wrapper_class, indices=None):
            return [False for _ in self._idx(indices)]
