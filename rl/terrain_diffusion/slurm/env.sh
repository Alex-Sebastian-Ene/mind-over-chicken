#!/bin/bash
# Shared settings for the terrain-diffusion jobs on Iridis. Sourced by every script here.
# Everything lives under one scratch directory; nothing touches /scratch/ase1g23/moc (PPO).
export TD_ROOT="${TD_ROOT:-/scratch/ase1g23/moc_diffusion}"
export TD_REPO="$TD_ROOT/repo"                  # checkout of feat/terrain-diffusion
export TD_CODE="$TD_REPO/rl/terrain_diffusion"
export TD_RAW="$TD_ROOT/raw"                    # the downloaded DEM (read-only after fetch)
export TD_DATA="$TD_ROOT/data"                  # prep_etopo.py output
export TD_RUNS="$TD_ROOT/runs"
export TD_VENV="${TD_VENV:-$TD_ROOT/venv}"
export ETOPO_URL="https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif"
export ETOPO_FILE="$TD_RAW/ETOPO_2022_v1_60s_N90W180_surface.tif"
export ETOPO_BYTES=465969062

module load python/3.12.6 >/dev/null 2>&1 || true
# Our venv sits on top of the read-only argus conda env (torch 2.9.1+cu128); see setup.sh.
export TD_PYTHON="${TD_PYTHON:-$TD_VENV/bin/python}"
export PYTHONUNBUFFERED=1
mkdir -p "$TD_ROOT/logs" "$TD_RUNS"
