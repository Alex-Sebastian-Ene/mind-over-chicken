#!/bin/bash
# Cut the balanced patch set from the downloaded DEM (CPU; expect 15-40 min and ~4 GB RAM).
#SBATCH --job-name=td-prep
#SBATCH -p amd_student
#SBATCH -N 1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=4
#SBATCH --mem=16G
#SBATCH --time=01:30:00
#SBATCH --output=/scratch/ase1g23/moc_diffusion/logs/%x-%j.out
set -euo pipefail
source /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/env.sh
export OMP_NUM_THREADS=$SLURM_CPUS_PER_TASK
test -f "$ETOPO_FILE" || { echo "no $ETOPO_FILE: run fetch.sh first (after approval)"; exit 1; }
cd "$TD_CODE"
"$TD_PYTHON" prep_etopo.py --src "$ETOPO_FILE" --out "$TD_DATA" --scales 1,2,4,8
ls -la "$TD_DATA"
