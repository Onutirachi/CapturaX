# Latency Measurements

Samples are in [audio-samples.csv](audio-samples.csv) (one row per sound onset pair). Raw
`--stats` logs stay local in `raw/` (logs are not committed).

## T029: SC-002 on Linux, `node.latency` variants

### CachyOS — 2026-11-17

**Environment**: CachyOS (kernel 7.3.1), KDE Plasma Wayland, PipeWire 1.6.10, i3-10105, ezcap
Live GAMER ULTRA on USB fed by a PS5, output the onboard IEC958 (digital stereo). Release build for
128 (the constant), debug builds of 0bf962c with `CAPTURAX_PW_LATENCY` for 64 and 256; `pw-dump`
showed both streams at the requested `node.latency` in each run.

**Method**: quickstart "Latency budget", Linux: `pipewire-measure.sh` records the card (left) and
the app's playback (right) in the null sink `capturax-measure` while the app keeps playing on the
real output; `analyze.py audio --stereo`; 25 s per run. Source: PS5 home menu with its background
music off, a navigation sound every 1–2 s (background −83 dBFS). The PS5 game menus, even paused,
kept music under the clicks and gave no onsets.

The result is the measured delay plus `out_latency_ms` from the run's `[audio-source]` line.

| `node.latency` | Pairs | Measured median / p95 | `out_latency_ms` | Result median / p95 | `wait_ms` at the end | Gaps or drops after t=1 |
|---|---|---|---|---|---|---|
| 64/48000 | 16 | 2 / 3 ms | 1.3 | **3.3 / 4.3 ms** | 3.4 | 0 |
| 128/48000 | 17 | 2 / 3 ms | 2.7 | **4.7 / 5.7 ms** | 4.8 | 0 |
| 256/48000 | 15 | 2 / 2 ms | 5.3 | **7.3 / 7.3 ms** | 7.4 | 0 |

All three meet SC-002 (median ≤ 25 ms, p95 ≤ 30 ms). The measured part is the bridge's 2 ms margin
in every variant: the sink records the card and the app in the same graph cycle, so the quantum
shows only in `out_latency_ms`, and `wait_ms` (margin plus the output's period) agrees. A
cross-correlation of a 25 s clip at 128 (PS5 game music, every 4 s window) gave 2.06 ms.

**Decision (maintainer, 2026-11-17)**: 128/48000 stays (research R3). 64 saves 1.4 ms, which
cannot be heard, and runs the whole graph at 1.33 ms while the preview plays; 256 adds 2.6 ms.
