# Shared settings for the #100 cluster scripts (sourced, not run).
# Everything heavy lives under $CIV_WAR_WORK (default: /scratch/$USER/civ_war, else $HOME/civ_war).
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
if [ -z "${CIV_WAR_WORK:-}" ]; then
  if [ -d "/scratch/$USER" ]; then CIV_WAR_WORK="/scratch/$USER/civ_war"; else CIV_WAR_WORK="$HOME/civ_war"; fi
fi
export CIV_WAR_WORK
export CARGO_TARGET_DIR="$CIV_WAR_WORK/target"
export CIV_GYM="$CARGO_TARGET_DIR/release/examples/civ_gym"
export CIV_EVAL="$CARGO_TARGET_DIR/release/examples/civ_eval"
VENV="$CIV_WAR_WORK/venv"
# Modules (Iridis X names; harmless elsewhere). Edit if your cluster differs.
PY_MODULE="${PY_MODULE:-python/3.12.6}"
GCC_MODULE="${GCC_MODULE:-gcc/14.2.0}"
if command -v module >/dev/null 2>&1; then
  module load "$PY_MODULE" >/dev/null 2>&1 || echo "note: module $PY_MODULE not loaded"
  module load "$GCC_MODULE" >/dev/null 2>&1 || echo "note: module $GCC_MODULE not loaded"
fi
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
