# Mind over Chicken

A pixel god game you play with your brain. Guide one civilisation from the fire pit to a rocket launch using calm, focus, blinks, head tilt and heart rate.

| Era | Your brain | What emerges |
|---|---|---|
| **Stone Age** | Blink throws lightning (light the fire, cook a mammoth). Calm keeps the fire alive. | Villagers hunt only when the fire gives them courage, starve, have babies |
| *Farming → Kingdoms* (time-skip) | The scholars pose three sums: those seconds silently label **focus** data | Goat trading, then writing, law and taxes |
| **Industry** | Heart rate above resting drives market volatility and herding | Goat bubbles, crashes, strikes and riots from trader agents |
| **Space Age** | Focus speeds the engineers and holds the 10-second countdown; calm keeps citizens brave enough to board | Liftoff to a new world |

**The trick for technical judges:** there is no calibration screen. While you relax, look around and do sums, the game records cue-labelled windows and retrains a per-player softmax classifier every 2.5 s (blocked cross-validation). The personal model blends in as its accuracy earns trust, and thresholds rise as it improves. Press **M** in game to see it.

## Run it

It's static files, no build step:

```bash
python -m http.server 8123
```

Then open http://localhost:8123.

**Emulated headset (default):** a placeholder that sends exactly the signals the Muse will. Use <kbd>Space</kbd> for calm, <kbd>F</kbd> for focus and <kbd>Shift</kbd> for a racing pulse. <kbd>B</kbd> or a click blinks, and the mouse stands in for head tilt. On a phone, hold the power tiles and tap the map.

**Real headset:** see [bridge/README.md](bridge/README.md). In short, `pip install -r bridge/requirements.txt`, then `python bridge/muse_bridge.py --board muse2`, and choose *Connect a real headset* in the game.

## Code map

- `src/brain/`: signal sources (emulated headset, WebSocket bridge), feature extraction, the personal model
- `src/world/terrain.js`: procedural island, repainted per era, plus BFS pathfinding fields
- `src/eras/`: one file per era, plus the time-skip cutscenes
- `src/render/sprites.js`: all pixel art, drawn in code
- `src/ui/`: HUD, power tiles, Oog the advisor
- `bridge/`: Python BrainFlow → WebSocket bridge

## Tests

```bash
python -m unittest discover tests
```

Team status board: issue #10.
