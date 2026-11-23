# Latency Measurements

T027: the quickstart "Latency check" (SC-003, research R16) and SC-001's times. The `--stats`
logs stay local in `raw/t027/` (logs are not committed). As in the earlier specs, the first 3 seconds (start) and the last line
(closing) are left out of every count.

## Windows 11, 2026-11-23

**Environment**: Windows 11 Pro 26300, AMD Ryzen 9 9950X3D, NVIDIA GeForce RTX 5070 Ti, commit
e3f3dde built as release **with debug assertions** (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`),
because `--screenshot-every` exists only in debug builds and a debug build would blur the frame
times. Header `[present] backend=dx12 mode=mailbox`, window maximized (1920×1022) on the 144 Hz
monitor. The ezcap with the PS5 in a game (real moving picture: 1080p files of 2–3 MB, 4K files
of 10 MB), output VAIO3 exclusive. Runs of about 30 s (the maintainer asked for shorter runs than
the quickstart's 60 s); the agent ran them and sent the burst's keys with `keybd_event`.

"Max > 3": seconds whose `ready_ms` maximum is over 3 ms. "Lost": frames received minus frames
presented over the run (±1 is a frame crossing a second's edge).

| Mode | Run | Seconds | `recv` | Lost | `repl` | `ready_ms` median | Max > 3 | Worst max | Worst `gap_ms` | Audio gap |
|---|---|---|---|---|---|---|---|---|---|---|
| 1080p60 | baseline | 28 | 60 | 0 | 0 | 0.7–0.8 | 0 | 1.4 | 17.9 | 0 |
| | every 1000 ms | 28 | 59–61 | 1 | 0 | 0.8 | 0 | 1.6 | 17.9 | 0 |
| 1080p120 | baseline | 26 | 77–109 | 1 | 0 | 0.7–0.8 | 1 | 5.2 | 10.1 | 8.0 ms |
| | every 1000 ms | 28 | 108–109 | 1 | 0 | 0.7 | 0 | 2.1 | 10.9 | 0 |
| | burst of 10 | 15 | 108–110 | 3 | 3 | 0.7–0.8 | 1 | 37.3 | 30.3 | 0 |
| 4K30 | baseline | 28 | 27–28 | −1 | 0 | 1.2–1.4 | 0 | 1.9 | 39.6 | 0 |
| | baseline, again | 28 | 26–28 | 0 | 0 | 1.2–1.4 | 0 | 2.0 | 38.4 | 0 |
| | every 1000 ms | 28 | 26–28 | 1 | 0 | 1.2–1.4 | **5** | **7.5** | 45.8 | 0 |
| | every 1000 ms, again | 28 | 26–28 | 1 | 0 | 1.2–1.4 | **2** | **6.8** | 43.0 | 0 |
| | every 1000 ms, `--present-log` | 28 | 26–28 | 0 | 0 | 1.2–1.4 | **16** | **9.2** | 47.3 | 0 |

The card delivers about 108 fps in its 1080p120 mode and 27–28 fps in its 4K30 mode, with or
without screenshots.

**Not from the screenshots**: the 1080p120 baseline's t=6 (77 frames, an 8 ms audio gap, while
the start's device notice was shown) has no screenshot in it. The burst run's only bad seconds
are t=3–4 (19:35:05–06, `ready_ms` max 16.6 and 37.3, 3 frames replaced), four seconds before
the burst (19:35:10); the burst's own seconds (t=8–9) have `ready_ms` max 2.5 and 1.3.

**Burst** (1080p120, 10 presses ≈ 45 ms apart): screenshots 1–9 written in order
(` (2)`…` (9)` in one second), the 6th press "not taken (Busy)": the limit of 4 waiting reached
on Windows. Each took ≈ 65 ms.

**4K**: with a screenshot every second, a few seconds have one frame presented late, up to
9.2 ms after it was ready (1.9–2.0 ms without screenshots), and the frame gap grows by the same
amount. No frame is lost and the audio has no gap. Not seen at 1080p. The cause was not looked
for (candidates: the RGB conversion, ≈ 95 ms, or the 33 MB clipboard copy, ≈ 110 ms, competing
for memory with the preview; or the outline flash and the notice): the maintainer accepted it as
rare and unseen, and SC-003 says so (spec change, 2026-11-23). A possible later improvement.

### SC-001 (key to notice)

From the 161 log lines of the runs above (contracts/cli.md); the total is the sum of the steps,
and the notice is posted 1 ms after the line.

| Mode | Taken | RGB | PNG | Written | Copied | Total | Limit |
|---|---|---|---|---|---|---|---|
| 1080p60 | +0–15 ms | 22–24 ms | 8–11 ms | 2–19 ms | 23–28 ms | 60–84 ms | 1 s |
| 1080p120 | +0–9 ms | 22–24 ms | 10–11 ms | 2–29 ms | 25–30 ms | 60–94 ms | 1 s |
| 4K30 | +0–36 ms | 90–97 ms | 39–48 ms | 5–61 ms | 106–131 ms | 254–343 ms | 2 s |

**Verdict (Windows)**: SC-001 passes with a wide margin. SC-003 passes at 1080p60 and 1080p120
(0 frames lost, spread unchanged, no audio gap) and, as amended, at 4K30.

## Linux (CachyOS, KDE Plasma Wayland), 2026-11-23

**Environment**: CachyOS (kernel 7.3.1-1-cachyos), KDE Plasma 6.8.2 Wayland, Intel Core
i3-10105, NVIDIA GeForce RTX 3050 (driver 615.78.08), the same commit e3f3dde built as release
with debug assertions. Header `[present] backend=vulkan mode=mailbox`, window maximized
(1920×1034) on the 1080p 120 Hz monitor. The ezcap moved to this PC, NV12 in every mode, with
the PS5 on Astro's PLAYROOM's opening scenes (a starfield and title screens: less detail than
the Windows game, 1080p files of 1.4–1.6 MB, 4K files of 4.0–4.8 MB). Audio paired with the
ezcap's input, output a test null sink (`capx_t027`). Runs of about 30 s; the agent ran them
over SSH and sent the burst's keys with `ydotool` (through the GlobalShortcuts portal binding).

| Mode | Run | Seconds | `recv` | Lost | `repl` | `ready_ms` median | Max > 3 | Worst max | Worst `gap_ms` | Audio gap |
|---|---|---|---|---|---|---|---|---|---|---|
| 1080p60 | baseline | 27 | 59–60 | 0 | 0 | 0.8 | 0 | 1.0 | 17.0 | 0 |
| | every 1000 ms | 27 | 59–60 | 0 | 0 | 0.8 | 0 | 1.4 | 17.3 | 0 |
| 1080p120 | baseline | 27 | 117–118 | 0 | 0 | 0.8 | 0 | 1.1 | 8.8 | 0 |
| | every 1000 ms | 27 | 117–118 | 0 | 0 | 0.8 | 0 | 1.0 | 8.8 | 0 |
| | burst of 10 | 26 | 117–118 | 0 | 0 | 0.8 | 0 | 2.3 | 9.7 | 0 |
| 4K30 | baseline | 27 | 29–30 | 0 | 0 | 2.5 | 0 | 2.8 | 34.8 | 0 |
| | every 1000 ms | 27 | 29–30 | 0 | 0 | 2.5 | 1 | 3.2 | 35.5 | 0 |

On Linux the card delivers its nominal rates (117–118 fps at 1080p120, 29–30 fps at 4K30),
where Windows' Media Foundation path gave about 108 and 27–28. The 4K30 `ready_ms` median is
higher than on Windows (2.5 against 1.2–1.4 ms) with or without screenshots: the preview's own
4K path on this PC, not the screenshots.

**Burst** (1080p120): each `ydotool` call takes about 100 ms here, so the 10 presses came
≈ 104 ms apart (Windows: ≈ 45 ms). All 10 were written in order, none refused: each took
≈ 115 ms, so at most one waited and the limit of 4 was not reached (it was reached on Linux in
acceptance check 5, presses 25 ms apart). The burst's seconds (t=11–13) have `ready_ms` max
1.0–2.3 ms and no lost frame. The first try had no keys: `ydotoold` was not running after a
reboot; the run was repeated.

**4K**: one second in the run with screenshots has a frame presented 3.2 ms after it was ready
(2.8 ms worst without screenshots); no lost frame, no audio gap. The late frames seen on Windows
at 4K (up to 9.2 ms) did not appear here.

### SC-001 (key to notice)

From the 103 log lines of the runs above.

| Mode | Taken | RGB | PNG | Written | Copied | Total | Limit |
|---|---|---|---|---|---|---|---|
| 1080p60 | +0–16 ms | 65–70 ms | 13–14 ms | 9–31 ms | 26–30 ms | 116–151 ms | 1 s |
| 1080p120 | +0–8 ms | 65–70 ms | 13–15 ms | 2–13 ms | 26–31 ms | 110–129 ms | 1 s |
| 4K30 | +0–33 ms | 257–275 ms | 45–49 ms | 4–15 ms | 96–103 ms | 419–464 ms | 2 s |

The RGB conversion is about 3 times slower than on the Ryzen 9 9950X3D (i3-10105, 4 cores); it
runs on the worker, off the preview's path.

**Verdict (Linux)**: SC-001 passes with a wide margin. SC-003 passes in all three modes: no
frame lost or replaced, the spread unchanged, no audio gap.
