// Just enough of a browser for the game's simulation code to run in Node.
// World.paint() bakes terrain into a canvas; here every canvas is a no-op sink.
// Import this module before any game module.

const noop = () => {};

function fakeContext(canvas) {
  const target = {
    canvas,
    createImageData: (w, h) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
    getImageData: (x, y, w, h) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
    putImageData: noop,
    measureText: (t) => ({ width: String(t).length * 4 }),
    createRadialGradient: () => ({ addColorStop: noop }),
    createLinearGradient: () => ({ addColorStop: noop }),
    createPattern: () => null,
  };
  // Any other drawing method or property: a harmless no-op / stored value.
  return new Proxy(target, { get: (t, k) => (k in t ? t[k] : noop) });
}

function fakeCanvas() {
  const c = { width: 300, height: 150, style: {} };
  let ctx = null;
  c.getContext = () => (ctx ||= fakeContext(c));
  return c;
}

if (typeof globalThis.document === 'undefined') {
  globalThis.document = {
    createElement: (tag) => (tag === 'canvas' ? fakeCanvas() : { style: {}, appendChild: noop, setAttribute: noop }),
    getElementById: () => null,
    querySelector: () => null,
    querySelectorAll: () => [],
    body: { dataset: {} },
  };
}

// Seeded Math.random so an episode replays exactly from its seed (the sim uses Math.random).
export function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
