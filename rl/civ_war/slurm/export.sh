#!/usr/bin/env bash
# Pack a finished run into one small tarball to copy back (#100): the two policies (MOCP), the
# evaluation, the training curves and the run's metadata.
#
#   bash rl/civ_war/slurm/export.sh [run-name]       # default full-s0
#   scp <cluster>:<work>/civ_war_<run>.tar.gz .      # then unpack into rl/civ_war/models/
set -euo pipefail
. "$(dirname "$0")/common.sh"
RUN="${1:-full-s0}"
RUN_DIR="$CIV_WAR_WORK/runs/$RUN"
STAGE="$CIV_WAR_WORK/export-$RUN"
rm -rf "$STAGE" && mkdir -p "$STAGE/curves"
cp "$RUN_DIR/council.mocp" "$STAGE/"
[ -f "$RUN_DIR/commander.mocp" ] && cp "$RUN_DIR/commander.mocp" "$STAGE/"
OLD=$(ls "$RUN_DIR"/league/council-c1_*-final.mocp 2>/dev/null | head -1 || true)
[ -n "$OLD" ] && cp "$OLD" "$STAGE/council-c1.mocp"
for f in eval_results.json train_meta.json state.json; do [ -f "$RUN_DIR/$f" ] && cp "$RUN_DIR/$f" "$STAGE/"; done
for d in "$RUN_DIR"/*/; do
  [ -f "$d/progress.csv" ] && cp "$d/progress.csv" "$STAGE/curves/$(basename "$d").csv"
done
TAR="$CIV_WAR_WORK/civ_war_$RUN.tar.gz"
tar -czf "$TAR" -C "$STAGE" .
ls -l "$TAR"
echo "copy back: scp <cluster>:$TAR . && tar -xzf civ_war_$RUN.tar.gz -C rl/civ_war/models/"
