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

## Results (the local run, 60 min on 8 CPU cores)

The cluster run (Iridis, one 64-core node, the `full` config up to 12 civs on 2560×1280) is
still training. These results are for the first policy, from the local run. All numbers are in
`eval_results.json` (`civ_eval`, unseen seeds 1 000 000+). Each world is warmed up for 300 s
under the rules, then played for 900 s with the same wandering player mood in every condition.

**Training** (`models/curves/*.csv`, `models/curves.svg`) took 60.2 min of wall time, with 7 env
processes and one learner:

| stage | setup | minutes | samples | episode return | survival |
|---|---|---|---|---|---|
| c1 | council: 2 civs, 640×320 | 12 | 72 k | −2.5 → −2.3 | 0.78 → 0.77 |
| c2 | council: 4 civs, 1280×640 | 20 | 133 k | −2.0 → +0.4 | 0.61 → 0.83 |
| c3 | council: 8 civs, 1280×640 | 12 | 104 k | +2.7 → +1.0 | 0.97 → 0.86 |
| k1 | commander: 4 civs, 1280×640 | 16 | 82 k | −1.3 → −0.5 | 0.85 → 0.94 |

The return is not comparable across stages: rewards depend on the world.

**All-council worlds against all-rules worlds** (same seeds; per civilisation):

| | duel 2×640² | four 4×1280² | eight 8×1280² | twelve 12×2560² (4 seeds) |
|---|---|---|---|---|
| survival | 0.67 vs 0.70 | 0.62 vs 0.65 | **0.85** vs 0.79 | **0.97** vs 0.91 |
| population after 900 s | 64 vs 54 | 84 vs 68 | **118** vs 81 | **197** vs 131 |
| villages lost | **0.28** vs 1.93 | **0.22** vs 2.00 | **0.22** vs 1.81 | **0.15** vs 1.77 |
| ages gained | +0.19 vs −0.84 | −1.39 vs −0.98 | +0.20 vs −0.39 | +0.57 vs +0.21 |
| wars per world | 22 vs 31 | 34 vs 65 | 31 vs 86 | 18 vs 74 |
| battles per world | 260 vs 43 | 384 vs 88 | 310 vs 102 | 219 vs 75 |

The councils keep more people and lose far fewer villages: they make fewer wars and sue for
peace. Their worlds also have many more pitched battles: a council that holds a rival stance
keeps border tension in the battle band (≥ 0.8), and battles are physics, not a decision.

**Head to head** (half the civilisations on the councils, half on the rules, in the same worlds;
a win is the higher end score `alive·10 + age + ln(1+pop) + 0.2·villages`):

| | duel | four | eight |
|---|---|---|---|
| councils vs rules: win rate (Elo gap) | 0.42 (−60) | 0.47 (−21) | 0.45 (−32) |
| final vs c1 checkpoint | 0.67 (+123) | 0.45 (−35) | 0.58 (+57) |
| trained vs scripted commander | 0.56 (+40) | 0.52 (+11) | 0.51 (+6) |

So after an hour the councils do **not** yet beat the rules head to head. Next to rule-driven
neighbours they keep more people but fewer villages: they capture 0.4–0.5 villages per
civilisation against the rules' 2.4–4.1. The commander is about even with the scripted one. The
longer cluster run is meant to fix this.

**Behaviour differs by traits** (one shared policy; the stances a council takes toward its
neighbours):

| | four: aggressive / friendly | eight: aggressive / friendly | duel: aggressive / friendly |
|---|---|---|---|
| rivalry or war stance | 0.50 / 0.17 | 0.33 / 0.26 | 0.54 / 0.20 |
| trade or alliance stance | 0.43 / 0.69 | 0.45 / 0.48 | 0.37 / 0.61 |
| alliances formed | 0.6 / 1.7 | 0.9 / 1.9 | 0.6 / 4.7 |
| share under arms | 0.14 / 0.05 | 0.09 / 0.08 | 0.18 / 0.10 |
| wars declared | 1.6 / 0.4 | 1.5 / 0.5 | 1.8 / 1.3 |

Aggressive means aggressiveness ≥ 0.6 and friendliness < 0.5; friendly is the reverse. The
correlation of aggressiveness with a hostile stance is 0.33–0.41.

**Calm against stress** (the player held calm, or stressed, the whole game):

| | duel | four | eight |
|---|---|---|---|
| councils: wars per world, calm / stress | 0.25 / 36 | 0.4 / 50 | 3.3 / 47 |
| councils: share of time at war | 0.01 / 0.44 | 0.02 / 0.48 | 0.01 / 0.23 |
| councils: rivalry or war stance | 0.04 / 0.43 | 0.08 / 0.44 | 0.12 / 0.40 |
| rules: wars per world, calm / stress | 5.3 / 42 | 10 / 56 | 15 / 107 |

**Strategy** (per all-council world of four civilisations; duel and eight in the JSON):

- 10.8 wars declared, 3.2 of them opportunistic (30 %): on a rival at war elsewhere, in a Dark
  Age, or split by strife;
- 3.2 gang-ups on the strongest, and 1.25 joins of an ally's war;
- 3.5 alliances formed;
- 10.8 peace offers, 49 % of them made while losing;
- 3.1 truces broken;
- during the 464 s spent at war on two fronts or more, an army was sent home 99 % of the time.

The war goal is almost always "take a village" or "raid". "Defend an ally" is never chosen, and
coalitions are rare.

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

Natively, 12 civilisations on 2560×1280 with about 3,700 agents cost 39 ms per simulated second
under the councils and 50 ms under the rules. The rules' worlds are busier with wars.

The wasm grows by about 160 KB: 97 KB of weights and about 65 KB of code.

## What's missing

- **The councils don't beat the rules head to head yet.** That is what the cluster run is for.
- **The commander is only as good as the scripted one.** It trained for 16 min.
- **Battles.** All-council worlds run many border battles.
- **Unused options.** The "defend an ally" war goal is never chosen.
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
