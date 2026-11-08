#!/bin/bash
# Progressive distillation 8 -> 4 DDIM steps (two rounds from the teacher's 16), then export.
#   sbatch --export=ALL,TEACHER=teacher distill.sbatch
#SBATCH --job-name=td-distill
#SBATCH -p ecsstudents_l4
#SBATCH -A ecsstudents
#SBATCH -N 1
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --mem=24G
#SBATCH --time=02:00:00
#SBATCH --signal=B:USR1@300
#SBATCH --output=/scratch/ase1g23/moc_diffusion/logs/%x-%j.out
set -euo pipefail
source /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/env.sh
TEACHER="${TEACHER:-teacher}"
ROUND_STEPS="${ROUND_STEPS:-20000}"
cd "$TD_CODE"
prev="$TD_RUNS/$TEACHER/ckpt.pt"
for n in 8 4; do
  out="$TD_RUNS/${TEACHER}_d$n"
  "$TD_PYTHON" train.py --data "$TD_DATA" --out "$out" --distill-from "$prev" --student-steps "$n" \
    --steps "$ROUND_STEPS" --lr 1e-4 --warmup 500 --device cuda &
  PID=$!
  trap 'kill -USR1 $PID; wait $PID; exit 0' USR1
  wait $PID
  prev="$out/ckpt.pt"
done
"$TD_PYTHON" export.py --ckpt "$TD_RUNS/$TEACHER/ckpt.pt" --out "$TD_RUNS/relief_${TEACHER}.mocd" --steps 8
"$TD_PYTHON" export.py --ckpt "$prev" --out "$TD_RUNS/relief_${TEACHER}_d4.mocd"
