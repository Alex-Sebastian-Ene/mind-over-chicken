#!/usr/bin/env python3
"""Offline development and demo-day server for Mind Over Chicken.

Features:
- Forces no-cache headers so browsers and judges never execute stale ES modules or WASM binaries.
- Ensures correct application/wasm and application/javascript MIME types.
- Provides CORS headers.
- Verifies that WebAssembly build artifacts exist before launching.
"""

import argparse
import http.server
import os
import socketserver
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


def main():
    parser = argparse.ArgumentParser(description="Serve Mind Over Chicken locally for demo and offline test.")
    parser.add_argument("--port", type=int, default=8000, help="Port to serve on (default: 8000)")
    parser.add_argument("--bind", default="127.0.0.1", help="Address to bind (default: 127.0.0.1)")
    args = parser.parse_args()

    wasm_file = ROOT / "pkg" / "mind_over_chicken_engine_bg.wasm"
    if not wasm_file.is_file():
        print(f"Warning: {wasm_file} was not found.", file=sys.stderr)
        print("Build WebAssembly first: wasm-pack build crates/engine --target web --out-dir ../../pkg", file=sys.stderr)

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
