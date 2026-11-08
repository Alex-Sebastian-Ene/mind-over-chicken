// The title screen: a short menu (Continue, New Game, Load, Sections, Calibrate, Options) and three
// sub-views that slide in place with a Back button. Arrows move between the entries, Enter picks,
// Escape goes back. The pure helpers at the top are what the tests check; initTitle wires the DOM.
import * as localSaves from '../game/saves.js';

export const MAIN_MENU = ['Continue', 'New Game', 'Load', 'Sections', 'Calibrate', 'Options'];
export const MODE_LABELS = { story: 'Story', galaxy: 'Galaxy', crowded: 'Crowded planet' };

// Which optional entries the main menu shows: Continue and Load only once a run is saved.
export function menuState(saves) {
  const any = Array.isArray(saves) && saves.length > 0;
  return { continue: any, load: any };
}

// "just now", "5 min ago", "3 h ago", "2 days ago", else the date.
export function timeAgo(ts, now = Date.now()) {
  const t = Number(ts);
  if (!Number.isFinite(t) || t <= 0) return '';
  const s = Math.max(0, (now - t) / 1000);
  if (s < 60) return 'just now';
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 7 * 86400) { const d = Math.floor(s / 86400); return `${d} day${d === 1 ? '' : 's'} ago`; }
  return new Date(t).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

// One saved run as the Load list shows it: "Run 3" over "Story · Industry · 5 min ago".
export function saveLabel(save, now = Date.now()) {
  const n = Math.floor(Number(save?.runNumber));
  const name = n > 0 ? `Run ${n}` : String(save?.name || 'Run').split(' · ')[0];
  const mode = MODE_LABELS[save?.mode] || '';
  const scene = save?.sceneLabel && save.sceneLabel.toLowerCase() !== mode.toLowerCase() ? save.sceneLabel : '';
  return { name, meta: [mode, scene, timeAgo(save?.updatedAt, now)].filter(Boolean).join(' · ') };
}

// The next entry for an arrow key, wrapping at both ends.
export function stepIndex(i, n, dir) {
  if (n <= 0) return -1;
  if (i < 0) return dir < 0 ? n - 1 : 0;
  return (i + dir + n) % n;
}

const ARROWS = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };

// deps:
//   saves        { listSaves(), latestSave(), deleteSave(id) }, src/game/saves.js by default
//   onContinue   (save) resume a saved run
//   onSection    (sceneIndex) a fresh run at that era (Galaxy and Crowded planet keep their own handlers)
//   onCalibrate  () open the guided calibration wizard
//   headsetLive  () true while a headset streams EEG
//   isBusy       () true while a panel or modal above the title owns the keyboard
export function initTitle(doc, { saves = localSaves, onContinue, onSection, onCalibrate, headsetLive = () => false, isBusy = () => false } = {}) {
  const root = doc.getElementById('titleScreen');
  if (!root) return null;
  const q = (s) => root.querySelector(s);
  const views = Object.fromEntries([...root.querySelectorAll('[data-title-view]')].map((v) => [v.dataset.titleView, v]));
  const contBtn = q('[data-title-menu="continue"]'), newBtn = q('[data-title-menu="new"]'), loadBtn = q('[data-title-menu="load"]');
  const contSub = q('[data-title-continue-sub]'), list = q('[data-save-list]');
  const calibBtn = q('[data-title-calibrate]'), status = doc.getElementById('bridgeStatus');
  let view = 'main', opener = null, latest = null, poll = null;

  const all = () => { try { return saves?.listSaves?.() || []; } catch { return []; } };
  const visible = (el) => !el.hidden && !el.closest('[hidden]') && !el.disabled && el.getClientRects().length > 0;
  // A radio group (the planet size) is one stop for Up/Down: its checked button.
  const items = () => [...(views[view]?.querySelectorAll('button, input') || [])]
    .filter((el) => visible(el) && (el.getAttribute('role') !== 'radio' || el.getAttribute('aria-checked') === 'true'));
  const focusFirst = () => items()[0]?.focus();

  function refresh() {
    const list0 = all();
    const show = menuState(list0);
    latest = show.continue ? (saves?.latestSave?.() ?? list0[0]) : null;
    contBtn.hidden = !latest;
    loadBtn.hidden = !show.load;
    newBtn.classList.toggle('btn-primary', !latest);
    if (contSub) contSub.textContent = latest ? [saveLabel(latest).name, latest.sceneLabel].filter(Boolean).join(' · ') : '';
    renderSaves(list0);
    return list0;
  }

  function renderSaves(list0) {
    if (!list) return;
    list.replaceChildren(...list0.map((s) => {
      const { name, meta } = saveLabel(s);
      const li = doc.createElement('li');
      li.className = 'save-row';
      const open = doc.createElement('button');
      open.type = 'button'; open.className = 'btn save-open';
      open.innerHTML = '<span class="save-name"></span><span class="save-meta"></span>';
      open.firstChild.textContent = name;
      open.lastChild.textContent = meta;
      open.addEventListener('click', () => onContinue?.(s));
      const del = doc.createElement('button');
      del.type = 'button'; del.className = 'btn save-del';
      del.textContent = '×';
      del.setAttribute('aria-label', `Delete ${name}`);
      // Two clicks: the first arms it ("Delete?"), so a stray click never loses a run.
      del.addEventListener('click', () => {
        if (!del.dataset.armed) { del.dataset.armed = '1'; del.textContent = 'Delete?'; return; }
        try { saves?.deleteSave?.(s.id); } catch {}
        const left = refresh();
        if (!left.length) show('main');
        else items()[0]?.focus();
      });
      del.addEventListener('blur', () => { delete del.dataset.armed; del.textContent = '×'; });
      li.append(open, del);
      return li;
    }));
  }

  function syncCalibrate() {
    const live = !!headsetLive();
    if (calibBtn) calibBtn.disabled = !live;
    // Only the idle line follows the headset; "Connecting…" and errors stay until the next try.
    if (status) {
      const want = live ? 'Muse connected.' : 'No headset connected.';
      if (!status.textContent || /^(Muse connected|No headset connected|Connected to Muse)/.test(status.textContent)) status.textContent = want;
    }
  }

  function show(name, focus = true) {
    if (!views[name]) return;
    for (const [k, v] of Object.entries(views)) v.hidden = k !== name;
    const back = name === 'main' && view !== 'main' ? opener : null;
    view = name;
    clearInterval(poll); poll = null;
    if (name === 'calibrate') { syncCalibrate(); poll = setInterval(syncCalibrate, 400); }
    if (!focus) return;
    if (back && visible(back)) back.focus(); else focusFirst();
  }

  for (const b of root.querySelectorAll('[data-title-open]')) {
    b.addEventListener('click', () => { opener = b; if (b.dataset.titleOpen === 'load') refresh(); show(b.dataset.titleOpen); });
  }
  for (const b of root.querySelectorAll('[data-title-back]')) b.addEventListener('click', () => show('main'));
  contBtn?.addEventListener('click', () => { if (latest) onContinue?.(latest); });
  for (const b of root.querySelectorAll('[data-section]')) {
    const i = Number(b.dataset.section);
    if (Number.isInteger(i)) b.addEventListener('click', () => onSection?.(i));
  }
  calibBtn?.addEventListener('click', () => { if (headsetLive()) onCalibrate?.(); });

  doc.addEventListener('keydown', (e) => {
    if (root.hidden || isBusy() || e.ctrlKey || e.metaKey || e.altKey) return;
    const t = e.target;
    const inside = root.contains(t);
    if (!inside && t !== doc.body && t !== doc.documentElement && t != null) return;
    if (e.code === 'Escape') {
      if (view !== 'main') { e.preventDefault(); show('main'); }
      return;
    }
    const dir = ARROWS[e.code];
    if (!dir) return;
    if (t?.tagName === 'INPUT' && (e.code === 'ArrowLeft' || e.code === 'ArrowRight')) return;
    // In the planet-size radio group, Left/Right pick the next size.
    const radio = t?.closest?.('[role="radiogroup"]');
    if (radio && (e.code === 'ArrowLeft' || e.code === 'ArrowRight')) {
      const r = [...radio.querySelectorAll('[role="radio"]')];
      const next = r[stepIndex(r.indexOf(t), r.length, dir)];
      e.preventDefault(); next?.focus(); next?.click();
      return;
    }
    const list0 = items();
    e.preventDefault();
    list0[stepIndex(list0.indexOf(t), list0.length, dir)]?.focus();
  });

  // Every return to the title (Quit to Title, boot) starts on the main menu with fresh saves.
  const reset = () => { if (root.hidden) return; refresh(); show('main', false); opener = null; focusFirst(); };
  try { new MutationObserver(reset).observe(root, { attributes: true, attributeFilter: ['hidden'] }); } catch {}
  reset();

  return { refresh, show, get view() { return view; }, syncCalibrate };
}
