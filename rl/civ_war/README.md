# PPO war councils (#100)

The civilisations of the Rust civ sim (`crates/moc-civ`) decide war and peace with a policy
trained by PPO, and their armies are commanded by a second one. Everything that ships is Rust:
the environment, the observations and actions, the physics masks, the rewards, the policy
runtime and the browser switch. Python is only the offline trainer in this folder.

**Rules carried over from #37.** PPO is only for simulation: it drives the simulated
civilisations, never the player's inputs, and the player's casts (peace, lightning, fireball,
calm and stress) still apply on top. In the sandbox it drives the rival civilisations and
leaves the player's own to the rules (`?wars=all` drives every civilisation, `?wars=rules`
turns it off). Without valid weights everything falls back to the rules.

## Results: the shipped policy (the cluster run)

The weights compiled into the game (`crates/moc-civ/policy/*.mocp`) come from Iridis job
2148953. It ran the `full` config for 460 min on one 64-core node: 60 env processes, a league
of 16 past checkpoints, about 20.3 M council samples and 0.9 M commander samples. Copies of the
weights, the curves and every evaluation are in `models/full/`.

| stage | setup | minutes | samples | episode return (first → last) | survival |
|---|---|---|---|---|---|
| c1 | council: 2 civs, 640×320 | 40 | 2.0 M | −3.2 → −0.4 | 0.63 → 0.78 |
| c2 | council: 4 civs, 1280×640 | 70 | 3.7 M | −0.3 → +1.6 | 0.80 → 0.85 |
| c3 | council: 8 civs, 1280×640 | 80 | 4.8 M | → +2.0 | → 0.88 |
| c4 | council: 10 civs, 2560×1280 | 90 | 5.5 M | → +5.6 | → 0.98 |
| c5 | council: 12 civs, 2560×1280 | 70 | 4.4 M | → +6.3 | → 0.95 |
| k1 | commander: 6 civs, 1280×640 | 60 | 0.65 M | −0.9 → −0.7 | 0.92 → 0.94 |
| k2 | commander: 10 civs, 2560×1280 | 50 | 0.25 M | −0.6 → −0.4 | 0.98 → 0.95 |

The first rows of c3–c5 have no finished episodes yet, so only the last value is given.

**Evaluation.** `eval_results.json` (= `models/full/eval_full_postfix.json`) was run by
`civ_eval` on Iridis with `politics::scan_borders` fixed: 32 unseen seeds per scenario and
condition, 300 s warm-up under the rules, then 900 s with the same wandering player mood in
every condition. Head to head means half the civilisations of each world are on one side and
half on the other. A win is the higher end score `alive·10 + age + ln(1+pop) + 0.2·villages`.

| | 4 civs, 1280×640 | 8 civs, 1280×640 | 12 civs, 2560×1280 |
|---|---|---|---|
| **councils vs rules**: win rate (Elo gap) | **0.57 (+51)** | **0.57 (+47)** | **0.62 (+83)** |
| councils vs the local policy of #115 | 0.67 (+120) | 0.60 (+67) | 0.53 (+24) |
| trained vs scripted commander | 0.54 (+31) | 0.46 (−27) | 0.51 (+6) |
| all-council worlds: survival, population, villages lost per civ | 0.69, 69, 1.31 | 0.87, 129, 0.68 | 0.95, 292, 0.36 |
| all-rules worlds: survival, population, villages lost per civ | 0.43, 43, 4.41 | 0.57, 63, 4.40 | 0.82, 172, 4.83 |

The councils now beat the rules head to head at every size. All-council worlds keep far more
people and lose a fraction of the villages.

**Behaviour differs by traits** (one shared policy):

| 4 civs, 1280×640 | aggressive | friendly |
|---|---|---|
| rivalry or war stance | 0.25 | 0.05 |
| trade or alliance stance | 0.64 | 0.91 |
| wars declared | 3.3 | 2.4 |

At 8 civs the rivalry or war stance is 0.15 against 0.03 and wars declared 2.6 against 0.9. The
correlation of aggressiveness with a hostile stance is 0.29–0.45. The share under arms no longer
differs by trait: about 0.2 for both.

**Calm against stress** (the player held calm, or stressed, the whole game):

| | 4 civs | 8 civs | 12 civs |
|---|---|---|---|
| councils: wars per world, calm / stress | 2.8 / 81 | 3.4 / 66 | 5.3 / 34 |
| councils: rivalry or war stance, calm / stress | 0.005 / 0.20 | 0.007 / 0.13 | 0.02 / 0.09 |
| rules: wars per world, calm / stress | 7.2 / 86 | 8.9 / 127 | 9.2 / 99 |

**Strategy**, per all-council world of four civilisations:
- 21.7 wars declared, 39 % of them opportunistic (on a rival at war elsewhere, in a Dark Age, or
  split by strife);
- 4.7 joins of an ally's war and 0.4 gang-ups on the strongest;
- 34 alliances formed;
- 62 peace offers, 68 % of them made while losing;
- 7.2 truces broken;
- 62 raids, most of them under the raid war goal, which this policy uses most.

With the trained commander, an army is sent home in only 9 % of the seconds spent at war on two
fronts (the scripted commander does it 99 % of the time).

**The `scan_borders` fix (in this branch).** `politics::scan_borders` cleared the nearest
village pair of every civilisation pair inside its loop, which wiped the (higher, lower) entry
right after the (lower, higher) pass wrote it. A civilisation therefore never marched on, or
found a battlefield with, a lower-numbered enemy: wars were one-sided. Both policies were
trained with the bug. Re-evaluated with the fix (`models/full/eval_*_{prefix,postfix}.json`):

| councils vs rules (win rate) | 4 civs | 8 civs | 12 civs |
|---|---|---|---|
| cluster policy, before the fix → after | 0.55 → 0.57 | 0.57 → 0.57 | 0.64 → 0.62 |
| local policy of #115, before → after | 0.51 → 0.48 | 0.48 → 0.48 | 0.58 → 0.56 |

The fix doesn't hurt the cluster policy, so no fine-tune was needed. It makes the rules' wars
much bloodier: villages lost per civilisation go from 1.5–1.9 to 4.4–4.8.

**The first policy (#115)** came from the local 60-minute run (`configs/local.json`, 8 cores).
It is still in `models/` as `council.mocp` and `commander.mocp`, and serves as the league
baseline. It won 0.42–0.58 against the rules.

**Runtime cost.** A council or commander decision takes 0.03–0.1 ms natively; a council decides
every 4 s and a commander every 1 s.

In the browser (wasm, `browser_timing.json`), the same world was timed at the same moment under
the rules, then the councils, then the rules again, with 1,500–2,100 agents:

| | rules | councils |
|---|---|---|
| one frame at ×1: mean, worst p95 | 1.74 ms, 4.7 ms | 1.93 ms, 5.5 ms |
| one frame at ×16: mean, worst p95 | 4.98 ms, 10.3 ms | 4.97 ms, 12.5 ms |

The difference is within the noise. That includes 8 rival civilisations arriving on their own
(#101), at about 2,100 agents.

With 16 rivals on the 2560×1280 planet (about 3,100–3,800 agents, 15–20 civilisations with
rebels):

| | rules | councils | rules again |
|---|---|---|---|
| one frame at ×1: mean, p95 | 0.61 ms, 1.0 ms | 0.67 ms, 1.1 ms | 0.67 ms, 1.3 ms |
| one frame at ×16: mean, p95 | 3.6 ms, 6.3 ms | 4.9 ms, 7.8 ms | 4.5 ms, 7.6 ms |

There were no fallbacks and no warnings.

Natively, 12 civilisations on 2560×1280 with about 3,700 agents cost 39 ms per simulated second
under the councils and 50 ms under the rules. The rules' worlds are busier with wars.

The wasm grows by about 160 KB: 97 KB of weights and about 65 KB of code.

## Robustness

- **In the sandbox:** the councils are on by default; `?council=rules` turns them off.
- **At load:** weights for another observation layout (`OBS_VERSION`, input count, heads), corrupt
  weights, weights failing their self-check, or weights containing NaN are refused. The rules
  decide and the console says why (`civ_ppo_error_ptr`/`len`).
- **At run time:** a decision whose observation or logits are not finite is made by the rules
  (or the scripted commander) instead, and counted (`civ_ppo_fallbacks`).
- **Layout changes:** each observation layout has a fingerprint, and a test ties it to
  `OBS_VERSION`. Change what the councils observe and the tests fail until the version is bumped
  (and the policy retrained), so stale weights can't silently misread new inputs.

## What's missing

- **The commander is about as good as the scripted one** (0.46–0.54), and rarely sends an army
  home on two fronts.
- **Unused options.** "Defend an ally" is almost never chosen, and "gang up on the strongest"
  is rare.
- **Battles.** All-council worlds still have more border battles than the rules' worlds.
- **Retraining.** The policies were trained before the `scan_borders` fix; a retrain on the
  fixed world may do better.
- **Rebels.** Rebel civilisations born mid-game follow the newest council in all-council worlds
  and the rules in mixed ones.


## How it plugs in

#69 gave every war-and-peace decision one seam: `politics::Policy::decide(&World, civ) ->
Vec<Decision>`, called once a second per civilisation. #100 adds two default methods to that
trait (the only edits to #69's files, marked `#100`):

- `Policy::command(&mut World)`, called right after the rules plan the marches each politics
  tick: the army commanders rewrite their civilisation's war marches here, and the council
  settles what a `Decision` can't say (trade and alliance diplomacy, the price of a broken truce);
- `Policy::mobilise(civ) -> Option<f32>`, read by `jobs::wants`: the share of the people under
  arms.

| File | What |
|---|---|
| `crates/moc-civ/src/council.rs` | The council: observation, masks, standing orders, peace that takes two, alliances, strategy stats. |
| `crates/moc-civ/src/commander.rs` | The army commander: objectives, splits, postures into marches; the scripted commander. |
| `crates/moc-civ/src/policy_rt.rs` | MLP runtime for the `MOCP` weights (multi-head, masked), with a load-time self-check. |
| `crates/moc-civ/src/gym.rs` | The training environment: worlds of n civs on a w×h planet, learner slots, league, rewards. |
| `crates/moc-civ/src/war_ffi.rs` | Browser switch (`civ_ppo_enable`); weights compiled in from `crates/moc-civ/policy/*.mocp`. |
| `crates/moc-civ/examples/civ_gym/` | The env server: JSON lines on stdin/stdout. |
| `crates/moc-civ/examples/civ_eval.rs` | Evaluation against the rules on unseen seeds; writes `eval_results.json`. |
| `crates/moc-civ/tests/war_council.rs` | Protocol round trip, observation size/version, masks, fallback, determinism. |
| `rl/civ_war/civ_env.py` | SB3 VecEnv over several `civ_gym` processes (masks to MaskablePPO). |
| `rl/civ_war/train.py` | Curriculum + league trainer; `configs/{local,full,smoke}.json`. |
| `rl/civ_war/export_policy.py` | Actor → `MOCP` (with PyTorch's logits for the self-check). |
| `rl/civ_war/slurm/` | Cluster scripts (setup, train, eval, export). |

`crates/moc-ai` (#82) was not reused: its runtime is fixed at 20 inputs and 5 actions with a
single categorical head, and parses JSON with serde, which `moc-civ` keeps out of its wasm. The
councils need any input size, several heads at once and action masks; `policy_rt.rs` is ~250
lines with no dependencies.

## The council (strategy): every 4 s of sim time, per civilisation

**Observation v1, 116 floats** (`council::obs_names()`), all normalised, none tied to the
planet's size (distances are in marching time: tiles / (warrior speed × 20 s); territory is a
share among the civilisations it knows):

- itself (24): age, population, warrior share and count, food days, stability, territory
  share, villages, fighters' morale, strife heat and level, its five traits (friendliness,
  aggressiveness, violence, fertility, cohesion), Dark Age, wars, recent losses, reputation,
  vassal/overlord, hunger, its current mobilisation;
- the player's mind (3): stress, calm, anger;
- each of its three nearest known neighbours (3 × 28): present, relative strength and
  population, tension both ways, conflict level, war state both ways, distance, reachable on
  foot, bordering, grudges both ways, trade recency, age gap, at war elsewhere, Dark Age, strife
  inside it, truce left, divine truce (the player's peace cast), allied, vassal link,
  weariness, its aggressiveness and friendliness, its reputation, battles fought, whether war
  is possible yet;
- the balance (5): civilisations known, an ally at war, a coalition against the strongest
  open, its share of all known strength, the threat round its villages.

**Action, MultiDiscrete [6, 6, 6, 5, 3, 5, 3]:**

| head | options |
|---|---|
| stance toward neighbour 1, 2, 3 | peace, trade, alliance, rivalry, war, submit |
| war goal | raid for food, take a village, take the capital, punish, defend an ally |
| coalition | none, join an ally's war, gang up on the strongest |
| mobilisation | 4 %, 12 %, 25 %, 40 %, 60 % under arms (60 % also mans the walls when threatened) |
| rebels | the rules, crush, concede |

What a stance does: **war** declares war (or raids, with the raid goal or before war is
possible); **peace / trade / alliance** sue for peace when at war, and otherwise warm relations
(trade needs warmth to happen; an alliance forms when the other side wants one too);
**rivalry** builds a grievance; **submit** sues for peace as a tributary. Coalitions declare war
on an ally's enemy or on the strongest civilisation others are already fighting. Opportunism is
not an action of its own: the observation shows who is at war elsewhere, in a Dark Age or split
by strife, and the stats count strikes on them.

**The physics the policy can't break** (masked, and ignored if handed in anyway): war only on a
civilisation it has met and can reach on foot; from age 7 (`WAR_FROM_AGE`) or after two battles;
with a grievance (tension ≥ 0.35, which the player's calm lowers and stress raises; raids need
0.15); never through a truce imposed by the player's peace cast; no war on one's vassal or
overlord. An ordinary truce can be broken, at a price: −0.25 reputation, +0.15 tension from
everyone who knows it, +0.5 grudge with the victim. **Peace takes two**: the other side accepts
by its own council's stance, or under the rules by war weariness; exhaustion (or 150 s of war)
forces it.

## The commander (operations): every second, per civilisation at war

**Observation v1, 66 floats**: its age and formations (age 11+), warriors, share in the field,
morale, losses, fronts, the threat at home and the odds there, the council's war goal (one-hot),
its previous orders, and four objectives (nearest enemy village, weakest-held one within reach,
the main enemy's capital, the own village under the heaviest threat), each with distance,
defenders, own fighters near, odds, hills, walls, a river on the way, siege progress, capital,
threat; plus the player's calm and stress.

**Action, MultiDiscrete [4, 3, 3]**: objective (nearest, weakest, capital, home); split (all on
it, main force plus flankers on a second objective, main force plus a reserve sent home);
posture (assault; stage: form up on the best ground short of the objective, on hills where
there are any, this side of any river, and wait for the numbers, from age 11 in formation;
regroup at home). Staging and assault are the same village march with a different rally point,
so an army can wait and then go in without walking home first.

The scripted commander (`commander::scripted`) is the baseline and the fallback when no
commander weights are loaded: defend home under real threat, otherwise the war goal's
objective, stage while outnumbered, fall back when badly outnumbered, keep a home reserve on two
fronts.

## Rewards

Council, per 4 s (`gym::council_reward`):
`0.02 survival + Δln(1+pop) + w_land·(0.5·Δln(1+tiles) + 0.3·villages taken) − 0.4·villages lost
+ 0.6·ages climbed − 1.0·collapses − 0.004·own dead + w_trade·0.005·(trade partners + allies)
± 0.15 per battle won/lost`, and −3 if the civilisation is wiped out; `w_land = 0.3 + 0.7·aggressiveness`,
`w_trade = 0.3 + 0.7·friendliness`, so personalities stay distinct with one shared policy.

Commander, per second (`gym::commander_reward`): `+1 per village taken − 1 per village lost
+ 0.03 per enemy killed − 0.03 per own dead ± 0.5 per battle won/lost`, −2 if wiped out.

## The environment (`civ_gym`)

```text
cargo build --release -p moc-civ --example civ_gym
{"cmd":"spec"}
{"cmd":"reset","seed":7,"n_civs":4,"size":[1280,640],"mode":"council","learners":2,"warmup":[200,450],"max_seconds":900,"mood":"wander","p_rules":0.35}
{"cmd":"step","actions":[[4,0,0,1,0,2,0],[1,1,0,1,0,1,0]]}   -> {obs, masks, rewards, dones, truncated, terminal_obs, infos, episodes}
{"cmd":"league","council":["a.mocp","b.mocp"],"commander":[]}
```

A world runs under the rules for a warm-up (the early ages), then `learners` of its
civilisations are driven by the trainer and the rest by the league: the newest snapshot, a
random past one, or the rules (`p_rules`). A slot whose civilisation dies ends and takes over
another. The player's mind wanders between neutral, calm, stressed and angry spells (60–240 s)
so the policy learns to read it. Planet size and civilisation count are parameters: the last
curriculum stages and the evaluation run at full size once #101's large continents land
(`gym::new_world` places extra civilisations on the home landmass until then).

## Training

```text
cargo build --release -p moc-civ --example civ_gym --example civ_eval
pip install -r rl/civ_war/requirements.txt          # CPU torch
python rl/civ_war/train.py --config local --install  # ~70 min on 8 cores
python rl/civ_war/plot_curves.py rl/civ_war/runs/local-s0 rl/civ_war/curves.svg
cargo run --release -p moc-civ --example civ_eval -- --council rl/civ_war/models/council.mocp     --commander rl/civ_war/models/commander.mocp --old rl/civ_war/models/council-c1.mocp --seeds 12
```

sb3-contrib MaskablePPO, 64×64 tanh actor and critic, CPU only. Curriculum (`configs/local.json`):
2 civs on a 640×320 planet → 4 civs on 1280×640 → 8 civs on 1280×640 (council, scripted
commander), then the commander under the finished council. Every few minutes the policy is
saved, exported and added to the league. Training seeds 0..500 000, evaluation 1 000 000+.
`configs/full.json` is the cluster run: 60 env processes, a league of 16, up to 12 civs on a
2560×1280 planet, two commander stages.

## Training on Iridis

Iridis X (Southampton), one CPU node; no GPU (PPO with 64×64 MLPs is bound by the simulation).

1. Copy the repo: `git clone -b feat/civ-ppo-wars https://github.com/Alex-Sebastian-Ene/mind-over-chicken.git`
   into `/scratch/$USER/…` (or `git pull` an existing clone).
2. `bash rl/civ_war/slurm/setup.sh` on the login node: installs Rust with rustup into `$HOME`
   if missing, builds `civ_gym` and `civ_eval` `--release`, and makes a venv in
   `/scratch/$USER/civ_war/venv` (modules `python/3.12.6`, `gcc/14.2.0`; CPU torch).
3. `sbatch -p <cpu partition> rl/civ_war/slurm/train.sbatch` (on Iridis X: `-p amd_student`;
   `-N 1` is in the script). `--array=0-2` trains three seeds; resubmitting an index resumes it
   from its checkpoints. `CIV_WAR_CONFIG=local` or `CIV_WAR_MINUTES_SCALE=0.5` change the run.
4. Watch it: `squeue -u $USER`, `tail -f /scratch/$USER/civ_war/runs/full-s0/train.log`.
5. `sbatch -p <cpu partition> rl/civ_war/slurm/eval.sbatch` (32 unseen seeds, all scenarios).
6. `bash rl/civ_war/slurm/export.sh full-s0` packs the policies, the evaluation and the curves
   into `/scratch/$USER/civ_war/civ_war_full-s0.tar.gz`.
7. Copy it back: `scp iridis:/scratch/$USER/civ_war/civ_war_full-s0.tar.gz .`, unpack into
   `rl/civ_war/models/`, copy `council.mocp` and `commander.mocp` to `crates/moc-civ/policy/`,
   rebuild the wasm (`crates/moc-civ/build-wasm.sh`), re-run `civ_eval`, and commit.
