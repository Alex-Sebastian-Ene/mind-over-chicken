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
