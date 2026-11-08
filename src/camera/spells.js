// camera (#33, #116, #122): the three core hand moves replace the old seven-cast bar.
//   1. Pinch       — grab / pick up a living entity under the cursor (or confirm selection)
//   2. Open Palm   — calm / peace / bless / bounty
//   3. Closed Fist — strike / lightning / fireball at the head reticle
//
// Legacy spell IDs remain in SPELL_BY_ID and ADVANCED_SPELLS so scenes and ?advanced_casts=1
// continue to resolve without breaking.
export const CAMERA_CASTS_ENABLED = true;

export const SPELLS = [
  { id: 'pinch', key: 1, gesture: 'Pinch', glyph: '🤏', name: 'Grab', cooldown: 400,
    hint: 'Pinch thumb and index: pick up a living creature under the cursor, move the cursor, release to drop.' },
  { id: 'palm', aimed: true, key: 2, gesture: 'Open_Palm', glyph: '🖐️', name: 'Calm', cooldown: 5000,
    hint: 'Open palm: calm fear, broker peace and bless the land.' },
  { id: 'fist', aimed: true, key: 3, gesture: 'Closed_Fist', glyph: '✊', name: 'Strike', cooldown: 1500,
    hint: 'Closed fist: strike lightning or a fireball at the cursor.' },
];

export const ADVANCED_SPELLS = [
  { id: 'point', key: 1, gesture: 'Pointing_Up', glyph: '☝️', name: 'Aim', cooldown: 0,
    hint: 'Legacy aim: jump to the next landmark.' },
  { id: 'fireball', aimed: true, key: 2, gesture: 'Closed_Fist', glyph: '✊', name: 'Fireball', cooldown: 1500,
    hint: 'Closed fist: lightning strikes the reticle.' },
  { id: 'spawn', aimed: true, key: 3, gesture: 'Open_Palm', glyph: '🖐️', name: 'Bounty', cooldown: 6000,
    hint: 'Open palm: food, goats or rocket parts, by era.' },
  { id: 'rain', aimed: true, key: 4, gesture: 'Victory', glyph: '✌️', name: 'Rain', cooldown: 10000,
    hint: 'Victory: rain calms fear and boosts the harvest.' },
  { id: 'bless', key: 5, gesture: 'Thumb_Up', glyph: '👍', name: 'Bless', cooldown: 9000,
    hint: 'Thumb up: faith, morale and courage rise.' },
  { id: 'disaster', aimed: true, key: 6, gesture: 'Thumb_Down', glyph: '👎', name: 'Disaster', cooldown: 14000,
    hint: 'Thumb down: a storm hits the reticle.' },
  { id: 'peace', key: 7, gesture: 'ILoveYou', glyph: '🤟', name: 'Peace', cooldown: 15000,
    hint: 'I-love-you: ends strikes, riots and panics.' },
];

export const SPELL_BY_ID = Object.fromEntries([
  ...ADVANCED_SPELLS.map((s) => [s.id, s]),
  ...SPELLS.map((s) => [s.id, s]),
]);

// Short readable names for the gesture readout.
export const GESTURE_LABEL = {
  None: 'no gesture',
  Pinch: 'pinch',
  Pointing_Up: 'pointing',
  Closed_Fist: 'fist',
  Open_Palm: 'open palm',
  Victory: 'victory',
  Thumb_Up: 'thumb up',
  Thumb_Down: 'thumb down',
  ILoveYou: 'I-love-you',
};

// Digit1..Digit3 and Numpad1..Numpad3 map to the three core moves.
export function spellForKeyCode(code, spells = SPELLS) {
  const m = /^(?:Digit|Numpad)([1-9])$/.exec(code || '');
  if (!m) return null;
  const k = Number(m[1]);
  return spells.find((s) => s.key === k) || null;
}
