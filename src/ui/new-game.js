// The New Game step on the title screen (epic #220): world size and difficulty, then Start.
// Each picker is a radio group (title.js treats one as a single Up/Down stop, Left/Right picks);
// a click stores the choice (src/game/options.js), which newGame() reads.
import { readOptions, saveOptions } from '../game/options.js';

export function initNewGameOptions(root, { storage = globalThis.localStorage } = {}) {
  if (!root) return null;
  const buttons = [...root.querySelectorAll('[role="radio"][data-option]')];
  const show = () => {
    const opts = readOptions({ storage, search: '' });
    for (const b of buttons) {
      const on = opts[b.dataset.option] === b.dataset.value;
      b.setAttribute('aria-checked', String(on));
      b.tabIndex = on ? 0 : -1;
    }
  };
  for (const b of buttons) {
    b.addEventListener('click', () => {
      saveOptions({ ...readOptions({ storage, search: '' }), [b.dataset.option]: b.dataset.value }, { storage });
      show();
    });
  }
  show();
  return { show };
}
