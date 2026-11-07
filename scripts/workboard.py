#!/usr/bin/env python3
"""Live workboard: who is working on what, and where branches overlap.

Rewrites one pinned issue ("[Workboard] Live: who's working on what") with:
  - every active branch: who pushed it, its PR, the issues it references,
    the areas and files it changes (measured against its PR's base, so stacked
    PRs don't double count), and how fresh it is;
  - overlaps: any file changed on more than one active branch;
  - claims: open issues with assignees or a "Claiming …" comment;
  - open issues nobody has claimed yet.

Runs in GitHub Actions (.github/workflows/workboard.yml) every 30 minutes and on
pushes, PRs and issue changes. Run it locally with `--dry-run` to print the board.
Uses the `gh` CLI for API calls (preinstalled on GitHub runners; GH_TOKEN in CI).
"""

import json
import os
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timezone
from urllib.parse import quote

REPO = os.environ.get("GITHUB_REPOSITORY", "Alex-Sebastian-Ene/mind-over-chicken")
TITLE = "[Workboard] Live: who's working on what"
GH = os.environ.get("GH_BIN") or shutil.which("gh") or r"C:\Program Files\GitHub CLI\gh.exe"

# Generated files: everyone touches them, so overlaps there are noise.
IGNORE_OVERLAP = re.compile(r"^(pkg/|Cargo\.lock$|.*\.lock$)")
# Files nearly every feature edits: flag them so merge order gets agreed early.
HOT_SPOTS = {"src/main.js", "index.html", "styles.css", "Cargo.toml"}
# Path prefix → area name (first match wins).
AREAS = [
    ("crates/engine/", "Rust engine"),
    ("crates/moc-planet/", "Rust planet"),
    ("crates/", "Rust"),
    ("pkg/", "wasm build"),
    ("src/eras/", "eras"),
    ("src/cosmos/", "globe & galaxy"),
    ("src/camera/", "camera"),
    ("src/ai/", "AI god"),
    ("rl/", "AI god"),
    ("src/brain/", "brain"),
    ("src/world/", "world"),
    ("src/ui/", "HUD"),
    ("src/", "game shell"),
    ("index.html", "game shell"),
    ("styles.css", "game shell"),
    ("bridge/", "headset bridge"),
    (".github/", "CI & deploy"),
    ("scripts/", "scripts"),
    ("tests/", "tests"),
    ("README.md", "docs"),
    ("CONTRIBUTING.md", "docs"),
    ("docs/", "docs"),
]
CLAIM_LINE = re.compile(r"^\W*((?:claim(?:ing|ed)?|owner)\b.*)$", re.I | re.M)
# "Owner: open" / "help wanted" lines mean nobody has it yet.
UNCLAIMED = re.compile(r"owner\W*(?:open|tbd|none)|help wanted|needs an owner", re.I)
STALE_HOURS = 3


def gh_json(*args):
    out = subprocess.run([GH, "api", *args], capture_output=True, text=True, encoding="utf-8")
    if out.returncode:
        raise RuntimeError(f"gh api {' '.join(args)} failed: {out.stderr.strip()}")
    return json.loads(out.stdout) if out.stdout.strip() else None


def gh_list(path):
    out = subprocess.run([GH, "api", "--paginate", path, "--jq", ".[]"], capture_output=True, text=True, encoding="utf-8")
    if out.returncode:
        raise RuntimeError(f"gh api {path} failed: {out.stderr.strip()}")
    return [json.loads(line) for line in out.stdout.splitlines() if line.strip()]


def area_of(path):
    for prefix, name in AREAS:
        if path == prefix or path.startswith(prefix):
            return name
    return "other"


def ago(iso):
    then = datetime.fromisoformat(iso.replace("Z", "+00:00"))
    mins = int((datetime.now(timezone.utc) - then).total_seconds() // 60)
    if mins < 60:
        return f"{mins} min ago", mins
    return f"{mins // 60} h {mins % 60:02d} min ago", mins


def cell(text):
    return str(text).replace("|", "\\|").replace("\n", " ")


def collect():
    repo = gh_json(f"repos/{REPO}")
    default = repo["default_branch"]
    branches = [b["name"] for b in gh_list(f"repos/{REPO}/branches?per_page=100")]
    prs = {p["head"]["ref"]: p for p in gh_list(f"repos/{REPO}/pulls?state=open&per_page=100") if p["head"]["repo"] and p["head"]["repo"]["full_name"] == REPO}
    issues = [i for i in gh_list(f"repos/{REPO}/issues?state=open&per_page=100") if "pull_request" not in i]

    active = []
    for name in branches:
        if name == default:
            continue
        pr = prs.get(name)
        base = pr["base"]["ref"] if pr else default
        cmp = gh_json(f"repos/{REPO}/compare/{quote(base, safe='')}...{quote(name, safe='')}")
        if not cmp or cmp.get("ahead_by", 0) == 0:
            continue  # nothing of its own: merged, or an empty claim branch
        files = [f["filename"] for f in cmp.get("files", [])]
        commits = cmp.get("commits", [])
        last = commits[-1] if commits else None
        people = sorted({(c.get("author") or {}).get("login") or c["commit"]["author"]["name"] for c in commits})
        text = " ".join(c["commit"]["message"] for c in commits) + " " + ((pr or {}).get("title") or "") + " " + ((pr or {}).get("body") or "")
        refs = sorted({int(n) for n in re.findall(r"#(\d+)", text)} - ({pr["number"]} if pr else set()))
        active.append({
            "name": name, "base": base, "pr": pr, "files": files, "people": people, "refs": refs,
            "last": last["commit"]["author"]["date"] if last else None, "ahead": cmp["ahead_by"], "behind": cmp.get("behind_by", 0),
            "included_in": None,
        })

    # A branch with no PR whose head is already inside another active branch adds nothing
    # of its own (e.g. a branch someone built a PR on top of): hide it from overlaps.
    # Branches with their own PR always stay visible.
    names = {b["name"] for b in active}
    for a in active:
        if a["pr"]:
            continue
        for b in active:
            if a is b or a["included_in"]:
                continue
            rel = gh_json(f"repos/{REPO}/compare/{quote(a['name'], safe='')}...{quote(b['name'], safe='')}")
            if rel and rel.get("status") in ("ahead", "identical") and b["name"] in names:
                a["included_in"] = b["name"]

    claims = []
    for issue in issues:
        found = [(a["login"], "assigned") for a in issue.get("assignees", [])]
        texts = [(issue["user"]["login"], issue.get("body") or "")]
        if issue.get("comments"):
            texts += [(c["user"]["login"], c.get("body") or "") for c in gh_list(f"repos/{REPO}/issues/{issue['number']}/comments?per_page=100")]
        for who, text in texts:
            for m in CLAIM_LINE.finditer(text):
                if not UNCLAIMED.search(m.group(1)):
                    found.append((who, m.group(1)))
        claims.append((issue, found))
    return default, active, claims, issues


def stacked(a, b, by_name):
    """True when one branch is built on the other (PR base chain, through inclusions)."""
    def chain(x):
        seen = set()
        while x and x["name"] not in seen:
            seen.add(x["name"])
            if x["included_in"]:
                seen.add(x["included_in"])
            nxt = by_name.get(x["base"])
            x = nxt
        return seen
    return a["name"] in chain(b) or b["name"] in chain(a) or bool(
        (a["included_in"] and a["included_in"] in chain(b)) or (b["included_in"] and b["included_in"] in chain(a)))


def render(default, active, claims):
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    out = [
        "> **Auto-updated** by `.github/workflows/workboard.yml` (every 30 min and on every push, PR and issue change). Don't edit this text by hand; edits are overwritten.",
        "> **Before you start anything:** check the overlaps below, then claim your issue (see `CONTRIBUTING.md`).",
        "",
        f"_Last updated {now}._",
        "",
        "## Active branches",
        "",
        "| Branch | Who | PR | Issues | Areas | Files | Last push |",
        "|---|---|---|---|---|---|---|",
    ]
    for b in sorted(active, key=lambda b: b["last"] or "", reverse=True):
        pr = b["pr"]
        pr_cell = f"#{pr['number']}{' (draft)' if pr.get('draft') else ''} → `{b['base']}`" if pr else f"no PR yet (vs `{b['base']}`)"
        areas = sorted({area_of(f) for f in b["files"]})
        hot = sorted(HOT_SPOTS.intersection(b["files"]))
        files = f"{len(b['files'])}" + (f" · edits {', '.join('`' + h + '`' for h in hot)}" if hot else "")
        when, mins = ago(b["last"]) if b["last"] else ("?", 0)
        if mins > STALE_HOURS * 60:
            when += " · idle?"
        refs = " ".join(f"#{n}" for n in b["refs"]) or "none"
        if b["included_in"]:
            pr_cell += f" · included in `{b['included_in']}`"
        out.append(f"| `{cell(b['name'])}` | {', '.join('@' + p if re.fullmatch(r'[A-Za-z0-9-]+', p) else p for p in b['people'])} | {pr_cell} | {refs} | {cell(', '.join(areas))} | {files} | {when} |")
    if not active:
        out.append("| (none) | | | | | | |")

    # Overlap = the same file changed on two branches that aren't built on each other.
    by_name = {b["name"]: b for b in active}
    live = [b for b in active if not b["included_in"]]
    overlaps = defaultdict(set)
    for i, a in enumerate(live):
        for b in live[i + 1:]:
            if stacked(a, b, by_name):
                continue
            for f in set(a["files"]) & set(b["files"]):
                if not IGNORE_OVERLAP.match(f):
                    overlaps[f].update((a["name"], b["name"]))
    out += ["", "## Overlaps: files changed on more than one active branch", ""]
    if overlaps:
        out += ["Talk on the PRs and agree who merges first before carrying on. Hot spots are expected; just agree merge order.", "",
                "| File | Branches |", "|---|---|"]
        for f in sorted(overlaps, key=lambda f: (-len(overlaps[f]), f)):
            mark = " (hot spot)" if f in HOT_SPOTS else ""
            out.append(f"| `{cell(f)}`{mark} | {', '.join('`' + n + '`' for n in sorted(overlaps[f]))} |")
    else:
        out.append("None right now.")

    claimed = [(i, c) for i, c in claims if c]
    unclaimed = [i for i, c in claims if not c and i["title"] != TITLE]
    out += ["", "## Claimed issues", "", "| Issue | Claimed by | Claim |", "|---|---|---|"]
    for issue, found in sorted(claimed, key=lambda x: x[0]["number"]):
        who = sorted({w for w, _ in found})
        line = next((t for _, t in found if t != "assigned"), "assigned")
        out.append(f"| #{issue['number']} {cell(issue['title'][:70])} | {', '.join('@' + w for w in who)} | {cell(line[:110])} |")
    if not claimed:
        out.append("| (none) | | |")
    out += ["", "## Open and unclaimed: pick one up", ""]
    out += [f"- #{i['number']} {i['title']}" for i in sorted(unclaimed, key=lambda i: i["number"])] or ["Everything is claimed."]
    return "\n".join(out) + "\n"


def publish(body, issues):
    existing = next((i for i in issues if i["title"] == TITLE), None)
    if existing:
        gh_json(f"repos/{REPO}/issues/{existing['number']}", "-X", "PATCH", "-f", f"body={body}")
        print(f"updated #{existing['number']}")
        return existing["number"]
    created = gh_json(f"repos/{REPO}/issues", "-X", "POST", "-f", f"title={TITLE}", "-f", f"body={body}")
    print(f"created #{created['number']}")
    try:  # best effort: pin it so it sits at the top of the issue list
        subprocess.run([GH, "api", "graphql", "-f", "query=mutation($id:ID!){pinIssue(input:{issueId:$id}){issue{number}}}", "-f", f"id={created['node_id']}"], capture_output=True)
    except Exception:
        pass
    return created["number"]


def main():
    default, active, claims, issues = collect()
    body = render(default, active, claims)
    if "--dry-run" in sys.argv:
        sys.stdout.reconfigure(encoding="utf-8")
        print(body)
        return
    publish(body, issues)


if __name__ == "__main__":
    main()
