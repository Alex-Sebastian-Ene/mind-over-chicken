#!/bin/bash
# Tiny real GPU job: checks the env, CUDA and the whole pipeline on synthetic terrain, then
# times 400 steps of the real model on the real data (~15 min in all).
#SBATCH --job-name=td-smoke
#SBATCH -p ecsstudents_l4
#SBATCH -A ecsstudents
#SBATCH -N 1
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --mem=16G
#SBATCH --time=00:30:00
#SBATCH --output=/scratch/ase1g23/moc_diffusion/logs/%x-%j.out
set -euo pipefail
source /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/env.sh
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv
"$TD_PYTHON" -c "import torch; print(torch.__version__, torch.cuda.is_available(), torch.cuda.get_device_name(0))"
cd "$TD_CODE"
"$TD_PYTHON" smoke_test.py --work "$TD_ROOT/smoke" --device cuda --steps 1000
# Real data, shipping widths: step time and memory for planning the full run.
"$TD_PYTHON" train.py --data "$TD_DATA" --out "$TD_ROOT/smoke/real_timing" --steps 400 --widths 24,32,48 \
  --batch 96 --crop 96 --log-every 50 --ckpt-every 400 --preview-every 400 --device cuda
nvidia-smi --query-gpu=memory.used --format=csv
