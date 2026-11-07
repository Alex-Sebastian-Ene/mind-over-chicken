# Post-play survey

*About 5 minutes. Answer every item, even if you're unsure. Enter the answers in `survey_responses.csv` (the column names are given in brackets).*

## A. System Usability Scale (SUS)

*1 = strongly disagree … 5 = strongly agree.* SUS (Brooke, 1996) is free to use with attribution.

| # | Statement | 1–5 |
|---|---|---|
| sus1 | I think that I would like to use this system frequently. | |
| sus2 | I found the system unnecessarily complex. | |
| sus3 | I thought the system was easy to use. | |
| sus4 | I think that I would need the support of a technical person to be able to use this system. | |
| sus5 | I found the various functions in this system were well integrated. | |
| sus6 | I thought there was too much inconsistency in this system. | |
| sus7 | I would imagine that most people would learn to use this system very quickly. | |
| sus8 | I found the system very cumbersome to use. | |
| sus9 | I felt very confident using the system. | |
| sus10 | I needed to learn a lot of things before I could get going with this system. | |

## B. Workload: NASA-TLX, raw (unweighted)

*0 = very low … 100 = very high, in steps of 5. For **Performance**, 0 = perfect and 100 = failure.* NASA-TLX (Hart & Staveland, 1988) is free to use.

| Column | Scale | Question | 0–100 |
|---|---|---|---|
| tlx_mental | Mental demand | How mentally demanding was the game? | |
| tlx_physical | Physical demand | How physically demanding was it? | |
| tlx_temporal | Temporal demand | How hurried or rushed was the pace? | |
| tlx_performance | Performance | How successful were you at what you were asked to do? (0 = perfect) | |
| tlx_effort | Effort | How hard did you have to work to reach your level of performance? | |
| tlx_frustration | Frustration | How insecure, discouraged, irritated, stressed or annoyed were you? | |

## C. Player experience (PXI)

Use the published **Player Experience Inventory** items (Abeele et al., 2020; link in `README.md`) for these constructs, on its −3…+3 scale. Store each construct's mean in the column given. We use these ten:

| Column | Construct |
|---|---|
| pxi_meaning | Meaning |
| pxi_curiosity | Curiosity |
| pxi_mastery | Mastery |
| pxi_autonomy | Autonomy |
| pxi_immersion | Immersion |
| pxi_progress | Progress feedback |
| pxi_audiovisual | Audiovisual appeal |
| pxi_challenge | Challenge |
| pxi_ease | Ease of control |
| pxi_goals | Goals and rules |

## D. Mind over Chicken items

*1 = strongly disagree … 7 = strongly agree.*

| Column | Statement |
|---|---|
| mind_control | I felt the world responded to my mind and body, not just to the keys or mouse. |
| mind_understand | I understood what my calm, focus, stress and blinks each did. |
| mind_fair | When something went wrong, I understood why. |
| mind_tension | The tension in the game felt exciting rather than stressful. |
| replay_intent | I would like to play another run. |
| replay_tomorrow | I would play this again tomorrow if I could. |
| recommend | I would recommend this game to a friend. |

## E. About you (optional)

| Column | Question |
|---|---|
| gamer | How often do you play video games? (never / monthly / weekly / daily) |
| headset_before | Have you used a brain or biosignal headset before? (yes / no) |
| access_need | Any accessibility need relevant to playing? (free text, optional) |
