# Three-day follow-up (retention check)

*Only for participants who opted in on the consent form. Run 3 days (±1) after their first session.*

## Option A: second session (preferred, 15 min)

1. Same laptop and build if possible (record the SHA), with **site data not cleared**, so it's their second run.
2. Say: *"Play as much or as little as you like. Tell me when you'd like to stop."*
3. Log it on `observation_sheet.csv` with `session = 2`:
   - `first_action` time, to compare with session 1;
   - T1–T3 outcomes and times, to see whether they remember;
   - the time until they **choose** to stop, as a voluntary-play measure;
   - any new issues.
4. Ask `replay_intent`, `replay_tomorrow` and `mind_understand` again, then add a `session = 2` row to `survey_responses.csv` with just those columns.

## Option B: remote check-in (2 min)

If they can't come back, send the three questions:

1. "Have you thought about the game since? What do you remember about how to play?" (Code recall 0–2: 0 = nothing, 1 = some controls or goals, 2 = what blinks, calm, focus and stress do.)
2. `replay_intent` (1–7)
3. `replay_tomorrow` (1–7)

Record them as a `session = 2` row in `survey_responses.csv`, filling only `replay_intent`, `replay_tomorrow` and `recall`.

## What we report

- **Retention rate:** participants who came back or answered ÷ those who opted in.
- **Learning:** the change in time to first meaningful action and in T1–T3 times, session 1 vs 2.
- **Intent drift:** the change in `replay_intent` from session 1 to session 2.

`scripts/playtest_summary.py` computes all three when `session = 2` rows are present.
