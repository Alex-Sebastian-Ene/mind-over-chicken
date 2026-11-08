"""Draw the training curves of a run as one dependency-free SVG (#100).

    python rl/civ_war/plot_curves.py rl/civ_war/runs/local-s0 rl/civ_war/curves.svg

One panel per metric (mean episode return, survival, mean step reward), every stage of the
curriculum one after another on a shared minutes axis.
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

COLORS = ["#2a6fdb", "#d9822b", "#2f9e44", "#c2255c", "#7048e8", "#1098ad", "#e67700"]
METRICS = [("mean_return", "mean episode return"), ("survival", "survival"), ("mean_step_reward", "reward per step")]


def load(run: Path):
    stages = []
    order = json.loads((run / "state.json").read_text())["done"] if (run / "state.json").exists() else []
    dirs = [run / s for s in order] + [d for d in sorted(run.iterdir()) if d.is_dir() and d.name not in order]
    offset = 0.0
    for d in dirs:
        p = d / "progress.csv"
        if not p.exists():
            continue
        rows = [r for r in csv.DictReader(open(p))]
        if not rows:
            continue
        pts = {m: [(offset + float(r["minutes"]), float(r[m])) for r in rows if r.get(m) not in ("", None)] for m, _ in METRICS}
        stages.append((d.name, pts))
        offset += float(rows[-1]["minutes"])
    return stages, offset


def svg(stages, total, out: Path):
    W, H, pad = 900, 210, 46
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H * len(METRICS) + 30}" font-family="sans-serif" font-size="11">',
             '<rect width="100%" height="100%" fill="white"/>']
    for i, (m, label) in enumerate(METRICS):
        y0 = 20 + i * H
        vals = [v for _, pts in stages for _, v in pts[m]]
        if not vals:
            continue
        lo, hi = min(vals), max(vals)
        if hi - lo < 1e-6:
            hi = lo + 1
        X = lambda t: pad + (W - 2 * pad) * t / max(total, 1e-6)  # noqa: E731
        Y = lambda v: y0 + H - 30 - (H - 50) * (v - lo) / (hi - lo)  # noqa: E731
        parts.append(f'<text x="{pad}" y="{y0 + 8}" font-weight="bold">{label}</text>')
        parts.append(f'<line x1="{pad}" y1="{y0 + H - 30}" x2="{W - pad}" y2="{y0 + H - 30}" stroke="#999"/>')
        parts.append(f'<text x="4" y="{Y(hi) + 4:.0f}">{hi:.2f}</text><text x="4" y="{Y(lo) + 4:.0f}">{lo:.2f}</text>')
        for k, (name, pts) in enumerate(stages):
            p = pts[m]
            if not p:
                continue
            c = COLORS[k % len(COLORS)]
            d = " ".join(f"{'M' if j == 0 else 'L'}{X(t):.1f},{Y(v):.1f}" for j, (t, v) in enumerate(p))
            parts.append(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="1.6"/>')
            if i == 0:
                parts.append(f'<text x="{X(p[0][0]) + 2:.0f}" y="{y0 + 22}" fill="{c}">{name}</text>')
        parts.append(f'<text x="{W - pad - 60}" y="{y0 + H - 14}">minutes ({total:.0f})</text>')
    parts.append("</svg>")
    out.write_text("\n".join(parts), encoding="utf-8")


if __name__ == "__main__":
    run, out = Path(sys.argv[1]), Path(sys.argv[2])
    stages, total = load(run)
    svg(stages, total, out)
    print(f"wrote {out} ({len(stages)} stages, {total:.0f} min)")
