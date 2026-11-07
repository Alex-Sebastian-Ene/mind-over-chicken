# Project README

## 1. Overview
A high-performance, web-based 2D pixel-art god-game sandbox inspired by *WorldBox*. Built with Rust and WebAssembly (Wasm), the game places the player in the role of a "god" overseeing a circular world containing 4 civilizations. The player shapes civilization traits and intervenes in the simulation using real-time biometric and camera inputs.

---

## 2. Inputs (Hardware & Data Intake)
Raw physical and biometric data streams captured from the player:

* **Muse 2 EEG Headset**
  * **Brainwaves:** Captures Gamma, Delta, and other wave frequencies to measure concentration and mental states.
  * **Accelerometer & Gyroscope:** Captures head orientation and movement.
* **External Camera**
  * **Face Detection:** Captures facial expressions (e.g., smiling) and concentration cues.
  * **Hand Gesture Detection:** Captures hand movements and positions in real time.
* **Fallback Input**
  * **Mouse:** Basic cursor input reserved for local development and testing.

---

## 3. Processing (Signal Translation & Control Mapping)
Translates raw sensor data into actionable game parameters and commands:

* **Stage 1 Calibration (Seed Generation)**
  * Samples 10 seconds of sensor input before the game begins.
  * Converts biometric and facial readings into a foundational behavioral seed (defining friendliness, aggressiveness, and violence levels) for the player's civilization.
* **Ambient World Modulation**
  * Continuously maps live EEG wave activity (e.g., Gamma levels) and facial expressions into global world variables such as general temperament, chaos levels, and weather states.
* **Active Command & UI Control**
  * Maps hand gestures to spell selection, activation, and spatial targeting on the screen.
  * Combines head tracking (accelerometer/gyroscope), facial focus, and hand gestures to navigate the UI.

---

## 4. Internals (Core Simulation Engine)
The underlying Rust/Wasm simulation loop and world rules:

* **World Structure**
  * Circular map housing 4 distinct civilizations (starting with 4 initial agents).
  * 1 civilization is initialized via the player's 10-second calibration seed; the other 3 are initialized with randomized seeds.
* **Agent Behavior & AI**
  * Autonomous agents operate without a tech tree or evolution system.
  * Agents act based on their civilization's behavioral parameters (aggressiveness vs. friendliness) and basic survival needs.
* **World Actions (God Powers)**
  * **Spawn Resources:** Drops resources at a gesture-targeted location.
  * **Send Fireball:** Launches a destructive fireball at a gesture-targeted location.
  * **Natural Disasters:** Triggers global or localized environmental events.

---

## 5. Presentation (Rendering & UI)
Visual output running in the browser:

* **Visual Style:** 2D pixel-art aesthetic (similar to *WorldBox*).
* **UI Layer:** Hands-free interface controlled via gestures, head movement, and concentration.

---

## 6. TODOs & Open Mechanics
Features and rules to be defined and implemented in future iterations:

* [ ] **Basic Needs:** Define specific agent survival needs (e.g., hunger, rest, reproduction).
* [ ] **Resource System:** Define resource types (e.g., food, materials) and natural vs. player-triggered spawning rules.
* [ ] **Inter-Civilization Interactions:** Define specific contact mechanics between civilizations (likely combat/attacking, plus any non-hostile interactions).
* [ ] **Population Growth & Death:** Define how new agents spawn and what happens when a civilization is wiped out.
* [ ] **Terrain & Weather Mechanics:** Define circular world terrain features and whether weather has mechanical effects on agents.
* [ ] **Sensor-to-Trait Mapping:** Finalize exact mathematical formulas linking EEG/camera inputs to civilization seeds and spells.
* [ ] **UI Layout:** Design the on-screen HUD and gesture navigation flow.
