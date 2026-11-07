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
- `hr` is meaningless. The synthetic PPG is noise, so BrainFlow returns anything from 140 to 220 bpm.
- `accel` is random noise near 0.9 g on each axis, so `motion` hovers around 0.1 to 0.5.

## Run with a Muse 2 or Muse S

| Headset | Built-in Bluetooth (Windows 10 build 19041+, macOS 10.15+) | BLED112 USB dongle |
|---|---|---|
| Muse 2 | `python muse_bridge.py --board muse2` | `python muse_bridge.py --board muse2-bled --serial-port COM3` |
| Muse S | `python muse_bridge.py --board muses` | `python muse_bridge.py --board muses-bled --serial-port COM3` |

1. Turn the headset on, wear it, and close the Muse phone app (or turn off the phone's Bluetooth).
   A headset already connected to a phone cannot be found.
2. Start the bridge and wait for `Streaming from Muse 2 (PPG, accel)`. The game shows the same
   status text. If several Muses are nearby, add `--serial-number Muse-XXXX` (the name printed on
   the headset or shown in the Muse app) or `--mac-address`.
3. Heart rate shows up about 16 s after connecting, once BrainFlow has enough PPG.
4. Tune blinks: run with `-v`, then do some natural and some hard blinks. The log prints
   `blink candidate ptp=...` and `blink rejected/accepted ptp=[AF7, AF8]`. Set
   `--blink-threshold` between your natural blinks and your hard blinks.

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
| `--no-ppg` | off | Don't enable or read PPG, so `hr` is always `null` |
| `--fake-blinks` | off | Synthetic board only: inject hard blinks |
| `--fake-blink-every` | `4` | Seconds between fake blinks (±0.5 s jitter) |
| `-v`, `--verbose` | off | Log blink candidates and heart-rate estimates, and turn on the BrainFlow logger |

Stop with Ctrl+C (or Ctrl+Break on Windows). The bridge stops the stream, releases the BrainFlow
session and closes the sockets.

## Protocol

The bridge sends text frames of JSON. The browser never needs to send anything.

A feature frame, about every 100 ms:

```json
{"t": 12.61,
 "bands": {"delta": 0.036, "theta": 0.026, "alpha": 0.079, "beta": 0.435, "gamma": 0.424},
 "blink": false,
 "hr": 71.2,
 "accel": [0.0312, -0.0127, 0.9981],
 "motion": 0.146,
 "quality": 0.93}
```

| Key | Meaning |
|---|---|
| `t` | Seconds since the bridge started |
| `bands` | `DataFilter.get_avg_band_powers` over all EEG channels for the last 2 s, with filters on (detrend plus 50/60 Hz notch). Bands are 2–4, 4–8, 8–13, 13–30 and 30–45 Hz. They already sum to about 1, and the browser renormalises them anyway |
| `blink` | A deliberate blink was detected since the previous frame (see below) |
| `hr` | Heart rate in bpm from `DataFilter.get_heart_rate` on PPG, refreshed every 2 s and taken as the median of the last 5 estimates. `null` if there is no PPG, it is still warming up, or it went quiet |
| `accel` | Accelerometer reading in g, averaged over the last 0.1 s. `null` if the board has no accelerometer |
| `motion` | 0 to 1 head movement: change of the gravity vector since the last frame plus some shake. Rises fast and decays over about 0.3 s |
| `quality` | 0 to 1, the mean of per-channel scores. A flat channel (robust SD under 0.5 µV) or a clipped one (more than 5 % of samples at min/max) scores 0. Noise scores 1 up to a robust SD of 40 µV, falling to 0 at 150 µV. The value is 0 when data has stalled |

A status message, sent on connect and whenever the status changes:

```json
{"type": "status", "text": "Streaming from Muse 2 (PPG, accel)"}
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

## Troubleshooting

- **Stuck on "Waiting for headset".** Check that the headset is on and not connected to a phone.
  Built-in Bluetooth needs Windows 10 build 19041 or newer. Try `--timeout 15`,
  `--serial-number Muse-XXXX`, or a BLED112 dongle. Run with `-v` to see BrainFlow's own log.
- **"cannot listen on 127.0.0.1:8765".** Another bridge is already running. Stop it, or use
  `--port 8766` and point the game at `ws://localhost:8766`.
- **The game says "No bridge answered".** Make sure the bridge is running. If your browser resolves
  `localhost` to IPv6 only, start the bridge with `--host localhost`.
- **No `hr`.** PPG needs about 16 s of data after connecting, and skin contact. `--no-ppg` turns it
  off. If the log says `could not enable PPG`, that firmware rejected the PPG preset command.
- **Lightning fires on normal blinks.** Raise `--blink-threshold`, using `-v` to see your values.
  **It never fires:** lower it, and blink harder and longer.
- **`quality` is low on a real headset.** Wet the sensors, move hair away from the TP9/TP10
  ear sensors, and sit still for a few seconds.

## BrainFlow facts this relies on

- The PPG preset commands come from BrainFlow's Supported Boards page
  (<https://brainflow.readthedocs.io/en/stable/SupportedBoards.html>, sections "Muse 2" and "Muse S"):
  - For Muse 2, `ANCILLARY_PRESET` "contains PPG data, to enable it use `board.config_board("p50")`.
    It also enables 5th channel for EEG".
  - For Muse S, PPG is enabled with `board.config_board("p61")`.
  - On both, `AUXILIARY_PRESET` holds gyro and accelerometer data and is "enabled by default".
  - Both default to Muse command `p21` (4 EEG channels plus IMU).
  - Built-in Bluetooth needs Windows 10.0.19041.0+ or macOS 10.15+.
  - The bridge calls `config_board` after `prepare_session` and before `start_stream`.
- The BLED112 variants (`MUSE_2_BLED_BOARD`, `MUSE_S_BLED_BOARD`) need `serial_port` and a BLED112
  dongle, and use the same `p50` / `p61` PPG commands. This comes from the BrainFlow 5.10.0 docs
  (<https://github.com/brainflow-dev/brainflow/blob/5.10.0/docs/SupportedBoards.rst>). The current
  stable docs no longer list the BLED boards, although `BoardIds` 21 and 22 still exist in
  BrainFlow 5.23.0.
- The Muse driver (<https://github.com/brainflow-dev/brainflow/blob/master/src/board_controller/muse/muse.cpp>)
  divides accelerometer values by 16384, so they are in g. It scales EEG as
  `(raw - 0x800) * 125/256`, so EEG is in µV and rails near ±1000 µV.
- The bridge picks presets with `BoardShim.get_board_presets` before asking for channels. In
  BrainFlow 5.23.0, querying a preset that a board lacks (for example `ANCILLARY` on the synthetic
  board, even with `get_board_descr`) makes a later `start_stream` in the same process fail with
  `GENERAL_ERROR:17`.
- `DataFilter.get_heart_rate(ppg_ir, ppg_red, sampling_rate, fft_size)` documents `fft_size` as
  "recommended 8192". In 5.23.0 it rejects `fft_size` values that are odd or under 1024, and needs
  at least `fft_size` samples. At the Muse's 64 Hz PPG rate the bridge uses 1024 samples (16 s),
  which gives 3.75 bpm resolution.
- Muse PPG rows are treated as ambient, infrared and red, in GATT characteristic order, as
  muse-lsl and Mind Monitor label them. BrainFlow's docs do not name the rows. **Not verified on
  hardware.**
