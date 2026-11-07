"""Head tracking (#53): the bridge's accelerometer + gyroscope complementary filter.

HeadTracker is plain Python, so its tests feed it synthetic IMU samples and run everywhere.
CI has only the standard library. When numpy, BrainFlow or websockets is missing, those modules
are stubbed only for the import of bridge/muse_bridge.py. The frame test needs the real BrainFlow
synthetic board and skips without it.

Synthetic samples use a right-handed head frame with z up when level. The accelerometer reads the
reaction to gravity (+1 g on z when level), and the gyro is in deg/s with the right-hand rule.
"""

import importlib.util
import json
import math
import random
import sys
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
BRIDGE = ROOT / "bridge" / "muse_bridge.py"
STUBBED = ("numpy", "brainflow", "brainflow.board_shim", "brainflow.data_filter", "websockets",
           "websockets.asyncio", "websockets.asyncio.server", "websockets.exceptions")


def load_bridge(stub):
    spec = importlib.util.spec_from_file_location("muse_bridge_under_test", BRIDGE)
    module = importlib.util.module_from_spec(spec)
    added = [name for name in STUBBED if stub and name not in sys.modules]
    for name in added:
        sys.modules[name] = mock.MagicMock(name=name)
    try:
        spec.loader.exec_module(module)
    finally:
        for name in added:          # only the stubs leave; real modules stay importable
            del sys.modules[name]
    return module


try:
    mb = load_bridge(stub=False)
    REAL_BRIDGE = True
except (ImportError, OSError):     # OSError: BrainFlow installed but its native library won't load
    mb = load_bridge(stub=True)
    REAL_BRIDGE = False

FS = 52.0                          # Muse 2 / Muse S auxiliary (IMU) rate
DT = 1.0 / FS
LEVEL = (0.0, 0.0, 1.0)


def feed(tracker, seconds, accel, gyro, rng=None, accel_noise=0.0, gyro_noise=0.0):
    """Feed `seconds` of samples; accel/gyro are tuples or callables of time within this segment.
    Returns the (pitch, roll, yaw) history."""
    out = []
    for i in range(int(round(seconds * FS))):
        t = i * DT
        a = accel(t) if callable(accel) else accel
        g = gyro(t) if callable(gyro) else gyro
        if rng is not None:
            a = [v + rng.gauss(0, accel_noise) for v in a]
            g = [v + rng.gauss(0, gyro_noise) for v in g]
        tracker.update(a, g, DT)
        out.append(tracker.angles())
    return out


def plus(u, v):
    return tuple(a + b for a, b in zip(u, v))


def gravity_pitch(a):
    return math.degrees(math.atan2(a[0], math.hypot(a[1], a[2])))


class HeadTrackerTest(unittest.TestCase):
    BIAS = (0.7, -0.4, 0.9)        # deg/s, a typical small MEMS zero-rate offset

    def settled(self, tracker, seconds=2.0):
        """Hold still so the first gyro-bias estimate is in."""
        feed(tracker, seconds, LEVEL, self.BIAS)
        self.assertIsNotNone(tracker.bias)

    def test_still_head_gives_stable_orientation(self):
        rng = random.Random(1)
        tr = mb.HeadTracker()
        hist = feed(tr, 30.0, LEVEL, self.BIAS, rng, accel_noise=0.01, gyro_noise=0.3)
        tail = hist[-int(10 * FS):]
        for k, name in enumerate(("pitch", "roll", "yaw")):
            values = [h[k] for h in tail]
            self.assertLess(max(abs(v) for v in values), 1.0, name)
            self.assertLess(max(values) - min(values), 1.0, f"{name} wanders")
        for est, true in zip(tr.bias, self.BIAS):
            self.assertAlmostEqual(est, true, delta=0.1)

    def test_no_gyro_use_before_bias_is_known(self):
        tr = mb.HeadTracker()
        hist = feed(tr, 0.5, LEVEL, (0.0, 0.0, 40.0))     # spinning, but the bias is unknown
        self.assertIsNone(tr.bias)
        self.assertEqual(hist[-1][2], 0.0)

    def test_constant_yaw_rate_integrates(self):
        tr = mb.HeadTracker()
        self.settled(tr)
        rate, seconds = 30.0, 2.0
        # +z rotation with z up is counter-clockwise from above, a turn to the LEFT: yaw goes negative.
        hist = feed(tr, seconds, LEVEL, plus(self.BIAS, (0.0, 0.0, rate)))
        tau = mb.YAW_RECENTER_TAU
        expected = -rate * tau * (1 - math.exp(-seconds / tau))   # integrator with the slow leak
        pitch, roll, yaw = hist[-1]
        self.assertAlmostEqual(yaw, expected, delta=0.5)
        self.assertLess(abs(yaw + rate * seconds), 0.06 * rate * seconds)  # within 6 % of plain integral
        self.assertLess(abs(pitch), 0.5)
        self.assertLess(abs(roll), 0.5)

    def test_yaw_does_not_depend_on_which_axis_points_up(self):
        # The same right turn seen by an IMU mounted upside down (rotated 180 deg about x).
        upright, flipped = mb.HeadTracker(), mb.HeadTracker()
        feed(upright, 2.0, LEVEL, (0.0, 0.0, 0.0))
        feed(flipped, 2.0, (0.0, 0.0, -1.0), (0.0, 0.0, 0.0))
        a = feed(upright, 1.0, LEVEL, (0.0, 0.0, -20.0))[-1][2]
        b = feed(flipped, 1.0, (0.0, 0.0, -1.0), (0.0, 0.0, 20.0))[-1][2]
        self.assertGreater(a, 15.0)                        # clockwise from above = right = positive
        self.assertAlmostEqual(a, b, delta=0.01)

    def tilt_ramp(self, axis, degrees, seconds):
        """Rotate about IMU `axis` (0 = x, 1 = y) by `degrees` at a constant rate, consistently on both
        sensors. Returns (fused tracker history, gyro-blind tracker history, final accel)."""
        rate = degrees / seconds

        def accel(t):
            th = math.radians(rate * min(t, seconds))
            # Reaction-to-gravity vector in a head frame rotated by th about x or y (right-hand rule).
            return (0.0, math.sin(th), math.cos(th)) if axis == 0 else (-math.sin(th), 0.0, math.cos(th))

        def gyro(t):
            w = [0.0, 0.0, 0.0]
            w[axis] = rate if t < seconds else 0.0
            return plus(self.BIAS, w)

        fused, blind = mb.HeadTracker(), mb.HeadTracker()
        self.settled(fused)
        self.settled(blind)
        hist = feed(fused, seconds + 5.0, accel, gyro)
        blind_hist = feed(blind, seconds + 5.0, accel, self.BIAS)   # gyro sees nothing
        return hist, blind_hist, accel(seconds), int(round(seconds * FS)) - 1

    def test_pitch_tilt_tracks_fast_and_converges_to_gravity(self):
        hist, blind, final, end = self.tilt_ramp(axis=1, degrees=20.0, seconds=0.5)
        target = gravity_pitch(final)
        self.assertAlmostEqual(target, -20.0, places=6)
        # At the end of the half-second nod the gyro has carried the fused pitch almost all the way;
        # gravity alone (same filter, no gyro) is still far behind.
        self.assertLess(abs(hist[end][0] - target), 1.5)
        self.assertGreater(abs(blind[end][0] - target), 10.0)
        self.assertLess(abs(hist[-1][0] - target), 0.3)
        self.assertLess(abs(hist[-1][1]), 0.3)
        self.assertLess(abs(hist[-1][2]), 0.5)             # a nod is not a turn

    def test_roll_tilt_tracks_fast_and_converges_to_gravity(self):
        hist, blind, final, end = self.tilt_ramp(axis=0, degrees=15.0, seconds=0.5)
        target = math.degrees(math.atan2(final[1], math.hypot(final[0], final[2])))
        self.assertAlmostEqual(target, 15.0, places=6)
        self.assertLess(abs(hist[end][1] - target), 1.5)
        self.assertGreater(abs(blind[end][1] - target), 7.0)
        self.assertLess(abs(hist[-1][1] - target), 0.3)
        self.assertLess(abs(hist[-1][0]), 0.3)

    def test_gravity_pulls_tilt_with_the_filter_time_constant(self):
        # The head is suddenly at 20 deg pitch but the gyro saw nothing: gravity alone corrects it.
        tr = mb.HeadTracker()
        self.settled(tr)
        th = math.radians(20.0)
        tilted = (-math.sin(th), 0.0, math.cos(th))
        hist = feed(tr, 5 * mb.TILT_TAU, tilted, self.BIAS)
        after_tau = hist[int(round(mb.TILT_TAU * FS)) - 1][0]
        self.assertAlmostEqual(after_tau, -20.0 * (1 - math.exp(-1)), delta=1.0)    # ~63 %
        self.assertLess(abs(hist[-1][0] + 20.0), 0.3)                               # > 99 %

    def test_turn_and_hold_recentres_yaw(self):
        tr = mb.HeadTracker()
        self.settled(tr)
        feed(tr, 0.5, LEVEL, plus(self.BIAS, (0.0, 0.0, -40.0)))     # 20 deg turn to the right
        turned = tr.angles()[2]
        self.assertGreater(turned, 19.0)
        hist = feed(tr, 3 * mb.YAW_RECENTER_TAU, LEVEL, self.BIAS)    # hold it
        yaws = [h[2] for h in hist]
        self.assertTrue(all(b <= a + 1e-9 for a, b in zip(yaws, yaws[1:])), "yaw should only fall")
        self.assertAlmostEqual(yaws[-1], turned * math.exp(-3), delta=0.2)

    def test_residual_gyro_drift_stays_bounded(self):
        # The gyro bias shifts by 1 deg/s after the bias was estimated (warm-up, temperature).
        drift = (0.0, 0.0, 1.0)
        no_learning = mb.HeadTracker(bias_tau=math.inf)
        self.settled(no_learning)
        yaws = [h[2] for h in feed(no_learning, 200.0, LEVEL, plus(self.BIAS, drift))]
        bound = 1.0 * mb.YAW_RECENTER_TAU                      # re-centring alone: drift x tau
        self.assertLess(max(abs(y) for y in yaws), bound + 0.1)
        self.assertGreater(abs(yaws[-1]), bound - 1.0)          # ...and it is a plateau, not a spin

        learning = mb.HeadTracker()
        self.settled(learning)
        yaws = [h[2] for h in feed(learning, 120.0, LEVEL, plus(self.BIAS, drift))]
        self.assertLess(max(abs(y) for y in yaws), 8.0)          # bias tracking soaks it up
        self.assertLess(abs(yaws[-1]), 0.5)
        self.assertAlmostEqual(learning.bias[2], self.BIAS[2] + 1.0, delta=0.05)

    def test_bias_estimate_waits_for_stillness(self):
        tr = mb.HeadTracker()
        feed(tr, 0.6, LEVEL, plus(self.BIAS, (0.0, 0.0, 50.0)))     # turning at connect
        feed(tr, 1.5, LEVEL, self.BIAS)
        for est, true in zip(tr.bias, self.BIAS):
            self.assertAlmostEqual(est, true, delta=0.01)

    def test_wrong_first_bias_heals(self):
        # A perfectly steady turn throughout the first second looks like bias. Once the head is
        # still for GYRO_BIAS_REACQUIRE seconds the estimate is replaced and yaw restarts at 0.
        tr = mb.HeadTracker()
        feed(tr, 1.2, LEVEL, plus(self.BIAS, (0.0, 0.0, 50.0)))
        self.assertAlmostEqual(tr.bias[2], self.BIAS[2] + 50.0, delta=0.01)
        feed(tr, mb.GYRO_BIAS_REACQUIRE + 0.5, LEVEL, self.BIAS)
        for est, true in zip(tr.bias, self.BIAS):
            self.assertAlmostEqual(est, true, delta=0.01)
        hist = feed(tr, 10.0, LEVEL, self.BIAS)
        self.assertLess(max(abs(h[2]) for h in hist), 0.01)

    def test_bad_samples_are_ignored(self):
        tr = mb.HeadTracker()
        self.assertIsNone(tr.angles())
        tr.update((0.0, 0.0, 0.0), (0.0, 0.0, 0.0), DT)          # dropped sample: no gravity yet
        self.assertIsNone(tr.angles())
        self.settled(tr)
        tr.update((float("nan"), 0.0, 1.0), (0.0, 0.0, 0.0), DT)
        tr.update((0.0, 0.0, 0.0), self.BIAS, DT)
        self.assertTrue(all(math.isfinite(v) and abs(v) < 0.5 for v in tr.angles()))

    def test_rotation_speed_is_reported(self):
        tr = mb.HeadTracker()
        self.settled(tr)
        self.assertLess(tr.rate, 0.1)
        tr.update(LEVEL, plus(self.BIAS, (30.0, 0.0, 40.0)), DT)
        self.assertAlmostEqual(tr.rate, 50.0, delta=0.5)


@unittest.skipUnless(REAL_BRIDGE, "needs BrainFlow, numpy and websockets (bridge/requirements.txt)")
class SyntheticBoardFrameTest(unittest.TestCase):
    """Runs the bridge's real frame code against BrainFlow's synthetic board (no WebSocket)."""

    KEYS = {"t", "bands", "blink", "hr", "ppg", "accel", "gyro", "orientation", "imu_calib", "motion", "quality", "eeg", "contact"}

    def test_frames_carry_gyro_and_orientation(self):
        from brainflow.board_shim import BoardShim, BrainFlowInputParams
        BoardShim.disable_board_logger()
        bridge = mb.Bridge(mb.parse_args(["--board", "synthetic", "--no-ppg"]))
        self.assertIsNotNone(bridge.gyro, "the synthetic board lists gyro rows in its AUXILIARY preset")
        self.assertEqual(bridge.gyro.preset, bridge.accel.preset)
        board = BoardShim(bridge.board_id, BrainFlowInputParams())
        bridge.board = board
        frames = []
        try:
            board.prepare_session()
            board.start_stream(mb.BUFFER_SAMPLES)
            deadline = time.monotonic() + 8.0
            while len(frames) < 15 and time.monotonic() < deadline:
                frame = bridge.compute_frame(time.monotonic())
                if frame is not None:
                    frames.append(frame)
                time.sleep(mb.FRAME_DT)
            self.assertGreaterEqual(len(frames), 15, "no frames from the synthetic board")

            bridge.gyro = None                      # a board without a gyro: same keys, nulls
            no_gyro = bridge.compute_frame(time.monotonic())
        finally:
            bridge.release_board()

        for f in frames:
            self.assertEqual(set(f), self.KEYS)
            json.dumps(f, allow_nan=False)
            self.assertEqual(len(f["accel"]), 3)
            self.assertEqual(len(f["gyro"]), 3)
            self.assertTrue(all(isinstance(v, float) for v in f["gyro"]))
            self.assertEqual(set(f["orientation"]), {"pitch", "roll", "yaw"})
            self.assertTrue(all(-180 <= v <= 180 for v in f["orientation"].values()))
            self.assertTrue(0 <= f["motion"] <= 1)
        self.assertEqual(set(no_gyro), self.KEYS)
        self.assertIsNone(no_gyro["gyro"])
        self.assertIsNone(no_gyro["orientation"])
        self.assertEqual(len(no_gyro["accel"]), 3)


if __name__ == "__main__":
    unittest.main()
