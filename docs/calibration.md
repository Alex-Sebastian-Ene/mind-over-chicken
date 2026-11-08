# Calibration seed

How the player's signals at the start of play become the seed for **their** civilisation (README §3.1 and §4.1, issue #54). The other three civilisations are always random.

## Configure it without editing code

| URL parameter | Values | Default |
|---|---|---|
| `calib` | window length in seconds, 1–120 | `10` (a placeholder) |
| `calib_mode` | `random` (dummy run) or `sensor` | `random` |
| `seed` | any integer: the run seed | random each load |

For example: `index.html?calib=6&calib_mode=sensor&seed=42`.

The same keys can be saved in `localStorage` under `moc.calibration` (JSON). In the game, **⚙ Settings › Calibration** sets the window length and the mode (random roll or your signals); it applies from the next run. The galaxy's "mind read", which births the three mind-worlds, uses the same window.

This is separate from the EEG panel's **Guided Calibration (Dot + Flash)** wizard, where you follow a dot with your head and blink when the screen flashes. That wizard pauses the game while it is open, and its steps stand still if the headset stops sending data (#176).

## What the window measures
The window only counts seconds of real play; menus and the era's opening camera move don't count. It averages:

| Feature | Source |
|---|---|
| `calm`, `focus` (0–1) | the brain model (personal classifier blended with the generic rule) |
| `stress` (0–1) | `brain.stress`, the same stress the eras read: max(EEG agitation, face anger while a face is visible, 0.15·(1 − calm)). About 0.15 at rest; agitation is measured against the player's resting level, which this window sets |
| `blinkRate` | deliberate blinks per 10 s |
| `joy`, `anger`, `surprise`, `sadness` (0–1) | the face emotion manager, only while a face is visible (#33). They are 0 without a camera. |

## Formula (`sensor` mode)
With `blinks = clamp(blinkRate / 6)`:

| Trait | Formula |
|---|---|
| **friendliness** | 0.55·calm + 0.35·joy + 0.10·(1 − anger) |
| **aggressiveness** | 0.50·stress + 0.35·anger + 0.15·focus |
| **violence** | 0.45·anger + 0.35·stress·(1 − calm) + 0.20·blinks |

All three are clamped to 0–1. A calm, smiling player seeds a friendly civilisation; a tense, angry one seeds an aggressive, violent one. A player at rest (stress about 0.15) seeds neither.

## Random setup (`random` mode, the dummy run)
`friendliness`, `aggressiveness` and `violence` (scaled by 0.8) are the first three draws of `mulberry32(seed)`, so the same seed always gives the same setup.

## Output
`G.calibration.result = { friendliness, aggressiveness, violence, source, seconds, seed, features }`. It's shown in the model panel (press **M**). The crowded-planet sandbox (#50, #52) applies it to the player's civilisation once founding is over, and logs what it applied (#190, `src/civ/calibration-traits.js`).

Code: `src/brain/calibration.js`. Tests: `tests/calibration.test.mjs`.
