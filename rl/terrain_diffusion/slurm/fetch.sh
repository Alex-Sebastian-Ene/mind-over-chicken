#!/bin/bash
# DOWNLOADS THE DATASET: run only after approval (#110).
#   ssh iridis 'bash -lc "bash /scratch/ase1g23/moc_diffusion/repo/rl/terrain_diffusion/slurm/fetch.sh"'
#
# NOAA NCEI ETOPO 2022 v1, 60 arc-second, ice surface, GeoTIFF: one file, 465,969,062 bytes
# (444 MiB). Public domain (CC0-1.0). Cite: NOAA NCEI 2022, ETOPO 2022 15 Arc-Second Global
# Relief Model, https://doi.org/10.25921/fd45-gt74
set -euo pipefail
source "$(dirname "$0")/env.sh"
mkdir -p "$TD_RAW"
# -C - resumes a partial download; --fail stops on HTTP errors.
curl --fail --location --retry 3 -C - -o "$ETOPO_FILE" "$ETOPO_URL"
size=$(stat -c %s "$ETOPO_FILE")
if [ "$size" != "$ETOPO_BYTES" ]; then
  echo "unexpected size $size (want $ETOPO_BYTES)" >&2
  exit 1
fi
sha256sum "$ETOPO_FILE" | tee "$ETOPO_FILE.sha256"
chmod a-w "$ETOPO_FILE"
