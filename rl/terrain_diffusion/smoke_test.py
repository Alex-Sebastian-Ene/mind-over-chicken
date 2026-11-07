"""End-to-end smoke test on synthetic terrain (no download, CPU is fine, ~minutes):
prep -> teacher training -> one distillation round -> export -> reload.

    python smoke_test.py --work /tmp/td_smoke [--device cuda]

Then check the Rust runtime reads the export:
    MOCD_WEIGHTS=/tmp/td_smoke/relief.mocd cargo test -p moc-diffusion --test detail -- --ignored
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def run(*args: str) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run([sys.executable, *args], cwd=HERE, check=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--steps", type=int, default=300)
    ap.add_argument("--batch", type=int, default=16)
    ap.add_argument("--crop", type=int, default=64)
    args = ap.parse_args()
    t0 = time.time()
    w = args.work
    data, teacher, student = w / "data", w / "teacher", w / "student4"
    run("prep_etopo.py", "--synthetic", "--out", str(data))
    log = str(max(5, args.steps // 6))
    common = ["--data", str(data), "--batch", str(args.batch), "--crop", str(args.crop), "--device", args.device,
              "--log-every", log, "--ckpt-every", log]
    run("train.py", *common, "--out", str(teacher), "--steps", str(args.steps), "--widths", "8,12,16",
        "--preview-every", str(args.steps), "--preview-steps", "8", "--warmup", str(max(5, args.steps // 6)))
    run("train.py", *common, "--out", str(student), "--steps", str(max(10, args.steps // 3)), "--distill-from",
        str(teacher / "ckpt.pt"), "--student-steps", "4", "--preview-every", str(max(10, args.steps // 3)), "--warmup", "10")
    run("export.py", "--ckpt", str(student / "ckpt.pt"), "--out", str(w / "relief.mocd"))
    rows = [json.loads(line) for line in (teacher / "metrics.jsonl").read_text().splitlines()]
    first, last = rows[0], rows[-1]
    print(f"teacher loss {first['loss']:.3f} -> {last['loss']:.3f} (val {first['val']:.3f} -> {last['val']:.3f})")
    print(f"smoke test passed in {time.time() - t0:.0f}s; previews in {teacher} and {student}")


if __name__ == "__main__":
    main()
