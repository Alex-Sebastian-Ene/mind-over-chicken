#!/usr/bin/env bash
# Evaluate a finished run against the rules on unseen seeds (#100):
#
#   sbatch -p <cpu-partition> rl/civ_war/slurm/eval.sbatch            # run full-s0
#   RUN=full-s1 sbatch -p <cpu-partition> rl/civ_war/slurm/eval.sbatch
#
#SBATCH -N 1
#SBATCH --job-name=civ-war-eval
#SBATCH --cpus-per-task=64
#SBATCH --mem=32G
#SBATCH --time=02:00:00
#SBATCH --output=civ-war-eval-%j.out
## ---- set these for your cluster, or pass them to sbatch ----
##SBATCH --partition=CHANGE_ME
##SBATCH --account=CHANGE_ME
set -euo pipefail
cd "${SLURM_SUBMIT_DIR:-$(pwd)}"
. rl/civ_war/slurm/common.sh
RUN_DIR="$CIV_WAR_WORK/runs/${RUN:-full-s0}"
OLD=$(ls "$RUN_DIR"/league/council-c1_*-final.mocp 2>/dev/null | head -1 || true)
CPUS="${SLURM_CPUS_PER_TASK:-$(nproc)}"
"$CIV_EVAL" --council "$RUN_DIR/council.mocp" --commander "$RUN_DIR/commander.mocp" ${OLD:+--old "$OLD"} \
  --seeds "${SEEDS:-32}" --threads "$CPUS" --scenarios "${SCENARIOS:-duel,four,eight,twelve_big}" \
  --out "$RUN_DIR/eval_results.json"
echo "wrote $RUN_DIR/eval_results.json"
