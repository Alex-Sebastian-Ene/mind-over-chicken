// Calibration → your civilisation (#190). The calibration window (#54) reads friendliness,
// aggressiveness and violence from your first seconds of play. In the civ sandbox your four
// peoples already take a temper from their own 5 s mind read while founded, so the calibration
// result is applied once founding is over, to your own (first) civilisation. Pure for tests.
const unit = (v) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : null);

// The three traits to apply, or null when there is no usable calibration result.
export function calibrationTraits(result) {
  if (!result) return null;
  const t = { friendliness: unit(result.friendliness), aggressiveness: unit(result.aggressiveness), violence: unit(result.violence) };
  return Object.values(t).some((v) => v === null) ? null : t;
}

// The sandbox's log line saying what was applied.
export function calibrationLine(t, name = 'your people') {
  const pct = (v) => `${Math.round(v * 100)}%`;
  return `Your calibration shaped ${name}: friendly ${pct(t.friendliness)}, aggressive ${pct(t.aggressiveness)}, violent ${pct(t.violence)}`;
}
