// camera (#33, #116, #122, #150, #159): the three core hand moves, keys 1–3. Mirrors
// crates/engine/src/camera/spells.rs, which is what the browser runs (RustCastController):
//   1. Pinch     — grab / pick up a living entity under the cursor
//   2. Open Palm — Calm: held, once per hold
//   3. Fireball  — a closed fist CHARGES it, opening the hand RELEASES it (#150); a held fist alone
//                  never casts. The separate "Closed Fist · Strike" is gone (#159), and so is the
//                  legacy seven-cast table: those gestures cast nothing.
export const CAMERA_CASTS_ENABLED = true;

export const SPELLS = [
  { id: 'pinch', key: 1, gesture: 'Pinch', glyph: '🤏', name: 'Grab', cooldown: 400,
    hint: 'Pinch thumb and index: pick up a living creature under the cursor, move the cursor, release to drop.' },
  { id: 'palm', aimed: true, key: 2, gesture: 'Open_Palm', glyph: '🖐️', name: 'Calm', cooldown: 5000,
    hint: 'Open palm: calm fear, broker peace and bless the land.' },
  { id: 'fireball', aimed: true, key: 3, gesture: 'Closed_Fist', release: 'Open_Palm', glyph: '🔥', name: 'Fireball', cooldown: 1500,
    hint: 'Close your fist to charge, open your hand to throw a fireball at the cursor (or hold 3 and release).' },
];

export const SPELL_BY_ID = Object.fromEntries(SPELLS.map((s) => [s.id, s]));

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
