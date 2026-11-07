// Loading screen: says what is loading, how far along it is, and what failed, so a slow
// engine or model load never looks like a frozen game.
//
// - Blocking steps (the game engine) hold the full-screen panel (#loadingScreen, in index.html
//   so it shows before any script runs). It fades out once every blocking step has finished.
// - Background steps (the AI god's PPO policy, the camera models, planets forming) show as a
//   small pill in the corner (#loadingPill) and never block play.
//
// Kept DOM-light and dependency-free so the Rust shell (#108 wave 2) can port it 1:1.
//
//   loading.start('engine', 'Starting the game engine', { blocking: true });
//   loading.done('engine');                       // or loading.fail('engine', 'why')
//   loading.track('policy', 'Loading the AI god', promise, { doneText: 'AI god ready' }); // background
//   loading.track('world-3', 'Forming a planet', promise, { group: 'worlds', plural: 'Forming {n} planets' });

const steps = new Map(); // id -> { label, state: 'run'|'done'|'fail', blocking, group, plural, note, t0 }
let screenEl = null, listEl = null, barEl = null, hintEl = null, pillEl = null;
let hideTimer = 0, hintTimer = 0, pillTimer = 0;
const SLOW_MS = 8000;      // show a "still loading" hint after this
const PILL_LINGER_MS = 1600; // keep a finished background step's tick on screen this long

function els() {
  if (typeof document === 'undefined') return false;
  screenEl ||= document.getElementById('loadingScreen');
  listEl ||= document.getElementById('loadingSteps');
  barEl ||= document.getElementById('loadingBar');
  hintEl ||= document.getElementById('loadingHint');
  pillEl ||= document.getElementById('loadingPill');
  return true;
}

const icon = (s) => (s.state === 'done' ? '✓' : s.state === 'fail' ? '⚠' : '…');

function renderScreen() {
  if (!els() || !screenEl) return;
  const blocking = [...steps.values()].filter((s) => s.blocking);
  if (listEl) {
    listEl.replaceChildren(...blocking.map((s) => {
      const li = document.createElement('li');
      li.className = `ls-step is-${s.state}`;
      li.textContent = `${icon(s)} ${s.label}${s.note ? `: ${s.note}` : ''}`;
      return li;
    }));
  }
  const total = blocking.length || 1;
  const finished = blocking.filter((s) => s.state !== 'run').length;
  if (barEl) barEl.style.setProperty('--p', `${Math.round((finished / total) * 100)}%`);
  const running = blocking.some((s) => s.state === 'run');
  clearTimeout(hideTimer);
  if (!running && blocking.length) {
    screenEl.classList.add('is-done');
    hideTimer = setTimeout(() => { screenEl.hidden = true; }, 450);
  } else if (running) {
    screenEl.hidden = false;
    screenEl.classList.remove('is-done');
  }
}

function renderPill() {
  if (!els() || !pillEl) return;
  const bg = [...steps.values()].filter((s) => !s.blocking);
  const running = bg.filter((s) => s.state === 'run');
  const failed = bg.filter((s) => s.state === 'fail' && performance.now() - s.t1 < 4000);
  const justDone = bg.filter((s) => s.state === 'done' && performance.now() - s.t1 < PILL_LINGER_MS);
  let text = '', kind = '';
  if (running.length) {
    // Group same-kind steps ("Forming 3 planets") and show the oldest other one.
    const groups = new Map();
    for (const s of running) { const k = s.group || s.label; groups.set(k, [...(groups.get(k) || []), s]); }
    text = [...groups.values()].map((g) => (g.length > 1 && g[0].plural ? g[0].plural.replace('{n}', g.length) : g[0].label)).join(' · ');
    kind = 'run';
  } else if (failed.length) {
    const s = failed[failed.length - 1];
    text = `${s.label}${s.note ? `: ${s.note}` : ' failed'}`;
    kind = 'fail';
  } else if (justDone.length) {
    const d = justDone[justDone.length - 1];
    text = d.doneText || `${d.label.replace(/^Loading (the )?/, '')} ready`;
    kind = 'done';
  }
  clearTimeout(pillTimer);
  if (!text) { pillEl.hidden = true; return; }
  pillEl.hidden = false;
  pillEl.dataset.kind = kind;
  pillEl.textContent = `${kind === 'done' ? '✓ ' : kind === 'fail' ? '⚠ ' : ''}${text}`;
  if (kind !== 'run') pillTimer = setTimeout(renderPill, kind === 'fail' ? 4000 : PILL_LINGER_MS);
}

function render() {
  renderScreen();
  renderPill();
  if (!els() || !hintEl) return;
  clearTimeout(hintTimer);
  const slow = [...steps.values()].find((s) => s.blocking && s.state === 'run');
  hintEl.textContent = '';
  if (slow) hintTimer = setTimeout(() => {
    if (slow.state === 'run') hintEl.textContent = `Still ${slow.label.charAt(0).toLowerCase()}${slow.label.slice(1)}… a slow connection can take a while.`;
  }, Math.max(0, SLOW_MS - (performance.now() - slow.t0)));
}

export const loading = {
  start(id, label, { blocking = false, group = null, plural = null, doneText = null } = {}) {
    steps.set(id, { label, state: 'run', blocking, group, plural, doneText, note: '', t0: performance.now(), t1: 0 });
    render();
  },
  done(id, note = '') {
    const s = steps.get(id);
    if (!s) return;
    Object.assign(s, { state: 'done', note, t1: performance.now() });
    render();
  },
  fail(id, note = '') {
    const s = steps.get(id);
    if (!s) return;
    Object.assign(s, { state: 'fail', note, t1: performance.now() });
    render();
  },
  // Start a step and finish it when the promise settles. Returns the same promise.
  track(id, label, promise, opts = {}) {
    this.start(id, label, opts);
    Promise.resolve(promise).then(() => this.done(id), (err) => this.fail(id, err?.message ? String(err.message).slice(0, 80) : ''));
    return promise;
  },
  // For tests and the debug tab.
  snapshot() {
    return [...steps.entries()].map(([id, s]) => ({ id, label: s.label, state: s.state, blocking: s.blocking, note: s.note }));
  },
  reset() { steps.clear(); render(); },
};
