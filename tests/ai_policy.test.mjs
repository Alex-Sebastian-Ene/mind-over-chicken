// The AI god's policy must take exactly the inputs observe.js gives, or autopilot.js refuses
// it ("policy expects N inputs") and the god never loads.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { loadPolicy, setRustAiPolicy } from '../src/ai/policy.js';
import { OBS_SIZE } from '../src/ai/observe.js';

test('the Rust policy reports the same input count as observe.js', async () => {
  setRustAiPolicy(class { act() { return 0; } probs() { return new Float32Array(5); } self_check() { return 0; } });
  try {
    const p = await loadPolicy();
    assert.equal(p.obsSize, OBS_SIZE);
  } finally {
    setRustAiPolicy(null);
  }
});

test('moc-ai is built for the same input count as observe.js', () => {
  const src = readFileSync(new URL('../crates/moc-ai/src/policy.rs', import.meta.url), 'utf8');
  const m = src.match(/pub const NUM_INPUTS: usize = (\d+);/);
  assert.ok(m, 'NUM_INPUTS not found in crates/moc-ai/src/policy.rs');
  assert.equal(Number(m[1]), OBS_SIZE);
});

test('the shipped JSON policy lists the same inputs as observe.js', () => {
  const p = JSON.parse(readFileSync(new URL('../src/ai/stone_policy.json', import.meta.url), 'utf8'));
  assert.equal(p.obs_names.length, OBS_SIZE);
});

// #181: the AI god says which policy is steering, and loads the real one.
test('the AI god reports a scripted fallback instead of only logging it', async () => {
  const { AIGod } = await import('../src/ai/autopilot.js');
  const seen = [];
  const warn = console.warn; console.warn = () => {};
  try {
    const god = new AIGod({ url: 'file:///no/such/policy.json', onStatus: (mode, err) => seen.push([mode, !!err]) });
    assert.equal(god.mode, 'loading');
    await new Promise((r) => setTimeout(r, 50));
    assert.equal(god.mode, 'scripted');
    assert.deepEqual(seen, [['scripted', true]]);
  } finally {
    console.warn = warn;
  }
});

test('the real wasm policy loads, reports ppo and acts on a real observation', async () => {
  await import('../rl/headless.mjs');   // initialises the engine wasm, as the browser does
  const { RustAiPolicy } = await import('../pkg/mind_over_chicken_engine.js');
  const { AIGod } = await import('../src/ai/autopilot.js');
  const { SimPlanet } = await import('../src/ai/planet.js');
  const { ACTIONS } = await import('../src/ai/observe.js');
  setRustAiPolicy(RustAiPolicy);
  try {
    const seen = [];
    const god = new AIGod({ url: 'wasm-real-policy-test', onStatus: (mode) => seen.push(mode) });
    await new Promise((r) => setTimeout(r, 50));
    assert.deepEqual(seen, ['ppo']);
    assert.equal(god.mode, 'ppo');
    const planet = new SimPlanet({ seed: 3, brain: 'proxy' });
    const obs = planet.observe();
    assert.equal(obs.length, OBS_SIZE);
    const a = god.policy.act(obs, { greedy: true });
    assert.ok(Number.isInteger(a) && a >= 0 && a < ACTIONS.length, `action ${a}`);
    assert.ok(god.policy.selfCheck() < 1e-3, 'the wasm policy matches the PyTorch export');
  } finally {
    setRustAiPolicy(null);
  }
});
