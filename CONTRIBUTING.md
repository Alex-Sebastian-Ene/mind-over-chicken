# Working together without stepping on each other

Several people **and AI agents** (Claude, Gemini) work on this repo at the same time. These rules keep us from building the same thing twice or rewriting each other's files. They apply to agents exactly as they apply to people.

## 1. Check the workboard first
The pinned issue **"[Workboard] Live: who's working on what"** is rebuilt automatically every 30 minutes and on every push, PR and issue change. It lists:
- every active branch: who pushed it, its PR, the issues it references, and the areas and files it changes;
- **overlaps**: files changed on more than one branch that aren't stacked on each other;
- claimed issues, and open issues nobody has claimed yet.

If what you're about to do is already on the board, talk to that person first.

## 2. Claim before you code
On the issue, do one of these:
- assign yourself, or
- comment a line starting with **`Claiming`**, e.g. `Claiming this on branch rust/render, touching crates/engine/ and src/main.js`.

The workboard picks up both. One issue, one owner. If you want to help on someone else's issue, ask on it first.

## 3. Branch early, push early, open a draft PR
- **One branch per issue**, named `area/short-name` (e.g. `rust/render`, `feat/camera-casts`, `fix/engine-review`).
- **Push within your first half hour** and open a **draft PR**. The board only sees what's pushed.
- **Stacked PRs** (built on another PR's branch) target that branch and say so in the first line: `Stacked on #31`.
- Reference issues in commits and PRs (`Refs #21`, `Fixes #8`).

## 4. When the board flags an overlap
- **Stop and comment on both PRs.** Agree who merges first, then the other rebases.
- **Want to change a file someone else owns?** Offer the change on their PR, or send a small PR into their branch. Don't push to their branch.
- **Hot spots** (`src/main.js`, `index.html`, `styles.css`, `Cargo.toml`) are touched by almost every feature. Keep your edits there small and marked with a comment naming the issue (e.g. `// camera (#33)`), and agree merge order early.

## 5. Owners review their areas
`.github/CODEOWNERS` maps folders to the person responsible for them. GitHub automatically requests their review on any PR that touches their area.

## 6. Notes for AI agents
- Read this file and the workboard before starting.
- Post your claim and progress on the issue.
- Never force-push or commit to a branch you didn't create.
- Claude agents commit as the repo owner (@Alex-Sebastian-Ene), so check the branch name and PR description to see which agent did what.
