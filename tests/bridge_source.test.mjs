// Head tracking (#53): BridgeSource turns bridge frames into aim tilt.
// Run: node --test tests/bridge_source.test.mjs   (also run by tests/test_bridge_source.py)

import test from 'node:test';
import assert from 'node:assert/strict';
import { BridgeSource } from '../src/brain/sources.js';

const bands = { delta: 0.2, theta: 0.2, alpha: 0.2, beta: 0.2, gamma: 0.2 };
const S = BridgeSource;

// Feed frames 0.1 s apart; returns the last normalised frame.
function feed(src, n, make, t0 = 0) {
  let out;
  for (let i = 0; i < n; i++) {
    const t = t0 + i * 0.1;
    out = src.normalise({ t, bands, blink: false, quality: 1, ...make(t, i) });
  }
  return out;
}

const orient = (yaw, pitch, roll = 0) => ({ accel: [0, 0, 1], gyro: [0, 0, 0], orientation: { pitch, roll, yaw } });
const near = (a, b, eps, msg) => assert.ok(Math.abs(a - b) <= eps, `${msg ?? ''} ${a} vs ${b}`);

test('the first second of orientation sets the neutral pose', () => {
  const src = new S('ws://test');
  const f = feed(src, S.NEUTRAL_FRAMES, () => orient(0, -8));
  assert.deepEqual(f.tilt, { x: 0, y: 0 });
  assert.deepEqual(src.aimZero, { yaw: 0, pitch: -8 });
});

test('yaw aims left/right and pitch up/down, ±25° spans the screen', () => {
  const src = new S('ws://test');
  feed(src, S.NEUTRAL_FRAMES, () => orient(0, -8));
  let f = feed(src, 20, () => orient(S.AIM_RANGE_DEG / 2, -8 + S.AIM_RANGE_DEG / 2), 1);
  near(f.tilt.x, 0.5, 0.01, 'half right');
  near(f.tilt.y, 0.5, 0.01, 'half down');
  f = feed(src, 20, () => orient(-60, -8 - 60), 3);
  near(f.tilt.x, -1, 1e-6, 'clamped left');
  near(f.tilt.y, -1, 1e-6, 'clamped up');
});

test('aim is lightly smoothed, not a jump', () => {
  const src = new S('ws://test');
  feed(src, S.NEUTRAL_FRAMES, () => orient(0, 0));
  const one = feed(src, 1, () => orient(S.AIM_RANGE_DEG, 0), 1);
  assert.ok(one.tilt.x > 0.5 && one.tilt.x < 1, `after one frame ${one.tilt.x}`);
  const three = feed(src, 3, () => orient(S.AIM_RANGE_DEG, 0), 1.1);
  assert.ok(three.tilt.x > 0.98, `after four frames ${three.tilt.x}`);
});

test('yaw neutral wraps across ±180°', () => {
  const src = new S('ws://test');
  feed(src, S.NEUTRAL_FRAMES, (t, i) => orient(i % 2 ? 179 : -179, 0));
  near(Math.abs(src.aimZero.yaw), 180, 0.01, 'circular mean');
  const before = src.aimTilt.x;
  const f = feed(src, 1, () => orient(-175, 0), 1);
  // The neutral has faded one 0.1 s step toward 0 (see the next test); -175° is still ~5° right of it.
  const target = (((-175 - src.aimZero.yaw + 540) % 360) - 180) / S.AIM_RANGE_DEG;
  assert.ok(target > 0.15 && target < 0.21, `target ${target}`);
  near(f.tilt.x, before + (target - before) * (1 - Math.exp(-S.AIM_SMOOTH * 0.1)), 1e-9, 'one smoothed frame');
});

test('the yaw neutral fades as the bridge re-centres yaw', () => {
  // Connected while turned 10° right. The bridge leaks that yaw back to 0, and so does the neutral:
  // a still head stays centred instead of drifting 10° left.
  const src = new S('ws://test');
  const tau = 20;  // bridge YAW_RECENTER_TAU
  const bridgeYaw = (t) => 10 * Math.exp(-t / tau);
  feed(src, S.NEUTRAL_FRAMES, (t) => orient(bridgeYaw(t), 0));
  const f = feed(src, 600, (t) => orient(bridgeYaw(t), 0), 1);
  near(f.tilt.x, 0, 0.02, 'still centred after a minute');
  near(src.aimZero.yaw, 0, 0.6);
});

test('without orientation it falls back to accelerometer tilt', () => {
  const src = new S('ws://test');
  const accelOnly = (ax, ay) => ({ accel: [ax, ay, 0.98], gyro: null, orientation: null });
  feed(src, 10, () => accelOnly(0.05, -0.02));
  const f = feed(src, 1, () => accelOnly(0.15, 0.08), 1);
  near(f.tilt.x, 0.26, 1e-9, 'ay → x');
  near(f.tilt.y, 0.26, 1e-9, 'ax → y');
  assert.equal(src.aimZero, null);
});

test('older bridges without the new keys still work', () => {
  const src = new S('ws://test');
  // An older bridge also sent a heart-rate field ("hr"); the game no longer reads it.
  const f = src.normalise({ t: 0, bands, blink: false, hr: 70, accel: [0, 0, 1], motion: 0.3, quality: 0.9 });
  assert.equal('hr' in f, false);
  assert.deepEqual(f.tilt, { x: 0, y: 0 });
  assert.equal(src.normalise({ t: 0.1, bands, accel: null, motion: 0 }).tilt, null);
});

test('motion passes through, clamped', () => {
  const src = new S('ws://test');
  assert.equal(feed(src, 1, () => ({ ...orient(0, 0), motion: 0.42 })).motion, 0.42);
  assert.equal(feed(src, 1, () => ({ ...orient(0, 0), motion: 1.7 })).motion, 1);
  assert.equal(feed(src, 1, () => orient(0, 0)).motion, 0);
});

test('recenter() takes a new neutral pose', () => {
  const src = new S('ws://test');
  feed(src, S.NEUTRAL_FRAMES, () => orient(0, 0));
  feed(src, 5, () => orient(15, 5), 1);
  src.recenter();
  const f = feed(src, S.NEUTRAL_FRAMES + 5, () => orient(15, 5), 2);
  near(f.tilt.x, 0, 0.02);
  near(f.tilt.y, 0, 1e-9);
});

// #185: Play again and the sandboxes keep a connected headset instead of swapping in the emulator.
test('liveBridge keeps a connected bridge and nothing else', async () => {
  const { liveBridge, SimSource } = await import('../src/brain/sources.js');
  const sim = { kind: 'sim', connected: true };
  const run = new S('ws://run');
  const preview = new S('ws://preview');
  assert.equal(liveBridge(sim, null, undefined), null, 'no bridge: fall back to the emulator');
  assert.equal(liveBridge(run, preview), null, 'a bridge that never connected is not kept');
  preview.connected = true;
  assert.equal(liveBridge(run, preview), null, 'an open socket with no EEG yet is not a headset (#174)');
  preview.lastFrameAt = 1;
  assert.equal(liveBridge(run, preview), preview, 'the title screen\'s streaming bridge');
  run.connected = true; run.lastFrameAt = 1;
  assert.equal(liveBridge(run, preview), run, 'the run\'s own bridge first');
  run.connected = false;   // the socket closed mid-run
  assert.equal(liveBridge(run), null);
  assert.equal(typeof SimSource, 'function');
});

// #174: connect() resolves when the socket opens; the run waits for the first EEG frame.
class FakeSocket {
  static last = null;
  constructor(url) { this.url = url; FakeSocket.last = this; setTimeout(() => this.onopen?.(), 0); }
  frame(t = 0.1) { this.onmessage?.({ data: JSON.stringify({ t, bands, blink: false, quality: 1 }) }); }
  close() { setTimeout(() => this.onclose?.(), 0); }
}

test('a bridge that connects but never streams does not count as a headset', async () => {
  const prev = globalThis.WebSocket;
  globalThis.WebSocket = FakeSocket;
  try {
    const src = new S('ws://silent');
    await src.connect();
    assert.equal(src.connected, true);
    await assert.rejects(src.waitForFrame(60), /No EEG from the headset/);
    const { liveBridge } = await import('../src/brain/sources.js');
    assert.equal(liveBridge(src), null);
    src.disconnect();

    const live = new S('ws://live');
    await live.connect();
    const waiting = live.waitForFrame(1000);
    FakeSocket.last.frame();
    await waiting;
    assert.ok(live.lastFrameAt > 0);
    assert.equal(liveBridge(live), live);
    // A socket that closes while waiting rejects at once; cancelling (disconnect) does too.
    const closing = new S('ws://closing');
    await closing.connect();
    const w2 = closing.waitForFrame(5000);
    FakeSocket.last.close();
    await assert.rejects(w2, /closed/);
    closing.disconnect();
    const cancel = new S('ws://cancel');
    await cancel.connect();
    const w3 = cancel.waitForFrame(5000);
    cancel.disconnect();
    await assert.rejects(w3, /Cancelled/);
    live.disconnect();
  } finally {
    globalThis.WebSocket = prev;
  }
});
