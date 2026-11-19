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

## T029: SC-007 on Linux, video with and without sound

### CachyOS — 2026-11-17

**Environment**: as above; release build of 0bf962c, `[present] backend=vulkan mode=mailbox`,
window 1280 × 720 (not fullscreen, not touched), ezcap at 1080p60 NV12, sound playing on the
IEC958 output. Four 60 s runs in a row, alternating the default and `--no-audio`. Counts leave
out the first 3 seconds and the last line, as spec #3.

| Run | Seconds | `ready_ms` median > 1.5 | Max > 3 ms | Worst max | `pres` outside 59–61 | `lat_ms` mean | `repl` |
|---|---|---|---|---|---|---|---|
| sound 1 | 58 | 0 | 0 | 1.1 | 0 | 9.84 | 0 |
| `--no-audio` 1 | 58 | 0 | 0 | 1.7 | 0 | 9.60 | 0 |
| sound 2 | 58 | 0 | 0 | 1.5 | 0 | 9.87 | 0 |
| `--no-audio` 2 | 58 | 0 | 0 | 1.7 | 0 | 9.59 | 0 |

SC-007 passes on Linux: spec #3's SC-003 holds with the sound playing. No `[audio]` second had a
gap or drop. Captured → presented (`lat_ms`, not part of SC-003) is about 0.25 ms higher with the
sound in both pairs, as in earlier unrecorded runs.

## T029: SC-004 on Linux, A/V offset

### CachyOS — 2026-11-17

**Computed** (spec SC-004's Linux definition): sound 4.7 ms (SC-002 above, 128) minus spec #3's
glass-to-glass video 29.6–30.1 ms median (runs L4 and L5) = the sound about **25 ms ahead** of
the image, inside the 45 ms ahead / 125 ms behind limits. It leaves out what the card adds before
the PC (the same for both) and the sound device's own delay.

**By ear**: the CachyOS PC has no speakers or headphones, so the maintainer listened on the
Windows PC through the existing VBAN service (`vban.service`, a `vban_out` sink set as default
while it runs; PipeWire moved the preview's playback to it). VBAN only adds delay to the sound,
so this checks the "behind" side, not the 25 ms ahead. About 5 minutes of play, release build of
0bf962c: the maintainer noticed no offset. No `[audio]` second had a gap or drop after the
first, including the move to `vban_out`.

**Decision (maintainer, 2026-11-17)**: SC-004 on Linux passes by the computed offset plus this
listening; the direct listening is left to Windows (T028), which runs the same bridge.

## T028: SC-001 on Windows, shared

### Windows 11 — 2026-11-19

**Environment**: Windows 11 Pro, ezcap Live GAMER ULTRA fed by a PS5, release build of `c434a91`
(`--stats`), input `HDMI (ezcap Live GAMER ULTRA)` shared at 48000/2/f32 with a 10 ms period,
output shared at 48000/2/f32 with a 2.67 ms period, volume 100. Spike: `audio-preview --engine
wasapi` built from `46b66bb` (the spike's last handoff commit), the same periods.

**Method**: quickstart "Latency budget", Windows. Voicemeeter's bus B3 carries the passthrough
monitor's headphone jack through the line input (left) and the preview's output (right);
`Voicemeeter Out B3` recorded 25 s with `sounddevice`; PS5 home menu, a navigation sound every
1–2 s; `analyze.py audio --stereo`.

| Output (right) | Player | Pairs | Median | p95 | Notes |
|---|---|---|---|---|---|
| `Voicemeeter In 5` (hardware strip) | app | 21 | **52 ms** | 54 ms | `shared-in5`, the SC-001 figure |
| `Voicemeeter In 5` | spike | 22 | 53 ms | 54 ms | not in the samples file |
| VAIO3 (virtual input) | app | 21 | 68 ms | 69 ms | `shared-vaio3` |
| VAIO3 | spike | 22 | 66 ms | 67 ms | not in the samples file |
| `Voicemeeter Input` (virtual input) | app | 20 | 99 ms | 100 ms | `shared-vminput` |
| the ezcap read by Voicemeeter (WDM), no app | — | 28 | −28 ms | −26 ms | the reference's own delay (spike: −26 ms) |

**SC-001 shared passes**: 52 ms median, p95 54 ms (budget 62 ms, p95 ≤ median + 5). The app and
the spike's engine agree within 1–2 ms on the same output; the app's `[audio]` lines showed
`wait_ms` about 10.8 ms and no gaps or drops in every run.

**Virtual inputs**: on the same session and output settings, a Voicemeeter virtual input adds its
own buffer before Voicemeeter's mixer, which the reference (a hardware strip) does not pass: 16 ms
on VAIO3 and 47 ms on `Voicemeeter Input`, for the app and the spike alike. The spike measured
59 ms on 2026-10-29 on a virtual input; today the same engine gives 66 ms on VAIO3, while the
reference moved 2 ms, so what changed since is in the virtual input, not in the app.

**Decision (maintainer, 2026-11-19)**: Windows measurements use `Voicemeeter In 5` from here on
(spec "Latency budget and how it is measured", research R10); the maintainer's everyday output stays
a virtual input, whose extra delay is recorded next to each result.

**The C920's microphone** (acceptance.md, T044 Windows): the late sound the maintainer heard
(key press to headset) is the camera's own 60–68 ms, the app's ~24 ms and `Voicemeeter Input`'s
~47 ms measured here, plus Voicemeeter to the G533. VAIO3 saves about 31 ms of it, `In 5` about
47 ms.

## T028: SC-007 and SC-004 on Windows, shared

### Windows 11 — 2026-11-19

**Environment**: as the shared SC-001 runs above, same session; the window restored, then F11
(fullscreen from a maximized window leaves the taskbar on top: handoff, found for later).
Spec #3's glass to glass: the PS5 browser showing `tools/latency/flash.html` (served from the PC),
the passthrough monitor and the preview fullscreen filmed together 25 s at 240 fps, analyzed with
`--fps 240`; samples in [video-samples.csv](video-samples.csv). Two runs in a row: the default
(sound playing on `Voicemeeter In 5`) and `--no-audio`.

| Run | Flashes | Median | p95 | Min | Max | Spec #3 SC-001 (median ≤ 32, p95 ≤ 35) |
|---|---|---|---|---|---|---|
| sound | 26 | **29.0 ms** | 33.0 ms | 25.7 | 34.7 | pass |
| `--no-audio` | 26 | **28.9 ms** | 32.8 ms | 25.2 | 33.5 | pass |

`--stats` over each whole run (seconds after the first 3; logs `raw/win-2026-11-19-t028-g2g-*.log`):

| Run | Seconds | `ready_ms` median > 1.5 | Max > 3 ms | `pres` outside 59–61 | `lat_ms` mean | `repl` |
|---|---|---|---|---|---|---|
| sound | 398 | 0 | 4 (worst 13.2, with a window move) | 1 (54, while the window changed) | 2.33 | 6 |
| `--no-audio` | 210 | 0 | 2 (worst 109.5, no input) | 1 (55, the same second) | 2.57 | 5 |

**SC-007 passes on Windows**: glass to glass is the same with and without sound (29.0 / 28.9 ms,
spec #3's L1 was 28.7 ms) and the render seconds look alike; the few spikes are in both runs, not
caused by the sound. The `[audio]` lines had one gap (10.7 ms) and drop (5.2 ms) in second 2, at
start-up, none after.

**SC-004 (computed)**: sound 52 ms (SC-001 shared, `In 5`) minus video 29.0 ms = the sound about
**23 ms behind** the image, inside 45 ms ahead / 125 ms behind. On the everyday VAIO3 (68 ms) it is
about 39 ms behind, also inside.

**By ear (maintainer, 2026-11-19)**: picture and sound of the preview look in sync; no offset
noticed while playing. With the passthrough on the left ear and the preview on the right, the two
beeps can be told apart, a little, but not the sound against the picture. SC-004 passes on Windows
for the shared setting (both exclusive: T039).
