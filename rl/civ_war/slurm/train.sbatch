#!/usr/bin/env bash
# Train the war councils on one CPU node (#100). Submit from the repo root:
#
#   sbatch -p <cpu-partition> rl/civ_war/slurm/train.sbatch            # e.g. -p amd_student on Iridis X
#   sbatch -p <cpu-partition> --array=0-2 rl/civ_war/slurm/train.sbatch  # three seeds / league members
#
# Partition and account are deliberately not set here: pass -p (and -A if your account needs one)
# on the command line, or uncomment the marked lines. Resubmitting the same array index resumes
# from its last checkpoint (state.json + checkpoints/ in the run directory).
#
# Optional environment: CIV_WAR_CONFIG (full | local | path.json, default full),
# CIV_WAR_MINUTES_SCALE (scales every stage's budget, default 1), CIV_WAR_WORK (work dir).
#SBATCH -N 1
#SBATCH --job-name=civ-war-train
#SBATCH --cpus-per-task=64
#SBATCH --mem=96G
#SBATCH --time=10:00:00
#SBATCH --output=civ-war-train-%A_%a.out
#SBATCH --array=0
## ---- set these for your cluster, or pass them to sbatch ----
##SBATCH --partition=CHANGE_ME
##SBATCH --account=CHANGE_ME
set -euo pipefail
cd "${SLURM_SUBMIT_DIR:-$(pwd)}"
. rl/civ_war/slurm/common.sh
SEED="${SLURM_ARRAY_TASK_ID:-0}"
CONFIG="${CIV_WAR_CONFIG:-full}"
CPUS="${SLURM_CPUS_PER_TASK:-$(nproc)}"
# One env process per core, leaving a few for the learner.
NPROCS=$(( CPUS > 8 ? CPUS - 4 : CPUS - 1 ))
OUT="$CIV_WAR_WORK/runs/${CONFIG##*/}-s$SEED"
mkdir -p "$OUT"
echo "job ${SLURM_JOB_ID:-local} seed $SEED config $CONFIG on $(hostname): $NPROCS env processes, out $OUT"
export OMP_NUM_THREADS=2 MKL_NUM_THREADS=2
srun --cpu-bind=none "$VENV/bin/python" -u rl/civ_war/train.py --config "$CONFIG" --out "$OUT" --seed "$SEED" \
  --n-procs "$NPROCS" --minutes-scale "${CIV_WAR_MINUTES_SCALE:-1}" 2>&1 | tee -a "$OUT/train.log"
echo "done: $(date)"
