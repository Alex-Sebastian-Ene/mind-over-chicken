# Mind over Chicken

A web-based pixel-art god-game sandbox inspired by *WorldBox*, played in 3D by default with a 2D classic view. The player is a god overseeing one circular world with four civilizations, shaping their behaviour through biometric calibration and intervening with real-time biometric and camera input.

This README is the canonical specification. If the code or an issue disagrees with it, this document wins until the specification is changed here.

---

## 1. Canonical decisions

| Area | Decision | Status |
| --- | --- | --- |
| **World** | One circular world containing **4 distinct civilizations**, each starting with **4 agents**. The four civilizations start in separate regions of the same world. The world has no edge: the map wraps both ways (a torus) and zooms out to a globe. | Canonical |
| **Seeds** | For the current dummy run, all four civilizations use **randomly generated seeds**. The player's civilization will later be seeded from biometric calibration. | Canonical for now |
| **Progression** | **Twenty emergent ages** ([docs/ages.md](docs/ages.md), data in `data/ages.json`): a civilization enters an age when its own simulation meets that age's conditions and can fall back in a collapse. **No tech tree, no research screen, no player-chosen upgrades, no evolution system.** | Canonical (decided by Alex) |
| **Stack** | Rust compiled to WebAssembly, rendered with WebGL2. The current JavaScript/Canvas build is the playable reference until the Rust port reaches parity (#17). | Canonical target |
| **Camera** | Camera input is **required**. Face detection supplies expressions and concentration cues. Hand tracking is for movement only: pinch to grab or pan, two hands to zoom (#261). No hand gesture casts a spell; gesture-driven UI navigation is deferred. | Required |
| **Head tracking** | Accelerometer **and gyroscope** supply head orientation and movement. | Required |
| **Heart rate** | Not an input. Removed in #99; stress (EEG agitation, or face anger) drives everything pulse used to. | Removed |
| **Calibration** | The 10-second window is a placeholder for the dummy run. The window must be configurable, and the dummy run uses a **random setup** instead of a real seed. | Canonical placeholder |
| **Ambient modulation** | EEG/facial signals driving global temperament, chaos, and weather. | Deferred; not important for the current milestone |
| **Gesture UI** | Hand gestures selecting menus and navigating the UI. | Deferred |
| **God powers** | Deliberate blinks throw lightning; holding focus (Rain or Calm) summons rain; hands grab/move living things and pan, and two hands zoom. The old hand/keyboard spell casts were removed (#261). | In scope (updated) |

---

## 2. Inputs (hardware and data intake)

Raw physical and biometric streams captured from the player:

* **Muse 2 EEG headset**
  * **Brainwaves:** Gamma, delta, theta, alpha, and beta band power for concentration and mental state.
  * **Accelerometer and gyroscope:** head orientation and movement. A headset that sends no orientation leaves the mouse and arrow keys aiming, with a one-time notice (#175).
  * **Signal handling:** a real headset run starts on the first EEG frame, not when the bridge socket opens; **Connect** waits up to 10 s, then offers Retry or the emulated headset (#174). Poor contact pauses the era until the signal recovers, with a toast showing the quality (#175). No heart rate or PPG is read (#99).
* **External camera**
  * **Face detection:** facial expressions (for example smiling) and concentration cues. The webcam stays off until the player clicks **Start Camera**; frames never leave the device.
  * **Manual mood:** a **Happy ↔ Mix ↔ Surprised** slider in the Camera panel replaces the face's mood once **No emotion** (ticked by default) is cleared. It works with no webcam or headset (#155).
  * **Hand-gesture detection:** hand movement and position in real time, used for movement only: grab, pan, tilt and zoom (#261). Gesture-driven UI navigation is deferred.
* **Fallback input**
  * **Mouse, keyboard, and touch:** local development, testing, and demo fallback only. They are not the product input model.

---

## 3. Processing (signal translation and control mapping)

### 3.1 Stage 1 calibration (seed generation)

* Samples the player's sensor input before the game begins.
* The **10-second duration is an arbitrary placeholder** for the dummy run; it must be configurable. It is set in **⚙ Settings › Calibration**, by URL or in `localStorage` ([docs/calibration.md](docs/calibration.md)).
* For the current dummy run, the player's civilization uses a **random setup** rather than a real calibration seed.
* In the target design, biometric and facial readings become a foundational behavioural seed for the player's civilization: **friendliness, aggressiveness, and violence**.
* The other three civilizations always start from randomized seeds.
* The exact sensor-to-trait formulas remain open; see [§6](#6-todos-and-open-mechanics).
* The EEG panel's guided calibration wizard pauses the game while it is open, and its steps stand still if the headset stops sending data (#176).

### 3.2 Ambient world modulation

Continuously maps live EEG activity and facial expressions into global world variables such as temperament, chaos, and weather.

**Status: deferred.** This is gameplay polish and does not block the current milestone.

### 3.3 Active command and UI control

* Target design: hand gestures move things and the camera; they do not select or cast spells (#261).
* Target design: head tracking (accelerometer plus gyroscope), facial focus, and hand gestures navigate the UI.
* **Current milestone:** head tracking and facial/biometric signals are required; hand input is movement only (pinch + hand pans, pinch + head motion tilts the 3D view; two-hand zoom), and gesture-driven UI navigation is **deferred**.

---

## 4. Internals (core simulation engine)

The Rust/Wasm simulation loop and world rules.

### 4.1 World structure

* One **circular map** with no edge: it wraps east–west and north–south (a torus).
* **4 distinct civilizations**, each starting with **4 initial agents**.
* One civilization is the player's civilization. In the target design it is initialized from the calibration seed; for the current dummy run it is randomized like the others.
* The other three civilizations are initialized from randomized seeds.
* The four civilizations share the same world and must be able to meet, compete, and interact.
* **Home world (campaign):** a 640×360 map from the Rust terrain generator, random each run (#140), seen through a 320×180 view that pans and wraps both ways (#139).
* **Crowded-planet sandbox:** the planet size is picked on the title screen (Random, Small 1280×640 with 8 peoples, Medium 1920×960 with 12, Large 2560×1280 with 16, Huge 3072×1536 with 20). The pick is remembered, and `?planet=` overrides it (#164, #219). The player's four peoples start the world; the rest are rivals who arrive on their own.

### 4.2 Agent behaviour and AI

* Agents are autonomous.
* There is **no tech tree and no evolution system**.
* Agents act on their civilization's behavioural parameters (aggressiveness versus friendliness) and basic survival needs, and take on age-specific jobs (hunting and farming through mining, trade, priesthood, soldiering, factory work and space flight) plus daily life: sleep, meals, festivals, mourning, families, migration.
* Progression is the **twenty emergent ages** in [docs/ages.md](docs/ages.md): conditions met by the simulation, with collapses back a stage. It is not a technology progression the player chooses.
* **War councils:** in the crowded-planet sandbox, trained PPO policies drive war and peace for the simulated civilizations: a council per civilization every 4 s and a commander per army at war (#115, #213). The civilizations the player founds stay on the rules, and `?council=rules` turns the councils off. The policy runs inside the wasm; Python is only the offline trainer (`rl/civ_war/`).
* **Forests:** fires spread slowly and burn out, woodcutters fell nearby trees, and burnt or felled forest grows back beside standing trees (#265).

### 4.3 World actions (god powers)

* **Blink lightning:** deliberate blinks strike at the head cursor. In the Stone Age, strike the fire pit to light it or the giant chicken for dinner (#232). In the galaxy/globe a blink is a world click: it selects a planet or lands, never a UI control.
* **Calm / focus / stress:** EEG bands (and the keyboard fallback) drive the era mechanics.
* **Pinch grab & two-hand zoom:** hand input moves living entities and the camera; it does not cast spells. In the crowded-planet sandbox a pinch picks up the nearest person and sets them down on land (#272). The Camera sidebar has a guided pinch movement calibration with the live camera and hand wireframe: left, right, up and down targets for each hand, auto-advancing and auto-closing when complete. Per-axis gains persist per browser; head tilt is not calibrated because it already works.
* **Rain or Calm:** the Insight/focus signal held for ~2 s summons rain, putting out forest fires and the Stone campfire.
* **Forest fires:** lightning aimed at a tree starts a fire that spreads to nearby trees.
* **Manual mood:** joy lifts morale (in the Stone Age the tribe dances); surprise sets off a sky flash and lightning once each time the slider crosses into the surprised side (#155).

**Status:** the hand-gesture and keyboard spell casts (fireball, bounty, rain, bless, disaster, peace, point) were removed in #261.

---

## 5. Presentation (rendering and UI)

* **Visual style:** pixel-art aesthetic, similar to *WorldBox*. The world is shown in 3D by default (WebGL heightmap); **T** switches to the 2D classic view.
* **Day and night:** in 3D the sun crosses the sky over a 150-second day; nights are moonlit and stay readable. An era's own darkness (the Stone Age before its fire) always wins (#242).
* **Zoom:** one continuous zoom from the surface to the globe and the galaxy, by scroll wheel, trackpad pinch, two-hand pinch, **Z**/**X** or the on-screen buttons. In 3D it zooms toward the cursor (#142, #207, #228, #248).
* **Galaxy picker:** **Left/Right** (or **A**/**D**, or a sideways scroll) choose a world; **Enter** or any zoom-in lands on it. Clicking a planet works too (#259).
* **Era intro:** each era opens with a 3.2-second camera move onto its stage instead of a card to dismiss. **Space**, **Enter** or a click skips it (#224).
* **Top bar:** EEG, 3D, AI God, Sound and **⚙ Settings**, plus a **● Camera on** chip while the webcam runs (#275).
* **Settings:** one panel, also on the title screen and closed with **Esc**: reduce motion, head aim (sensitivity and inversion), the calibration window, camera on at start, and Advanced: the Camera (**C**), Model (**M**) and Debug (**`**) panels. Everything is saved in the browser (#275). **Shift+C** re-centres the headset's head pose (#239).
* **UI layer:** the target is a hands-free interface controlled through gestures, head movement, and concentration.
* **Current milestone:** the HUD and existing fallback controls remain; gesture navigation is deferred.

---

## 6. TODOs and open mechanics

### Required next

* [ ] **Circular world with four civilizations:** one circular map, four separate starting regions, four agents per civilization, and random seeds for the dummy run. Tracked in #52.
* [ ] **Camera input:** face detection and expression/concentration cues. Hand tracking is movement only (#261); gesture-driven UI navigation remains deferred. Tracked in #33.
* [ ] **Head tracking:** accelerometer plus gyroscope orientation, neutral-point calibration, and aim. Tracked in #53.
* [ ] **Calibration seed:** configurable window, random dummy setup, and the final sensor-to-trait mapping for friendliness, aggressiveness, and violence. Tracked in #54.
* [ ] **Inter-civilization interactions:** define contact, combat, and non-hostile interaction between the four civilizations.
* [ ] **Basic needs:** define survival needs such as hunger, rest, and reproduction.
* [ ] **Resource system:** define resource types such as food and materials, and the rules for natural versus player-triggered spawning.
* [ ] **Population growth and death:** define how new agents spawn, how agents die, and what happens when a civilization is wiped out.
* [ ] **Terrain and weather:** define the circular world's terrain features and whether weather has mechanical effects on agents.
* [ ] **UI layout:** define the on-screen HUD and the future gesture-navigation flow.

### Deferred

* [ ] **Ambient world modulation:** EEG and facial expressions driving global temperament, chaos, and weather.
* [ ] **Gesture UI:** gesture-based menu selection and UI navigation.

Tracked in #55.

---

## 7. Current build status

The Rust/Wasm engine and WebGL renderer are landing under #17; the JavaScript game shell remains playable while the port reaches parity. The current build has three entry points: the five-scene campaign, the galaxy sandbox (home world plus three brain-seeded worlds), and the crowded-planet sandbox (`src/civ/`). The crowded-planet sandbox implements the canonical world model; the campaign is still the default button. Camera face mood, the manual mood slider and hand grab/pan/zoom are active; gyroscope fusion and the configurable calibration seed are in. Treat the build as a reference implementation, not as the specification.

* **Campaign:** Stone Age (light the fire, feed the tribe with a giant chicken, keep the flame with calm) → Farming · Kingdoms montage (the scholars ink your thoughts letter by letter, faster the harder you focus; no arithmetic, #223) → Industry (stress moves the goat market) → a short Information Age time skip → Space Age (hold focus through a ten-second countdown).
* **Keys (emulated headset):** **Space** calm, **F** focus / Rain or Calm, **Shift** stress, **B** or click to blink, mouse or arrows to aim, drag to pan, **E** / **C** the EEG / Camera sidebars, **M** the model panel, **`** the Debug panel, **G** the AI God, **N** the next world. The demo's full hotkey card is in [docs/demo.md](docs/demo.md).

---

## 8. Tracking

| Area | Issue |
| --- | --- |
| Rust/Wasm/WebGL port | #17 |
| Camera and face input | #33 |
| Circular world with four civilizations | #52 |
| Accelerometer plus gyroscope head tracking | #53 |
| Calibration seed | #54 |
| Deferred scope (ambient, gesture UI navigation) | #55 |
| Continuous README/code coherence loop | #56 |
