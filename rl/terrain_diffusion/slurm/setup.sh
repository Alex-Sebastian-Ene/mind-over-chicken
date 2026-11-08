#!/bin/bash
# ONE-OFF SETUP ON THE LOGIN NODE (approved on #110: "reuse argus env", option B).
#   ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/setup.sh"'
# (the first time, clone the repo by hand: see README.md "Iridis".)
#
# No CUDA download: PyTorch 2.9.1+cu128 comes read-only from the existing
# /scratch/ase1g23/envs/argus conda env. We make a small venv on top of it
# (--system-site-packages, so argus's torch and numpy are visible but never modified) and
# install only tifffile + imagecodecs into it (GeoTIFF decoding, about 60 MB).
set -euo pipefail
source "$(dirname "$0")/env.sh"
ARGUS_PY=/scratch/ase1g23/envs/argus/bin/python

cd "$TD_REPO"
git fetch -q origin
git checkout -q feat/terrain-diffusion
git pull -q --ff-only

if [ ! -x "$TD_VENV/bin/python" ]; then
  "$ARGUS_PY" -m venv --system-site-packages "$TD_VENV"
fi
# --no-deps: numpy is argus's; nothing else may be pulled in.
"$TD_VENV/bin/python" -m pip install -q --no-deps --disable-pip-version-check tifffile imagecodecs
"$TD_VENV/bin/python" -m pip cache purge -q 2>/dev/null || true
"$TD_VENV/bin/python" -c "import torch, numpy, tifffile, imagecodecs; print('torch', torch.__version__, 'from', torch.__file__); print('numpy', numpy.__version__, 'tifffile', tifffile.__version__, 'imagecodecs', imagecodecs.__version__)"
du -sh "$TD_VENV"
