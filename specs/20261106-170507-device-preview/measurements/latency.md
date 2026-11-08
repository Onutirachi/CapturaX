# Latency Measurements

Raw `--stats` logs are in [raw/](raw/). As in the app shell, the first 3 seconds (start) and the
last line (closing) are left out of every count.

## T064: UI presents between slow video frames (research R16)

### Windows 11 — 2026-11-07

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, release builds of commit
59b2dde ("before") and of T064 ("after"), header `[present] backend=dx12 mode=mailbox` in every
run. Window 1071 × 565 on a 144 Hz monitor with G-SYNC on, the app focused. Each run is about
30 s; "chrome" runs: the maintainer moved the pointer over the video and stopped, again and
again, so the chrome showed and hid. Logs: [raw/t064/](raw/t064/).

The C920 delivered 15 fps (its 1080p30 mode in low light), so its period came from the measured
intervals (about 66 ms), not from the mode's 30 fps.

| Source | Run | Seconds | Seconds with UI | `ready_ms` median | Max > 3 ms | Worst max | `pres` |
|---|---|---|---|---|---|---|---|
| C920, 15 fps | before, idle | 27 | 0 | 0.8–0.9 | 0 | 1.3 | 15–16 |
| | after, idle | 27 | 0 | 0.8–0.9 | 0 | 1.6 | 14–16 |
| | before, chrome | 26 | 19 | 0.8–**3.3** | **13** | 21.5 | 14–16 |
| | after, chrome | 26 | 17 | 0.7–0.8 | 2 (5.0, 12.9) | 12.9 | 14–16 |
| Test video 1080p60 | before, idle | 27 | 0 | 0.6–0.7 | 0 | 1.9 | 60–61 |
| | after, idle | 27 | 0 | 0.7 | 0 | 2.1 | 60–61 |
| | before, chrome | 27 | 19 | 0.6–0.7 | 4 (18.5–26.9) | 26.9 | 60–61 |
| | after, chrome | 27 | 17 | 0.6–0.7 | 0 | 1.7 | 60–61 |
| Phone virtual camera, 30 fps | before, idle | 27 | 0 | 0.6–0.7 | 1 (484) | 484 | 18–31 |
| | after, idle | 25 | 0 | 0.6–0.7 | 0 | 1.6 | 30–31 |

The phone runs happened by accident: the first runs started from an empty settings file, so the
start order picked the phone's virtual camera ("cam" heuristic). They are kept as a third
source. The 484 ms stall there had no UI activity, so it does not come from T064.

**Not reproduced**: a first "after, chrome" run with the C920 had 13 seconds over 3 ms, with
peaks of 33–114 ms around t=19–22 (some in seconds without UI images) and a 484 ms stall. The
maintainer saw no stall in the video. A second run (the one in the table) had none of it, so the
first is taken as noise from the camera and the system. Its log was overwritten.

**Maintainer, C920 with G-SYNC**: the chrome and the cursor are much smoother after T064. A few
slight hitches remain in the animation. They are expected: a UI change that falls within one
refresh plus 2 ms of the next expected frame waits for that frame, and a webcam's frames come
irregularly. No video stall seen.

**Verdict**: idle unchanged. While the chrome animates, `ready_ms` is better after T064 with
every source. Against the app shell's SC-001 rule (median ≤ 1.5 ms every second, max ≤ 3 ms in
95% of the seconds), the after runs give: test video 27 of 27 seconds; C920 24 of 26 (92%,
before: 13 of 26 and 2 seconds with a median over 1.5 ms). SC-003 itself is for the real card at
1080p60 and is measured at T028; this spec's SC-001 (camera latency with the ezcap) is not
affected by this change and is not measured here.

### CachyOS KDE Plasma Wayland — 2026-11-07

**Environment**: CachyOS, NVIDIA GeForce RTX 3050, release builds of 59b2dde ("before") and of
T064 ("after"), each in its own folder with the same settings file, header `[present]
backend=vulkan mode=mailbox` in every run. Window 1294 × 720 on a 1080p 120 Hz monitor without
VRR. Source: the ezcap at 1080p120 NV12 (Auto max FPS) with no HDMI input, so it sent its own
"Please Connect a Video Source" picture at 120 fps. Each run is 32 s; "chrome" runs: the agent
moved the pointer over SSH (`ydotool`) to the top of the window, which shows the toolbar, then
out of the window, every 4–5 s. Logs: [raw/t064/](raw/t064/) (`lin-*`).

| Source | Run | Seconds | Seconds with UI | `ready_ms` median | Max > 3 ms | Worst max | `pres` |
|---|---|---|---|---|---|---|---|
| ezcap, 120 fps | before, idle | 28 | 0 | 0.7–2.6 | 3 (3.3, 3.6, 82.6) | 82.6 | 110–120 |
| | after, idle | 28 | 0 | 0.7–2.6 | 4 (3.1–3.2) | 3.2 | 111–120 |
| | before, chrome | 27 | 13 | 0.8–2.6 | 4 (3.2–3.6) | 3.6 | 111–120 |
| | after, chrome | 27 | 13 | 0.8–2.4 | 5 (3.4–4.3) | 4.3 | 111–120 |

The medians rise from about 0.8 to 1.3–2.6 ms after a few seconds in every run, with or without
UI activity: the GPU clocks drop on Linux (seen with the test video in Phase 2), which T024–T026
address. In every run the card delivered 111 frames at t=5 with one gap of about 85 ms
(`recv=111`), so the gap comes from the card or its driver a few seconds after opening, not from
the present path. The 82.6 ms stall in the "before, idle" run (t=19, 10 frames replaced) had no
UI activity, so it does not come from T064.

**Verdict**: no change at 120 fps, as expected: a frame is due every 8.3 ms, less than one
refresh plus the margin, so a UI change always rides on the next frame, as before T064. The
small differences between runs are noise from the clock drop above.

## T025: GPU performance request on Linux (research R7)

### CachyOS, RTX 3050, driver 615.78.08 — 2026-11-07

The agent ran `gpu_performance 60 0000:01:00.0` (release build of the T024 example) over SSH,
with `nvidia-smi --query-gpu=timestamp,pstate,clocks.gr,clocks.mem,pcie.link.gen.current,power.draw
--format=csv -lms 500` running from 5 s before to 10 s after. No other GPU load; desktop idle.
Logs: [raw/t025/](raw/t025/).

| Phase | Samples | P-state | Graphics | Memory | PCIe | Power |
|---|---|---|---|---|---|---|
| before | 11 | P8 | 210 MHz | 405 MHz | Gen1 | 5.8–6.0 W |
| request, first second | 2 | P0 | 210 MHz | 405 MHz | Gen3 | 6.1 W |
| request held | 121 | P0 | 1777 MHz | 7001 MHz | Gen3 | 22.3–23.1 W |
| after release | 26 | P8 | 210–360 MHz | 405 MHz | Gen1 | 5.8–6.7 W |

The example logged `PowerMizer prefers maximum performance on 0000:01:00.0 (was Adaptive)` at
0.023 s and `PowerMizer back to Adaptive` at 60.024 s; no root, no error. The clocks reach full
speed within about 1 s of the request, stay there for the whole minute, and drop back to P8 in
the next sample after the release.

A second check with `nvidia-settings` showed the request is per process: set from
nvidia-settings alone, the mode lasts only until it exits; set to Adaptive from nvidia-settings
while the example holds the request, the GPU stays at P0 (research R7).

**Verdict**: research R7 holds on this PC; the keep-alive fallback is not needed. Cost while the
request is held: about **+17 W** at idle (6 → 23 W), within R7's estimate of +10–20 W; the cost
while previewing is measured with L4/L6 in T028 (FR-012).

## T028: Latency budget (SC-001 to SC-003, FR-012)

### Windows 11 — 2026-11-07 (L1, L2)

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, release build of 8f191a7,
header `[present] backend=dx12 mode=mailbox`. Preview on the left monitor (Samsung, 1080p at
143.98 Hz, FreeSync that the driver does not take as G-SYNC); the passthrough on the right
monitor (G-SYNC). Source: PS5 at 1080p60 showing `flash.html` through the ezcap (Windows id
`mf:\\?\usb#vid_32ed&pid_3201&mi_00#9&37ee7830&0&0000#…`, USB serial `00000001`), mode pinned
to `1920x1080@60` (NV12, BT.709, `Auto (Completa)`) in a portable copy (`target/t028`). The
maintainer placed the window, pressed F11 for L1 and filmed 25 s at 240 fps; after the L2 clip
the maintainer went through the chrome, the color range menu, the settings panel, language and
theme switches, then F11, chrome and menu in fullscreen, then Esc, in the L2 log. Logs:
[raw/t028/](raw/t028/) (`l1.txt`, `l2.txt`).

| Run | Window | n | Median | p95 | Min | Max | Budget (SC-001) |
|---|---|---|---|---|---|---|---|
| L1 | fullscreen | 24 | **28.7** | **31.7** | 25.5 | 31.9 | median ≤ 32, p95 ≤ 35: **pass** |
| L2 | windowed | 23 | **29.7** | **32.3** | 25.3 | 32.5 | **pass** |

**SC-003 reading of the `--stats` logs**:

| Log | Seconds | Seconds with UI | `ready_ms` median | Median > 1.5 | Max > 3 | `pres` outside 59–61 | Gaps > 50 ms (with `ui=0`) | Worst gap |
|---|---|---|---|---|---|---|---|---|
| L1 | 328 | 26 | 0.6–1.4 | 0 | 48 (85.4% ok) | 13 (96.0% ok) | 25 (19) | 436 ms |
| L2 + states | 366 | 101 | 0.6–0.8 | 0 | 74 (79.8% ok) | 31 (91.5% ok) | 46 (41) | 675 ms |

- The medians are clean (0.6–0.8 ms) and the clips have no outlier sample, so the stalls did
  not fall inside the filmed 25 s.
- **Stalls with no UI activity**: bursts of seconds with present gaps of 50–675 ms and frames
  replaced in the inbox (`repl` up to 5), `ready_ms` max ≈ gap − 16 ms: a frame was ready and
  the video thread did not present it for that long, so the wait is in the present path, not in
  capture. Bursts: L1 t=76–114, L2 t=43–69 and t=234–258. They fall when the maintainer was
  working in other windows (the region windows of `analyze.py` took the focus while the app
  ran), so they look like the app shell's focus-change stalls (229–401 ms, research R8), which
  SC-006 and T052 investigate. The maintainer does not remember whether the focus moved at
  those times, so a check followed (`raw/t028/focus.txt`, same build and window, 134 s): with
  the app left alone for 70 s (t=4–73) there is no second with a max over 3 ms or a gap over
  30 ms; with ten clicks into another window and back about every 5 s (t=74–102) every focus
  change gives a stall of 80–200 ms (`repl` 2–4), and the max stays at 4–28 ms for a few
  seconds after each one; clean again after the last click. **The stalls are the focus-change
  stalls** (SC-006, T052); the cause is investigated there with PresentMon.
- With the UI active (fullscreen chrome and menu, settings panel, language and theme switches)
  there is no stall of its own beyond those bursts: a few seconds with a max of 4.5–8.9 ms.

**Verdict (Windows)**: SC-001 passes, fullscreen and windowed. SC-003 passes on the medians;
the max and `pres` rules fail only because of the focus-change stalls, which SC-003 leaves to
SC-006 (T052); with the app focused and left alone it passes.

### CachyOS KDE Plasma Wayland — 2026-11-07 (L4, L5, L6)

**Environment**: CachyOS, NVIDIA GeForce RTX 3050 (driver 615.78.08), 1080p 120 Hz monitor
without VRR, header `[present] backend=vulkan mode=mailbox`. Source: PS5 at 1080p60 showing
`flash.html` through the ezcap, mode pinned in the settings file to `1920x1080@60` (NV12,
BT.709, `Auto (Completa)`). L4 and L5: release build of 8f191a7 (with the GPU request). L6:
release build of cfed7fe (T064, before T024–T026): the same video path without the request.
The agent started each run over SSH with `--stats` and `nvidia-smi … -lms 500`, entered
fullscreen with F11 (L4, L6) and moved the pointer off the video; the maintainer filmed 25 s at
240 fps with the passthrough next to the preview, chrome idle; after the clip the agent showed
and hid the chrome 3 times (`ydotool`). Clips analyzed with `--fps 240` (the phone stores 29.1
fps as the playback rate). Logs: [raw/t028/](raw/t028/), samples in
[video-samples.csv](video-samples.csv), table in [summary.md](summary.md).

| Run | Window | GPU request | n | Median | p95 | Min | Max | Budget (SC-002) |
|---|---|---|---|---|---|---|---|---|
| L4 | fullscreen | yes | 25 | **30.1** | **34.4** | 25.0 | 42.6 | median ≤ 35, p95 ≤ 40: **pass** |
| L5 | windowed 1280 × 720 | yes | 23 | **29.6** | **33.4** | 26.0 | 33.8 | **pass** |
| L6 | fullscreen | no | 24 | 37.9 | 46.0 | 20.6 | 48.7 | recorded (would fail) |

Samples rejected by the analyzer (camera skipped frames): 1, 2 and 1.

**GPU while previewing (FR-012)**, from the `nvidia-smi` logs:

| Run | P-state while previewing | Graphics | Power (mean) |
|---|---|---|---|
| L4 | P0 (515 of 517 samples; P8 only before the first frame) | 1777 MHz | 24.7 W |
| L5 | P0 (394 of 396) | 1777 MHz | 24.4 W |
| L6 | P8 (212 of 224; P0/P5 in the first seconds) | ~250 MHz | 8.2 W |

The request costs about **+16.5 W** while previewing and saves about **8 ms** of median and
**12 ms** of p95 glass-to-glass latency; without it the Linux budget is missed.

**SC-003 reading of the `--stats` logs** (first 3 seconds and last line left out):

| Run | Seconds | Seconds with UI | `ready_ms` median | Median > 1.5 | Max > 3 | `pres` outside 59–61 |
|---|---|---|---|---|---|---|
| L4 | 252 | 13 | 0.7–0.8 (outside the F11 switch) | 4 (t=6–9, F11 switch) | 12 (95.2% ok) | 1 (58, t=253) |
| L5 | 192 | 11 | 0.8 | 0 | 0 | 0 |
| L6 | 106 | 5 | 2.6–3.1 | 104 | 42 (60.4% ok) | 3 |

- **Entering fullscreen (L4)**: for about 4 seconds after F11 the frames waited one, then two
  refreshes before they were ready (`ready_ms` 16, then 30 ms median, t=6–9), then went back to
  0.8 ms on their own. L6 shows the same at its F11 (median 6.4, then 22.4 ms, t=6–7).
  Probably KWin switching the fullscreen window to direct scanout; to be checked
  against the app shell's Linux fullscreen runs before it is called a regression.
- **Chrome in fullscreen (L4, t=233–250)**: the medians stay at 0.7–0.8 ms, but 7 of the 9
  seconds with UI images have a max of 3.3–8.9 ms (`gap_ms` up to 24.8). In the window (L5) the
  same chrome passes give no max over 3 ms.
- **Interface states** (`ui.txt`, build of L4, no clip, 130 seconds, 85 with UI images): the
  maintainer went through the chrome, the color range menu, the settings panel, language and
  theme switches in the window, then F11 and the chrome and menu in fullscreen. Windowed
  (t=4–82, by the log's shape): one second with a max over 3 ms (3.1), medians 0.7–0.8. From
  t=83 to the end (fullscreen, UI active almost every second): `ready_ms` max 3.1–12.7 ms in 30
  seconds, falling about 1 ms a second and starting again (a beat between the video and the
  UI presents), and 5 seconds with a median of 2.4–5.0 ms; `pres` 60 throughout. Whole run:
  76.9% of the seconds with a max ≤ 3 ms.
- The fullscreen spikes with UI are not new: the app shell's Linux runs with the clock locked
  gave max over 3 ms in 36 of 38 seconds of fullscreen chrome (highest 4.3 ms; app shell
  measurements/latency.md, "1080 fullscreen chrome", clock locked). Idle fullscreen, where the
  latency budget is measured, is clean.
- L6 confirms what the app shell saw at idle on Linux: with the GPU at P8, `ready_ms` sits at
  2.6 ms and the SC-003 rule fails.

**Verdict (Linux)**: SC-002 passes, fullscreen and windowed, with the GPU request; without it
(L6) it fails by 3 ms of median and 6 ms of p95. SC-003 passes windowed in every interface
state; in fullscreen it passes at idle but not while the UI animates (max up to 12.7 ms, 5
seconds with a median over 1.5 ms), as in the app shell, plus about 4 seconds after F11. No
frame is dropped (`pres` 60). The maintainer accepted it as a known limit of fullscreen on KWin
(2026-11-07, recorded in spec SC-003), to be investigated only if it becomes a problem in use;
likely causes to start from: the UI layer drawn at full screen size, or KWin leaving direct
scanout while the chrome shows.

## T029: The default mode with a 60 Hz and a 120 Hz source (research R3)

### Windows 11 — 2026-11-07 (L3, L3b)

Same setup as L1 (preview fullscreen on the Samsung at 143.98 Hz, passthrough on the right
monitor, portable copy `target/t028`), with `mode = "auto-max-fps"`: the ezcap opens at
1920 × 1080 · 120 NV12 and delivers about 112 frames a second with either source.

- **L3**: PS5 at 1080p60 showing `flash.html`.
- **L3b**: the CachyOS PC's HDMI output into the ezcap as its only display, set to 1920 × 1080 at
  119.88 Hz (`kscreen-doctor`; KDE had picked 239.96 Hz), `flash.html` in Firefox `--kiosk` from
  a file. The PS5's 120 Hz output is only on in games that support it, so it could not be held.

| Run | Source | Card mode | n | Median | p95 | Min | Max |
|---|---|---|---|---|---|---|---|
| L1 (T028) | 60 Hz | 1080p60 | 24 | 28.7 | 31.7 | 25.5 | 31.9 |
| L3 | 60 Hz | 1080p120 | 24 | **34.0** | **39.6** | 28.2 | 41.1 |
| L3b | 119.88 Hz | 1080p120 | 17 | **31.5** | **36.1** | 26.5 | 38.1 |

- L3b has 17 samples (8 rejected because the camera skipped frames), under the 20 the method
  asks for.
- **Cost of the default with a 60 Hz source**: +5.3 ms of median and +7.9 ms of p95 against the
  60 mode, at the top of the 3–5 ms the spike measured. It is over SC-001's budget (median ≤ 32,
  p95 ≤ 35), which is stated for the 1080p60 mode. The spread is wider too (28–41 ms against
  25–32 ms): a new source frame waits for the card's next 120 Hz slot, which is not aligned with
  the source.
- **With a 120 Hz source** the median is 2.8 ms over L1, not under it. The delays are measured
  against the passthrough, which also shows the source at 120 Hz, so the faster source helps
  the reference as much as the preview; the camera cannot measure from the source itself. Read
  as: against a 120 Hz display of the same source, the preview is about 31.5 ms behind.
- Logs: [raw/t028/l3.txt](raw/t028/l3.txt), [raw/t028/l3b.txt](raw/t028/l3b.txt). Clean up to
  the end of each clip (`ready_ms` median 0.5 ms, max ≤ 3.6 ms except one second at 21 ms in
  L3), then focus-change stalls once the maintainer moved to other windows (T052).

**Verdict (research R3, FR-008)**: the default Auto max FPS made the most common case (a
console at 60 Hz) about 5 ms slower than the 1080p60 mode, over the SC-001 budget; it only pays
off in smoothness with a 120 Hz source. The maintainer changed the default (2026-11-07): Auto
source when shown, else 1920 × 1080 at about 60 fps when the device has it, else Auto max
resolution (T065); Auto max FPS stays in the menu, and the bubbles show its effect. L3b was not
filmed again for the 20 samples (the decision did not depend on it). Optional levers not tried.

## T040: Calibrating the latency estimate (SC-007)

### Windows 11 — 2026-11-08

**Inputs**: L1 (T028), 1080p60, fullscreen on the 143.98 Hz monitor (`R/2` = 3.5 ms, no
variable refresh in the estimate), camera median **28.7 ms**. L1's log predates the measured
part (T036), so the measured part comes from T039 (`raw/t039/run2.txt`, same card, 1080p60,
same PC): `lat_ms` median **2.2 ms** (2.1 over all 521 seconds of both runs at 60 and 120 fps;
it is the app's own part and does not depend on the mode).

**With the first formula** (`F + (Windows ? F : 0) + C + R/2`, C = 5 ms): estimate 16.7 + 16.7 +
5 + 3.5 = 41.8 ms, Total 2.2 + 41.8 = **44 ms**, 15.3 ms over the median, so SC-007 fails.
Calibrating C on it gives 28.7 − 2.2 − 33.3 − 3.5 = **−10.3 ms**, which cannot be card
processing plus the monitor's delay.

**Why**: one frame period too many. The card spreads the USB transfer over the whole frame
period (research R11: 8.2 ms at 1080p120 at the source pad), so it sends while it receives from
HDMI, and reception and transfer overlap: the whole frame is at the host about one frame period
after it starts, not two. On Windows the measured part starts when the whole frame reaches the
app; on Linux the driver stamps the first packet, so that period is already inside the measured
part. Corrected formula (data-model.md, maintainer 2026-11-08):
`(Windows ? F : 0) + C + (vrr ? 0 : R/2)`.

**`C_WINDOWS`** = 28.7 − 2.2 − 16.7 − 3.5 = **6.3 ms** (close to the spike's 5 ms). Total for L1:
2.2 + 16.7 + 6.3 + 3.5 = 28.7 → "~29 ms" (unit test `windows_total_matches_the_l1_median`).
Against L2 (windowed, median 29.7 ms, same monitor): "~29 ms", 0.7 ms off. Both within SC-007's
5 ms. Against L3 (the 1080p120 mode with a 60 Hz source, median 34.0 ms) the estimate gives
2.2 + 8.3 + 6.3 + 3.5 = 20.3 → "~20 ms", 14 ms under: the card's wait for its next 120 Hz slot
with an unaligned source (T029) is not in the formula. With a 119.88 Hz source in the same mode
the maintainer's screenshot (2026-11-08) shows "~20 ms" against L3b's 31.5 ms, 11 ms under: the
card seems to hold the frame longer in its 120 modes, and that is not in the formula either.
SC-007 is stated for the 1080p60 setups only; the bubble reads low in the 1080p120 mode.

**Maintainer check** (2026-11-08, release build of the corrected formula, windowed on the 144 Hz
monitor, the CachyOS PC's HDMI output at 1920 × 1080 · 60 Hz into the ezcap in the 1080p60
mode): the Total display shows **"~28 ms"** with `60 / 60 FPS`, 0.7 ms under L1 and 1.7 ms under
L2: **SC-007 passes on Windows**.

### CachyOS — provisional

L4 (fullscreen, 120 Hz without VRR, `R/2` = 4.2 ms): median **30.1 ms**, but no Linux log with
`lat_ms` exists at 1080p60 (L4 and T027 predate T036, and the ezcap is at the Windows PC). With
the measured part assumed at about 17.5 ms (the transfer over one frame period plus about 1 ms in
the app), `C_LINUX` = 30.1 − 17.5 − 4.2 ≈ **8.4 ms**, set as provisional. To finish in the Linux
batch (with T034): a `--stats` run with the L4 setup (1080p60, fullscreen, same monitor, no clip
needed), `C_LINUX` = 30.1 − `lat_ms` median − 4.2, then the maintainer checks the Total against
L4 and L5.
