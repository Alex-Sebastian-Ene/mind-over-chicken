# Headset bridge

`muse_bridge.py` reads an EEG headset through [BrainFlow](https://brainflow.org) and sends a
feature frame to the game every ~100 ms over a WebSocket. The game's `BridgeSource`
(`src/brain/sources.js`) connects to `ws://localhost:8765` by default.

## Install

Python 3.9 or newer.

```powershell
cd bridge
python -m venv .venv            # keep .venv out of git
.venv\Scripts\activate          # macOS/Linux: source .venv/bin/activate
pip install -r requirements.txt
```

## Run without hardware (synthetic board)

```powershell
python muse_bridge.py                          # BrainFlow synthetic board on ws://127.0.0.1:8765
python muse_bridge.py --fake-blinks            # plus a hard blink about every 4 s (tests lightning)
python muse_bridge.py --fake-blinks --fake-blink-every 2
```

`--fake-blinks` adds a blink-shaped 350 µV bump to the two "frontal" channels at scheduled times.
The bump then goes through the same detector as real data. Expect these differences from a real headset:

- `quality` reads about 0.4, because BrainFlow's synthetic channels get louder with channel
  index (about 10 µV × index) and the higher ones look "noisy".
- `accel` is random noise near 0.9 g on each axis, so `motion` hovers around 0.1 to 0.5.
- `gyro` is random noise between 0.8 and 1.0 deg/s on each axis. The filter takes that offset as
  gyro bias, so `orientation.yaw` stays near 0. Pitch and roll sit near 35°, because the synthetic
  "gravity" points along (1, 1, 1).

## Run with a Muse 2 or Muse S

| Headset | Built-in Bluetooth (Windows 10 build 19041+, macOS 10.15+) | BLED112 USB dongle |
|---|---|---|
| Muse 2 | `python muse_bridge.py --board muse2` | `python muse_bridge.py --board muse2-bled --serial-port COM3` |
| Muse S | `python muse_bridge.py --board muses` | `python muse_bridge.py --board muses-bled --serial-port COM3` |

1. Turn the headset on, wear it, and close the Muse phone app (or turn off the phone's Bluetooth).
   A headset already connected to a phone cannot be found.
2. Start the bridge and wait for `Streaming from Muse 2 (accel, gyro)`. The game shows the same
   status text. If several Muses are nearby, add `--serial-number Muse-XXXX` (the name printed on
   the headset or shown in the Muse app) or `--mac-address`.
3. Tune blinks: run with `-v`, then do some natural and some hard blinks. The log prints
   `blink candidate ptp=...` and `blink rejected/accepted ptp=[AF7, AF8]`. Set
   `--blink-threshold` between your natural blinks and your hard blinks.
4. Head tracking: hold your head still for the first couple of seconds after the bridge starts
   streaming, so it can measure the gyro bias. Then hold it still for another second after the game connects,
   so the game can take your neutral pose. See [Head tracking](#head-tracking).

## Flags

| Flag | Default | Meaning |
|---|---|---|
| `--board` | `synthetic` | `synthetic`, `muse2`, `muse2-bled`, `muses`, `muses-bled` |
| `--serial-port` | | BLED112 dongle port (`COM3`, `/dev/ttyACM0`). Required for `*-bled` boards |
| `--mac-address` | | Headset MAC (optional, built-in Bluetooth) |
| `--serial-number` | | Headset name, such as `Muse-1A2B`, to pick one Muse in a crowded room |
| `--timeout` | BrainFlow's | Bluetooth discovery timeout, in seconds |
| `--host` | `127.0.0.1` | Bind address. `localhost` binds both IPv4 and IPv6 loopback. `0.0.0.0` exposes the bridge to your LAN |
| `--port` | `8765` | WebSocket port |
| `--blink-threshold` | `200` | µV peak-to-peak (mean of AF7 and AF8) over 0.4 s for a blink to count |
| `--fake-blinks` | off | Synthetic board only: inject hard blinks |
| `--fake-blink-every` | `4` | Seconds between fake blinks (±0.5 s jitter) |
| `-v`, `--verbose` | off | Log blink candidates and turn on the BrainFlow logger |

Stop with Ctrl+C (or Ctrl+Break on Windows). The bridge stops the stream, releases the BrainFlow
session and closes the sockets.

## Protocol

The bridge sends text frames of JSON. The browser never needs to send anything.

A feature frame, about every 100 ms:

```json
{"t": 12.61,
 "bands": {"delta": 0.036, "theta": 0.026, "alpha": 0.079, "beta": 0.435, "gamma": 0.424},
 "blink": false,
 "accel": [0.0312, -0.0127, 0.9981],
 "gyro": [0.41, -1.87, 12.3],
 "orientation": {"pitch": 1.79, "roll": -0.73, "yaw": -6.2},
 "motion": 0.146,
 "quality": 0.93}
```

| Key | Meaning |
|---|---|
| `t` | Seconds since the bridge started |
| `bands` | `DataFilter.get_avg_band_powers` over all EEG channels for the last 2 s, with filters on (detrend plus 50/60 Hz notch). Bands are 2–4, 4–8, 8–13, 13–30 and 30–45 Hz. They already sum to about 1, and the browser renormalises them anyway |
| `blink` | A deliberate blink was detected since the previous frame (see below) |
| `accel` | Accelerometer reading in g, averaged over the last 0.1 s. `null` if the board has no accelerometer |
| `gyro` | Gyroscope reading in deg/s, averaged over the last 0.1 s. This is the raw reading: the bias is not removed. `null` if the board has no gyroscope in the accelerometer's preset |
| `orientation` | Fused head orientation in degrees: `pitch`, `roll` and `yaw`. See [Head tracking](#head-tracking). `null` without a gyroscope, and then the game falls back to `accel` tilt |
| `motion` | 0 to 1 head movement. It is the larger of two terms. The accelerometer term is the change of the gravity vector since the last frame plus some shake. The gyroscope term is the bias-corrected rotation speed: 0 at 5 deg/s, 1 at 125 deg/s. Rises fast and decays over about 0.3 s |
| `quality` | 0 to 1, the mean of per-channel scores. A flat channel (robust SD under 0.5 µV) or a clipped one (more than 5 % of samples at min/max) scores 0. Noise scores 1 up to a robust SD of 40 µV, falling to 0 at 150 µV. The value is 0 when data has stalled |

A status message, sent on connect and whenever the status changes:

```json
{"type": "status", "text": "Streaming from Muse 2 (accel, gyro)"}
```

Typical texts are `Connecting to Muse 2`, `Waiting for headset: Muse 2 not found, retrying`,
`Waiting for headset data`, `Headset data stopped, reconnecting` and `Bridge shutting down`.

### Blink detection

The detector reads AF7 and AF8 on a Muse. On the synthetic board it reads the first two EEG
channels. Each channel's last 0.4 s is linearly detrended and its peak-to-peak is measured. A
blink fires when all of these hold:

- The mean peak-to-peak is at or above `--blink-threshold`, and each channel reaches at least half of it.
- AF7 and AF8 are positively correlated (r ≥ 0.3). Blinks are vertical EOG and move both sides
  together, while sideways eye movements push them in opposite directions.
- Neither channel exceeds 900 µV, which would be a contact pop or a railed channel.
- `motion` is below 0.8.
- At least 0.5 s has passed since the last blink.
- The signal has dropped back below 60 % of the threshold since the last blink, so one long blink
  isn't counted twice.

### Head tracking

`HeadTracker` in `muse_bridge.py` fuses the accelerometer and gyroscope with a complementary filter.
It is plain Python, and `tests/test_head_tracking.py` drives it with synthetic samples. The bridge
reads both sensors from the same preset (AUXILIARY on the Muse and the synthetic board). Every
frame it re-reads the last 0.5 s, and the timestamp row picks out the samples the filter has not
seen yet, so each sample is integrated exactly once with `dt = 1 / sampling rate`.

**Axes.** The filter uses the IMU's own axes, which BrainFlow passes through unchanged. It assumes the
accelerometer and gyroscope share one right-handed frame, as on any single-chip IMU. It does not
assume which axis points up.

**Pitch and roll.** The filter keeps `up`, a unit vector in the head frame: what a still
accelerometer reads, the reaction to gravity, pointing up. For each sample:

1. Rotate `up` by the bias-corrected gyro: `du/dt = u × ω`, with ω in rad/s.
2. Blend it toward the measured accelerometer direction:
   `u = normalise(α·u_gyro + (1 − α)·a/|a|)`, where `α = τ / (τ + dt)`.
3. Compute the angles:
   `pitch = atan2(u_x, hypot(u_y, u_z))` and `roll = atan2(u_y, hypot(u_x, u_z))`.

The gyro gives a fast response and gravity removes the gyro's drift. Pitch is the elevation of the
IMU's x axis above the horizontal and roll is that of its y axis. These are the accelerometer-only
tilt formulas applied to the fused vector, so pitch rises with `accel[0]` and roll with `accel[1]`.
Samples under 0.3 g (a dropped sample) don't correct tilt.

**Yaw.** Yaw is rotation about the vertical. It integrates `ω · up`, which equals gyro z when the
head is level. Positive yaw is a turn to the right, clockwise seen from above. Gravity can't see
yaw, so yaw leaks back toward 0 with a 20 s time constant. Gyro bias the filter misses then gives
a bounded offset (bias × 20 s) instead of an endless spin. A turn you hold for 2 s loses about 10 %.

**Gyro bias.** A steady window averages the gyro and restarts whenever a sample is more than 4 deg/s
from its mean.

- The first estimate is the mean of the first 1 s of steady gyro. Until then the gyro is ignored:
  tilt comes from the accelerometer alone and yaw holds 0.
- After that, the bias follows a slow average (10 s) of samples within 4 deg/s of it, so it tracks
  temperature drift.
- If the window stays steady for 5 s at a value more than 4 deg/s from the estimate, the first
  estimate was wrong. That happens when the head turned steadily during the first second. The
  window mean replaces the estimate and yaw restarts at 0.

| Constant | Value | Meaning |
|---|---|---|
| `TILT_TAU` | 1.0 s | Complementary filter time constant. α = 0.981 at the Muse's 52 Hz and 0.996 at the synthetic board's 250 Hz |
| `YAW_RECENTER_TAU` | 20 s | Yaw leaks back to 0 with this time constant |
| `GYRO_BIAS_INIT` | 1.0 s | Steady gyro averaged into the first bias estimate |
| `GYRO_BIAS_TAU` | 10 s | Time constant of the bias average while the head is still |
| `GYRO_BIAS_REACQUIRE` | 5 s | Steady gyro, far from the estimate, that replaces it |
| `GYRO_STILL` | 4 deg/s | Below this the gyro counts as steady or the head as still |
| `GYRO_MOTION_FLOOR`, `GYRO_MOTION_FULL` | 5, 125 deg/s | Rotation speeds that map to `motion` 0 and 1 |
| `IMU_WINDOW` | 0.5 s | IMU data re-read every frame. New samples are picked by timestamp |

**In the game.** `BridgeSource` (`src/brain/sources.js`) aims with `orientation` when the frame has one:

- Yaw moves the aim left and right, and pitch moves it up and down.
- The mean of the first 10 frames (about 1 s) is the neutral pose.
- ±25° from neutral spans the screen, clamped at the edges, with light smoothing (rate 15/s).
- The yaw neutral fades to 0 over 20 s, in step with the bridge's own re-centring. Without the fade,
  a fixed yaw offset would become a permanent bias.
- `BridgeSource.AIM_SIGN` flips either axis.
- `recenter()` takes a new neutral pose.

Without `orientation`, the game uses the older accelerometer tilt. Pitch keeps that path's sign,
rising with `accel[0]`, so the vertical aim behaves as before.

**Not verified on hardware.** The axis signs above follow from the math, but nobody has yet checked
them on a Muse. If the aim moves the wrong way, flip `BridgeSource.AIM_SIGN.yaw` or `.pitch`.

## Troubleshooting

- **Stuck on "Waiting for headset".** Check that the headset is on and not connected to a phone.
  Built-in Bluetooth needs Windows 10 build 19041 or newer. Try `--timeout 15`,
  `--serial-number Muse-XXXX`, or a BLED112 dongle. Run with `-v` to see BrainFlow's own log.
- **"cannot listen on 127.0.0.1:8765".** Another bridge is already running. Stop it, or use
  `--port 8766` and point the game at `ws://localhost:8766`.
- **The game says "No bridge answered".** Make sure the bridge is running. If your browser resolves
  `localhost` to IPv6 only, start the bridge with `--host localhost`.
- **Lightning fires on normal blinks.** Raise `--blink-threshold`, using `-v` to see your values.
  **It never fires:** lower it, and blink harder and longer.
- **`quality` is low on a real headset.** Wet the sensors, move hair away from the TP9/TP10
  ear sensors, and sit still for a few seconds.
- **The aim moves the wrong way.** Flip `BridgeSource.AIM_SIGN.yaw` or `.pitch` in
  `src/brain/sources.js`.
- **The aim creeps sideways while you sit still.** The gyro bias estimate is off. Restart the
  bridge and hold still for the first second, or just wait: yaw re-centres within about 20 s.

## BrainFlow facts this relies on

- From BrainFlow's Supported Boards page
  (<https://brainflow.readthedocs.io/en/stable/SupportedBoards.html>, sections "Muse 2" and "Muse S"):
  - On both, `AUXILIARY_PRESET` holds gyro and accelerometer data and is "enabled by default".
  - Both default to Muse command `p21` (4 EEG channels plus IMU). The bridge sends no
    `config_board` command, so it streams exactly that.
  - Built-in Bluetooth needs Windows 10.0.19041.0+ or macOS 10.15+.
- The BLED112 variants (`MUSE_2_BLED_BOARD`, `MUSE_S_BLED_BOARD`) need `serial_port` and a BLED112
  dongle. This comes from the BrainFlow 5.10.0 docs
  (<https://github.com/brainflow-dev/brainflow/blob/5.10.0/docs/SupportedBoards.rst>). The current
  stable docs no longer list the BLED boards, although `BoardIds` 21 and 22 still exist in
  BrainFlow 5.23.0.
- The Muse driver (<https://github.com/brainflow-dev/brainflow/blob/master/src/board_controller/muse/muse.cpp>)
  divides accelerometer values by 16384, so they are in g. It scales EEG as
  `(raw - 0x800) * 125/256`, so EEG is in µV and rails near ±1000 µV.
- The same driver multiplies gyro values by `MUSE_GYRO_SCALE_FACTOR` (0.007476, which is
  245 deg/s ÷ 32768, defined in `muse_constants.h`), so the gyro is in deg/s. Accelerometer and gyro
  rows are written to `AUXILIARY_PRESET` without any axis remapping or sign change. Each BLE packet
  holds 3 IMU samples, and the driver gives each one its own timestamp, interpolated between
  packets. `get_board_descr` reports the Muse 2/S auxiliary preset as accel rows 1–3 and gyro
  rows 4–6 at 52 Hz. On the synthetic board it is accel rows 2–4 and gyro rows 5–7 at 250 Hz.
- The bridge picks presets with `BoardShim.get_board_presets` before asking for channels. In
  BrainFlow 5.23.0, querying a preset that a board lacks (for example `ANCILLARY` on the synthetic
  board, even with `get_board_descr`) makes a later `start_stream` in the same process fail with
  `GENERAL_ERROR:17`.
