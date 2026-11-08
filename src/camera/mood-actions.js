// Joy and surprise as game actions the player can see (#152). Pure decisions, no DOM: how strong
// the joy buff is, whether a surprise event may fire, how big its flash and shake are, and the
// words the HUD and coach use so the player learns which face did what. Scenes and camera.js
// read these; tests/mood_actions.test.mjs pins them.
//
// Joy is a continuous buff: it rises from JOY_START (a talking or polite face stays below it) to
// full at JOY_FULL. Surprise is a one-shot: the camera fires it after surprise ≥ 0.6 holds 0.3 s
// (emotion.js), the manual slider when it crosses into the surprised side (manual-mood.js), and
// either way at most once per SURPRISE_COOLDOWN.
import { flashAlpha } from '../ui/motion.js';

export const JOY_START = 0.3;          // below this the face isn't smiling (talking, a twitch)
export const JOY_FULL = 0.8;           // a clear smile gives the whole buff
export const SURPRISE_COOLDOWN = 4;    // seconds between surprise events, camera or slider
export const SURPRISE_SHOW = 1.5;      // seconds the HUD shouts "Surprise!" after one fires
export const SURPRISE_FLASH = 0.45;    // asked-for flash; flashAlpha caps it (0.15 reduced motion)
export const SURPRISE_SHAKE = 1.6;     // G.shake for a surprise; none under reduced motion

const clamp01 = (v) => Math.min(1, Math.max(0, Number.isFinite(v) ? v : 0));

// Is a mood live (camera or slider on)? G.mood says so with `active`; an onMood payload without
// it counts as live. No mood, or the camera off, is never live: no effect without a face.
export function moodLive(mood) {
  return !!mood && mood.active !== false && mood.source !== 'none';
}

// The joy buff, 0..1: 0 below JOY_START, rising linearly to 1 at JOY_FULL.
export function joyBuff(mood) {
  if (!moodLive(mood)) return 0;
  return clamp01((clamp01(mood.joy) - JOY_START) / (JOY_FULL - JOY_START));
}

// May a surprise fire now (seconds)? Once per SURPRISE_COOLDOWN, whatever the source.
export function surpriseReady(nowSec, lastAtSec, cooldown = SURPRISE_COOLDOWN) {
  return lastAtSec == null || !Number.isFinite(lastAtSec) || nowSec - lastAtSec >= cooldown;
}

// Does this onMood payload carry a surprise event? Only the one-shot counts, never a face that
// merely stays surprised (the old code re-flashed on every "strong" re-send).
export function surpriseFires(mood) {
  return moodLive(mood) && mood.surpriseTriggered === true;
}

// How big a surprise looks: a capped flash (motion.js), a short shake and a few bolts.
export function surpriseEffect(reduced = false) {
  return { flash: flashAlpha(SURPRISE_FLASH, reduced), shake: reduced ? 0 : SURPRISE_SHAKE, bolts: reduced ? 1 : 3 };
}

// What joy and surprise do in each kind of scene: `joy`/`surprise` for the cause line, `tag`
// for the short HUD chip.
export const MOOD_EFFECTS = {
  stone: { tag: 'tribe dances', joy: 'the tribe dances (more faith, less fear, faster births)', surprise: 'lightning cracks over the camp and the tribe jumps' },
  industry: { tag: 'markets steady', joy: 'the markets steady (less fear, more business confidence)', surprise: 'lightning hits the mill and the market lurches' },
  space: { tag: 'crowd cheers', joy: 'the crowd cheers (courage rises)', surprise: 'lightning over the launch pad makes the crowd gasp' },
  timeskip: { tag: 'the years glow', joy: 'the passing years glow', surprise: 'a thunderclap echoes down the centuries' },
  civ: { tag: 'festivals', joy: 'festivals break out across your civilisation', surprise: 'a thunderclap shakes the whole world' },
  surface: { tag: 'tribe dances', joy: 'the tribe on this world dances', surprise: 'lightning flashes over this world' },
};
const GENERIC = { tag: 'buff on', joy: 'the world brightens', surprise: 'the sky flashes' };

// Which MOOD_EFFECTS row a scene uses (null for one the table doesn't know).
export function sceneKind(scene) {
  const id = String(scene?.id ?? '');
  if (id.startsWith('timeskip')) return 'timeskip';
  if (MOOD_EFFECTS[id]) return id;
  return MOOD_EFFECTS[scene?.era] ? scene.era : null;
}

function effects(kind) { return MOOD_EFFECTS[kind] || GENERIC; }

// The cause line when joy takes over: names the emotion and what it does here.
export function joyCause(kind, source = 'camera') {
  return `${source === 'manual' ? '😊 Manual joy' : '😊 Your smile'}: ${effects(kind).joy}`;
}

// The cause line when a surprise fires.
export function surpriseCause(kind, source = 'camera') {
  return `${source === 'manual' ? '😮 Manual surprise' : '😮 Your surprised face'}: ${effects(kind).surprise}!`;
}

// The HUD mood chip: null while no mood is live, else { text, tone, title }. tone is 'surprise'
// just after a surprise fired, 'joy' while the buff is on, otherwise 'idle'.
export function moodChip(mood, kind, nowSec) {
  if (!moodLive(mood)) return null;
  const e = effects(kind);
  const joy = joyBuff(mood);
  const since = Number.isFinite(mood.surpriseAt) ? nowSec - mood.surpriseAt : Infinity;
  const joyText = joy > 0 ? `😊 Joy ${Math.round(joy * 100)}% · ${e.tag}` : '😊 Smile for a buff';
  const surpriseText = since < SURPRISE_SHOW ? '😮 Surprise!'
    : since < SURPRISE_COOLDOWN ? `😮 Surprise in ${Math.ceil(SURPRISE_COOLDOWN - since)} s` : '😮 Surprise ready';
  const tone = since < SURPRISE_SHOW ? 'surprise' : joy > 0 ? 'joy' : 'idle';
  const title = `Smile: ${e.joy}. Look surprised: ${e.surprise} (once every ${SURPRISE_COOLDOWN} s).`;
  return { text: `${joyText} · ${surpriseText}`, tone, title };
}

// Sparkles rising from joy: how many to spawn this frame (rate per second at full joy). Callers
// pass a random 0..1 so the fraction carries over fairly without state.
export function sparkleCount(joy, dt, rate = 14, random = Math.random()) {
  const n = clamp01(joy) * Math.max(0, dt) * rate;
  return Math.floor(n) + (random < n - Math.floor(n) ? 1 : 0);
}

export const JOY_COLORS = ['#ffe066', '#ff9ec8', '#fff4c2', '#9be38a'];
