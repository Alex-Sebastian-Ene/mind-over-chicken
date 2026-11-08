// A simulated planet: its own World, its own Stone Age, its own AI brain. This is the only
// kind of world the AI god is allowed to drive. The PPO trainer (rl/env_server.mjs) builds
// exactly this object, and the browser can run as many as it likes (galaxy planets, demos).
//
// Rules this file enforces:
// - The AI's calm lives on the AI's own brain (aiInput), never on the player's headset.
// - A human may watch and play along (attachHuman): their blinks always go through, their
//   calm and focus count on top of the AI's (SharedBrain), and AI lightning never starts the
//   cooldown that would refuse the human's next blink. The AI adds; it never blocks.
// - Nothing here is created for, or attached to, the human's own campaign world.

import { World } from '../world/terrain.js';
import { StoneAge } from '../eras/stone.js';
import { Brain } from '../brain/brain.js';
import { SimSource } from '../brain/sources.js';
import { ProxyBrain, SharedBrain } from './proxyBrain.js';
import { observe, decodeAction, BOLT_COOLDOWN } from './observe.js';

const noop = () => {};
export const silentAdvisor = { say: noop, setEra: noop, update: noop };
export const silentAudio = new Proxy({}, { get: () => noop });

export class SimPlanet {
  // brain: 'sim' = the real Brain fed by an emulated headset that belongs to the AI (what the
  // policy was trained against, default); 'proxy' = a cheaper stand-in for many background planets.
  // world: an already generated World for this seed (e.g. built off-thread by the galaxy, #68).
  // width/height: the World's size (the home world's 640×360 when the AI god takes over the
  // campaign map, #129; the default 320×180 for the trainer and galaxy planets).
  constructor({ seed, biome, brain = 'sim', advisor = silentAdvisor, audio = silentAudio, world = null, width, height } = {}) {
    this.seed = Number.isFinite(seed) ? seed >>> 0 : (Math.random() * 1e9) >>> 0;
    this.world = world || new World(this.seed, { biome, width, height });
    if (brain === 'sim') {
      this.aiBrain = new Brain(new SimSource());
      this.aiInput = this.aiBrain.source.input;
    } else {
      this.aiBrain = new ProxyBrain();
      this.aiInput = this.aiBrain.input;
    }
    this.brainKind = brain;
    this.human = null;
    this.god = null;
    this.outcome = null;
    this.aiLastBolt = -Infinity;
    this.G = {
      world: this.world, brain: this.aiBrain, advisor, audio, hud: null,
      // Never the player's source: an inert stand-in so nothing mistakes this world for theirs.
      source: { kind: 'ai', input: this.aiInput, addStartle: noop, blink: noop, setAim: noop, disconnect: noop },
      scene: null, sceneIndex: 0, runTime: 0, shake: 0, simulated: true,
      stats: { lightning: 0, smitten: 0, births: 0, deaths: 0, recessions: 0, restarts: 0 },
      eraTimes: [], eraStart: 0, finished: false,
      complete: () => { this.outcome ||= 'win'; },
      fail: (reason) => { this.outcome ||= 'lose'; this.failReason ||= reason || null; },
      finish: noop,
    };
    this.scene = new StoneAge();
    this.scene.simulated = true;
    this.scene.label = 'Simulated planet · Stone Age';
    this.G.scene = this.scene;
    this.scene.enter(this.G);
  }

  // A fresh Stone Age (a new tribe) on the same world, e.g. after a win or a loss (#68).
  restart() {
    this.outcome = null;
    this.failReason = null;
    this.aiLastBolt = -Infinity;
    this.scene = new StoneAge();
    this.scene.simulated = true;
    this.scene.label = 'Simulated planet · Stone Age';
    this.G.scene = this.scene;
    this.scene.enter(this.G);
    if (this.god) { this.god.timer = 0; this.god.last = null; }
    return this;
  }

  // Opt-in, per planet: let an AI god (src/ai/autopilot.js) drive this world.
  setGod(god) { this.god = god; if (!god) this.aiInput.calm = false; return this; }

  // A human watches / plays along. Pass null to leave.
  attachHuman(humanBrain) {
    this.human = humanBrain || null;
    this.G.brain = this.human ? new SharedBrain(this.aiBrain, this.human) : this.aiBrain;
    return this;
  }

  // The human's blink on this world. Always delivered; only the scene's own cooldown applies.
  humanBlink(aim) { this.scene.onBlink(aim); }

  observe() { return observe(this.scene, this.G, { lastBolt: this.aiLastBolt }); }

  // Apply one AI decision: calm on the AI's own brain, lightning through scene.onBlink.
  act(action, rng = Math.random) {
    const scene = this.scene;
    const { aim, calm } = decodeAction(scene, action, rng);
    this.aiInput.calm = calm;
    if (aim && scene.t - this.aiLastBolt >= BOLT_COOLDOWN) {
      const before = scene.lastBolt, strikes = scene.strikes;
      if (!this.human) { this.aiBrain.aim.x = aim.x; this.aiBrain.aim.y = aim.y; }
      scene.onBlink(aim);
      if (scene.strikes !== strikes) {
        this.aiLastBolt = scene.t;
        if (this.human) scene.lastBolt = before; // AI bolts never refuse the human's next blink
      }
    }
    return { aim, calm };
  }

  // Advance the simulation (the human's brain, if attached, is stepped by the game loop).
  advance(dt) {
    this.G.brain.update(dt);
    this.G.runTime += dt;
    this.scene.update(dt);
  }

  // Browser convenience: let the god (if any) decide, then advance.
  update(dt) {
    if (this.god) this.god.update(this, dt);
    this.advance(dt);
  }
}
