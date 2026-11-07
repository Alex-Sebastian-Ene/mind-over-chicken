#!/bin/bash
# Tiny real GPU job: checks the env, CUDA and the whole pipeline on synthetic terrain (~10 min).
#SBATCH --job-name=td-smoke
#SBATCH -p ecsstudents_l4
#SBATCH -A ecsstudents
#SBATCH -N 1
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --mem=16G
#SBATCH --time=00:20:00
#SBATCH --output=/scratch/ase1g23/moc_diffusion/logs/%x-%j.out
set -euo pipefail
source /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/env.sh
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv
"$TD_PYTHON" -c "import torch; print(torch.__version__, torch.cuda.is_available(), torch.cuda.get_device_name(0))"
cd "$TD_CODE"
"$TD_PYTHON" smoke_test.py --work "$TD_ROOT/smoke" --device cuda --steps 1000
