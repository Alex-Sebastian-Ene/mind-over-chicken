# The AI god: PPO on the Stone Age

A reinforcement-learning "AI god" that plays the game's real Stone Age simulation, trained
with PPO (stable-baselines3) against the actual game modules running headless in Node, then
run in the browser from a small JSON file of weights.

## Two rules

1. **PPO is only for simulation.** The AI god drives the training env and simulated
   planets (`src/ai/planet.js`), nothing else. It is opt-in and per world, is never attached
   to the human's campaign world, and never reads or writes the player's headset inputs
   (`G.source.input.*`). Its calm and focus are its own values, on the planet's own brain.
   When a human watches an AI planet, the human's blinks, calm and focus still apply on
   top: calm/focus are the higher of the two (`SharedBrain`), the human's blinks always go
   through, and AI lightning never starts the cooldown that would refuse the human's next
   blink. The AI adds actions; it never blocks or replaces the player's.
2. **The world is a planet.** The 320 x 180 map is equirectangular: it wraps east-west
   (longitude), with poles at the top and bottom, and three continents. The env runs the
   whole planet (home continent, rafts, colonies on the other two continents), observes
   planet-wide quantities (colonies founded, rafts in transit, share of people overseas), and
   nothing in it assumes x is bounded: horizontal distances take the short way round
   (`wrapDx`), aims wrap instead of clamping, random strikes land anywhere on the globe.
   The sim's own x-wrap lands with the Rust port.

`node rl/test_rules.mjs` checks both rules against the real sim.

## Files

| File | What |
|---|---|
| `src/ai/observe.js` | **The spec**: observation vector, action decoding, scripted baseline. Shared by trainer and browser. |
| `src/ai/planet.js` | `SimPlanet`: own World + StoneAge + AI brain. What the trainer builds and what the browser runs. |
| `src/ai/proxyBrain.js` | `ProxyBrain` (cheap AI brain) and `SharedBrain` (AI + watching human, human on top). |
| `src/ai/autopilot.js` | `AIGod`: loads the policy once, decides every 0.25 s on the planet it is given. |
| `src/ai/policy.js` | Dependency-free MLP forward pass for the exported JSON. |
| `src/ai/stone_policy.json` | The trained actor. |
| `rl/env_server.mjs` | Headless env over newline-delimited JSON on stdin/stdout; also `--selftest` baselines. |
| `rl/headless.mjs` | Fake `document`/canvas so `World.paint()` runs in Node; seeded `Math.random`. |
| `rl/civ_env.py` | `StoneAgeEnv` (gymnasium.Env) and `StoneAgeVecEnv` (SB3 VecEnv over N Node processes). |
| `rl/train_ppo.py` | Train, evaluate, export, and re-check the export through `policy.js` in the real sim. |
| `rl/evaluate.py` | Random vs scripted vs PPO on held-out worlds, with behaviour statistics. |
| `rl/export_policy.py` | Actor weights to `src/ai/stone_policy.json`. |
| `rl/test_rules.mjs` | Checks the two rules. |

## Setup

Node 18+ on PATH (22 used here). Python 3.10+. Keep the venv outside the repo.

```bash
python -m venv ../rl-venv
../rl-venv/Scripts/python -m pip install torch --index-url https://download.pytorch.org/whl/cpu
../rl-venv/Scripts/python -m pip install -r rl/requirements.txt
```

## Run

```bash
# Smoke-test the env alone (no Python): random / scripted / wait / json policies
node rl/env_server.mjs --selftest --policy random --episodes 5
node rl/env_server.mjs --selftest --policy scripted --episodes 20 --quiet
node rl/test_rules.mjs

# Train, evaluate, export (wins ~98% by 130k steps; 360k steps took 19.5 min on a busy 4-core laptop)
python rl/train_ppo.py --timesteps 360000 --n-envs 8

# Re-evaluate or re-export a saved model
python rl/evaluate.py --model rl/models/stone_ppo.zip --episodes 60
python rl/export_policy.py --model rl/models/stone_ppo.zip --out src/ai/stone_policy.json

# The exported JSON through the browser runtime, inside the real sim
node rl/env_server.mjs --selftest --policy json --episodes 20 --seed 1000000 --quiet
```

`--brain sim` (default) gives the AI the real `Brain` fed by its own emulated headset
(`SimSource`), including band-power noise, the personal classifier and its rising
threshold; `--brain proxy` uses `ProxyBrain` (about 3x faster, no classifier).

## The environment (reimplement 1:1 in Rust from this section)

**Planet.** `World(seed)` + `StoneAge` entered on it, as in `SimPlanet`. All sim randomness
draws from one generator seeded per episode (`mulberry32(seed ^ 0x9e3779b9)` replaces
`Math.random`). Training seeds are drawn from `[0, 500000)`; evaluation uses `1000000 + i`.

**Time.** One decision every `DECISION_DT = 0.25` s of game time, simulated as 4 substeps of
`1/16` s, each `brain.update(dt); scene.update(dt)`, stopping early on win/lose. Episode cap
`EPISODE_SECONDS = 120` s (480 decisions) → `truncated`. Win (`G.complete`) or lose
(`G.fail`, ≤ 3 alive) → `terminated`.

**Actions** (`Discrete(5)`), applied at the start of the step:

| # | name | effect |
|---|---|---|
| 0 | `wait` | AI calm input off |
| 1 | `strike_fire` | bolt at `aimAt(fire.x, fire.y)`, calm off |
| 2 | `strike_mammoth` | bolt at `aimAt(m.x, m.y - 2)` for the living mammoth nearest the fire (wrapped distance); no bolt if none; calm off |
| 3 | `calm` | AI calm input on for this step (no bolt) |
| 4 | `strike_random` | bolt at `{x: U(0,1), y: U(0,1)}` (anywhere on the planet), calm off |

`aimAt(x, y) = { x: wrapX(x) / (W-1), y: clamp(y / (H-1), 0, 1) }`, `wrapX(x) = ((x % W) + W) % W`.
A bolt goes through `scene.onBlink(aim)`, which refuses it within `BOLT_COOLDOWN = 0.5` s of
the previous one. With a human attached, AI bolts keep their own cooldown and restore
`scene.lastBolt` afterwards (see rule 1).

AI calm: with `--brain sim` the AI's own `SimSource.input.calm` (latent calm → 0.95 at rate
1.1/s while on, → 0.15 at rate 1.6/s while off, then band powers → classifier → `Brain.calm`
smoothed at rate 4/s). With `proxy`: the same latent dynamics, `calm` = latent smoothed at 4/s.

**Observation** (`Box(20)`, float32, fixed scaling only: no running normalisation):

| # | name | definition |
|---|---|---|
| 0 | `fire_lit` | 1 if `fire.lit` else 0 |
| 1 | `fire_heat` | `clamp(fire.heat, 0, 1)` |
| 2 | `food` | `clamp(food / 10, 0, 1.5)` |
| 3 | `alive` | `clamp(n / 15, 0, 2)`, n = living people on every continent and raft |
| 4 | `hunger` | mean hunger of the living (0 if n = 0) |
| 5 | `starving` | share of the living with hunger ≥ 0.9 |
| 6 | `fear` | mean fear of the living |
| 7 | `hold_active` | 1 while the calm challenge runs |
| 8 | `hold_progress` | `clamp(hold.progress / 5, 0, 1)` |
| 9 | `holds` | `clamp(hold.successes / 2, 0, 1)` |
| 10 | `mammoth_dist` | `clamp(d / 100, 0, 1)`, d = wrapped distance fire → nearest living mammoth; 1 if none |
| 11 | `mammoths` | `clamp(living mammoths / 4, 0, 1)` |
| 12 | `colonies` | `clamp(colonies founded / (continents - 1), 0, 1)` |
| 13 | `rafts` | `clamp(rafts with state boarding or sailing / 2, 0, 1)` |
| 14 | `overseas` | share of the living settled on another continent (`p.colony`) |
| 15 | `time` | `clamp(scene.t / 120, 0, 1)` |
| 16 | `calm` | `clamp(brain.calm, 0, 1)` |
| 17 | `calm_margin` | `clamp(brain.calm - brain.threshold('calm', 0.55), -1, 1)` |
| 18 | `bolt_ready` | 1 if `scene.t - lastBolt ≥ 0.5` and the scene is not done |
| 19 | `meat` | `clamp(Σ max(0, meat.amount) / 10, 0, 1)` |

Wrapped distance: `hypot(wrapDx(ax - bx), ay - by)`, `wrapDx(dx) = dx - W * round(dx / W)`.

**Reward** (per step, `REWARD` in `env_server.mjs`). Every term is something a player would
call good or bad; the only shaping is potential-based (Ng et al. 1999), so it cannot change
which policy is optimal.

| term | weight |
|---|---|
| each step (0.25 s) | −0.01 |
| birth while the tribe (before the step) is below 15 | +1.0 each |
| birth once the tribe is at 15+ | +0.2 each (no farming babies instead of winning) |
| death (starved, trampled) | −1.5 each |
| successful calm hold, first 2 | +2.0 each |
| successful calm hold beyond 2 | +0.5 each |
| villager smitten by lightning | −0.1 each |
| win (`G.complete`) | +10 |
| lose (`G.fail`) | −10 |
| shaping `Φ(s') − Φ(s)` | `Φ = 0.3·hold.progress + 0.3·clamp(calm,0,1)` while a hold is active, else 0; `Φ = 0` at termination |

"Births towards 15" = `min(births, max(0, 15 − alive_before))`; likewise holds towards 2.
Births, deaths and smites are deltas of `G.stats.births/deaths/smitten` over the step.

**Scripted baseline** (`scriptedAction`, observation only): fire out → strike fire (when the
bolt is ready); hold active → calm; food < 2, no meat lying around, a mammoth alive and the
bolt ready → strike mammoth; otherwise wait.

## Policy JSON (`src/ai/stone_policy.json`, format `mlp-policy/1`)

```jsonc
{
  "format": "mlp-policy/1",
  "obs_version": 2,                 // must equal OBS_VERSION in src/ai/observe.js
  "obs_names": ["fire_lit", ...],   // input order (20)
  "actions": ["wait", ...],         // output order (5)
  "decision_dt": 0.25,
  "activation": "tanh",             // after every layer except the last
  "layers": [                        // 20 -> 64 -> 64 -> 5
    { "w": [[...20], ...64 rows], "b": [...64] },
    { "w": [[...64], ...64 rows], "b": [...64] },
    { "w": [[...64], ...5 rows],  "b": [...5] }
  ],
  "meta": { "timesteps": ..., "eval": {...} },
  "check": { "obs": [[...20], ...], "probs": [[...5], ...] }
}
```

`w` is row-major `out x in` (PyTorch `Linear.weight`): `y_i = b_i + Σ_j w[i][j]·x_j`. The last
layer gives logits; softmax gives action probabilities, argmax is the greedy action (what
the evaluation reports). Inputs are the raw observation vector above: no extra
normalisation. `check` holds probe inputs and the probabilities PyTorch produced; a port
should reproduce them to ~1e-6 (`MLPPolicy.selfCheck()` does this in JS).

## In the browser

```js
const planet = new SimPlanet({ seed }).setGod(new AIGod());   // opt-in, per world
planet.attachHuman(G.brain);   // optional: the player watches and plays along
planet.update(dt);             // every frame: the god decides every 0.25 s, then the sim steps
planet.humanBlink(aim);        // route the player's blinks to it while it is on screen
```

`AIGod` fetches `./stone_policy.json` relative to `src/ai/policy.js` once (shared by all
planets) and plays the scripted god until it arrives or if it fails. For many background
galaxy planets use `new SimPlanet({ seed, biome, brain: 'proxy' })`.

## What the env needs from the sim (API for the Rust core)

Construction and stepping:
- `Planet::new(seed, biome)`: world generation (three continents, harbours, east-west wrap) plus
  the Stone Age entered on it, with one seeded RNG for everything.
- `step(dt)`: one `scene.update(dt)`, called 4 times per decision with `dt = 1/16`.
- `blink(aim, caster)`: `StoneAge.onBlink` (fire relight, cooking a mammoth, smiting,
  fear and faith), with a separate 0.5 s cooldown per caster (`Human` / `Ai`). The JS port
  emulates this by restoring `scene.lastBolt` after an AI bolt.
- The AI's own brain: `set_ai_calm(bool)`, then `calm`, `threshold('calm', 0.55)`, `setCue`,
  `stress` (`max(1 - calm, anger)`; the AI has no face, so `1 - calm`), either the full
  `Brain` + `SimSource` dynamics or `ProxyBrain`. A human brain can be layered on top as
  `max(ai, human)` for calm and focus, with the human's face anger counting toward stress.

Read-only state for the observation, reward and info:
- `scene.t`, `done`, `strikes`, `lastBolt`
- `fire {x, y, heat, lit}`, `food`, `hold {active, progress, successes}`
- `people[] {alive, hunger, fear, colony}` (or `living()`), `mammoths[] {x, y, alive}`,
  `meat[] {amount}`, `boats[] {state}`, `colonies` (count), `world.continents.len()`
- `stats {births, deaths, smitten}` counters, plus an outcome flag `Win | Lose | Running`
  (`G.complete` / `G.fail`)

The env can then live wholly in Rust: a batched `reset(seeds)` / `step(actions) ->
(obs[N,20], reward[N], terminated[N], truncated[N])` exposed through PyO3 drops the JSON pipe.

## Hook for src/main.js (not applied: main.js is owned elsewhere)

An opt-in AI planet the player can watch and play along with. The campaign pauses while it
is on screen and is never driven by the AI. `G` is a free key today.

```js
import { SimPlanet } from './ai/planet.js';
import { AIGod } from './ai/autopilot.js';
let aiPlanet = null;   // a simulated planet run by the AI god; never the campaign world
// keydown, next to KeyM:
if (G && e.code === 'KeyG') { aiPlanet = aiPlanet ? null : new SimPlanet().setGod(new AIGod()).attachHuman(G.brain); return; }
// brain.onBlink listener: send the player's blinks to the world on screen
if (!banner && !cosmos.active) { if (aiPlanet) aiPlanet.humanBlink(aim); else G.scene?.onBlink?.(aim); }
// frame(), right after G.brain.update(dt):
if (aiPlanet) aiPlanet.update(dt);
// frame(): pause the campaign while watching:  `} else if (G.sandbox || aiPlanet) {`
// render():  const scene = aiPlanet?.scene || G?.scene;
// HUD: wanted = aiPlanet ? aiPlanet.scene : ...;  hud.update(dt, aiPlanet ? Object.assign(Object.create(G), { scene: aiPlanet.scene }) : ...)
```

## Results (2026-11-07)

**Throughput** (Node 22, i5-10300H, 4 cores / 8 threads, shared with other jobs). One Node
env with the real brain: about 270 steps/s on a random policy (30 villagers) and 420 on the
scripted one; about 940 with `--brain proxy`. Profiling shows about 40% of the time is the
personal classifier retraining every 2.5 s (`PersonalModel.fit`) and about 20% brain
ingest. 8 parallel envs: about 550 steps/s of raw stepping, and 300 to 530 steps/s while
training (PPO updates included).

**Training.** PPO, MlpPolicy pi/vf [64, 64] tanh, 8 envs x 128 steps, batch 256, 10 epochs,
lr 3e-4, gamma 0.99, GAE 0.95, clip 0.2, entropy 0.01, brain = sim. 360,448 steps in 19.5 min
(slower than planned because the machine was busy). It wins 68% of episodes at 105k steps
and 98% at 131k (6 min); after that it only fine-tunes.

**Evaluation**: 60 held-out worlds (seeds 1000000 to 1000059), the real brain, greedy actions.

| god | win rate | lose rate | mean return | mean time to win (s) | alive at end | deaths | calm holds | bolts | smitten |
|---|---|---|---|---|---|---|---|---|---|
| random | 0% | 0% | -5.19 | - | 30.0 | 0.00 | 0.00 | 153.3 | 84.3 |
| scripted | 100% | 0% | 17.60 | 36.4 | 15.9 | 0.20 | 2.03 | 3.2 | 0.1 |
| PPO | 97% | 0% | 17.55 | 39.1 | 19.4 | 0.13 | 1.93 | 8.3 | 0.2 |

The exported JSON run through `src/ai/policy.js` in the real sim (10 held-out worlds) wins
10 of 10 (mean 37.6 s), and its probabilities match PyTorch to 5e-7.

**What it learned.**
- It throws lightning at the fire pit on its very first decision, and never at a lit fire.
- While the calm challenge runs it is calm on 100% of steps. It also starts being calm
  just before the challenge begins (heat 0.50 to 0.69), so the brain's calm reading is
  already high when the 5-second timer starts. The scripted god does not do this.
- It strikes mammoths about 2.6 times as often as the scripted god (8 bolts against 3), mostly
  when the larder is empty and sometimes when it is full. The result is more babies (19 alive
  at the end against 16) and slightly fewer deaths.
- It never uses random lightning and almost never smites a villager.
- Its weakness: in about 1 world in 30 a hold fails and the fire goes out. The policy then
  keeps trying to be calm instead of relighting the fire, and the episode times out. The
  scripted god relights the fire. A fix is more training on "fire went out" states, or adding
  the scripted relight as a rule.
