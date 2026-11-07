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
