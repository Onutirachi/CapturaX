# Latency Budget (T041)

SC-001 to SC-003 as defined in [spec.md](../spec.md) and measured as in
[quickstart.md](../quickstart.md) "Latency budget". Raw `--stats` logs are in [raw/](raw/); the
first 3 seconds (start) and the last line (closing) are left out of every count.

## Windows 11 — 2026-11-05

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, release build, test video
`1080p60`. Main monitor 1920 × 1080 at 144 Hz with G-SYNC set to full screen only; the 1440p
runs use NVIDIA DSR 1.78x (2560 × 1440) on the same monitor. Header in every run:
`[present] backend=dx12 mode=mailbox adapter="NVIDIA GeForce RTX 5070 Ti"` (FR-003). The header's
`surface` is the window's size at start; fullscreen runs switch to the monitor's size after it.

"Focused" runs: the app had focus, so the monitor followed its presents (variable refresh,
research R4). "Unfocused": another window had focus and the app stayed visible, as on a monitor
without variable refresh.

### Interface states, window still

| Run | Seconds | UI active | `ready_ms` median | Max > 3 ms | `pres` 59–61 | Notes |
|---|---|---|---|---|---|---|
| 1080 window idle | 38 | 0 | 1.1–1.2 | 0 | 38 | |
| 1080 window chrome | 59 | 35 | 1.1–1.2 | 1 (4.2) | 59 | |
| 1080 window panel | 57 | 56 | 1.1–1.2 | 1 (4.3) | 57 | |
| 1080 window unfocused idle | 64 | 1 | 1.0–1.5 | 2 | 64 | focus change at t=14: 13.1 ms |
| 1080 window unfocused chrome | 61 | 37 | 1.3–1.5 (8.1) | 2 | 60 | focus change at t=4: **401 ms** |
| 1080 fullscreen idle | 72 | 0 | 1.1–1.2 | 1 | 72 | t=20: 51.4 ms |
| 1080 fullscreen chrome | 52 | 36 | 1.1–1.2 | 1 (4.1) | 52 | |
| 1080 fullscreen panel | 59 | 56 | 1.0–1.2 | 1 (3.7) | 59 | |
| 1440 window idle | 55 | 0 | 1.1–1.2 | 1 (5.2) | 55 | |
| 1440 window chrome | 46 | 25 | 1.2–1.3 | 1 (5.4) | 46 | |
| 1440 window panel | 47 | 45 | 1.0–1.2 | 1 | 47 | t=31: **625 ms**, seen by the maintainer |
| 1440 window unfocused idle | 57 | 3 | 1.2–1.4 | 1 (3.3) | 57 | focus change: no stall |
| 1440 window unfocused chrome | 55 | 36 | 1.2–1.5 | 0 | 55 | |
| 1440 fullscreen idle | 49 | 0 | 1.2–1.3 | 2 | 49 | t=18: 51.7 ms |
| 1440 fullscreen chrome | 78 | 53 | 1.2–1.3 | 0 | 78 | |
| 1440 fullscreen panel | 65 | 64 | 1.2–1.3 | 2 | 65 | t=18: 72.7 ms |
| **All** | **914** | | **913 of 914 ≤ 1.5** | **20 (2.2%)** | **913 (99.9%)** | |

Of the 20 seconds over 3 ms, 12 are isolated 3–5 ms peaks (about one a minute, with or without
UI activity) and 8 are over 10 ms: the focus changes, the fullscreen stalls about 18–20 s after
start (only in the idle and panel runs, never with the chrome moving), and the one 625 ms stall.

Extra logs, not counted: `win-1080-win-idle-click` (a click to focus at t=8: **229 ms**) and
`win-1080-fs-panel-esc` (Esc pressed twice by mistake, fullscreen left and re-entered: 39–54 ms
at each change, and 130 and 210 ms stalls).

### Moving and resizing (SC-003)

| Run | Held still mid-drag | Slow resize | Fast drag | Longest gap |
|---|---|---|---|---|
| 1080 window | `pres` 60, median 1.1 ms | `pres` 59–60, gaps 19–54 ms, median about 4 ms | `pres` 48–49, about 28 ms per present | 101 ms (release) |
| 1440 window (DSR) | — | `pres` 60, gaps ≤ 21 ms, median 3.4–4.4 ms | `pres` 60, median 1.3 ms, gaps 18–19 ms | 21 ms |

The maintainer sees short hitches of the video and the window while dragging and resizing on the
1080p monitor and accepts them (SC-003 relaxed, 2026-11-05).

### Verdicts

- **SC-001**: met. Median ≤ 1.5 ms in 913 of 914 seconds (the exception is the 401 ms focus
  change); max ≤ 3 ms in 97.8% of the seconds (≥ 95% required). The stalls at presentation mode
  changes are recorded apart (research R4).
- **SC-002**: met. 99.9% of the seconds present 59–61 frames.
- **SC-003**: met. Held still mid-drag the video keeps 60; moving and resizing never stall for
  hundreds of milliseconds (longest gap 101 ms).

### Found during the run (fixed before these logs)

- Every UI image was presented on its own, so the next video frame waited for the compositor's
  vsync: up to 14 ms max and 8 ms median while the chrome moved
  (`raw/before/before-ui-ride-win-1080-win-chrome.txt`). UI changes now ride on the next video
  frame and are uploaded as they arrive; UI drawing is paced to the refresh also for input
  (pointer moves asked for up to 413 images a second). Research R3.
- The docks glided into place during a resize (their position was animated, not only the
  hiding), an open tooltip stayed where its button had been, and the focus ring was a dot in
  the control's middle.

### Open

- Stalls at presentation mode changes on Windows (research R4): investigated in spec #3.
- Variable refresh: the cursor runs at the video's rate while the app has focus (research R4,
  F52 for spec #3).

## CachyOS KDE Plasma Wayland

Pending: the same runs on the maintainer's CachyOS PC, with the NVIDIA GPU clock state
(`nvidia-smi -q -d PERFORMANCE`) noted for each.
