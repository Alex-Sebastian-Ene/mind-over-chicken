#!/usr/bin/env python3
"""Offline development and demo-day server for Mind Over Chicken.

Features:
- Forces no-cache headers so browsers and judges never execute stale ES modules or WASM binaries.
- Ensures correct application/wasm and application/javascript MIME types.
- Provides CORS headers.
- Verifies that WebAssembly build artifacts exist before launching, checking staleness against crates/engine.
- Optional --rebuild flag to automatically invoke wasm-pack build.
"""

import argparse
import http.server
import os
import shutil
import socketserver
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class NoCacheHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()

    def guess_type(self, path):
        if str(path).endswith(".wasm"):
            return "application/wasm"
        if str(path).endswith(".js"):
            return "application/javascript"
        return super().guess_type(path)


def rebuild_pkg():
    print("Rebuilding WebAssembly package (crates/engine)...")
    cmd = ["wasm-pack", "build", "--target", "web", "--out-dir", "../../pkg"]
    engine_dir = ROOT / "crates" / "engine"
    res = subprocess.run(cmd, cwd=str(engine_dir))
    if res.returncode != 0:
        print("Error: wasm-pack build failed.", file=sys.stderr)
        sys.exit(res.returncode)
    print("WASM rebuild complete.")


def check_pkg_sync(auto_rebuild=False):
    wasm_file = ROOT / "pkg" / "mind_over_chicken_engine_bg.wasm"
    engine_src = ROOT / "crates" / "engine" / "src"

    if not wasm_file.is_file():
        print(f"Warning: {wasm_file} was not found.", file=sys.stderr)
        if auto_rebuild and shutil.which("wasm-pack"):
            rebuild_pkg()
            return
        print("Build WebAssembly first: wasm-pack build crates/engine --target web --out-dir ../../pkg", file=sys.stderr)
        return

    wasm_mtime = wasm_file.stat().st_mtime
    stale_files = []

    if engine_src.is_dir():
        for rs_file in engine_src.rglob("*.rs"):
            if rs_file.stat().st_mtime > wasm_mtime:
                stale_files.append(rs_file.relative_to(ROOT))

    cargo_toml = ROOT / "crates" / "engine" / "Cargo.toml"
    if cargo_toml.is_file() and cargo_toml.stat().st_mtime > wasm_mtime:
        stale_files.append(cargo_toml.relative_to(ROOT))

    if stale_files:
        print(f"\n⚠️  [Chore #41] pkg/ is out of sync with crates/engine!", file=sys.stderr)
        print(f"   The following source files are newer than the committed WASM binary:", file=sys.stderr)
        for f in stale_files[:5]:
            print(f"     - {f}", file=sys.stderr)
        if len(stale_files) > 5:
            print(f"     - ... and {len(stale_files) - 5} more files", file=sys.stderr)

        if auto_rebuild and shutil.which("wasm-pack"):
            rebuild_pkg()
        else:
            print("   Run: wasm-pack build crates/engine --target web --out-dir ../../pkg", file=sys.stderr)
            print("   Or run server with --rebuild to compile automatically.\n", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="Serve Mind Over Chicken locally for demo and offline test.")
    parser.add_argument("--port", type=int, default=8000, help="Port to serve on (default: 8000)")
    parser.add_argument("--bind", default="127.0.0.1", help="Address to bind (default: 127.0.0.1)")
    parser.add_argument("--rebuild", action="store_true", help="Auto-rebuild WebAssembly package before starting")
    args = parser.parse_args()

    check_pkg_sync(auto_rebuild=args.rebuild)

    handler = NoCacheHTTPRequestHandler
    with socketserver.TCPServer((args.bind, args.port), handler) as httpd:
        url = f"http://{args.bind}:{args.port}/"
        print(f"Mind Over Chicken running at: {url}")
        print("Serving with cache-busting headers enabled. Press Ctrl+C to stop.")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down server.")


if __name__ == "__main__":
    main()
