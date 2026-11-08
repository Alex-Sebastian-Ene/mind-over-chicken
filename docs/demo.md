# Mind over Chicken: 3-Minute Demo Script & Presentation Playbook

A structured, rehearsed 3-minute (180-second) live presentation and judging playbook for **Mind over Chicken**.

---

## 1. Quick Reference & Setup Checklist

### Pre-Flight Hardware & Software Checklist (T-5 minutes)
- [ ] **Display**: 1080p+ screen or projector connected. Browser at 100% zoom.
- [ ] **Audio**: Volume at 65%. Browser tab unmuted. Click once anywhere on screen to unlock WebAudio context.
- [ ] **Headset**:
  - **Option A (Muse 2 hardware)**: Bluetooth paired. `python bridge/muse_bridge.py --board muse2` running in terminal, showing green stream. On the title screen, **Connect a real headset → Connect**. The run starts on the first EEG frame: the status reads *"Waiting for the first EEG from the headset…"* and gives up after 10 s with Retry or the emulated headset.
  - **Option B (Emulated Headset fallback)**: Title screen button **"Play with the emulated headset"**.
- [ ] **Settings**: **⚙ Settings** (top bar or title screen) holds reduce motion (no shake, softer lightning flashes, no camera sweeps), head-aim sensitivity and inversion, the calibration window, the keyboard controls and "camera on at start". Esc closes it. Everything is saved in the browser. Turn on reduce motion if anyone watching is sensitive to flashes.
- [ ] **Webcam**: Camera positioned at eye level with even lighting. Press `C` (or **⚙ Settings → Advanced → Camera**) to open the sidebar, then **"Start Camera"** and allow the permission prompt. A **● Camera on** chip shows in the top bar while it runs.
- [ ] **Mood without a webcam** (optional): in the Camera sidebar, clear **No emotion** and use the **Happy ↔ Mix ↔ Surprised** slider. Joy lifts morale; moving into Surprised sets off a sky flash and lightning once.
- [ ] **Pinch calibration** (optional): Camera sidebar → **🎯 Pinch Movement Calibration** opens a pop-up. Follow the live camera + wireframe to the LEFT, RIGHT, UP and DOWN targets with each hand. It auto-advances, saves per-axis gains and closes when complete. Gains persist in the browser. Esc closes it. Head tilt is not calibrated.
- [ ] **Hotkeys Reference Card**:
  - `Space`: Hold Calm (high alpha)
  - `F`: Hold Focus / Insight. Hold 5 s at the fading campfire to rekindle it. The Rain spell is camera-only: hold an open palm to the camera and hold Insight ~2 s to put out a forest fire
  - `Shift`: Spike Stress (drops calm)
  - `B`: Deliberate Blink. On a surface it throws lightning (a tree strike starts a forest fire); in the galaxy/globe it selects/lands like a left click
  - `Space` / `Enter` / click during an era's opening camera move: skip it
  - `3`: Hold to charge the charged fireball, release to throw (or fist → pull back → thrust + open palm with the camera). Its colour follows joy (red) / surprise (blue)
  - `I` or the bottom-bar **🛡️ People** tile: Toggle people invincibility in Stone Age (no hunger, starvation, lightning or trample deaths)
  - `T`: Toggle 3D Heightmap Terrain vs 2D Classic
  - `Z` / `X` (or scroll, or a trackpad pinch): one continuous zoom out to the globe and galaxy / back in to the surface. In 3D the zoom goes toward the cursor
  - `Left` / `Right` (or `A` / `D`) in the galaxy: choose a world; `Enter` or zoom in to land on it. `N`: visit the next world
  - `M`: Toggle Personalized Brain Model & Judge Panel
  - `G`: Toggle PPO AI God Autopilot
  - `E` / `C`: Toggle the EEG / Camera sidebars; `` ` ``: the Debug panel; `Shift+C`: re-centre the headset's head pose
  - Hands: pinch + hand movement pans, pinch + head motion tilts the 3D view, two hands zoom. The only hand cast is the charged fireball: fist, pull back, thrust and open the palm (#291); the old gesture spells are gone (#261).

---

## 2. Second-by-Second Demo Timeline (Total: 3:00)

```
0:00 ─── 0:20  [01] The Hook & Neural Setup (20s)
0:20 ─── 1:05  [02] Era I: Stone Age & Divine Interventions (45s)
1:05 ─── 1:45  [03] Era II: Industry & Market Volatility (40s)
1:45 ─── 2:20  [04] Cosmos: Galaxy & Brain-Seeded Worlds (35s)
2:20 ─── 2:50  [05] Judge Reveal: Real-Time ML & Rust Engine (30s)
2:50 ─── 3:00  [06] The Punchline & Vision (10s)
```

---

### Act 1: The Hook & Neural Setup (0:00 – 0:20, 20s)

#### Spoken Script
> *"Judges, god games have always given you a mouse to rule the world. Mind over Chicken gives the world your mind.*  
> *I am wearing an EEG headset. There are no invisible difficulty sliders: this entire civilization is compiled to Rust and WebAssembly, and its physics, markets, and survival are driven directly by my real-time brainwaves, head movement, and facial expressions."*

#### Operator Actions
- **0:00**: Title screen visible. Click **"Play with the emulated headset"** (or **Connect a real headset → Connect** if the Muse is streaming).
- **0:05**: The camera sweeps down onto the 640×360 home world in **3D WebGL mode** (3.2 s; pixel-art heightmap terrain, the sun crossing a 150-second day). There is no card to dismiss. Under reduced motion the sweep is dropped.
- **0:12**: Show the power tiles (`Lightning`, `Rain`, `Markets`). Point out that the 10-second calibration window is active, recording baseline alpha/beta rhythms.

---

### Act 2: Era I · Stone Age & Divine Interventions (0:20 – 1:05, 45s)

#### Spoken Script
> *"We start in the Stone Age. My tribe is freezing, and Cluck, the High Priest (a chicken), is quietly judging me.*  
> *Watch the reticle. When I perform a deliberate double-blink—[BLINK]—lightning strikes the fire pit, igniting civilization's first flame.*  
> *Now I strike a giant chicken across the river—[BLINK]—barbecuing dinner for the hunters.*  
> *Notice the fire fading! The game demands focus. As I hold Insight... [HOLD 5s]... my beta rhythm locks in, the fire roars back, and a baby is born.*  
> *And if a stray bolt sets the forest alight, I open my palm and hold focus for two seconds to call down the rain."*

#### Operator Actions
- **0:22**: Aim mouse/head at the fire pit. Press `B` (or physical blink). Lightning crashes, fire ignites, audio synth roars.
- **0:30**: Aim at a giant chicken. Press `B`. A roast chicken appears, food count rises to 5+.
- **0:36**: Fire heat drops under 50%; the Insight hold begins. Hold `F` (or focus for the headset). The ring fills to 100%, the advisor celebrates and the tribe grows.
- **0:50**: Aim at a tree and press `B`: a forest fire starts and spreads. Hold an open palm to the camera and hold Insight for about 2 s: the Rain spell puts it out.
- **0:50**: Show the camera hands: pinch over a villager to carry them, then pinch with both hands and spread them to zoom. Optional: hold `3` (or, on camera, make a fist, pull it back, then thrust and open the palm) to throw the charged fireball (#291).

---

### Act 3: Era II · Industry & Market Volatility (1:05 – 1:45, 40s)

#### Spoken Script
> *"Fast forward to the Industrial Era. We traded sticks for the Chicken Exchange futures ticker.*  
> *Here, market prices don't follow fixed scripts; they model chartist momentum versus fundamental value. And the market is hyper-sensitive to divine anxiety.*  
> *Watch what happens when my stress spikes—[STRESS SPIKE]—the ticker goes red, panic spreads, and we trigger an emergent market crash! The workers go on strike.*  
> *To fix it, the game tests my emotional regulation: I breathe, calm down, and hold composure through the IPO window until green bars return and industry flourishes."*

#### Operator Actions
- **1:05**: The Farming · Kingdoms montage plays first: hold `F` and the scholars ink your thoughts (Writing, Law, Taxes) letter by letter, faster the harder you focus. There is no arithmetic. Press `Enter` to skip it if you are short of time. Then the era switches to Industry and the Chicken Exchange chart appears in retro amber pixels.
- **1:15**: Hold `Shift` (or tense up, or scowl at the camera) to spike stress: calm drops to 0 and the `Stress` meter fills.
- **1:22**: Market drops sharply; recession alert triggers, siren sound plays, red crash banner appears.
- **1:30**: Release `Shift`, hold `Space` (`Calm` meter > 80%). The market stabilizes, confidence rebounds, and the IPO completes.

---

### Act 4: Cosmos: The Galaxy & Brain-Seeded Worlds (1:45 – 2:20, 35s)

#### Spoken Script
> *"Now we zoom out.*  
> *Pressing Z pulls our camera from the 3D surface into an orbital WebGL globe shader... and further out into the Galaxy.*  
> *Look at these three planets orbiting our star. They weren't procedurally randomized: they were biologically seeded from the first ten seconds of my thoughts!*  
> *Stillwater was born from my calm. Lumen from my focus. Thrum from my stress. Flicker from my blinks.*  
> *And right now, our on-device PPO reinforcement learning AI god—trained with neural networks in PyTorch and running in browser WebAssembly—is steering their autonomous trade and orbital colony ships in real time."*

#### Operator Actions
- **1:45**: Press `Z` twice (or scroll out). One continuous zoom from the 3D surface → rotating pixel-art globe → galaxy star field with elliptical orbits.
- **1:58**: Press `Left` / `Right` to choose **Stillwater** or **Thrum** (the hint reads *"◀ ▶ choose a world"*), then `Enter` or scroll in to land. Clicking a planet works too. Point out the live telemetry: *"Growth +1.2%/s from calm; Market volatility steered by stress"*.
- **2:10**: Press `G` to demonstrate the PPO AI God autopilot steering simulated worlds.

---

### Act 5: Judge Reveal: Real-Time Machine Learning & Rust Engine (2:20 – 2:50, 30s)

#### Spoken Script
> *"Here is the technical secret.*  
> *I press M to open the Judge Panel.*  
> *We did not hardcode brain thresholds. While I was playing, an on-device machine-learning pipeline extracted log-band-power features, ran blocked 4-fold cross-validation, and trained a personalized multinomial model directly inside WebAssembly in just 2.8 milliseconds.*  
> *The entire engine—from the 3D heightmap raycaster to the procedural WebAudio sound synthesizer—is written in pure Rust with zero external audio or texture dependencies, running at a rock-solid 60 FPS."*

#### Operator Actions
- **2:20**: Press `M`. The Judge Model Panel opens.
- **2:25**: Highlight the stats:
  - Samples recorded across `rest`, `calm`, and `focus`.
  - Cross-validation accuracy (e.g., **89.1%**, outperforming the 85% requirement).
  - Train latency: **2.8 ms**.
  - Total Rust engine WASM payload: under 250 KB.

---

### Act 6: The Punchline & Vision (2:50 – 3:00, 10s)

#### Spoken Script
> *"Mind over Chicken isn't just a game; it is an emotional biofeedback mirror disguised as divine power.*  
> *To conquer the stars, you don't need faster fingers. You need to master your own mind. Thank you!"*

#### Operator Actions
- **2:55**: Press `Z` to zoom back to the home planet.
- **3:00**: Close presentation. Invite judges to try the headset, or to pinch and pan the world with their hands in front of the camera.

---

## 3. Contingency Playbook (What to do if...)

| Incident | Immediate Operator Action | Backup Pitch Line |
|---|---|---|
| **Bluetooth drops on Muse 2** | Click **"Emulated headset"** on the title screen (or reload with `?source=sim`). Use `Space` for calm, `F` for focus, `Shift` for stress, `B` for blink. | *"Our bridge supports hot-failover to an emulated biometric source so the simulation never stutters."* |
| **Era pauses with a "poor contact" toast** | Re-seat the headband; the era resumes by itself once the signal recovers. Or switch to the emulated headset. | *"The game won't score you on a bad signal: it waits for a clean one."* |
| **Webcam lighting is too dark for hand tracking** | Drag with the mouse to pan and scroll to zoom. For mood, clear **No emotion** in the Camera sidebar and use the Happy ↔ Surprised slider. | *"Hands only move the world, so a mouse does the same job, and the mood slider stands in for the face."* |
| **Fire pit doesn't light on first blink** | Press `B` firmly; the reticle auto-aims toward the fire pit when within 14 pixels. | *"The tribe demands divine conviction—another lightning strike does the trick."* |
| **Speaker runs ahead of time (finishes early)** | Press `T` to toggle 3D camera mode and right-drag to orbit around the terrain, or click a planet in the galaxy view. | *"Notice how the 3D raycaster projects screen coordinates directly onto the dynamic heightfield."* |
| **Speaker runs behind time (running late)** | Skip the Time-skip cutscene immediately by pressing `Enter`. Jump straight to Industry. | *"Let's accelerate directly to the Industrial Era to see the market mechanics."* |

---

## 4. Technical Talking Points for Q&A

1. **Why Rust & WebAssembly?**
   - Memory safety and guaranteed 60 FPS deterministic simulation without JavaScript garbage-collection pauses.
   - Cross-compilation to native headless binaries for automated regression testing and reinforcement learning training (PyO3).

2. **Is the machine learning model running on a remote server?**
   - No. 100% of feature extraction, blocked cross-validation, and softmax regression training execute locally inside the browser WebAssembly runtime. No user brain data or camera frames ever leave the device.

3. **How does audio synthesis work without files?**
   - Sound effects are synthesized procedurally in Rust via WebAudio oscillators, bandpass noise filters, and exponential gain curves. Zero audio downloads.
