// Death returns the camera home (#297). One transition, returnHome({ reason, level }), for
// every death path:
//
//   home campaign era fails            the fail screen stays up (#86, #172) and the camera eases
//                                      to the home globe behind it; Retry lands home again
//   an AI world's capital tribe dies   (you're on its surface) a short beat showing the cause,
//                                      then the home globe; the tribe regathers off-screen
//   the civ sandbox wipes you out      the home globe
//   a win, a manual quit, the title    nothing
//   or end screen
//
// Pure (no DOM, Node-tested in tests/return_home.test.mjs): returnHomePlan() decides whether to
// go, to which level and with which words; ReturnHome runs the one-shot move on a ZoomState
// (zoom.js, or the Cosmos built on it). It never locks input: once the zoom is asked for, the
// player can take it anywhere, and a player already moving the camera, landing, or in an era
// intro is left alone. Under reduced motion the move is a cut, not a glide (#90).

export const DEATHS = Object.freeze({ HOME_FAIL: 'home-fail', AI_TRIBE: 'ai-tribe', CIV_WIPED: 'civ-wiped' });
export const RETURN_BEAT = 1.5;     // seconds a dead world stays on screen, showing the cause
export const HOME_FAIL_BEAT = 0.5;  // the fail card opens first, then the camera eases out behind it
export const SWAP_AT = 0.9;         // leaving another world: home takes over once its globe is this far out
export const HOME_LINE = 'Your tribe is gone. The home world still needs you.';

const isDeath = (reason) => Object.values(DEATHS).includes(reason);
// "The tribe froze in the dark: blink on the fire pit." → "The tribe froze in the dark" (the
// advice after the colon is for the home campaign's fail card, not another world's god).
const firstClause = (text) => String(text).split(': ')[0].replace(/[.!]+$/, '').trim();

// Should this death take the camera home, and how?
//   reason     one of DEATHS (anything else, such as 'win' or 'quit', never moves the camera)
//   level      1 = the home globe (default), 2 = the galaxy; never 0: a death never lands you
//   world      the lost world's name, for the toast; cause: what killed it, shown during the beat
//   screen     'title' / 'end' when one of those screens is open; finished: the run is over
//   intro      an era intro is playing; navigating: the player is zooming, dragging or landing
//   returning  a return is already under way (repeated deaths are ignored)
//   watching   the dead world's surface is the one on screen (AI-world deaths only count then)
//   home       { forming, landable } of the home planet; at: { home, level } where the camera is
// Returns { go: false, why } or { go: true, why, level, delay, cause, toast, line }.
export function returnHomePlan({
  reason, level = 1, world = null, cause = null, screen = null, finished = false, intro = false,
  navigating = false, returning = false, watching = true, home = null, at = null,
} = {}) {
  const no = (why) => ({ go: false, why });
  if (!isDeath(reason)) return no('not a death');
  if (returning) return no('already returning');
  if (screen) return no(`${screen} screen open`);
  if (finished) return no('run finished');
  if (intro) return no('era intro');
  if (reason === DEATHS.AI_TRIBE && !watching) return no('not watching that world');
  if (navigating) return no('player is navigating');
  let to = level >= 2 ? 2 : 1;
  let note = null;
  if (home?.forming) { to = 2; note = 'Home is still forming. Here is the galaxy.'; }
  else if (home && home.landable === false) note = 'Home runs on its own here. Land once its surface is ready.';
  if (at?.home && at.level === to) return no('already home');
  const away = reason !== DEATHS.HOME_FAIL;
  const name = world || (reason === DEATHS.CIV_WIPED ? 'Your civilisation' : 'That world');
  return {
    go: true, why: reason, level: to,
    delay: away ? RETURN_BEAT : HOME_FAIL_BEAT,
    cause: away && cause ? `${name}: ${firstClause(cause)}` : null,
    toast: away ? `${name} is lost.${note ? ` ${note}` : ' Back to your home world.'}` : note,
    line: away ? HOME_LINE : null,
  };
}

// Is the camera looking at home? (focus null is the hand-played home; in the galaxy sandbox the
// autonomous home is a planet like any other.)
export const atHome = (cam) => !cam.focus || cam.focus === cam.homePlanet?.();
// The player is already moving the camera: a zoom in flight, the wheel or a pinch still held, a drag.
export const zoomBusy = (cam) => (cam.hold || 0) > 0 || Math.abs(cam.z - cam.target) > 0.05 || !!cam.drag || !!cam.pinch;

// The one-shot move. hooks: cause(text) during the beat, say(toast) and advise(line) on the way
// out, reduced() → true for a cut instead of a glide.
export class ReturnHome {
  constructor(cam, hooks = {}) {
    this.cam = cam;
    this.hooks = hooks;
    this.job = null;
  }

  get active() { return !!this.job; }

  // A death. ask: { reason, level, world, cause, planet } (planet: the AI world that died; it only
  // counts while its surface is on screen). ctx: { screen, finished, intro, navigating, G }.
  request(ask = {}, ctx = {}) {
    const cam = this.cam, home = cam.homePlanet?.() || null;
    const plan = returnHomePlan({
      ...ask, screen: ctx.screen, finished: ctx.finished, intro: ctx.intro,
      navigating: !!ctx.navigating || zoomBusy(cam),
      returning: !!this.job,
      watching: ask.planet ? !!cam.aiSurfaceInView && cam.world() === ask.planet : true,
      home: home && { forming: !!home.forming, landable: cam.canLand(cam.isCampaign(home) ? null : home) },
      at: { home: atHome(cam), level: cam.target },
    });
    if (!plan.go) return plan;
    this.job = { plan, t: plan.delay, phase: 'beat', focus: cam.focus, target: cam.target, G: ctx.G };
    if (plan.cause) this.hooks.cause?.(plan.cause);
    return plan;
  }

  cancel() { this.job = null; }

  // Every frame. ctx as in request(): the title or end screen, the run ending or an era intro
  // drop the return, and so does the player moving the camera themselves.
  update(dt, ctx = {}) {
    const j = this.job;
    if (!j) return;
    const cam = this.cam;
    if (ctx.screen || ctx.finished || ctx.intro || cam.focus !== j.focus || cam.target !== j.target) { this.job = null; return; }
    if (j.phase === 'beat') {
      if ((j.t -= dt) <= 0) this.depart();
    } else if (cam.z >= SWAP_AT) this.arrive();
  }

  depart() {
    const j = this.job, cam = this.cam;
    if (j.plan.toast) this.hooks.say?.(j.plan.toast);
    if (j.plan.line) this.hooks.advise?.(j.plan.line);
    // On another world: ease out to its globe first, so the dead surface curls away instead of
    // cutting to home; home takes over at globe height.
    if (!atHome(cam) && j.plan.level === 1 && cam.z < SWAP_AT && !this.hooks.reduced?.()) {
      cam.zoomTo(1, j.G);
      j.phase = 'leaving';
      j.target = cam.target;
      return;
    }
    this.arrive();
  }

  arrive() {
    const j = this.job, cam = this.cam, home = cam.homePlanet?.();
    this.job = null;
    if (home && !home.forming && !atHome(cam)) cam.focusPlanet(home, j.G);
    cam.zoomTo(j.plan.level, j.G);
    if (this.hooks.reduced?.()) { cam.z = cam.target; cam.vz = 0; }
  }
}
