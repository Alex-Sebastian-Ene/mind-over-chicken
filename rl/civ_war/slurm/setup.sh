#!/usr/bin/env bash
# One-time setup on the cluster login node (#100): Rust (rustup, into $HOME) if missing, the
# civ_gym env server and civ_eval built --release for this machine, and a Python venv with the
# training tooling (CPU torch). Re-run after `git pull` to rebuild.
#
#   bash rl/civ_war/slurm/setup.sh
set -euo pipefail
. "$(dirname "$0")/common.sh"
mkdir -p "$CIV_WAR_WORK"
if ! command -v cargo >/dev/null 2>&1; then
  echo "installing Rust with rustup into $HOME/.cargo"
  # The modules' LD_LIBRARY_PATH (gcc, python) breaks the system curl: run the installer without it.
  RUSTUP_INIT="$CIV_WAR_WORK/rustup-init.sh"
  env -u LD_LIBRARY_PATH curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$RUSTUP_INIT"
  env -u LD_LIBRARY_PATH sh "$RUSTUP_INIT" -y --profile minimal --default-toolchain stable
  . "$HOME/.cargo/env"
fi
cd "$REPO"
cargo build --release -p moc-civ --example civ_gym --example civ_eval
ls -l "$CIV_GYM" "$CIV_EVAL"
PY=python3
command -v python3.12 >/dev/null 2>&1 && PY=python3.12
if [ ! -x "$VENV/bin/python" ]; then
  "$PY" -m venv "$VENV"
fi
"$VENV/bin/python" -m pip install --quiet --upgrade pip
"$VENV/bin/python" -m pip install --quiet "torch==2.15.0" --index-url https://download.pytorch.org/whl/cpu \
  || "$VENV/bin/python" -m pip install --quiet torch --index-url https://download.pytorch.org/whl/cpu
grep -v '^torch' rl/civ_war/requirements.txt > "$CIV_WAR_WORK/requirements-notorch.txt"
"$VENV/bin/python" -m pip install --quiet -r "$CIV_WAR_WORK/requirements-notorch.txt"
"$VENV/bin/python" -c "import torch, stable_baselines3, sb3_contrib; print('torch', torch.__version__, 'sb3', stable_baselines3.__version__, 'contrib', sb3_contrib.__version__)"
echo "{\"cmd\":\"spec\"}" | "$CIV_GYM" | head -c 200; echo
echo "setup done: work dir $CIV_WAR_WORK"
