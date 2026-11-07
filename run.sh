#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

PORT="${PORT:-8000}"
MODE="${1:-game}"
shift || true

VENV="bridge/.venv"
PY="$VENV/bin/python"
URL="http://127.0.0.1:$PORT/"

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

port_busy() {
  (ss -ltn 2>/dev/null || netstat -ltn 2>/dev/null) | grep -q "[:.]$1 " || return 1
}

free_port() {
  if port_busy "$PORT"; then
    pkill -f "NoCacheHandler.*$PORT" 2>/dev/null || true
    sleep 0.3
  fi
  port_busy "$PORT" && die "port $PORT is busy (PORT=$((PORT + 1)) ./run.sh to use another)"
  if [ ${#bridge_args[@]} -gt 0 ] && port_busy 8765; then
    pkill -9 -f "[m]use_bridge.py" 2>/dev/null || true
    sleep 0.4
  fi
  [ ${#bridge_args[@]} -gt 0 ] && port_busy 8765 \
    && die "port 8765 is busy: another bridge is already running (stop it, or use --port)"
  return 0
}

ensure_bridge() {
  if [ ! -x "$PY" ]; then
    say "creating Python venv in $VENV"
    python3 -m venv "$VENV" || die "could not create venv (python3-venv installed?)"
    "$VENV/bin/pip" -q install --upgrade pip >/dev/null
  fi
  if ! "$PY" -c 'import brainflow, websockets, numpy' >/dev/null 2>&1; then
    say "installing bridge requirements (first run only, ~1 min)"
    "$VENV/bin/pip" -q install -r bridge/requirements.txt \
      || die "pip install failed; see bridge/README.md"
  fi
}

case "$MODE" in
  game)             bridge_args=() ;;
  bridge|synthetic) bridge_args=(--fake-blinks "$@") ;;
  muse2)            bridge_args=(--board muse2 "$@") ;;
  muses)            bridge_args=(--board muses "$@") ;;
  muse2-bled)       bridge_args=(--board muse2-bled "$@") ;;
  muses-bled)       bridge_args=(--board muses-bled "$@") ;;
  *)                bridge_args=("$MODE" "$@") ;;
esac

if [ "$MODE" = "-h" ] || [ "$MODE" = "--help" ] || [ "$MODE" = "help" ]; then
  cat <<EOF
usage: ./run.sh [mode] [bridge flags...]

  game        emulated headset only, no bridge (keyboard/mouse demo)
  synthetic   bridge on BrainFlow's synthetic board, fake blinks every ~4 s
  muse2       real Muse 2 over native Bluetooth (default for this headset)
  muses       real Muse S over native Bluetooth
  muse2-bled  Muse 2 through a BLED112 dongle (pass --serial-port /dev/ttyACM0)
  muses-bled  Muse S through a BLED112 dongle

Extra flags go to bridge/muse_bridge.py, e.g. tuning the blink threshold:

  ./run.sh muse2 -v --blink-threshold 250
  ./run.sh muse2 --serial-number Muse-1A2B --timeout 20
  ./run.sh synthetic --fake-blink-every 2

PORT=8001 ./run.sh muse2   serves the game on another port.
EOF
  exit 0
fi

free_port
[ ${#bridge_args[@]} -gt 0 ] && ensure_bridge

bridge_pid=""
cleanup() {
  [ -n "$bridge_pid" ] && kill "$bridge_pid" 2>/dev/null || true
  [ -n "${server_pid:-}" ] && kill "$server_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

say "serving $(pwd) at $URL (no-cache)"
python3 -c '
import http.server, socketserver, sys
port = int(sys.argv[1])
class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()
    def log_message(self, format, *args):
        pass
socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", port), NoCacheHandler) as httpd:
    httpd.serve_forever()
' "$PORT" >/dev/null 2>&1 &
server_pid=$!

for _ in $(seq 1 50); do
  curl -sf -o /dev/null "$URL" && break
  sleep 0.1
done

if [ ${#bridge_args[@]} -gt 0 ]; then
  say "starting headset bridge: ${bridge_args[*]}"
  "$PY" bridge/muse_bridge.py "${bridge_args[@]}" &
  bridge_pid=$!
fi

if [ -z "${NO_OPEN:-}" ]; then
  ( sleep 0.5
    if command -v xdg-open >/dev/null; then xdg-open "$URL" >/dev/null 2>&1 || true
    elif command -v open >/dev/null; then open "$URL" || true
    fi ) &
fi

cat <<EOF

  Mind over Chicken  ->  $URL
  ${MODE} mode. Ctrl+C stops everything.

EOF

if [ ${#bridge_args[@]} -gt 0 ]; then
  printf '  Bridge flags: %s\n' "${bridge_args[*]}"
  printf '  In the game, open "Connect a real headset" and press Connect.\n'
  case "$MODE" in
    muse2|muses) printf '  Headset on, worn, and not connected to the Muse phone app.\n' ;;
  esac
  printf '\n'
fi

wait "$server_pid"
