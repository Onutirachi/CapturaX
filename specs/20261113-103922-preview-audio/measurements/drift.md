# Drift and Continuity Measurements

One row per run in [audio-drift.csv](audio-drift.csv); the pairs are in
[audio-samples.csv](audio-samples.csv). Raw `--stats` logs stay local in `raw/` (logs are not
committed).

## T029: SC-003 and SC-005 on Linux

### CachyOS — 2026-11-17

**Environment**: as [latency.md](latency.md) T029; release build of 0bf962c, `node.latency`
128/48000, the preview on the IEC958 output, window 1280 × 720 untouched.

**Method**: quickstart "Drift and continuity": `pipewire-measure.sh record` for 1860 s (31
minutes) with `--stats` logged. Source: the PS5 home menu with its background music off; the
maintainer moved in the menu every 1–2 s during minutes 0–1 and 29–31 and left it silent in
between (background −84 dBFS), so the analyzer has pairs at the start and at minute 30.
`kde-inhibit` kept the screen on (it blanked once in the first minutes).

| Run | Pairs | Minute 0 median | Minute 30 median | Difference | Slope |
|---|---|---|---|---|---|
| pipewire-128-drift | 133 (43 / 9 / 23 / 58 in minutes 0, 1, 29, 30) | 2.0 ms | 2.0 ms | **+0.0 ms** | 0.000 ms/min |

**`[audio]` lines** (1940 seconds): no gap or drop after the first second. `wait_ms` settled at
4.8 ms by second 8 (from 7.4 ms with the priming's extra period, as research R15) and stayed at
4.8/4.8 every second to the end; `rate_ppm` within 0 to +6 after second 10.

**Video** in the same run: 1935 seconds, `ready_ms` median ≤ 1.5 in every second, max over 3 ms in
one second (t=60, 6.6 ms, about when the screen was woken), `pres` 59–61 in every second, no
replaced frame.

SC-003 (minute 30 within 10 ms of minute 0, `wait_ms` steady) and SC-005 (no gap or drop) pass on
Linux. SC-005's listening part (no click, dropout or pitch change heard) was not done on Linux:
the PC has no speakers; the stats cover it, and the maintainer heard none in the 5 minutes through
VBAN of SC-004.
