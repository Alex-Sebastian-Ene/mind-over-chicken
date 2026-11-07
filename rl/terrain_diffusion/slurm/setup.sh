#!/bin/bash
# ONE-OFF SETUP ON THE LOGIN NODE. Downloads software: run only after approval (#110).
#   ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/setup.sh"'
# (the first time, clone the repo by hand: see README.md "Iridis".)
#
# Installs into $TD_VENV (sizes checked against the indexes on 2026-11-07):
#   torch 2.9.1+cu128 from download.pytorch.org, the build the existing
#   /scratch/ase1g23/envs/argus env already runs on these L4 nodes: 900.9 MB wheel, plus
#   3,158 MB of nvidia-*-cu12 runtime wheels and triton 3.5.1 from PyPI (cuDNN 707 MB,
#   cuBLAS 594 MB, NCCL 322 MB, ...): about 4.1 GB downloaded, about 7 GB installed;
#   numpy, tifffile, imagecodecs (GeoTIFF decoding): about 60 MB.
set -euo pipefail
source "$(dirname "$0")/env.sh"

cd "$TD_REPO"
git fetch -q origin
git checkout -q feat/terrain-diffusion
git pull -q --ff-only

if [ ! -x "$TD_VENV/bin/python" ]; then
  python3 -m venv "$TD_VENV"
fi
"$TD_VENV/bin/python" -m pip install -q --upgrade pip
"$TD_VENV/bin/python" -m pip install -q "torch==2.9.1" --index-url https://download.pytorch.org/whl/cu128
"$TD_VENV/bin/python" -m pip install -q -r "$TD_CODE/requirements.txt"
"$TD_VENV/bin/python" -m pip cache purge -q || true   # keep the home/scratch quota clean
"$TD_VENV/bin/python" -c "import torch, numpy, tifffile; print('torch', torch.__version__, 'cuda build', torch.version.cuda)"
du -sh "$TD_VENV"
