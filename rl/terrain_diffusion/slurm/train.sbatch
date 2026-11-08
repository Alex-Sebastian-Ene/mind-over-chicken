#!/bin/bash
# Teacher training on one L4 (measured: ~0.03 s/step at batch 96-128, so 200k steps < 2 h). Resumes from $RUN/ckpt.pt; Slurm sends USR1 five minutes before
# the time limit and train.py checkpoints and exits cleanly, so resubmitting continues.
#   sbatch --export=ALL,RUN=teacher_a,STEPS=150000,WIDTHS=24:32:48 train.sbatch   (":" because --export splits on ",")
#SBATCH --job-name=td-train
#SBATCH -p ecsstudents_l4
#SBATCH -A ecsstudents
#SBATCH -N 1
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --mem=24G
#SBATCH --time=03:00:00
#SBATCH --signal=B:USR1@300
#SBATCH --output=/scratch/ase1g23/moc_diffusion/logs/%x-%j.out
set -euo pipefail
source /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/env.sh
RUN="${RUN:-teacher}"
STEPS="${STEPS:-200000}"
WIDTHS="${WIDTHS:-24:32:48}"
WIDTHS="${WIDTHS//:/,}"
BATCH="${BATCH:-128}"
cd "$TD_CODE"
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader
"$TD_PYTHON" train.py --data "$TD_DATA" --out "$TD_RUNS/$RUN" --steps "$STEPS" --widths "$WIDTHS" \
  --batch "$BATCH" --crop 96 --preview-every 10000 --device cuda &
PID=$!
trap 'kill -USR1 $PID; wait $PID' USR1
wait $PID
