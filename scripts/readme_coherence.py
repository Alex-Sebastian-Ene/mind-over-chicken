#!/usr/bin/env python3
"""Audit the README spec against the code and keep the drift visible.

This is the loop behind ``.github/workflows/readme-coherence.yml``:

* every check below has a claim in README.md and a code detector;
* a required claim that is missing from code raises a *code drift* issue;
* a canonical claim that has disappeared from README raises a *README drift* issue;
* a deferred claim that has appeared in code raises a *decision needed* issue;
* an existing open issue that already tracks the gap is reported as *tracked*
  instead of creating a duplicate;
* the pinned report issue is rewritten on every run.

Run locally with ``--dry-run`` to print the report. Use ``--watch N`` to keep
checking every N seconds. Publishing uses the ``gh`` CLI and GH_TOKEN, exactly
like ``scripts/workboard.py``.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README_PATH = ROOT / "README.md"
REPO = os.environ.get("GITHUB_REPOSITORY", "Alex-Sebastian-Ene/mind-over-chicken")
REPORT_TITLE = "[Coherence] README vs code: live report"
REPORT_MARKER = "<!-- readme-coherence-report -->"
ISSUE_MARKER = "<!-- readme-coherence:{key} -->"
GH = os.environ.get("GH_BIN") or shutil.which("gh") or "gh"

CODE_GLOBS = (
    "src/**/*.js",
    "crates/**/*.rs",
    "bridge/**/*.py",
    "rl/**/*.py",
    "rl/**/*.mjs",
    "index.html",
    "styles.css",
)


def read(path):
    return path.read_text(encoding="utf-8")


def code_files():
    for pattern in CODE_GLOBS:
        for path in ROOT.glob(pattern):
            if path.is_file():
                try:
                    yield path, read(path)
                except UnicodeDecodeError:
                    continue


FILES = list(code_files())
README = read(README_PATH)

# Every check is one row in the report. ``status`` is the README contract:
#   required  - the code must contain it now
#   deferred  - the code must not contain it yet; if it does, ask for a decision
#   target    - the code may lag while the linked port issue is open
CHECKS = [
    {
        "key": "stack.rust",
        "group": "stack",
        "claim": "Rust compiled to WebAssembly, rendered with WebGL2 (port tracked in #17).",
        "readme": r"Rust compiled to WebAssembly|Rust/Wasm|Rust \+ WebAssembly",
        "status": "target",
        "patterns": [r"\bCargo\.toml\b", r"wasm32-unknown-unknown", r"web_sys|wasm_bindgen|wgpu|WebGL2|webgl2"],
        "paths": (),
        "hint": "Add the Rust/Wasm/WebGL build or update README §1 if the JS build is now canonical.",
    },
    {
        "key": "world.circular",
        "group": "world-four-civs",
        "claim": "One circular world.",
        "readme": r"one circular world|circular map|circular world",
        "status": "required",
        "patterns": [r"\bcircular\b", r"\bradius\b", r"\bpolar\b", r"\bround\s+world\b"],
        "paths": ("src/world/", "src/cosmos/", "crates/moc-civ/src/terrain.rs", "crates/moc-planet/src/"),
        "hint": "World generation must produce one circular map, not a rectangular continent set.",
    },
    {
        "key": "world.four_civilizations",
        "group": "world-four-civs",
        "claim": "4 distinct civilizations on that world.",
        "readme": r"4 distinct civilizations|four civilizations|4 civilizations",
        "status": "required",
        "patterns": [
            r"\bCIV(?:S|ILIZATIONS?)?(?:_COUNT)?\s*[:=]\s*4\b",
            r"civilizations?\s*[:=]\s*\[[^\]]*,[^\]]*,[^\]]*,[^\]]*\]",
            r"for\s*\([^)]*<\s*4[^)]*\)\s*\{[^}]*civ",
            r"START_CIVS\s*:\s*usize\s*=\s*4",
            r"HOME_REGIONS\s*:\s*usize\s*=\s*4",
        ],
        "paths": ("src/", "crates/"),
        "hint": "Create four civilization records with separate spawn regions and ownership.",
    },
    {
        "key": "world.four_agents",
        "group": "world-four-civs",
        "claim": "Each civilization starts with 4 agents.",
        "readme": r"4 initial agents|4 agents|four agents",
        "status": "required",
        "patterns": [
            r"\b(?:AGENTS?_PER_CIV|CIV_START_AGENTS|START_AGENTS|POP_START|INITIAL_AGENTS)\s*[:=]\s*4\b",
            r"for\s*\([^)]*<\s*4[^)]*\)\s*\{[^}]*agent",
            r"START_AGENTS\s*:\s*usize\s*=\s*4",
        ],
        "paths": ("src/", "crates/"),
        "hint": "Spawn four agents per civilization; do not reuse the single-civilization population constants.",
    },
    {
        "key": "world.interaction",
        "group": "world-four-civs",
        "claim": "The four civilizations can meet, compete, and interact.",
        "readme": r"meet, compete, and interact|contact, combat|interact",
        "status": "required",
        "patterns": [
            r"\b(?:civ|faction)[A-Za-z_]*\.(?:relations?|contact|attitude|atWar|diplomacy)\b",
            r"\b(?:contact|interact|attack|combat)[A-Za-z_]*\([^)]*civ",
            r"\bwar_state\b|\btension\s*\(|EVENT\.WAR|CAST\.peace|\bat_war\b",
        ],
        "paths": ("src/", "crates/"),
        "hint": "Add civilization ownership/contact detection and a first interaction hook.",
    },
    {
        "key": "agents.no_tech_tree",
        "group": "agents",
        "claim": "No tech tree and no evolution system.",
        "readme": r"no tech tree|No tech tree|no evolution",
        "status": "required",
        "patterns": [r"\btech[_ ]?tree\b", r"\bresearch[_ ]?tree\b", r"\bevolution[_ ]?system\b", r"\bevolve\s*\("],
        "paths": ("src/", "crates/"),
        "forbidden": True,
        "hint": "Remove the tech-tree/evolution code, or update README §1/§4 if it is now intended.",
    },
    {
        "key": "input.camera",
        "group": "camera",
        "claim": "Camera input is required.",
        "readme": r"Camera input is \*\*required\*\*|Camera input is required|camera",
        "status": "required",
        "patterns": [r"getUserMedia\s*\(", r"\bMediaPipe\b", r"FaceLandmarker", r"face[-_ ]?api", r"src/camera/"],
        "paths": ("src/", "index.html", "crates/"),
        "hint": "Add the opt-in camera capture layer and wire it into the input frame (#33).",
    },
    {
        "key": "input.camera_face",
        "group": "camera",
        "claim": "Face detection supplies expressions and concentration cues.",
        "readme": r"Face detection|facial expressions|expression",
        "status": "required",
        "patterns": [r"FaceLandmarker", r"blendshape", r"emotion", r"expression", r"face[A-Za-z_]*detect"],
        "paths": ("src/", "index.html", "crates/"),
        "hint": "Implement the face/expression manager first; gesture casts can stay deferred.",
    },
    {
        "key": "input.head_tracking_gyro",
        "group": "head-tracking",
        "claim": "Accelerometer and gyroscope supply head tracking.",
        "readme": r"Accelerometer \*\*and gyroscope\*\*|accelerometer and gyroscope|gyroscope",
        "status": "required",
        "patterns": [r"\bgyro(?:scope)?\b", r"get_gyro", r"gyro_channels"],
        "paths": ("bridge/", "src/", "crates/"),
        "hint": "Read gyroscope channels in the bridge and fuse them with the accelerometer.",
    },
    {
        "key": "calibration.configurable",
        "group": "calibration",
        "claim": "The calibration window is configurable; 10 seconds is a placeholder.",
        "readme": r"10-second window is a placeholder|10-second duration is an arbitrary placeholder",
        "status": "required",
        "patterns": [
            r"\bCALIBRATION_(?:SECONDS|DURATION|WINDOW)\b",
            r"calibration[_A-Za-z]*\s*[:=]\s*(?:10|\[|rand)",
            r"calibration[A-Za-z_]*\([^)]*seconds",
        ],
        "paths": ("src/", "crates/"),
        "hint": "Make the calibration duration a config value instead of a hard-coded 10.",
    },
    {
        "key": "calibration.random_dummy",
        "group": "calibration",
        "claim": "The dummy run uses a random setup.",
        "readme": r"random setup|randomly generated seeds|random.ly seeded",
        "status": "required",
        "patterns": [r"random[A-Za-z_]*seed", r"seed\s*[:=]\s*rand", r"randomize[A-Za-z_]*civ", r"random\s+setup"],
        "paths": ("src/", "crates/"),
        "hint": "Add an explicit random-seed mode for the four civilizations.",
    },
    {
        "key": "calibration.traits",
        "group": "calibration",
        "claim": "The seed sets friendliness, aggressiveness, and violence.",
        "readme": r"friendliness, aggressiveness, and violence|friendliness",
        "status": "required",
        "patterns": [r"\bfriendliness\b", r"\baggressiveness\b", r"\bviolence\b"],
        "paths": ("src/", "crates/"),
        "hint": "Define the trait payload and the sensor-to-trait formula.",
    },
    {
        "key": "deferred.ambient",
        "group": "deferred",
        "claim": "Ambient world modulation is deferred.",
        "readme": r"Ambient world modulation|ambient modulation",
        "status": "deferred",
        "patterns": [r"\bweather\b", r"chaos(?:Level|_level)\b", r"ambient[A-Za-z_]*modulat", r"(?:world|global)[A-Za-z_]*temperament"],
        "paths": ("src/", "crates/"),
        "hint": "If this is now implemented, update README §1/§3.2 and close the deferred scope.",
    },
    {
        "key": "deferred.gesture_ui",
        "group": "deferred",
        "claim": "Gesture UI is deferred.",
        "readme": r"Gesture UI|gesture-driven",
        "status": "deferred",
        # Gesture *casting* is in scope (decided on #55); only menu selection / UI navigation by
        # gesture is still deferred, so casting code must not trip this check.
        "patterns": [r"gesture[A-Za-z_]*(?:select|navigate)", r"menu[A-Za-z_]*select", r"hand[A-Za-z_]*navigate"],
        "paths": ("src/", "crates/"),
        "hint": "If gesture selection/navigation is now in, update README §1/§3.3 and the gesture issue.",
    },
    {
        "key": "powers.god_powers",
        "group": "camera",
        "claim": "God powers are implemented: blink lightning and hand-gesture casts (decided on #55).",
        "readme": r"God powers.*In scope|Status: implemented",
        "status": "required",
        "patterns": [r"\bfireball\b", r"spawn[A-Za-z_]*resource", r"natural[A-Za-z_]*disaster", r"\bdisaster\b"],
        "paths": ("src/", "crates/"),
        "hint": "God powers are in scope: keep the casts wired (CAMERA_CASTS_ENABLED = true) or update README §1/§4.3.",
    },
]

GROUPS = {
    "stack": {
        "title": "[Coherence] Code drift: Rust/Wasm/WebGL target",
        "tracking": [r"\bRust\b", r"\bWasm\b", r"\bWebGL\b", r"#17"],
    },
    "world-four-civs": {
        "title": "[Coherence] Code drift: circular world with four civilizations",
        "tracking": [r"four civilizations", r"4 civilizations", r"circular world"],
    },
    "agents": {
        "title": "[Coherence] Code drift: tech tree or evolution system",
        "tracking": [r"tech tree", r"evolution"],
    },
    "camera": {
        "title": "[Coherence] Code drift: camera input",
        "tracking": [r"\[Input\].*Camera", r"Camera:.*face", r"face emotion", r"camera input", r"#33"],
    },
    "head-tracking": {
        "title": "[Coherence] Code drift: gyroscope head tracking",
        "tracking": [r"gyroscope", r"head tracking", r"head-tracking"],
    },
    "calibration": {
        "title": "[Coherence] Code drift: calibration seed",
        "tracking": [r"calibration seed", r"calibration", r"random setup"],
    },
    "deferred": {
        "title": "[Coherence] Decision needed: deferred scope is implemented",
        "tracking": [r"deferred", r"ambient modulation", r"gesture ui", r"god powers"],
    },
}


def files_for(check):
    prefixes = check.get("paths") or ()
    if not prefixes:
        return FILES
    return [
        (path, text)
        for path, text in FILES
        if any(path.relative_to(ROOT).as_posix() == prefix or path.relative_to(ROOT).as_posix().startswith(prefix) for prefix in prefixes)
    ]


def grep(patterns, files=None):
    files = FILES if files is None else files
    found = []
    for path, text in files:
        for pattern in patterns:
            for match in re.finditer(pattern, text, re.I | re.M):
                line = text.count("\n", 0, match.start()) + 1
                found.append((path.relative_to(ROOT).as_posix(), line, match.group(0)))
    return found


def readme_has(pattern):
    return re.search(pattern, README, re.I | re.M) is not None


def evaluate():
    rows = []
    for check in CHECKS:
        claim_ok = readme_has(check["readme"])
        hits = grep(check["patterns"], files_for(check))
        if check.get("gated_by") and grep([check["gated_by"]], files_for(check)):
            hits = []
        if not claim_ok:
            verdict = "README DRIFT"
            evidence = "claim not found in README"
        elif check["status"] == "deferred":
            if hits:
                verdict = "NEEDS DECISION"
                evidence = ", ".join(f"{p}:{line}" for p, line, _ in hits[:3])
            else:
                verdict = "PASS"
                evidence = "not implemented"
        elif check.get("forbidden"):
            if hits:
                verdict = "CODE DRIFT"
                evidence = ", ".join(f"{p}:{line}" for p, line, _ in hits[:3])
            else:
                verdict = "PASS"
                evidence = "not found"
        elif hits:
            verdict = "PASS"
            evidence = ", ".join(f"{p}:{line}" for p, line, _ in hits[:3])
        else:
            verdict = "CODE DRIFT"
            evidence = "not found"
        rows.append({**check, "claim_ok": claim_ok, "hits": hits, "verdict": verdict, "evidence": evidence})
    return rows


def open_issues():
    if not (os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")):
        return []
    if not shutil.which(GH) and not os.path.isfile(GH):
        return []
    out = subprocess.run(
        [GH, "api", "--paginate", f"repos/{REPO}/issues?state=open&per_page=100", "--jq", ".[]"],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if out.returncode:
        print(f"gh api failed: {out.stderr.strip()}", file=sys.stderr)
        return []
    return [json.loads(line) for line in out.stdout.splitlines() if line.strip() and "pull_request" not in json.loads(line)]


def tracking_issue(group_key, issues):
    group = GROUPS[group_key]
    for issue in issues:
        title = issue.get("title") or ""
        if title == REPORT_TITLE or title.startswith("[Coherence]"):
            continue
        # Match the title, not the body: a broad deferred-scope issue must not
        # swallow every narrower gap it mentions.
        if any(re.search(pattern, title, re.I) for pattern in group["tracking"]):
            return issue
    return None


def group_rows(rows, key):
    return [row for row in rows if row["group"] == key]


def group_verdicts(rows):
    verdicts = {}
    for row in rows:
        verdicts.setdefault(row["group"], set()).add(row["verdict"])
    return verdicts


def badge(verdict):
    return {
        "PASS": "PASS",
        "TRACKED": "TRACKED",
        "CODE DRIFT": "CODE DRIFT",
        "README DRIFT": "README DRIFT",
        "NEEDS DECISION": "DECISION NEEDED",
    }.get(verdict, verdict)


def report_body(rows, issues):
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    verdicts = group_verdicts(rows)
    out = [
        REPORT_MARKER,
        "> **Auto-updated** by `.github/workflows/readme-coherence.yml`. Do not edit this text by hand; edits are overwritten.",
        "> **Rule:** the README is the canonical spec. If the code disagrees, fix the code or the README — do not leave both.",
        "",
        f"_Last checked {now}._",
        "",
        "## Checks",
        "",
        "| Check | README claim | Code evidence | Verdict |",
        "|---|---|---|---|",
    ]
    for row in rows:
        out.append(
            f"| `{row['key']}` | {row['claim'].replace('|', '\\|')} | {row['evidence'].replace('|', '\\|')} | **{badge(row['verdict'])}** |"
        )
    out += ["", "## Groups", ""]
    for key, group in GROUPS.items():
        group_rows_ = group_rows(rows, key)
        if not group_rows_:
            continue
        found = verdicts.get(key, set())
        if "CODE DRIFT" in found or "README DRIFT" in found or "NEEDS DECISION" in found:
            tracked = tracking_issue(key, issues)
            if tracked:
                out.append(f"- **{group['title']}** — tracked in #{tracked['number']} {tracked['title']}")
            else:
                out.append(f"- **{group['title']}** — not tracked; a coherence issue is open or will be opened")
        else:
            out.append(f"- **{group['title']}** — coherent")
    out += [
        "",
        "## How to resolve",
        "",
        "1. If the README is right, fix the code and reference the check key in the PR.",
        "2. If the code is right, update the README section that makes the claim.",
        "3. If you cannot decide, comment on the coherence issue with the two options; the next run will keep it visible.",
        "",
    ]
    return "\n".join(out)


def issue_body(key, rows, tracked):
    group = GROUPS[key]
    failing = [row for row in rows if row["group"] == key and row["verdict"] != "PASS"]
    out = [
        ISSUE_MARKER.format(key=key),
        f"Auto-maintained by `scripts/readme_coherence.py` for **{group['title']}**.",
        "",
    ]
    if tracked:
        out += [f"A possible tracking issue already exists: #{tracked['number']} {tracked['title']}.", ""]
    out += ["## Mismatches", ""]
    for row in failing:
        out += [
            f"### `{row['key']}` — {badge(row['verdict'])}",
            "",
            f"- **README claim:** {row['claim']}",
            f"- **Code evidence:** {row['evidence']}",
            f"- **Hint:** {row['hint']}",
            "",
        ]
    out += [
        "## Resolution rule",
        "",
        "- README is right → change the code.",
        "- Code is right → change the README.",
        "- Neither is obvious → comment with the decision and keep this issue open until the next run agrees.",
        "",
    ]
    return "\n".join(out)


def gh_json(*args):
    proc = subprocess.run([GH, "api", *args], capture_output=True, text=True, encoding="utf-8")
    if proc.returncode:
        raise RuntimeError(proc.stderr.strip())
    return json.loads(proc.stdout) if proc.stdout.strip() else None


def publish(rows, issues):
    report = report_body(rows, issues)
    existing = next((issue for issue in issues if issue.get("title") == REPORT_TITLE), None)
    if existing:
        gh_json(f"repos/{REPO}/issues/{existing['number']}", "-X", "PATCH", "-f", f"body={report}")
        report_number = existing["number"]
    else:
        created = gh_json(f"repos/{REPO}/issues", "-X", "POST", "-f", f"title={REPORT_TITLE}", "-f", f"body={report}")
        report_number = created["number"]
        try:
            subprocess.run(
                [GH, "api", "graphql", "-f", "query=mutation($id:ID!){pinIssue(input:{issueId:$id}){issue{number}}}", "-f", f"id={created['node_id']}"],
                capture_output=True,
            )
        except Exception:
            pass
    print(f"report: #{report_number}")

    existing_by_key = {}
    for issue in issues:
        body = issue.get("body") or ""
        for key in GROUPS:
            if ISSUE_MARKER.format(key=key) in body:
                existing_by_key[key] = issue

    for key, group in GROUPS.items():
        failing = any(row["group"] == key and row["verdict"] != "PASS" for row in rows)
        tracked = tracking_issue(key, issues)
        issue = existing_by_key.get(key)
        if failing and not tracked:
            body = issue_body(key, rows, None)
            if issue:
                gh_json(f"repos/{REPO}/issues/{issue['number']}", "-X", "PATCH", "-f", f"body={body}")
                print(f"updated: #{issue['number']} {group['title']}")
            else:
                created = gh_json(f"repos/{REPO}/issues", "-X", "POST", "-f", f"title={group['title']}", "-f", f"body={body}")
                print(f"created: #{created['number']} {group['title']}")
        elif issue and (not failing or tracked):
            gh_json(f"repos/{REPO}/issues/{issue['number']}/comments", "-X", "POST", "-f", "body=The README and code now agree, or another open issue tracks this. Closing the coherence issue.")
            gh_json(f"repos/{REPO}/issues/{issue['number']}", "-X", "PATCH", "-f", "state=closed")
            print(f"closed: #{issue['number']} {group['title']}")


def main():
    watch = None
    if "--watch" in sys.argv:
        index = sys.argv.index("--watch")
        watch = int(sys.argv[index + 1]) if index + 1 < len(sys.argv) else 60
    while True:
        rows = evaluate()
        issues = [] if "--dry-run" in sys.argv else open_issues()
        report = report_body(rows, issues)
        if "--dry-run" in sys.argv or not issues:
            print(report)
            for key, group in GROUPS.items():
                failing = any(row["group"] == key and row["verdict"] != "PASS" for row in rows)
                tracked = tracking_issue(key, issues) if issues else None
                if failing and not tracked:
                    print("\n" + "=" * 80)
                    print(issue_body(key, rows, None))
        else:
            publish(rows, issues)
        if watch is None:
            break
        time.sleep(watch)


if __name__ == "__main__":
    main()
