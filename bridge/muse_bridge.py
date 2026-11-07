#!/usr/bin/env python3
"""Mind over Chicken headset bridge.

Reads an EEG headset through BrainFlow and broadcasts one JSON feature frame
every ~100 ms to every browser connected on ws://HOST:PORT (default
ws://127.0.0.1:8765). The browser side is BridgeSource in src/brain/sources.js.

Frame:
    {"t": 12.3,                       # seconds since the bridge started
     "bands": {"delta": .., "theta": .., "alpha": .., "beta": .., "gamma": ..},
     "blink": false,                  # deliberate blink since the previous frame
     "hr": 71.2 | null,               # heart rate from PPG, bpm
     "accel": [x, y, z] | null,       # accelerometer, g
     "gyro": [x, y, z] | null,        # gyroscope, deg/s (null if the board has none)
     "orientation": {"pitch": .., "roll": .., "yaw": ..} | null,
                                      # fused head orientation, degrees (see HeadTracker)
     "motion": 0.0,                   # head movement, 0..1
     "quality": 1.0}                  # crude EEG signal quality, 0..1
Status (on connect and whenever it changes):
    {"type": "status", "text": "Streaming from Muse 2"}

Run `python muse_bridge.py --help` for flags; see README.md next to this file.
"""

import argparse
import asyncio
import json
import logging
import math
import random
import signal
import sys
import time
from collections import deque

import numpy as np
from brainflow.board_shim import (BoardIds, BoardShim, BrainFlowError, BrainFlowInputParams,
                                  BrainFlowPresets)
from brainflow.data_filter import DataFilter, DetrendOperations
from websockets.asyncio.server import broadcast, serve
from websockets.exceptions import ConnectionClosed

log = logging.getLogger("muse_bridge")

# --board choice -> (BrainFlow board id, display name, config_board commands that turn PPG/IMU on).
# On Muse 2, "p50" sets the 5-EEG+PPG preset and "p61" activates PPG + IMU streaming (same as muse-2/core/device.py).
BOARDS = {
    "synthetic": (BoardIds.SYNTHETIC_BOARD, "Synthetic board", None),
    "muse2": (BoardIds.MUSE_2_BOARD, "Muse 2", ("p50", "p61")),
    "muse2-bled": (BoardIds.MUSE_2_BLED_BOARD, "Muse 2 (BLED112)", ("p50", "p61")),
    "muses": (BoardIds.MUSE_S_BOARD, "Muse S", ("p61",)),
    "muses-bled": (BoardIds.MUSE_S_BLED_BOARD, "Muse S (BLED112)", ("p61",)),
}

DEFAULT = BrainFlowPresets.DEFAULT_PRESET.value
AUXILIARY = BrainFlowPresets.AUXILIARY_PRESET.value
ANCILLARY = BrainFlowPresets.ANCILLARY_PRESET.value

FRAME_DT = 0.1            # seconds between frames
BAND_WINDOW = 2.0         # seconds of EEG for band powers and quality
MIN_BAND_WINDOW = 1.0     # start sending frames once this much EEG is buffered
BLINK_WINDOW = 0.4        # seconds of frontal EEG checked for a blink
BLINK_REFRACTORY = 0.5    # seconds before another blink can fire
BLINK_REARM = 0.6         # signal must fall below this fraction of the threshold to re-arm
BLINK_MIN_CORR = 0.3      # AF7/AF8 must move together (vertical EOG); saccades are anti-correlated
ARTIFACT_PTP = 900.0      # µV; above this it is a contact pop / railed channel, not a blink
BLINK_MAX_MOTION = 0.8    # ignore "blinks" during big head jerks
HR_EVERY = 2.0            # seconds between heart-rate updates
HR_SECONDS = 15.0         # PPG history used per estimate (BrainFlow needs fft_size >= 1024)
ACCEL_FRAME = 0.1         # seconds of accelerometer averaged into "accel"
ACCEL_SHAKE = 0.3         # seconds of accelerometer used for the shake term
IMU_WINDOW = 0.5          # seconds of accel/gyro read per frame; new samples are picked by timestamp
ACCEL_MIN_G = 0.3         # |accel| below this is a dropped sample or free fall: no gravity to fuse
TILT_TAU = 1.0            # s, complementary filter: alpha = TILT_TAU / (TILT_TAU + dt) = 0.98 at 52 Hz
YAW_RECENTER_TAU = 20.0   # s, yaw leaks back to 0 with this time constant, so gyro drift can't pile up
GYRO_BIAS_INIT = 1.0      # s of still gyro averaged into the first bias estimate (yaw holds 0 until then)
GYRO_BIAS_TAU = 10.0      # s, afterwards the bias follows the gyro slowly while the head is still
GYRO_BIAS_REACQUIRE = 5.0 # s of steady gyro far from the bias estimate that replace the estimate
GYRO_STILL = 4.0          # deg/s, |gyro - bias| under this counts as "head still"
GYRO_MOTION_FLOOR = 5.0   # deg/s of head rotation that still reads as motion 0
GYRO_MOTION_FULL = 125.0  # deg/s of head rotation that reads as motion 1 (a quick nod or turn)
STALL_STATUS = 3.0        # seconds without new EEG before telling the browser
STALL_RECONNECT = 12.0    # seconds without new EEG before reconnecting
RETRY_DELAY = 3.0         # seconds between connection attempts
BUFFER_SAMPLES = 45000    # BrainFlow ring buffer per preset (~3 min at 250 Hz)
FRONTAL_NAMES = (("AF7", "AF8"), ("Fp1", "Fp2"))


def clamp(x, lo=0.0, hi=1.0):
    return lo if x < lo else hi if x > hi else x


def finite(x, ndigits=4, default=0.0):
    """JSON-safe float: the browser's JSON.parse rejects NaN/Infinity."""
    x = float(x)
    return round(x, ndigits) + 0.0 if math.isfinite(x) else default   # + 0.0: no "-0.0" in JSON


def next_pow2(n):
    p = 1
    while p < n:
        p *= 2
    return p


class Sensor:
    """Where a sensor lives in BrainFlow: which preset, which rows, which sampling rate."""

    def __init__(self, preset, channels, fs, ts_channel):
        self.preset, self.channels, self.fs, self.ts_channel = preset, channels, fs, ts_channel

    def __repr__(self):
        name = {DEFAULT: "DEFAULT", AUXILIARY: "AUXILIARY", ANCILLARY: "ANCILLARY"}.get(self.preset)
        return f"{name} preset rows {self.channels} @ {self.fs} Hz"


def board_presets(board_id):
    """Presets this board actually has. Only query these: in BrainFlow 5.23 asking for a preset
    the board lacks (e.g. ANCILLARY on the synthetic board) makes a later start_stream fail with
    GENERAL_ERROR:17 in the same process."""
    try:
        return set(BoardShim.get_board_presets(board_id))
    except (AttributeError, BrainFlowError):
        return {DEFAULT, AUXILIARY, ANCILLARY}   # older BrainFlow without get_board_presets


def find_sensor(board_id, getter, presets):
    """First preset in `presets` whose descriptor has channels for `getter`. A board without the
    sensor raises BrainFlowError for that field; skip it silently."""
    available = board_presets(board_id)
    for preset in presets:
        if preset not in available:
            continue
        try:
            channels = list(getter(board_id, preset))
            if not channels:
                continue
            fs = BoardShim.get_sampling_rate(board_id, preset)
            try:
                ts = BoardShim.get_timestamp_channel(board_id, preset)
            except BrainFlowError:
                ts = None
            return Sensor(preset, channels, fs, ts)
        except BrainFlowError:
            continue
    return None


def frontal_rows(board_id, eeg_channels):
    """AF7/AF8 (Muse) or Fp1/Fp2 if the board names them; otherwise the first two EEG rows."""
    try:
        names = [n.strip() for n in BoardShim.get_eeg_names(board_id)]
    except BrainFlowError:
        names = []
    for pair in FRONTAL_NAMES:
        if all(p in names for p in pair) and len(names) == len(eeg_channels):
            return [eeg_channels[names.index(p)] for p in pair], list(pair)
    rows = eeg_channels[:2]
    labels = [names[eeg_channels.index(r)] if len(names) == len(eeg_channels) else f"row {r}" for r in rows]
    return rows, labels


class HeadTracker:
    """Head orientation from accelerometer + gyroscope: a complementary filter in plain Python
    (no numpy or BrainFlow), so tests can drive it with synthetic samples.

    Axes are the IMU's own. BrainFlow passes the Muse's through unchanged, accel in g and gyro in
    deg/s. The filter assumes accel and gyro share one right-handed frame, as on any single-chip IMU.
    It does not assume which axis points up.

    Tilt: `up` is the unit vector a still accelerometer reads (the reaction to gravity, pointing up),
    in the head frame. Each sample rotates it by the bias-corrected gyro (du/dt = u x w, w in rad/s)
    and then blends it toward the measured accel direction:
        u = normalise(alpha * u_gyro + (1 - alpha) * a / |a|),   alpha = tilt_tau / (tilt_tau + dt)
    The gyro gives the fast response and gravity removes its drift. alpha is 0.98 at the Muse's 52 Hz.
        pitch = atan2(u_x, hypot(u_y, u_z))   elevation of the IMU x axis above the horizontal
        roll  = atan2(u_y, hypot(u_x, u_z))   elevation of the IMU y axis
    These are the accel-only tilt formulas applied to the fused vector, so the zero point and signs
    match the raw "accel" values: pitch rises with accel x, roll with accel y.

    Yaw: rotation about the vertical. It integrates the gyro component along `up` (gyro z when the
    head is level) and is positive for a turn to the right, clockwise seen from above. Gravity can't
    see yaw, so yaw leaks back toward 0 with time constant `yaw_tau`. Bias left in the gyro then
    gives a bounded offset (bias x yaw_tau) instead of an endless spin.

    Gyro bias: a "steady window" averages the gyro and restarts whenever a sample is more than
    `still` deg/s from its mean. The first `bias_init` seconds of steady gyro give the first
    estimate. After that the bias follows a slow average (`bias_tau`) of samples within `still`
    deg/s of it. If the window stays steady for `bias_reacquire` seconds at a value more than `still`
    from the estimate, the estimate was wrong (the head was turning steadily while it was taken).
    The window mean replaces it and yaw restarts at 0. Until the first estimate exists the gyro is
    ignored: tilt comes from the accelerometer alone and yaw holds 0.
    """

    def __init__(self, tilt_tau=TILT_TAU, yaw_tau=YAW_RECENTER_TAU, bias_init=GYRO_BIAS_INIT,
                 bias_tau=GYRO_BIAS_TAU, bias_reacquire=GYRO_BIAS_REACQUIRE, still=GYRO_STILL):
        self.tilt_tau, self.yaw_tau = tilt_tau, yaw_tau
        self.bias_init, self.bias_tau, self.bias_reacquire = bias_init, bias_tau, bias_reacquire
        self.still = still
        self.reset()

    def reset(self):
        self.up = None              # unit vector in the head frame; None until the first sample
        self.yaw = 0.0              # degrees, positive = turned right
        self.bias = None            # deg/s per axis, once estimated
        self.rate = 0.0             # deg/s, bias-corrected rotation speed of the latest sample
        self._reset_window()

    def calib_progress(self):
        """0.0..1.0 gyro bias calibration progress."""
        if self.bias is not None:
            return 1.0
        return clamp(self._win_time / max(1e-6, self.bias_init))

    def tare(self):
        """Zero yaw heading and force a fresh gyro bias calibration on the next still hold."""
        self.yaw = 0.0
        self.bias = None
        self.rate = 0.0
        self._reset_window()

    def _reset_window(self):
        self._win_sum, self._win_n, self._win_time = [0.0, 0.0, 0.0], 0, 0.0

    def angles(self):
        """(pitch, roll, yaw) in degrees, or None before the first usable sample."""
        if self.up is None:
            return None
        ux, uy, uz = self.up
        return (math.degrees(math.atan2(ux, math.hypot(uy, uz))),
                math.degrees(math.atan2(uy, math.hypot(ux, uz))),
                self.yaw)

    def update(self, accel, gyro, dt):
        """Feed one sample: accel [x, y, z] in g, gyro [x, y, z] in deg/s, dt in seconds.
        Returns the bias-corrected rotation speed in deg/s (0 until the bias is known)."""
        ax, ay, az = (float(v) for v in accel)
        gx, gy, gz = (float(v) for v in gyro)
        if dt <= 0 or not all(math.isfinite(v) for v in (ax, ay, az, gx, gy, gz)):
            return self.rate
        norm = math.sqrt(ax * ax + ay * ay + az * az)
        meas = (ax / norm, ay / norm, az / norm) if norm > ACCEL_MIN_G else None
        if self.up is None:
            if meas is None:
                return self.rate
            self.up = meas

        w = self._remove_bias(gx, gy, gz, dt)          # deg/s, or None while the bias is unknown
        ux, uy, uz = self.up
        if w is not None:
            wx, wy, wz = (math.radians(v) for v in w)
            # A world-fixed vector seen from a frame rotating at w changes as du/dt = u x w.
            ux, uy, uz = (ux + (uy * wz - uz * wy) * dt,
                          uy + (uz * wx - ux * wz) * dt,
                          uz + (ux * wy - uy * wx) * dt)
        if meas is not None:
            alpha = self.tilt_tau / (self.tilt_tau + dt)
            ux, uy, uz = (alpha * ux + (1 - alpha) * meas[0],
                          alpha * uy + (1 - alpha) * meas[1],
                          alpha * uz + (1 - alpha) * meas[2])
        n = math.sqrt(ux * ux + uy * uy + uz * uz)
        if n > 1e-9:
            self.up = (ux / n, uy / n, uz / n)

        if w is not None:
            ux, uy, uz = self.up
            # w . up is the turn rate about the vertical, counter-clockwise from above by the
            # right-hand rule (a turn to the left), so negate it for "positive = right".
            self.rate = math.sqrt(w[0] * w[0] + w[1] * w[1] + w[2] * w[2])
            yaw_rate = -(w[0] * ux + w[1] * uy + w[2] * uz)
            # Zero Angular Velocity Update (ZUPT): when the head is stationary (< 1.5 deg/s),
            # do not integrate micro-tremor or residual bias into yaw.
            if self.rate >= 1.5 or not math.isfinite(self.bias_tau):
                self.yaw += yaw_rate * dt
            if math.isfinite(self.yaw_tau):
                self.yaw *= math.exp(-dt / self.yaw_tau)
            self.yaw = (self.yaw + 180.0) % 360.0 - 180.0
        return self.rate

    def _remove_bias(self, gx, gy, gz, dt):
        g = (gx, gy, gz)
        if self._win_n and math.dist(g, [s / self._win_n for s in self._win_sum]) > self.still:
            self._reset_window()                                   # the gyro moved: not steady
        self._win_sum = [s + v for s, v in zip(self._win_sum, g)]
        self._win_n += 1
        self._win_time += dt
        if self.bias is None:
            if self._win_time >= self.bias_init:
                self.bias = [s / self._win_n for s in self._win_sum]
                self._reset_window()
            return None
        if self._win_time >= self.bias_reacquire:
            steady = [s / self._win_n for s in self._win_sum]
            if math.dist(steady, self.bias) > self.still:
                self.bias, self.yaw = steady, 0.0                  # the estimate was wrong
            self._reset_window()
        d = [gx - self.bias[0], gy - self.bias[1], gz - self.bias[2]]
        if math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) < self.still and math.isfinite(self.bias_tau):
            tau = 2.0 if math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) < 2.0 else self.bias_tau
            k = 1.0 - math.exp(-dt / tau)
            self.bias = [b + k * v for b, v in zip(self.bias, d)]
            d = [gx - self.bias[0], gy - self.bias[1], gz - self.bias[2]]
        return d


class Bridge:
    def __init__(self, args):
        self.args = args
        self.board_id, self.label, self.ppg_command = BOARDS[args.board]
        self.board_id = self.board_id.value
        self.clients = set()
        self.status = "Starting bridge"
        self.streaming_status = "Streaming"
        self.t0 = time.monotonic()
        self.board = None
        self.ppg_enabled = False
        self.frames_sent = 0
        self.blinks_sent = 0
        self._reset_signal_state()

        # Channel layout, read once from BrainFlow's board descriptors.
        bid = self.board_id
        self.eeg = find_sensor(bid, BoardShim.get_eeg_channels, [DEFAULT])
        if self.eeg is None:
            raise SystemExit(f"{self.label}: BrainFlow reports no EEG channels")
        try:
            raw_names = [n.strip() for n in BoardShim.get_eeg_names(bid)]
        except BrainFlowError:
            raw_names = []
        self.eeg_names = (raw_names[:len(self.eeg.channels)] if len(raw_names) >= len(self.eeg.channels)
                          else [f"CH{i + 1}" for i in range(len(self.eeg.channels))])
        self.frontal, frontal_labels = frontal_rows(bid, self.eeg.channels)
        self.accel = find_sensor(bid, BoardShim.get_accel_channels, [AUXILIARY, DEFAULT])
        # The gyro is fused with the accelerometer sample by sample, so it must come from the same
        # preset (same rate and timestamps). Muse 2/S and the synthetic board keep both in AUXILIARY.
        # find_sensor only queries presets get_board_presets lists (see board_presets).
        self.gyro = None
        if self.accel is not None and len(self.accel.channels) == 3:
            gyro = find_sensor(bid, BoardShim.get_gyro_channels, [self.accel.preset])
            self.gyro = gyro if gyro is not None and len(gyro.channels) == 3 else None
        self.ppg = None if args.no_ppg else find_sensor(bid, BoardShim.get_ppg_channels,
                                                       [ANCILLARY, AUXILIARY, DEFAULT])
        log.info("EEG: %s, blink channels %s", self.eeg, "/".join(frontal_labels))
        log.info("Accelerometer: %s", self.accel or "not available")
        log.info("Gyroscope: %s", self.gyro or "not available (orientation will be null)")
        log.info("PPG: %s", self.ppg or ("disabled (--no-ppg)" if args.no_ppg else "not available"))

        self.fake_blinks = bool(args.fake_blinks)
        if self.fake_blinks and args.board != "synthetic":
            log.warning("--fake-blinks only applies to --board synthetic; ignoring it")
            self.fake_blinks = False

    def _reset_signal_state(self):
        self.last_blink_at = -1e9
        self.blink_armed = True
        self.prev_accel_mean = None
        self.motion = 0.0
        self.head = HeadTracker()
        self.imu_last_ts = None
        self.imu_last_at = None
        self.hr_estimates = deque(maxlen=5)
        self.hr = None
        self.ppg_bpm = 72.0
        self.ppg_wave = []
        self.last_hr_at = -1e9
        self.last_ppg_ts = None
        self.last_ppg_new_at = time.monotonic()
        self.last_eeg_ts = None
        self.last_eeg_new_at = time.monotonic()
        self.fake_next = None
        self.fake_times = deque()

    # ------------------------------------------------------------------ websocket side

    def set_status(self, text):
        if text == self.status:
            return
        self.status = text
        log.info("status: %s", text)
        broadcast(self.clients, json.dumps({"type": "status", "text": text}))

    async def handler(self, ws):
        self.clients.add(ws)
        peer = getattr(ws, "remote_address", None)
        log.info("client connected %s (%d total)", peer, len(self.clients))
        try:
            await ws.send(json.dumps({"type": "status", "text": self.status}))
            async for raw in ws:
                try:
                    msg = json.loads(raw)
                    if isinstance(msg, dict) and msg.get("type") in ("recenter", "calibrate"):
                        self.head.tare()
                        log.info("head orientation tared / recalibrating gyro bias")
                except (ValueError, TypeError):
                    pass
        except ConnectionClosed:
            pass
        finally:
            self.clients.discard(ws)
            log.info("client disconnected %s (%d left)", peer, len(self.clients))

    # ------------------------------------------------------------------ board lifecycle

    def make_params(self):
        a = self.args
        p = BrainFlowInputParams()
        if a.serial_port:
            p.serial_port = a.serial_port
        if a.mac_address:
            p.mac_address = a.mac_address
        if a.serial_number:
            p.serial_number = a.serial_number
        p.timeout = int(a.timeout) if a.timeout else 15
        return p

    async def connect_board(self):
        attempt = 0
        while True:
            attempt += 1
            self.set_status(f"Connecting to {self.label}" if attempt == 1
                            else f"Waiting for headset ({self.label}), attempt {attempt}")
            board = BoardShim(self.board_id, self.make_params())
            self.board = board  # so shutdown can release it even mid-connect
            try:
                # prepare_session scans BLE / opens the dongle and can block; guard against BlueZ GATT deadlock.
                await asyncio.wait_for(asyncio.to_thread(board.prepare_session), timeout=22.0)
                self.ppg_enabled = False
                if self.ppg is not None and self.ppg_command:
                    cmds = (self.ppg_command,) if isinstance(self.ppg_command, str) else self.ppg_command
                    try:
                        for cmd in cmds:
                            await asyncio.to_thread(board.config_board, cmd)
                            await asyncio.sleep(0.25)
                        self.ppg_enabled = True
                        log.info("PPG enabled with config_board(%r)", cmds)
                    except BrainFlowError as e:
                        log.warning("could not enable PPG with %r: %s (continuing without heart rate)",
                                    cmds, e)
                elif self.ppg is not None:
                    self.ppg_enabled = True   # synthetic: PPG is always streamed
                board.start_stream(BUFFER_SAMPLES)
                self._reset_signal_state()
                extras = []
                if self.ppg_enabled:
                    extras.append("PPG")
                if self.accel:
                    extras.append("accel")
                if self.gyro:
                    extras.append("gyro")
                if self.fake_blinks:
                    extras.append(f"fake blinks every ~{self.args.fake_blink_every:g}s")
                self.streaming_status = f"Streaming from {self.label}" + (f" ({', '.join(extras)})" if extras else "")
                self.set_status(self.streaming_status)
                return
            except (BrainFlowError, asyncio.TimeoutError) as e:
                log.warning("connect failed: %s", e)
                self.release_board()
                self.set_status(f"Waiting for headset: {self.label} not found, retrying")
                await asyncio.sleep(RETRY_DELAY)

    def release_board(self):
        board, self.board = self.board, None
        if board is None:
            return
        try:
            if board.is_prepared():
                try:
                    board.stop_stream()
                except BrainFlowError:
                    pass  # not streaming yet
                board.release_session()
                log.info("board session released")
        except BrainFlowError as e:
            log.warning("error while releasing board: %s", e)

    async def run(self):
        while True:
            await self.connect_board()
            await self.stream_loop()       # returns when data stalls
            self.release_board()
            await asyncio.sleep(2.5)       # let BlueZ finish GATT teardown before rescanning

    async def stream_loop(self):
        next_tick = time.monotonic()
        last_report = time.monotonic()
        while True:
            now = time.monotonic()
            try:
                frame = self.compute_frame(now)
            except BrainFlowError as e:
                log.warning("read failed: %s", e)
                frame = None
            if frame is not None:
                broadcast(self.clients, json.dumps(frame, allow_nan=False))
                self.frames_sent += 1
                self.blinks_sent += frame["blink"]
                if frame["blink"]:
                    log.info("blink (t=%.1f)", frame["t"])

            stalled_for = now - self.last_eeg_new_at
            if stalled_for > STALL_RECONNECT and self.board_id != BoardIds.SYNTHETIC_BOARD.value:
                self.set_status("Headset data stopped, reconnecting")
                return
            if stalled_for > STALL_STATUS:
                self.set_status("Waiting for headset data")
            elif self.status.startswith("Waiting for headset data"):
                self.set_status(self.streaming_status)

            if now - last_report > 10:
                last_report = now
                log.info("clients=%d frames=%d blinks=%d hr=%s quality=%s",
                         len(self.clients), self.frames_sent, self.blinks_sent,
                         self.hr, frame["quality"] if frame else "-")

            next_tick += FRAME_DT
            delay = next_tick - time.monotonic()
            if delay < -1.0:          # fell far behind (laptop slept?): resync instead of bursting
                next_tick = time.monotonic()
                delay = 0
            await asyncio.sleep(max(0.0, delay))

    # ------------------------------------------------------------------ features

    def compute_frame(self, now):
        board = self.board
        eeg = self.eeg
        fs = eeg.fs
        data = board.get_current_board_data(int(BAND_WINDOW * fs), DEFAULT)
        if data.shape[1] < int(MIN_BAND_WINDOW * fs):
            return None   # still warming up

        ts = data[eeg.ts_channel] if eeg.ts_channel is not None else None
        newest = float(ts[-1]) if ts is not None else float(data.shape[1])
        if self.last_eeg_ts is None or newest != self.last_eeg_ts:
            self.last_eeg_ts = newest
            self.last_eeg_new_at = now

        rows = {ch: np.array(data[ch], dtype=np.float64) for ch in eeg.channels}
        if self.fake_blinks and ts is not None:
            self.inject_fake_blinks(rows, ts)

        eeg_block = np.ascontiguousarray(np.vstack([rows[ch] for ch in eeg.channels]))
        bands = self.band_powers(eeg_block, fs)
        quality = self.signal_quality(eeg_block, now)
        eeg_wave, contact = self.eeg_telemetry(eeg_block, fs, now)
        accel, gyro, orientation, motion = self.read_imu(board, now)
        blink = self.detect_blink([rows[ch] for ch in self.frontal], fs, now, motion)
        hr = self.read_hr(board, now)

        return {
            "t": round(now - self.t0, 3),
            "bands": bands,
            "blink": bool(blink),
            "hr": hr,
            "ppg": self.ppg_wave,
            "accel": accel,
            "gyro": gyro,
            "orientation": orientation,
            "imu_calib": finite(self.head.calib_progress(), 2),
            "motion": finite(motion, 3),
            "quality": finite(quality, 3),
            "eeg": eeg_wave,
            "contact": contact,
        }

    def band_powers(self, eeg_block, fs):
        names = ("delta", "theta", "alpha", "beta", "gamma")
        try:
            avg, _ = DataFilter.get_avg_band_powers(eeg_block, list(range(eeg_block.shape[0])), fs, True)
        except BrainFlowError as e:
            log.debug("band powers failed: %s", e)
            return {n: 0.2 for n in names}
        return {n: finite(v, 5) for n, v in zip(names, avg)}

    def signal_quality(self, eeg_block, now):
        """Crude 0..1: flat or clipped channels score 0, huge noise scores low."""
        if now - self.last_eeg_new_at > STALL_STATUS:
            return 0.0
        scores = []
        for raw in eeg_block:
            x = raw.copy()
            DataFilter.detrend(x, DetrendOperations.LINEAR.value)
            sd = 1.4826 * float(np.median(np.abs(x - np.median(x))))  # robust: blinks barely move it
            lo, hi = float(raw.min()), float(raw.max())
            clipped = (np.count_nonzero(raw == hi) + np.count_nonzero(raw == lo)) / raw.size if hi > lo else 1.0
            if sd < 0.5 or clipped > 0.05:
                scores.append(0.0)               # flat (off head / dead) or railed
            else:
                scores.append(clamp(1.0 - (sd - 40.0) / 110.0))   # 1 up to 40 µV, 0 at 150 µV
        return float(np.mean(scores)) if scores else 0.0

    def eeg_telemetry(self, eeg_block, fs, now):
        """Per-electrode detrended recent samples (~0.1 s at ~64 Hz) and contact quality badges
        (GOOD / FAIR / POOR, matching Muse 2 DSP thresholds) for the EEG side panel."""
        wave = {}
        contact = {}
        stalled = now - self.last_eeg_new_at > STALL_STATUS
        step = max(1, int(round(fs / 64)))
        n_wave = max(4, int(round(FRAME_DT * fs)))
        n_qual = min(eeg_block.shape[1], int(round(fs)))
        for idx, name in enumerate(self.eeg_names[:4]):
            if idx >= eeg_block.shape[0]:
                break
            raw = eeg_block[idx]
            seg = raw[-n_qual:].copy()
            DataFilter.detrend(seg, DetrendOperations.LINEAR.value)
            sd = float(np.std(seg))
            railed = float(np.count_nonzero(np.abs(seg) > 1000.0) / seg.size) if seg.size else 1.0
            if stalled or railed > 0.15 or sd < 1.0 or sd > 450.0:
                contact[name] = "POOR"
            elif railed > 0.03 or sd > 180.0:
                contact[name] = "FAIR"
            else:
                contact[name] = "GOOD"
            tail = seg[-n_wave::step] if seg.size >= n_wave else seg[::step]
            wave[name] = [finite(v, 1) for v in tail]
        return wave, contact

    def detect_blink(self, frontal, fs, now, motion):
        n = int(BLINK_WINDOW * fs)
        segs = []
        for row in frontal:
            seg = np.ascontiguousarray(row[-n:], dtype=np.float64).copy()
            if seg.size < n // 2:
                return False
            DataFilter.detrend(seg, DetrendOperations.LINEAR.value)
            segs.append(seg)
        ptp = np.array([s.max() - s.min() for s in segs])
        mean_ptp = float(ptp.mean())
        thr = self.args.blink_threshold

        if mean_ptp < BLINK_REARM * thr:
            self.blink_armed = True
        if mean_ptp < thr:
            if self.args.verbose and mean_ptp > 0.5 * thr:
                log.debug("blink candidate ptp=%.0f uV below threshold %.0f", mean_ptp, thr)
            return False

        corr = 1.0
        if len(segs) >= 2 and np.std(segs[0]) > 0 and np.std(segs[1]) > 0:
            corr = float(np.corrcoef(segs[0], segs[1])[0, 1])
        reason = None
        if ptp.max() > ARTIFACT_PTP:
            reason = "artifact (contact pop / railed)"
        elif ptp.min() < 0.5 * thr:
            reason = "only one frontal channel"
        elif corr < BLINK_MIN_CORR:
            reason = f"channels disagree (corr {corr:.2f}, likely eye movement)"
        elif motion > BLINK_MAX_MOTION:
            reason = "head moving"
        elif not self.blink_armed:
            reason = "same blink (not re-armed)"
        elif now - self.last_blink_at < BLINK_REFRACTORY:
            reason = "refractory"
        if reason:
            log.debug("blink rejected ptp=%s corr=%.2f: %s", np.round(ptp).tolist(), corr, reason)
            return False
        self.last_blink_at = now
        self.blink_armed = False
        log.debug("blink accepted ptp=%s corr=%.2f", np.round(ptp).tolist(), corr)
        return True

    def read_imu(self, board, now):
        """Accelerometer, plus gyroscope when the board has one.
        Returns (accel, gyro, orientation, motion); gyro and orientation are None without a gyro."""
        s = self.accel
        if s is None:
            return None, None, None, 0.0
        try:
            d = board.get_current_board_data(max(2, int(round(IMU_WINDOW * s.fs))), s.preset)
        except BrainFlowError:
            return None, None, None, 0.0
        if d.shape[1] == 0:
            return None, None, None, 0.0
        acc = d[s.channels]                      # 3 x n, in g
        n_frame = max(1, int(round(ACCEL_FRAME * s.fs)))
        n_shake = max(2, int(round(ACCEL_SHAKE * s.fs)))
        mean = acc[:, -n_frame:].mean(axis=1)   # latest reading, lightly averaged
        # Motion: change of the (low-passed) gravity vector since the last frame plus
        # a little within-window shake. ~0 when still, saturates on a quick nod or turn.
        jump = 0.0 if self.prev_accel_mean is None else float(np.linalg.norm(mean - self.prev_accel_mean))
        self.prev_accel_mean = mean
        shake_win = acc[:, -n_shake:]
        shake = float(np.linalg.norm(shake_win.std(axis=1))) if shake_win.shape[1] > 1 else 0.0
        raw = clamp((jump + 0.15 * shake - 0.015) / 0.12)

        gyro = orientation = None
        if self.gyro is not None:
            gyr = d[self.gyro.channels]          # 3 x n, in deg/s
            dt = 1.0 / s.fs
            rates = [self.head.update(acc[:, i], gyr[:, i], dt) for i in self.new_imu_columns(d, s, now)]
            if rates:
                # Rotation is what the gyro sees best: a turn barely moves the gravity vector.
                spin = sum(rates) / len(rates)
                raw = max(raw, clamp((spin - GYRO_MOTION_FLOOR) / (GYRO_MOTION_FULL - GYRO_MOTION_FLOOR)))
            g_mean = gyr[:, -n_frame:].mean(axis=1)
            bias = self.head.bias or (0.0, 0.0, 0.0)
            gyro = [finite(g_mean[k] - bias[k], 3) for k in range(3)]
            angles = self.head.angles()
            if angles is not None:
                orientation = {k: finite(v, 2) for k, v in zip(("pitch", "roll", "yaw"), angles)}
        self.motion = max(raw, self.motion * 0.7)   # fast attack, ~0.3 s decay
        return [finite(v, 4) for v in mean], gyro, orientation, self.motion

    def new_imu_columns(self, d, s, now):
        """Columns of `d` the head tracker hasn't seen yet, so each sample is integrated once.
        Frames re-read the last IMU_WINDOW seconds; the timestamp row says which samples are new."""
        n = d.shape[1]
        if s.ts_channel is not None:
            ts = d[s.ts_channel]
            last, self.imu_last_ts = self.imu_last_ts, float(ts[-1])
            return range(n) if last is None else [int(i) for i in np.nonzero(ts > last)[0]]
        last, self.imu_last_at = self.imu_last_at, now     # no timestamps: go by wall-clock time
        k = n if last is None else min(n, int(round((now - last) * s.fs)))
        return range(n - k, n)

    def read_hr(self, board, now):
        s = self.ppg
        if s is None or not self.ppg_enabled:
            self.ppg_wave = []
            return None
        fft = max(1024, next_pow2(int(HR_SECONDS * s.fs)))   # BrainFlow: even and >= 1024
        try:
            d = board.get_current_board_data(fft, s.preset)
        except BrainFlowError:
            return self.hr
        if s.ts_channel is not None and d.shape[1]:
            newest = float(d[s.ts_channel, -1])
            if newest != self.last_ppg_ts:
                self.last_ppg_ts, self.last_ppg_new_at = newest, now
            elif now - self.last_ppg_new_at > 10:
                self.hr_estimates.clear()     # PPG went quiet: stop reporting a stale pulse
                self.hr = None
                self.ppg_wave = []
                return None

        ch = s.channels
        # PPGProcessor from muse-2/core/dsp.py: ambient cancellation (IR - Ambient), invert so systolic
        # peak points upward, normalize by 3*std, and detect inter-beat peaks over a 2.5s window.
        if len(ch) >= 2 and d.shape[1] >= 16:
            win_n = min(d.shape[1], int(s.fs * 3))
            ambient = np.asarray(d[ch[0], -win_n:], dtype=np.float64)
            ir_sig = np.asarray(d[ch[1], -win_n:], dtype=np.float64)
            sig = -(ir_sig - ambient - np.mean(ir_sig - ambient))
            std_val = float(np.std(sig))
            norm_wave = sig / (std_val * 3.0) if std_val > 1e-3 else sig
            tail_n = max(4, int(round(FRAME_DT * s.fs)))
            self.ppg_wave = [finite(v, 3) for v in norm_wave[-tail_n:]]

            if d.shape[1] >= int(s.fs * 2):
                recent = norm_wave[-int(s.fs * 2.5):]
                thresh = float(np.mean(recent) + 0.45 * np.std(recent))
                peaks = [
                    i for i in range(1, len(recent) - 1)
                    if recent[i] > thresh and recent[i] > recent[i - 1] and recent[i] > recent[i + 1]
                ]
                if len(peaks) >= 2:
                    intervals = np.diff(peaks) / s.fs
                    med_int = float(np.median(intervals))
                    if 0.35 <= med_int <= 1.5:  # 40 to 170 BPM
                        raw_bpm = 60.0 / med_int
                        self.ppg_bpm = 0.92 * self.ppg_bpm + 0.08 * raw_bpm
                        self.hr = round(self.ppg_bpm, 1)

        if now - self.last_hr_at < HR_EVERY:
            return self.hr
        self.last_hr_at = now
        if d.shape[1] < fft:
            return self.hr                   # not enough PPG for BrainFlow FFT yet (~16 s on a Muse)
        # Muse PPG rows are ambient, infrared, red (in GATT characteristic order).
        ir, red = (d[ch[1]], d[ch[2]]) if len(ch) >= 3 else (d[ch[0]], d[ch[-1]])
        try:
            bpm = DataFilter.get_heart_rate(np.ascontiguousarray(ir, dtype=np.float64),
                                            np.ascontiguousarray(red, dtype=np.float64), s.fs, fft)
        except BrainFlowError as e:
            log.debug("heart rate failed: %s", e)
            return self.hr
        if math.isfinite(bpm) and 40 <= bpm <= 200:
            self.hr_estimates.append(bpm)
            log.debug("heart rate estimate %.1f bpm", bpm)
        if self.hr_estimates and self.hr is None:
            self.hr = round(float(np.median(self.hr_estimates)), 1)
        return self.hr

    # ------------------------------------------------------------------ synthetic helpers

    def inject_fake_blinks(self, rows, ts):
        """Add blink-shaped bumps to the frontal channels at scheduled times, so the real
        detector (and the delta-band bump) is exercised end to end without hardware."""
        newest = float(ts[-1])
        every = max(1.0, self.args.fake_blink_every)
        if self.fake_next is None:
            self.fake_next = newest + 2.0
        while newest >= self.fake_next:
            self.fake_times.append(self.fake_next)
            self.fake_next += every + random.uniform(-0.5, 0.5)
        while self.fake_times and self.fake_times[0] < newest - BAND_WINDOW - 1.0:
            self.fake_times.popleft()
        if not self.fake_times:
            return
        amp = max(350.0, 1.6 * self.args.blink_threshold)
        bump = np.zeros_like(ts, dtype=np.float64)
        for tb in self.fake_times:
            bump += amp * np.exp(-0.5 * ((ts - tb) / 0.08) ** 2)
        for i, ch in enumerate(self.frontal):
            rows[ch] = rows[ch] + bump * (1.0 if i == 0 else 0.9)


# ---------------------------------------------------------------------- entry point

def parse_args(argv=None):
    p = argparse.ArgumentParser(
        description="Stream EEG band powers, blinks, heart rate, head motion and head orientation from "
                    "a BrainFlow board to the Mind over Chicken browser game over a WebSocket.")
    p.add_argument("--board", choices=list(BOARDS), default="synthetic",
                   help="headset to read (default: synthetic, no hardware needed)")
    p.add_argument("--serial-port", help="BLED112 dongle port for *-bled boards, e.g. COM3 or /dev/ttyACM0")
    p.add_argument("--mac-address", help="headset MAC address (optional for native Bluetooth Muse boards)")
    p.add_argument("--serial-number", help="headset name, e.g. Muse-1A2B (picks one Muse in a crowded room)")
    p.add_argument("--timeout", type=int, default=0,
                   help="Bluetooth discovery timeout in seconds (default: BrainFlow's own)")
    p.add_argument("--host", default="127.0.0.1", help="WebSocket bind address (default 127.0.0.1)")
    p.add_argument("--port", type=int, default=8765, help="WebSocket port (default 8765)")
    p.add_argument("--blink-threshold", type=float, default=200.0,
                   help="blink threshold in microvolts peak-to-peak on the frontal channels over "
                        f"{BLINK_WINDOW:g} s (default 200; natural blinks usually stay below it)")
    p.add_argument("--no-ppg", action="store_true", help="do not enable or read PPG (no heart rate)")
    p.add_argument("--fake-blinks", action="store_true",
                   help="synthetic board only: inject a hard blink every few seconds")
    p.add_argument("--fake-blink-every", type=float, default=4.0,
                   help="seconds between fake blinks (default 4)")
    p.add_argument("-v", "--verbose", action="store_true",
                   help="log blink candidates and heart-rate estimates (use it to tune --blink-threshold)")
    args = p.parse_args(argv)
    if args.board.endswith("-bled") and not args.serial_port:
        p.error(f"--board {args.board} needs --serial-port (the BLED112 dongle, e.g. COM3)")
    return args


def install_stop_signals(task):
    """Ctrl+C (SIGINT) is handled by asyncio.run itself. Also treat Ctrl+Break / closing the
    console (Windows SIGBREAK) and SIGTERM (POSIX) as a clean stop: cancel the main task so
    its finally block releases the headset."""
    loop = asyncio.get_running_loop()
    if sys.platform == "win32":
        signal.signal(signal.SIGBREAK, lambda *_: loop.call_soon_threadsafe(task.cancel))
    else:
        try:
            loop.add_signal_handler(signal.SIGTERM, task.cancel)
        except (NotImplementedError, RuntimeError):
            pass


async def amain(args):
    bridge = Bridge(args)
    try:
        server = await serve(bridge.handler, args.host, args.port, ping_interval=20, ping_timeout=20)
    except OSError as e:
        log.error("cannot listen on %s:%d (%s). Is another bridge already running? Try --port.",
                  args.host, args.port, e)
        return 1
    log.info("WebSocket server on ws://%s:%d  (Ctrl+C to stop)", args.host, args.port)
    install_stop_signals(asyncio.current_task())
    try:
        await bridge.run()
    finally:
        # Runs on Ctrl+C too (asyncio cancels this task): stop the stream, free the headset,
        # then close the sockets so browsers see a clean 1001 "going away".
        bridge.set_status("Bridge shutting down")
        bridge.release_board()
        server.close()
        try:
            await asyncio.wait_for(server.wait_closed(), 3)
        except (asyncio.TimeoutError, asyncio.CancelledError):
            pass
    return 0


def main(argv=None):
    args = parse_args(argv)
    logging.basicConfig(level=logging.DEBUG if args.verbose else logging.INFO,
                        format="%(asctime)s %(message)s", datefmt="%H:%M:%S")
    logging.getLogger("websockets").setLevel(logging.WARNING)
    if args.verbose:
        BoardShim.enable_board_logger()
    else:
        BoardShim.disable_board_logger()
        DataFilter.disable_data_logger()
    code = 0
    try:
        code = asyncio.run(amain(args))
    except (KeyboardInterrupt, asyncio.CancelledError):
        log.info("stopped")
    finally:
        try:
            BoardShim.release_all_sessions()   # belt and braces if Ctrl+C hit mid-connect
        except BrainFlowError:
            pass
    return code or 0


if __name__ == "__main__":
    sys.exit(main())
