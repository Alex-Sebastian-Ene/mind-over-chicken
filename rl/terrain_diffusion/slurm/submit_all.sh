#!/bin/bash
# Submit prep -> teacher -> distillation as a dependency chain (one GPU job at a time).
# Run on the login node after setup.sh and fetch.sh (both need approval).
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"
prep=$(sbatch --parsable "$D/prep.sbatch")
train=$(sbatch --parsable --dependency=afterok:$prep --export=ALL,RUN=teacher "$D/train.sbatch")
distill=$(sbatch --parsable --dependency=afterok:$train --export=ALL,TEACHER=teacher "$D/distill.sbatch")
echo "prep $prep -> train $train -> distill $distill"
