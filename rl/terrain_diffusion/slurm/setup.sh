#!/bin/bash
# ONE-OFF SETUP ON THE LOGIN NODE. Downloads software: run only after approval (#110).
#   ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/setup.sh"'
# (the first time, clone the repo by hand: see README.md "Iridis".)
#
# Installs into $TD_VENV:
#   torch 2.9.1 + CUDA 12.8 wheels from download.pytorch.org (the same build the existing
#   /scratch/ase1g23/envs/argus env runs on these L4 nodes): ~0.9 GB torch wheel plus ~2.5 GB
#   of nvidia-* runtime wheels from PyPI, ~3.4 GB download, ~7 GB installed;
#   numpy, tifffile, imagecodecs (GeoTIFF decoding): ~60 MB.
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
