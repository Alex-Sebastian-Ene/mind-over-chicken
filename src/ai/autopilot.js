// The AI god: plays a SIMULATED planet (src/ai/planet.js) with the PPO policy trained in rl/.
//
// PPO is only for simulation. The god is opt-in and per world: it only ever drives a
// SimPlanet it was explicitly given, never the human's campaign world, and it never touches
// the player's headset inputs. Its calm is pressed on the planet's own AI brain.
//
//   (main.js imports SimPlanet from src/ai/planet.js and AIGod from this file)
//   const planet = new SimPlanet({ seed: 42 }).setGod(new AIGod());
//   planet.attachHuman(G.brain);          // optional: you watch and play along
//   planet.update(dt);                    // every frame
//   planet.humanBlink(aim);               // route the player's blinks while watching it
//
// It sees exactly what the trainer saw (src/ai/observe.js), decides every 0.25 s of game
// time, and plays the scripted baseline god until the policy file has loaded (or if it fails).

import { scriptedAction, ACTIONS, DECISION_DT, OBS_SIZE } from './observe.js';
import { loadPolicy } from './policy.js';

const cache = new Map(); // one fetch per policy URL, shared by every planet

export function sharedPolicy(url) {
  const key = String(url || 'default');
  if (!cache.has(key)) {
    cache.set(key, loadPolicy(url).then((p) => {
      if (p.obsSize !== OBS_SIZE) throw new Error(`policy expects ${p.obsSize} inputs, observe.js gives ${OBS_SIZE}`);
      return p;
    }));
  }
  return cache.get(key);
}

// One decision on a SimPlanet: observe, choose, apply. Returns {action, name, obs}.
export function autopilotStep(planet, policy = null, { greedy = true } = {}) {
  const obs = planet.observe();
  const action = policy ? policy.act(obs, { greedy }) : scriptedAction(obs);
  planet.act(action);
  return { action, name: ACTIONS[action], obs };
}

export class AIGod {
  constructor({ url, greedy = true, policy = null } = {}) {
    this.greedy = greedy;
    this.policy = policy;
    this.error = null;
    this.timer = 0;
    this.last = null;
    if (!policy) {
      sharedPolicy(url)
        .then((p) => { this.policy = p; })
        .catch((e) => { this.error = e; console.warn('AI god: policy unavailable, playing the scripted god.', e); });
    }
  }

  // Call every frame with the planet it drives.
  update(planet, dt) {
    if (!planet?.scene || planet.scene.id !== 'stone' || planet.scene.done) return this.last;
    this.timer -= dt;
    if (this.timer > 0) return this.last;
    this.timer = Math.max(0, this.timer + DECISION_DT);
    this.last = autopilotStep(planet, this.policy, { greedy: this.greedy });
    return this.last;
  }

  status() {
    return `${this.policy ? 'PPO' : 'scripted'}: ${this.last?.name ?? '...'}`;
  }
}
