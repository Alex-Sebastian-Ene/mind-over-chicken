# Mind over Chicken

A web-based 2D pixel-art god-game sandbox inspired by *WorldBox*. The player is a god overseeing one circular world with four civilizations, shaping their behaviour through biometric calibration and intervening with real-time biometric and camera input.

This README is the canonical specification. If the code or an issue disagrees with it, this document wins until the specification is changed here.

---

## 1. Canonical decisions

| Area | Decision | Status |
| --- | --- | --- |
| **World** | One circular world containing **4 distinct civilizations**, each starting with **4 agents**. The four civilizations start in separate regions of the same world. | Canonical |
| **Seeds** | For the current dummy run, all four civilizations use **randomly generated seeds**. The player's civilization will later be seeded from biometric calibration. | Canonical for now |
| **Progression** | **No tech tree and no evolution system.** The eras are fixed content/levels that present the civilization's ages; they are not research or evolution branches. | Canonical |
| **Stack** | Rust compiled to WebAssembly, rendered with WebGL2. The current JavaScript/Canvas build is the playable reference until the Rust port reaches parity (#17). | Canonical target |
| **Camera** | Camera input is **required**. Face detection supplies expressions and concentration cues. Hand-gesture detection is part of the camera input, but gesture-driven spell selection and UI navigation are deferred. | Required |
| **Head tracking** | Accelerometer **and gyroscope** supply head orientation and movement. | Required |
| **Calibration** | The 10-second window is a placeholder for the dummy run. The window must be configurable, and the dummy run uses a **random setup** instead of a real seed. | Canonical placeholder |
| **Ambient modulation** | EEG/facial signals driving global temperament, chaos, and weather. | Deferred; not important for the current milestone |
| **Gesture UI** | Hand gestures selecting, activating, and targeting spells, and navigating the UI. | Deferred |
| **God powers** | Spawn resources, send fireball, trigger natural disasters. | Deferred |

---

## 2. Inputs (hardware and data intake)

Raw physical and biometric streams captured from the player:

* **Muse 2 EEG headset**
  * **Brainwaves:** Gamma, delta, theta, alpha, and beta band power for concentration and mental state.
  * **Accelerometer and gyroscope:** head orientation and movement.
* **External camera**
  * **Face detection:** facial expressions (for example smiling) and concentration cues.
  * **Hand-gesture detection:** hand movement and position in real time. Detection is in scope; gesture-driven UI and spellcasting are deferred.
* **Fallback input**
  * **Mouse, keyboard, and touch:** local development, testing, and demo fallback only. They are not the product input model.

---

## 3. Processing (signal translation and control mapping)

### 3.1 Stage 1 calibration (seed generation)

* Samples the player's sensor input before the game begins.
* The **10-second duration is an arbitrary placeholder** for the dummy run; it must be configurable.
* For the current dummy run, the player's civilization uses a **random setup** rather than a real calibration seed.
* In the target design, biometric and facial readings become a foundational behavioural seed for the player's civilization: **friendliness, aggressiveness, and violence**.
* The other three civilizations always start from randomized seeds.
* The exact sensor-to-trait formulas remain open; see [§6](#6-todos-and-open-mechanics).

### 3.2 Ambient world modulation

Continuously maps live EEG activity and facial expressions into global world variables such as temperament, chaos, and weather.

**Status: deferred.** This is gameplay polish and does not block the current milestone.

### 3.3 Active command and UI control

* Target design: hand gestures select and activate spells and target locations in the world.
* Target design: head tracking (accelerometer plus gyroscope), facial focus, and hand gestures navigate the UI.
* **Current milestone:** head tracking and facial/biometric signals are required; gesture-driven spell selection and UI navigation are **deferred**.

---

## 4. Internals (core simulation engine)

The Rust/Wasm simulation loop and world rules.

### 4.1 World structure

* One **circular map**.
* **4 distinct civilizations**, each starting with **4 initial agents**.
* One civilization is the player's civilization. In the target design it is initialized from the calibration seed; for the current dummy run it is randomized like the others.
* The other three civilizations are initialized from randomized seeds.
* The four civilizations share the same world and must be able to meet, compete, and interact.

### 4.2 Agent behaviour and AI

* Agents are autonomous.
* There is **no tech tree and no evolution system**.
* Agents act on their civilization's behavioural parameters (aggressiveness versus friendliness) and basic survival needs.
* Eras are fixed content/levels, not a technology progression.

### 4.3 World actions (god powers)

* **Spawn Resources:** drop resources at a targeted location.
* **Send Fireball:** launch a destructive fireball at a targeted location.
* **Natural Disasters:** trigger global or localized environmental events.

**Status: deferred.** These are not part of the current milestone. The eventual cast path is camera gestures plus head tracking.

---

## 5. Presentation (rendering and UI)

* **Visual style:** 2D pixel-art aesthetic, similar to *WorldBox*.
* **UI layer:** the target is a hands-free interface controlled through gestures, head movement, and concentration.
* **Current milestone:** the HUD and existing fallback controls remain; gesture navigation is deferred.

---

## 6. TODOs and open mechanics

### Required next

* [ ] **Circular world with four civilizations:** one circular map, four separate starting regions, four agents per civilization, and random seeds for the dummy run. Tracked in #52.
* [ ] **Camera input:** face detection and expression/concentration cues. Hand-gesture detection is part of the camera input; gesture-driven UI remains deferred. Tracked in #33.
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
* [ ] **Gesture UI:** gesture-based spell selection, activation, targeting, and navigation.
* [ ] **God powers:** spawn resources, fireball, and natural disasters.

Tracked in #55.

---

## 7. Current build status

The Rust/Wasm engine and WebGL renderer are landing under #17; the JavaScript game shell remains playable while the port reaches parity. The current build runs one hand-played civilization on the home world plus three autonomous brain-seeded worlds, one civilization each — not four civilizations sharing one circular world. It has no camera input, uses accelerometer tilt without gyroscope fusion, and trains a hidden calm/focus classifier instead of producing civilization traits. Treat the build as a reference implementation, not as the specification.

---

## 8. Tracking

| Area | Issue |
| --- | --- |
| Rust/Wasm/WebGL port | #17 |
| Camera and face input | #33 |
| Circular world with four civilizations | #52 |
| Accelerometer plus gyroscope head tracking | #53 |
| Calibration seed | #54 |
| Deferred scope (ambient, gesture UI, god powers) | #55 |
| Continuous README/code coherence loop | #56 |
