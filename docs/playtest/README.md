# Student playtest kit (#94)

A lightweight, repeatable playtest that answers two questions with player data rather than expert opinion:

1. **Can a first-time player understand the game?**
2. **Do they want another run?**

Run it once per iteration, before deciding which UX issues (#84–#91) to tackle next.

| File | What it is |
|---|---|
| [`consent.md`](consent.md) | Consent form and data-handling notes, read aloud and signed before anything else |
| [`script.md`](script.md) | Minute-by-minute facilitator script, including neutral think-aloud prompts |
| [`tasks.md`](tasks.md) | The six tasks, what counts as success, and what to time |
| [`observation_sheet.csv`](observation_sheet.csv) | One row per observed event: task outcomes, timings, confusion moments, issues |
| [`survey.md`](survey.md) | Post-play survey: SUS, NASA-TLX (raw), PXI, replay intent, game-specific items |
| [`survey_responses.csv`](survey_responses.csv) | Template for typing survey answers in, one row per participant |
| [`retention.md`](retention.md) | The three-day follow-up: a second session and a short check-in |
| [`report_template.md`](report_template.md) | Where the results go: severity × frequency table, top 3 fixes |
| [`../../scripts/playtest_summary.py`](../../scripts/playtest_summary.py) | Turns the two CSVs into the numbers and the ranked issue table |

## Sample

- **Qualitative iteration:** 6–12 students. That's enough to find most usability problems.
- **Quantitative claims** (comparing two builds): 20–30.
- **Mix:** gamers and non-gamers, people who have and haven't used a headset, and at least one participant with an accessibility need (low vision, colour blindness, motor or attention differences) per iteration.
- **Not:** anyone on the team, or anyone who has seen the game before. They know too much.

## Setup per session

- **Hardware and build:** a laptop with Chrome, a mouse and the current build (Pages or `python -m http.server`). Record the commit SHA on the sheet.
- **Inputs:** the emulated headset is the default. If you have a Muse 2, run half the sessions with it (`bridge/README.md`). The camera is optional, and one task is done with the camera off.
- **Recording:** a screen recording only with explicit consent (see `consent.md`), and no face video. A second person takes notes on `observation_sheet.csv` if possible.
- **Reset:** clear site data between participants (DevTools → Application → Clear storage) so every player gets a first run.

## Session (40–45 minutes)

| Time | Step |
|---|---|
| 5 min | Consent (`consent.md`) |
| 5 min | Setup: headset or emulated keys, camera on or off; nothing else is explained |
| 20–25 min | Play, with think-aloud, working through `tasks.md` in order |
| 5 min | Second run (task 5): do they choose to go again, and is it faster? |
| 5–10 min | Survey (`survey.md`), then a short interview |

## After the sessions

1. Type the observation rows and survey answers into the two CSVs, using participant codes only (`P01`, `P02`, …).
2. Run `python scripts/playtest_summary.py docs/playtest/observation_sheet.csv docs/playtest/survey_responses.csv`.
3. Copy the output into `report_template.md`, add the top 3 fixes, and link the report on #94 and on the UX issues it informs.
4. Three days later, run the follow-up in `retention.md`.

## Sources

- Games User Research, *How to run a games user research playtest*: https://gamesuserresearch.com/how-to-run-a-games-user-research-playtest/
- Playtesting in education: https://www.intechopen.com/chapters/84668
- Player Experience Inventory (PXI), Abeele et al., 2020: https://www.sciencedirect.com/science/article/pii/S1071581919301302
- Game-based learning (constructive alignment): https://citl.illinois.edu/game-based-learning
- SUS: Brooke, J. (1996). *SUS: a "quick and dirty" usability scale.*
- NASA-TLX: Hart & Staveland (1988); NASA Ames, free to use.
