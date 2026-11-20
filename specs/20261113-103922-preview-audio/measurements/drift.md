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

## T028: SC-003 and SC-005 on Windows, shared

### Windows 11 — 2026-11-19

**Environment**: as [latency.md](latency.md) T028; release build of `c434a91` for the first two
runs, then the same with the output's resync (T047, T048). Input `HDMI (ezcap Live GAMER ULTRA)`
shared 48000/2/f32, 10 ms period; output shared 48000/2/f32, 2.67 ms period (`IAudioClient3`).

**Method**: quickstart "Drift and continuity": Voicemeeter's bus B3 carries the line input (left)
and the preview's output (right), recorded 1860 s from `Voicemeeter Out B3` with `sounddevice`
streamed to a 16-bit WAV; `--stats` logged. PS5 menu sounds every 1–2 s at the start and the
end; the VAIO3 runs had a game played in between, whose music hides some menu sounds (fewer
pairs, and the last minutes missing in two runs).

| Run | Output | Pairs | First minute | Last minute | Difference | Slope |
|---|---|---|---|---|---|---|
| shared-in5-drift | `Voicemeeter In 5` | 313 | 0: 69.0 ms | 30: 93.0 ms | **+24.0 ms** | +0.77 ms/min |
| shared-vaio3-drift | VAIO3 | 95 | 1: 67.0 ms | 27: 81.5 ms (25: 87.0) | **+14.5 ms** | +0.51 ms/min |
| shared-vaio3-resync | VAIO3, with the resync | 163 | 0: 62.5 ms | 24: 44.0 ms | **−18.5 ms** | — |

**The delay grows after the app.** In both runs without the resync the app's `[audio]` lines
showed `wait_ms` steady (10–11.5 ms on `In 5`, 9.5–12.4 ms on VAIO3) and `rate_ppm` around 0,
while the measured delay grew about 0.8 ms per minute. More checks on the same day:

- `In 5`, the same app session after the run: 91 ms; the app restarted: 59 ms; the restarted app
  58 minutes later: 97 ms (+38 ms, 0.67 ms/min, linear, no reset of its own).
- VAIO3 after 35 minutes: 82–85 ms; **only the output reopened** (the Audio menu, another output
  and back): 58 ms. So the growth is after the app's output (`GetCurrentPadding`), not in the
  input or the bridge. Cause unknown (research R16).

**With the resync** (spec FR-003a): the output reopened at app t=362 s and t=1077 s (the second
waited 7 minutes of game sound with no quiet moment). The delay went from 62–65 ms (minutes 0–5)
to 46 ms (minute 6) and stayed at 41–44 ms in minutes 21–24 (93 pairs). The maintainer heard
nothing at either reopen and played the rest by ear on the preview, "working very well".
Minutes 25–30 have no pairs (the game's music, background −43 dBFS).

Each open starts at a different delay: 62–67 ms from a fresh app, 42–58 ms after a reopen. Part
of the same open question (research R16).

**Continuity** (`[audio]` lines): no gap or drop after the first second on `In 5` (2036 seconds);
on VAIO3 while playing a game, four short ones without the resync (t=578, 581, 666, 1288: gap
8–16 ms, drop 8–16 ms) and one with it (t=1393: 8 ms); none heard by the maintainer. None in the
menu-only runs.

**SC-003 on Windows (maintainer, 2026-11-19)**: passes with the resync. The last minute is
18.5 ms *below* minute 0, outside the 10 ms band, but the rule is there to stop growth and the
delay stays at or below its start. Without the resync it fails (+24 ms on `In 5`, +14.5 to +20
ms on VAIO3). **SC-005 on Windows**: passes (the short gaps while playing were not heard). The
30-minute run on the G533 directly: below.

## T028: the G533 directly

### Windows 11 — 2026-11-19

**Environment**: as the drift runs above; output `Alto-falantes (2- Logitech G533 Gaming
Headset)` (wireless, through its dongle, no surround), shared 48000/8/f32, 10 ms period
(`IAudioClient3` grants no shorter one on it), 1056-frame (22 ms) buffer; Voicemeeter released
it. No recording is possible on this output: by ear and `--stats`. The maintainer played on the
PS5.

**First run** (release of `4575a8b`, 1.5 periods = 15 ms queued; 22 minutes, stopped early): the
maintainer heard pops for 30–60 s at a time. The `[audio]` lines show three stretches (t=299–332,
939–995, 1035–1062, and single drops up to t=1290): `wait_ms` rose from 12 to 52 ms with the rate
at +1000 ppm and 30–180 ms of input dropped every second; the resync's reopen (t=332) or nothing
in particular (t=996) ended them. Between them, none.

**Diagnosis** (branch `exp/20261113-103922-g533-underrun`, not merged: a per-second line of the
output thread's wakes): the thread woke every ~11.5 ms all along (largest interval 12.3 ms), but
in those stretches Windows took nothing at some wakes (720 frames still queued, nothing written)
and 20 ms at the next, more than the 15 ms queued: 13–24 empty queues a second and 6–11 % fewer
frames written than played. Each empty queue is a short silence (the pop), and the time it lost
piled up in the bridge until it dropped input. Research R17.

**With the whole buffer queued** (the same build with the queue at 22 ms; 19 minutes): the
double takes came again (stretches of up to 22 wakes a second with nothing written) with no empty
queue and no shortfall; the maintainer heard no pop. Left in the stats, none heard: gaps of 10–20
ms when Windows took 22 ms at once (three in 19 minutes) and short input hiccups (gap 10 ms and
drop 8 ms, as on VAIO3); one stop of the whole system at t=1086 (no output wake for 98 ms, a video
frame late), which no queue covers. Adopted as spec FR-003b (T049).

**Run with T049** (release of T049 on `13be24c`; 30 minutes, 1801 `[audio]` lines): the
maintainer played and heard no pop. `wait_ms` median 12.1 ms, highest 32.1 ms, no stretch like
the first run's; three resync reopens (t=314, 648, 1525: 48–58 ms dropped at each, the reopen's
alignment in a quiet moment); eight short gaps of 10–20 ms (four within t=1091–1135, the rest
input hiccups with drops of 6–8 ms), none heard. **T028's G533 run passes** (SC-005 by ear and
stats).
