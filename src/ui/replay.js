// The thin DOM layer for replayability (#91). All rules live in src/game/replay.js; this file
// only paints them: the seed/share-code box and modifier choices in New Game, the compact run
// chip in the top bar, and the scores, chronicle and unlocks on the end screen.

import { readOptions, saveOptions } from '../game/options.js';
import {
  CHALLENGES, MODIFIERS, challengeDateLabel, challengeFor, encodeShareCode,
  mindPlusUnlocked, normalizeModifiers, parseSeedText, randomSeed, readReplaySettings,
  readUnlocks, runLabel, saveReplaySettings, unlockListText, validateShareCode,
} from '../game/replay.js';

const hexSeed = (seed) => (Number(seed) >>> 0).toString(16).toUpperCase().padStart(8, '0');

export function compactRunLabel(replay) {
  if (!replay) return '';
  const bits = [`#${hexSeed(replay.seed).slice(0, 6)}`];
  if (replay.challenge === 'daily') bits.push('Daily');
  else if (replay.challenge === 'weekly') bits.push('Weekly');
  if (replay.modifiers?.length) bits.push(`${replay.modifiers.length} mod${replay.modifiers.length === 1 ? '' : 's'}`);
  if (replay.mindPlus) bits.push('Mind+');
  return bits.join(' · ');
}

async function copyText(text, button) {
  let ok = false;
  try {
    if (globalThis.navigator?.clipboard?.writeText) {
      await globalThis.navigator.clipboard.writeText(text);
      ok = true;
    }
  } catch { ok = false; }
  if (!ok) {
    // Old browsers / insecure pages: a hidden textarea and execCommand still count as one click.
    try {
      const area = document.createElement('textarea');
      area.value = text;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      ok = document.execCommand?.('copy') === true;
      area.remove();
    } catch { ok = false; }
  }
  if (button) {
    const old = button.textContent;
    button.textContent = ok ? 'Copied!' : 'Copy failed';
    setTimeout(() => { button.textContent = old; }, 1400);
  }
  return ok;
}

export function initReplayUi(doc, {
  storage = globalThis.localStorage,
  now = () => Date.now(),
  getOptions = () => readOptions({ storage, search: '' }),
  setOptions = (opts) => saveOptions(opts, { storage }),
} = {}) {
  const $ = (id) => doc.getElementById(id);
  const q = (sel, root = doc) => root.querySelector(sel);
  const saved = readReplaySettings({ storage }) || {};
  let progress = readUnlocks({ storage });
  const baseOptions = getOptions();
  const initialSeed = saved.seed ?? randomSeed(now());
  const state = {
    seed: initialSeed,
    challenge: saved.challenge || 'none',
    challengeKey: saved.challengeKey || '',
    modifiers: normalizeModifiers(saved.modifiers || []),
    mindPlus: !!(saved.mindPlus && mindPlusUnlocked(progress)),
    error: '',
  };

  const seedInput = $('runSeedInput');
  const codeEl = $('runShareCode');
  const copyBtn = $('copyRunCode');
  const challengeLabel = $('challengeLabel');
  const mindToggle = $('mindPlusToggle');
  const mindHint = $('mindPlusHint');
  const hudChip = $('runSeedChip');
  const endBox = $('endReplay');
  const endRank = $('endRank');
  const endSeedLine = $('endSeedLine');
  const endScores = $('endScores');
  const endChronicle = $('endChronicle');
  const endModifiers = $('endModifiers');
  const endShareCode = $('endShareCode');
  const endCopy = $('endCopyCode');
  const endMind = $('endMindPlus');
  const endNewUnlocks = $('endNewUnlocks');

  const options = () => getOptions();
  const shareCode = () => {
    const opts = options();
    return encodeShareCode({
      seed: state.seed,
      worldSize: opts.worldSize,
      difficulty: opts.difficulty,
      challenge: state.challenge,
      modifiers: state.modifiers,
      mindPlus: !!(state.mindPlus && mindPlusUnlocked(progress)),
    });
  };
  const persist = () => {
    const opts = options();
    saveReplaySettings({ ...state, worldSize: opts.worldSize, difficulty: opts.difficulty }, { storage });
  };

  function setOptionsAndRadios(worldSize, difficulty) {
    const opts = options();
    setOptions({ ...opts, ...(worldSize ? { worldSize } : {}), ...(difficulty ? { difficulty } : {}) });
    // The options picker keeps its own aria state; move it to match a pasted share code.
    for (const b of doc.querySelectorAll('[role="radio"][data-option]')) {
      const key = b.dataset.option;
      const val = key === 'worldSize' ? worldSize : key === 'difficulty' ? difficulty : null;
      if (val == null) continue;
      b.setAttribute('aria-checked', String(b.dataset.value === val));
      b.tabIndex = b.dataset.value === val ? 0 : -1;
    }
  }

  function syncControls() {
    const opts = options();
    const code = shareCode();
    if (seedInput && doc.activeElement !== seedInput) seedInput.value = String(state.seed);
    if (codeEl) codeEl.textContent = code;
    if (copyBtn) copyBtn.title = `Copy share code ${code}`;
    if (hudChip && hudChip.dataset.live === '1') hudChip.title = `Copy this run's share code ${code}`;
    for (const b of doc.querySelectorAll('[data-challenge]')) {
      b.setAttribute('aria-pressed', String(b.dataset.challenge === state.challenge));
    }
    if (challengeLabel) {
      challengeLabel.textContent = state.error
        || (state.challenge === 'none' ? '' : challengeDateLabel(state.challenge, now()));
      challengeLabel.dataset.error = state.error ? '1' : '0';
    }
    for (const box of doc.querySelectorAll('[data-modifier]')) {
      box.checked = state.modifiers.includes(box.dataset.modifier);
    }
    const allowed = mindPlusUnlocked(progress);
    if (!allowed) state.mindPlus = false;
    if (mindToggle) {
      mindToggle.disabled = !allowed;
      mindToggle.checked = !!state.mindPlus;
      mindToggle.setAttribute('aria-checked', String(!!state.mindPlus));
    }
    if (mindHint) mindHint.textContent = allowed ? 'one difficulty harder' : 'finish a run to unlock';
    renderUnlocks();
    persist();
  }

  function renderUnlocks() {
    const text = unlockListText(progress);
    for (const el of doc.querySelectorAll('[data-unlocks]')) el.textContent = text;
  }

  function applySeedText(text) {
    const raw = String(text ?? '').trim();
    if (/^MOC[\s-]/i.test(raw)) {
      const valid = validateShareCode(raw);
      if (!valid.ok) { state.error = valid.error; syncControls(); return false; }
      const code = valid.run;
      state.error = '';
      state.seed = code.seed;
      state.challenge = code.challenge;
      state.challengeKey = code.challenge !== 'none' ? challengeFor(code.challenge, now()).key : '';
      state.modifiers = normalizeModifiers(code.modifiers);
      state.mindPlus = !!code.mindPlus && mindPlusUnlocked(progress);
      setOptionsAndRadios(code.worldSize, code.difficulty);
      syncControls();
      return true;
    }
    const parsed = parseSeedText(raw);
    if (parsed == null) { state.error = 'Enter a seed number or a share code.'; syncControls(); return false; }
    state.error = '';
    state.seed = parsed.seed;
    state.challenge = 'none';
    state.challengeKey = '';
    syncControls();
    return true;
  }

  function setChallenge(kind) {
    if (!CHALLENGES.some((c) => c.id === kind)) kind = 'none';
    state.error = '';
    state.challenge = kind;
    if (kind === 'daily' || kind === 'weekly') {
      const ch = challengeFor(kind, now());
      state.seed = ch.seed;
      state.challengeKey = ch.key;
    }
    syncControls();
  }

  // The New Game UI's choices, rolled to today's challenge if the date has moved on.
  function settings() {
    if (state.challenge === 'daily' || state.challenge === 'weekly') {
      const ch = challengeFor(state.challenge, now());
      if (ch.key !== state.challengeKey) {
        state.seed = ch.seed;
        state.challengeKey = ch.key;
      }
    }
    const opts = options();
    const allowed = mindPlusUnlocked(progress);
    persist();
    return {
      seed: state.seed,
      worldSize: opts.worldSize,
      difficulty: opts.difficulty,
      challenge: state.challenge,
      challengeKey: state.challengeKey,
      modifiers: [...state.modifiers],
      mindPlus: !!(state.mindPlus && allowed),
    };
  }

  // Called when a run starts: the seed chip and the controls now show what is actually running.
  function onRunStart(G) {
    const r = G?.replay;
    if (!r) return;
    state.seed = r.seed;
    state.challenge = r.challenge || 'none';
    state.challengeKey = r.challengeKey || '';
    state.modifiers = normalizeModifiers(r.modifiers);
    state.mindPlus = !!r.mindPlus;
    setOptionsAndRadios(r.worldSize, r.baseDifficulty || r.difficulty);
    if (hudChip) {
      hudChip.hidden = false;
      hudChip.dataset.live = '1';
      hudChip.textContent = compactRunLabel(r);
      hudChip.title = `Copy this run's share code ${r.shareCode}`;
      hudChip.setAttribute('aria-label', `Run seed ${hexSeed(r.seed)}. Copy share code`);
    }
    syncControls();
  }

  function hideHud() {
    if (!hudChip) return;
    delete hudChip.dataset.live;
    hudChip.hidden = true;
  }

  function showEnd({ G, result, chronicle = [], progress: nextProgress = null, newUnlocks = [] } = {}) {
    progress = nextProgress || progress;
    const replay = G?.replay;
    if (endBox) endBox.hidden = false;
    if (endRank) endRank.textContent = `${result?.rank || 'E'} · ${result?.rankLabel || 'A Pebble'} · ${result?.total ?? 0}/100`;
    if (endSeedLine) endSeedLine.textContent = replay ? `${runLabel(replay)} · ${replay.shareCode}` : '';
    if (endScores) {
      endScores.replaceChildren(...(result?.categories || []).map((c) => {
        const li = doc.createElement('li');
        const name = doc.createElement('span'); name.textContent = c.label;
        const value = doc.createElement('b'); value.textContent = String(c.score);
        const blurb = doc.createElement('small'); blurb.textContent = c.blurb || '';
        li.append(name, value, blurb);
        return li;
      }));
    }
    if (endChronicle) {
      endChronicle.replaceChildren(...chronicle.map((line) => {
        const li = doc.createElement('li'); li.textContent = line; return li;
      }));
    }
    if (endModifiers) {
      const mods = normalizeModifiers(replay?.modifiers);
      endModifiers.textContent = mods.length
        ? `Modifiers: ${mods.map((id) => MODIFIERS.find((m) => m.id === id)?.label || id).join(', ')}`
        : 'Modifiers: none';
    }
    if (endShareCode) endShareCode.textContent = replay?.shareCode || '';
    if (endCopy) endCopy.title = `Copy share code ${replay?.shareCode || ''}`;
    const allowed = mindPlusUnlocked(progress);
    if (endMind) { endMind.disabled = !allowed; endMind.checked = false; }
    if (endNewUnlocks) {
      endNewUnlocks.textContent = newUnlocks.length
        ? `New unlock${newUnlocks.length === 1 ? '' : 's'}: ${newUnlocks.map((u) => u.label).join(' · ')}`
        : '';
    }
    renderUnlocks();
  }

  function againSettings() {
    const base = settings();
    const allowed = mindPlusUnlocked(progress);
    if (endMind?.checked && allowed) base.mindPlus = true;
    return base;
  }

  // ---------- wiring ----------
  seedInput?.addEventListener('change', () => { applySeedText(seedInput.value); });
  seedInput?.addEventListener('keydown', (e) => {
    if (e.code === 'Enter') { e.preventDefault(); applySeedText(seedInput.value); }
  });
  $('runSeedRandom')?.addEventListener('click', () => {
    state.error = '';
    state.seed = randomSeed(now());
    state.challenge = 'none';
    state.challengeKey = '';
    syncControls();
  });
  for (const b of doc.querySelectorAll('[data-challenge]')) {
    b.addEventListener('click', () => setChallenge(b.dataset.challenge));
  }
  for (const box of doc.querySelectorAll('[data-modifier]')) {
    box.addEventListener('change', () => {
      state.error = '';
      const want = new Set(state.modifiers);
      if (box.checked) want.add(box.dataset.modifier); else want.delete(box.dataset.modifier);
      state.modifiers = normalizeModifiers([...want]);
      syncControls();
    });
  }
  mindToggle?.addEventListener('change', () => {
    state.mindPlus = !!mindToggle.checked && mindPlusUnlocked(progress);
    syncControls();
  });
  for (const b of doc.querySelectorAll('[role="radio"][data-option]')) {
    b.addEventListener('click', () => { syncControls(); });
  }
  copyBtn?.addEventListener('click', () => copyText(shareCode(), copyBtn));
  endCopy?.addEventListener('click', () => copyText(endShareCode?.textContent || shareCode(), endCopy));
  hudChip?.addEventListener('click', () => copyText(hudChip.dataset.code || shareCode(), hudChip));
  // Remember the share code shown in the chip, so a copy after the run uses exactly that run.
  const originalOnRunStart = onRunStart;
  function onRunStartWithCode(G) {
    originalOnRunStart(G);
    if (hudChip && G?.replay?.shareCode) hudChip.dataset.code = G.replay.shareCode;
  }

  syncControls();

  return {
    get settings() { return settings; },
    settings,
    againSettings,
    onRunStart: onRunStartWithCode,
    hideHud,
    showEnd,
    refresh: syncControls,
    refreshUnlocks: () => { progress = readUnlocks({ storage }); renderUnlocks(); },
    get progress() { return progress; },
    get shareCode() { return shareCode(); },
    // Test/playtest handle.
    state,
  };
}
