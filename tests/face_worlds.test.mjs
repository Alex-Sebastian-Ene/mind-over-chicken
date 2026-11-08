// The world cards read the player's real expression when the camera sees one.
import test from 'node:test';
import assert from 'node:assert/strict';
import { faceFeeling, planetFromMood } from '../src/ai/personality.js';

test('a smile reads calm, anger reads stress, no expression falls back to the headset', () => {
  const smile = faceFeeling({ joy: 0.9, surprise: 0, anger: 0, sadness: 0 });
  const angry = faceFeeling({ joy: 0, surprise: 0.1, anger: 0.9, sadness: 0 });
  assert.ok(smile.calm > angry.calm && angry.stress > smile.stress);
  assert.equal(faceFeeling({ joy: 0, surprise: 0, anger: 0, sadness: 0 }), null);
  assert.equal(faceFeeling(null), null);
  assert.notDeepEqual(planetFromMood(smile, 0).traits, planetFromMood(angry, 0).traits, 'different faces make different worlds');
});
