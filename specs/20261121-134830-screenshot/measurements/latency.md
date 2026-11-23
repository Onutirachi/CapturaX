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

## Linux

To do: the ezcap is on the Windows PC. The Linux checks of T026 timed a test-video 4K30
screenshot at ≈ 350 ms (rgb 267–280 ms on the i3-10105).
