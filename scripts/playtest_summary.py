#!/usr/bin/env python3
"""Summarise a playtest (#94): task success, timings, issues by severity x frequency, SUS,
NASA-TLX, PXI, the game's own items, and three-day retention.

Usage:
    python scripts/playtest_summary.py docs/playtest/observation_sheet.csv [docs/playtest/survey_responses.csv] [--json]

Standard library only. Rows for participant P00 are the templates' examples and are ignored.
"""
from __future__ import annotations

import csv
import json
import statistics
import sys
from collections import defaultdict

EXAMPLE = "P00"
SUCCESS_ANY = {"S", "H"}  # unaided, or after the hint
PXI = ["pxi_meaning", "pxi_curiosity", "pxi_mastery", "pxi_autonomy", "pxi_immersion",
       "pxi_progress", "pxi_audiovisual", "pxi_challenge", "pxi_ease", "pxi_goals"]
GAME_ITEMS = ["mind_control", "mind_understand", "mind_fair", "mind_tension",
              "replay_intent", "replay_tomorrow", "recommend"]
TLX = ["tlx_mental", "tlx_physical", "tlx_temporal", "tlx_performance", "tlx_effort", "tlx_frustration"]


def read(path):
    with open(path, newline="", encoding="utf-8") as f:
        return [r for r in csv.DictReader(f) if (r.get("participant") or "").strip() not in ("", EXAMPLE)]


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def describe(values):
    """Median and interquartile range of the numbers given (None if empty)."""
    xs = sorted(v for v in values if v is not None)
    if not xs:
        return None
    if len(xs) == 1:
        return {"n": 1, "median": xs[0], "q1": xs[0], "q3": xs[0]}
    q = statistics.quantiles(xs, n=4, method="inclusive")
    return {"n": len(xs), "median": statistics.median(xs), "q1": q[0], "q3": q[2]}


def sus_score(row):
    """SUS 0-100: odd items score x-1, even items 5-x, the sum times 2.5. None if incomplete."""
    total = 0.0
    for i in range(1, 11):
        x = num(row.get(f"sus{i}"))
        if x is None:
            return None
        total += (x - 1) if i % 2 else (5 - x)
    return total * 2.5


def tlx_raw(row):
    xs = [num(row.get(k)) for k in TLX]
    return None if any(x is None for x in xs) else sum(xs) / len(xs)


def summarise(obs, survey=None):
    obs = [r for r in obs if (r.get("participant") or "").strip() not in ("", EXAMPLE)]
    survey = [r for r in (survey or []) if (r.get("participant") or "").strip() not in ("", EXAMPLE)]
    s1 = [r for r in obs if (r.get("session") or "1").strip() == "1"]
    people = sorted({r["participant"] for r in s1})
    n = len(people)
    out = {"participants": n}

    out["setup_seconds"] = describe(num(r["seconds"]) for r in s1 if r["type"] == "setup")
    out["first_action_seconds"] = describe(num(r["seconds"]) for r in s1 if r["type"] == "first_action")

    # Tasks: the last recorded outcome per participant and task.
    last = {}
    for r in s1:
        if r["type"] == "task" and r.get("task"):
            last[(r["participant"], r["task"])] = r
    tasks = defaultdict(list)
    for (p, t), r in last.items():
        tasks[t].append(r)
    out["tasks"] = {}
    for t in sorted(tasks):
        rows = tasks[t]
        codes = [r["outcome"].strip().upper() for r in rows]
        out["tasks"][t] = {
            "n": len(rows),
            "unaided": sum(c == "S" for c in codes) / len(rows),
            "any_success": sum(c in SUCCESS_ANY for c in codes) / len(rows),
            "partial": sum(c == "P" for c in codes) / len(rows),
            "fail": sum(c == "F" for c in codes) / len(rows),
            "seconds": describe(num(r["seconds"]) for r in rows),
        }

    # Confusion moments per participant.
    conf = defaultdict(int)
    for r in s1:
        if r["type"] == "confusion":
            conf[r["participant"]] += 1
    out["confusions_per_participant"] = describe(conf.get(p, 0) for p in people)

    # Issues: severity x share of participants who hit it.
    hit, sev, notes = defaultdict(set), defaultdict(list), defaultdict(list)
    for r in s1:
        if r["type"] == "issue" and r.get("issue_code"):
            code = r["issue_code"].strip()
            hit[code].add(r["participant"])
            s = num(r.get("severity"))
            if s is not None:
                sev[code].append(s)
            if r.get("note"):
                notes[code].append(r["note"])
    issues = []
    for code in hit:
        severity = max(sev[code]) if sev[code] else 1.0
        share = len(hit[code]) / n if n else 0.0
        issues.append({"issue_code": code, "severity": severity, "participants": len(hit[code]),
                       "share": share, "priority": severity * share, "example": (notes[code] or [""])[0]})
    issues.sort(key=lambda i: (-i["priority"], -i["severity"], i["issue_code"]))
    out["issues"] = issues
    out["top3"] = [i["issue_code"] for i in issues[:3]]

    # Survey (session 1).
    sv1 = [r for r in survey if (r.get("session") or "1").strip() == "1"]
    if sv1:
        out["sus"] = describe(sus_score(r) for r in sv1)
        out["nasa_tlx_raw"] = describe(tlx_raw(r) for r in sv1)
        out["pxi"] = {k: describe(num(r.get(k)) for r in sv1) for k in PXI}
        out["game_items"] = {k: describe(num(r.get(k)) for r in sv1) for k in GAME_ITEMS}

    # Retention: session 2 rows (observations or survey).
    back = {r["participant"] for r in obs if (r.get("session") or "").strip() == "2"} | \
           {r["participant"] for r in survey if (r.get("session") or "").strip() == "2"}
    if back:
        def first_action(p, session):
            xs = [num(r["seconds"]) for r in obs if r["participant"] == p and r["type"] == "first_action"
                  and (r.get("session") or "1").strip() == session]
            return xs[0] if xs else None

        def intent(p, session):
            xs = [num(r.get("replay_intent")) for r in survey if r["participant"] == p
                  and (r.get("session") or "1").strip() == session]
            return xs[0] if xs else None

        fa = [(first_action(p, "2"), first_action(p, "1")) for p in back]
        it = [(intent(p, "2"), intent(p, "1")) for p in back]
        out["retention"] = {
            "returned": len(back),
            "of_participants": n,
            "first_action_change_seconds": describe(a - b for a, b in fa if a is not None and b is not None),
            "replay_intent_change": describe(a - b for a, b in it if a is not None and b is not None),
        }
    return out


def fmt(d, unit=""):
    if not d:
        return "–"
    return f"{d['median']:.0f}{unit} (IQR {d['q1']:.0f}–{d['q3']:.0f}, n={d['n']})"


def markdown(s):
    lines = [f"**Participants:** {s['participants']}", "",
             "| Measure | Median (IQR) or rate |", "|---|---|",
             f"| Setup time | {fmt(s['setup_seconds'], ' s')} |",
             f"| Time to first meaningful action | {fmt(s['first_action_seconds'], ' s')} |",
             f"| Confusion moments per participant | {fmt(s['confusions_per_participant'])} |"]
    for t, r in s["tasks"].items():
        lines.append(f"| {t}: unaided / any success | {r['unaided']:.0%} / {r['any_success']:.0%} (n={r['n']}) |")
    if "sus" in s:
        lines.append(f"| SUS | {fmt(s['sus'])} |")
        lines.append(f"| NASA-TLX raw | {fmt(s['nasa_tlx_raw'])} |")
        for k, v in s["game_items"].items():
            lines.append(f"| {k} | {fmt(v)} |")
    lines += ["", "| Rank | Issue | Severity | Participants | Priority | Example |", "|---|---|---|---|---|---|"]
    for i, it in enumerate(s["issues"], 1):
        lines.append(f"| {i} | {it['issue_code']} | {it['severity']:.0f} | {it['participants']} ({it['share']:.0%}) | {it['priority']:.2f} | {it['example']} |")
    lines += ["", f"**Top 3 to fix:** {', '.join(s['top3']) or '–'}"]
    if "retention" in s:
        r = s["retention"]
        lines += ["", f"**Retention:** {r['returned']}/{r['of_participants']} came back · first-action change {fmt(r['first_action_change_seconds'], ' s')} · replay-intent change {fmt(r['replay_intent_change'])}"]
    return "\n".join(lines)


def main(argv):
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # dashes and dots on Windows consoles
    except (AttributeError, ValueError):
        pass
    args = [a for a in argv if not a.startswith("--")]
    if not args:
        print(__doc__)
        return 2
    obs = read(args[0])
    survey = read(args[1]) if len(args) > 1 else []
    s = summarise(obs, survey)
    print(json.dumps(s, indent=2) if "--json" in argv else markdown(s))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
