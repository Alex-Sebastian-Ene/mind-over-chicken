// Galaxy panel details for one world (#68): who runs it, its settlement tier, stocks, stability,
// strife, milestones and a short history. Plain DOM, rebuilt only when something changes.

import { TIERS, CONFLICT, MILESTONES, tierName } from './planet-sim.js';
import { ownerLabel, ERA_LABEL, ERA_NAMES, fmtPop } from './galaxy.js';
import { fmtNum } from './surface.js';

const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };

// One line for the planet list: tier for simulated worlds.
export function worldMeta(p) {
  if (p.home && !p.autonomous) return null;
  if (p.pop <= 0) return null;
  const strife = p.conflict ? ` · ${CONFLICT[p.conflict].name.toLowerCase()}` : p.factions ? ' · split' : '';
  return `${tierName(p)}${strife}`;
}

let lastKey = '';
export function renderWorldDetail(root, p) {
  if (!root) return;
  if (!p || p.forming) { if (lastKey !== 'none') { lastKey = 'none'; root.replaceChildren(); root.hidden = true; } return; }
  const campaign = p.home && !p.autonomous;
  const hist = (p.history || []).filter((h) => h.kind !== 'milestone').slice(0, 4);   // milestones show as chips
  const key = [p.name, ownerLabel(p), p.tier, p.eraIndex, Math.round(p.pop), Math.round(p.food), Math.round(p.wood), Math.round(p.stone), Math.round(p.goats),
    Math.round((p.stability ?? 0) * 20), p.conflict, !!p.factions, (p.milestones || []).length, hist.map((h) => h.text).join('|')].join(';');
  if (key === lastKey) return;
  lastKey = key;
  root.hidden = false;
  const head = el('p', 'wd-head');
  head.append(el('b', null, p.name), document.createTextNode(` · ${ownerLabel(p)}`));
  const parts = [head];
  if (campaign) {
    parts.push(el('p', 'wd-line', `${ERA_LABEL[ERA_NAMES[p.eraIndex]] || ''} · ${p.pop} people · hand-played`));
  } else if (p.pop > 0) {
    const next = TIERS[(p.tier ?? 0) + 1];
    parts.push(el('p', 'wd-line', `${tierName(p)} · ${ERA_LABEL[ERA_NAMES[p.eraIndex]]} · ${fmtPop(p.pop)} people${next ? ` · next: ${next.name.toLowerCase()} at ${fmtNum(next.enter.all.population)}` : ''}`));
    parts.push(el('p', 'wd-line wd-stock', `food ${fmtNum(p.food)} · wood ${fmtNum(p.wood)} · stone ${fmtNum(p.stone)} · goats ${fmtNum(p.goats)}`));
    const stab = el('p', 'wd-line wd-stab');
    const bar = el('span', 'wd-bar'); bar.style.setProperty('--p', `${Math.round((p.stability ?? 0) * 100)}%`);
    if ((p.stability ?? 0) < 0.3) bar.classList.add('is-low');
    stab.append(el('span', null, `stability ${Math.round((p.stability ?? 0) * 100)}%`), bar);
    parts.push(stab);
    if (p.conflict || p.factions) {
      const war = CONFLICT[p.conflict || 0];
      const f = p.factions ? ` · rebels hold ${Math.round(p.factions[1].share * 100)}%` : '';
      parts.push(el('p', 'wd-line wd-war', `${p.conflict ? war.name : 'Uneasy peace'}${f}`));
    }
  } else {
    parts.push(el('p', 'wd-line', 'Barren · waiting for settlers'));
  }
  const ms = p.milestones || [];
  if (ms.length) {
    const ul = el('ul', 'wd-miles');
    ul.setAttribute('aria-label', 'Milestones');
    for (const id of ms.slice(-5)) ul.append(el('li', null, MILESTONES[id] || id));
    parts.push(ul);
  }
  if (hist.length) {
    const ol = el('ol', 'wd-hist');
    ol.setAttribute('aria-label', `${p.name} history`);
    for (const h of hist) { const li = el('li', `is-${h.kind}`, h.text); ol.append(li); }
    parts.push(ol);
  }
  root.replaceChildren(...parts);
}
