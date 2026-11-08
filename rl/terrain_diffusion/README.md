# Learned terrain relief (#110): training tooling

Offline PyTorch tooling for the small diffusion model that gives random maps realistic
relief. Nothing here ships: the game runs the exported weights in Rust
(`crates/moc-diffusion`), in wasm, off the main thread for the big map.

## How it fits

`moc-terrain` builds a coarse map (plates, continents) and calls its `ReliefDetail` hook
with coarse heights, uplift and land weight. `moc-diffusion`'s `LearnedDiffusion` implements
the hook: the procedural stage first shapes the coarse grid (DLA ranges along the uplift,
erosion), then the model resamples it onto the model grid (2 model pixels per coarse cell),
adds a random **roughness** field so regions get different landforms, and runs a few DDIM
steps of a conditional UNet over the whole torus (wrap-around padding, so no patches and no
seams). The generated residual is added to the coarse heights, and the coarse land/sea
layout is kept. Every run is random.

In the game, the engine's `RustWorldGen.use_learned_relief(true)` turns it on for one wasm
instance; only the worldgen worker calls it, so the model never runs on the main thread.
No weights (`crates/moc-diffusion/weights/relief.mocd` absent, as now), or weights that fail
to load, means exactly today's procedural relief.

| File | What |
|---|---|
| `model.py` | `ReliefUNet`: space-to-depth, 3 levels, FiLM time conditioning, no global ops. Mirrored op-for-op by `crates/moc-diffusion/src/unet.rs`. |
| `diffusion.py` | Cosine VP schedule, v-prediction loss, DDIM sampler (mirrored by `sampler.rs`). |
| `data.py` | Metres to game units, the coarse/uplift/land/roughness conditioning, augmentation, synthetic terrain. |
| `prep_etopo.py` | Global DEM to a balanced, multi-scale patch set with landform classes. |
| `train.py` | Teacher training and progressive distillation; checkpoint/resume, EMA, previews. |
| `export.py` | Checkpoint to `.mocd` (f16 weights + JSON header), with a reload self-check. |
| `mocd.py` | The `.mocd` container (reader/writer). |
| `make_fixtures.py` | Synthetic-weight fixtures for the Rust parity tests. |
| `smoke_test.py` | prep, train, distil, export on synthetic terrain: no download needed. |
| `slurm/` | Iridis jobs: `setup.sh`, `fetch.sh` (both download, approval first), `prep.sbatch`, `smoke.sbatch`, `train.sbatch`, `distill.sbatch`, `submit_all.sh`. |

## Data

NOAA NCEI **ETOPO 2022 v1, 60 arc-second, ice surface** GeoTIFF: one 444 MiB file, public
domain (CC0-1.0). Cite: NOAA National Centers for Environmental Information, 2022, *ETOPO 2022
15 Arc-Second Global Relief Model*, https://doi.org/10.25921/fd45-gt74.

`prep_etopo.py` area-averages it to 1, 2, 4 and 8 arc-min, cuts 128 x 128 patches with
square ground pixels between 78S and 78N, and classifies each one: ocean, coast, polar_coast,
glaciated, plains, plateau, basin, hills, low_mountains, high_mountains (relief thresholds
are quantiles per scale). Open ocean is capped at 6% of the set. Training draws classes
evenly (ocean at half weight), then scales evenly, then a patch, with a random 96 px crop and
one of 8 flips/rotations, so neither ocean nor flat plains dominate. Longitudes 150E-180E are
held out for validation.

Heights become game units (sea 0, peaks ~1, floor -1) with a log curve, so lowlands keep
their relief: land `ln(1 + m/500) / ln(13)`, sea `-ln(1 + |m|/1500) / ln(5)`.

## Run

Local (CPU torch is enough):

```bash
python rl/terrain_diffusion/smoke_test.py --work ../td_smoke
MOCD_WEIGHTS=../td_smoke/relief.mocd cargo test -p moc-diffusion --test detail -- --ignored
python rl/terrain_diffusion/make_fixtures.py   # after changing model.py or diffusion.py
```

Iridis (approved on #110: the ETOPO download, and reusing the existing argus env's CUDA
torch read-only, with only tifffile + imagecodecs installed in our own small venv):

```bash
ssh iridis 'git clone -b feat/terrain-diffusion https://github.com/Alex-Sebastian-Ene/mind-over-chicken /scratch/ase1g23/moc_diffusion/repo'
ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/setup.sh"'
ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/fetch.sh"'
ssh iridis 'bash -lc "sbatch /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/smoke.sbatch"'
ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/submit_all.sh"'
```

Then copy `runs/relief_d4.mocd` to `crates/moc-diffusion/weights/relief.mocd`; the crate
embeds it at build time when present.

Everything lives under `/scratch/ase1g23/moc_diffusion` (`raw/`, `data/`, `runs/`, `logs/`,
`venv/`); `/scratch/ase1g23/envs/argus` is only read. Jobs request `-N 1` and one L4; train and distil resume from their checkpoints.
