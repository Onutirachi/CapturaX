# Latency (SC-001, T009)

The ambient light must not add latency or dropped frames (FR-004). Each run: the ezcap with the
PS5, windowed 1720 × 720 (bars at the sides), `capturax --stats`, 60 one-second `[loop]` lines
after the first 5 (contracts/stats.md). `ready_ms` per line is median/max; below, the median of
the medians and the p95 and max of the per-second maxima.

## CachyOS (RTX 3050, KDE Plasma Wayland, 120 Hz screen), 2026-11-24

Release build at 76c95f1 (glow tuned: alpha 0.9, gate 0.04–0.12). The agent ran it over SSH;
the PS5 was in Astro's Playroom.

| Mode | Ambient | `ready_ms` median | per-second max, p95 | max | `repl` | `pres` min | `glow` median (min) | `lat_ms` median |
|---|---|---|---|---|---|---|---|---|
| 1080p60 | off | 0.80 | 0.90 | 0.9 | 0 | 60 | 0 (0) | 10.0 |
| 1080p60 | on | 0.80 | 1.00 | 1.5 | 0 | 59 | 59 (55) | 10.0 |

Result: no change in the median, +0.1 ms at the p95 of the maxima, no replaced frames. Within
the budget.

## Windows (RTX 5070 Ti, Windows 11), 2026-11-24

Release build, the PS5 in Astro's Playroom. The agent ran it on the PC.

| Mode | Ambient | Pacing | `ready_ms` median | per-second max, p95 | max | `repl` | `pres` min | `glow` median (min) | `lat_ms` median |
|---|---|---|---|---|---|---|---|---|---|
| 1080p60 | off | – | 0.70 | 1.30 | 1.6 | 0 | 60 | 0 (0) | 2.1 |
| 1080p60 | on | 16.6 ms, alpha 0.9 | 0.70 | 1.20 | 1.4 | 0 | 60 | 39 (36) | 2.0 |
| 1080p60 | on | 15 ms, alpha 1.0 | 0.70 | 1.30 | 1.5 | 0 | 60 | 60 (59) | 2.1 |

Result: no change in latency. With the 16.6 ms minimum the frames' arrival jitter skipped about
a third of the glow updates (39 a second at 60 fps); the minimum went to 15 ms (research R1) and
the glow updates with every frame.

### Run 3: bubbles pinned, 2026-11-26

Release build at c14b820, the PS5 in Astro's Playroom, windowed 1720 × 720, ambient light on
(100 %), the frame rate and latency bubbles pinned (`pinned = ["fps", "latency"]`), the pointer
outside the window so the controls were hidden. The agent ran it on the PC (research R9).

| Mode | Ambient | Pinned | `ready_ms` median | per-second max, p95 | max | `repl` | `pres` min | `glow` median (min) | `lat_ms` median | `ui` |
|---|---|---|---|---|---|---|---|---|---|---|
| 1080p60 | on | fps, latency | 0.60 | 1.00 | 1.4 | 0 | 60 | 60 (60) | 1.9 | 0 |

Result: within the budget and no worse than run 1 (0.70 / 1.30 / 1.6, `repl=0`). The UI did not
redraw at all with the controls hidden (`ui=0` from second 5 on): both pinned texts stayed the
same every second (`60/60` and the latency rounded to whole milliseconds), and Slint redraws
only when a text changes. So the "about 1 per second" of the quickstart is an upper bound; a
pinned bubble costs a redraw only in a second when its text changes. While the controls still
showed (seconds 3 and 4, 31 partial uploads a second), `up_ms` was 0.0 / 0.1, well below a
whole upload.

### Glass to glass, 2026-11-24

Spec #3's method (tools/latency, `summary.md` and `video-samples.csv` here): the PS5 browser
showing `flash.html` at 1080p60, the passthrough on the right monitor (LG 27G2, G-SYNC), the
preview on the left one (Samsung, 143.98 Hz), both filmed at 240 fps (the clips store 29.5 fps:
`--fps 240`). Release build at 86833d7, `[present] backend=dx12 mode=mailbox`, controls hidden;
windowed runs at 1720 × 720 with bars at the sides. The maintainer filmed; the agent ran the app
and the analysis.

The first three runs were made with Windows' multiplane overlays (MPO) turned off: the
maintainer had set `HKLM\SOFTWARE\Microsoft\Windows\Dwm\OverlayTestMode = 5` that day. The
last run is after removing it and restarting.

| Variant | MPO | Window | Ambient | n | Median | p95 | Min | Max |
|---|---|---|---|---|---|---|---|---|
| L2-off-nompo | off | 1720 × 720 | off | 21 | 43.4 | 48.1 | 36.1 | 56.6 |
| L2-on-nompo | off | 1720 × 720 | on, 100 % | 24 | 42.2 | 46.2 | 37.1 | 49.2 |
| L1-nompo | off | fullscreen | off (no bars) | 24 | 29.0 | 32.0 | 26.1 | 32.4 |
| **L2-on** | on | 1720 × 720 | on, 100 % | 23 | **28.9** | **31.6** | 25.3 | 32.1 |

During every run the app's own numbers were the same: `pres=60`, `repl=0`, `ready_ms` 0.7–1.1,
`lat_ms` 2.1–3.1, and `glow=60` with the ambient light on.

Result: the ambient light adds no latency (FR-004, SC-001). With MPO on it is within spec #3's
budget (median ≤ 32, p95 ≤ 35) and matches spec #3's L2 (2026-11-07: 29.7 / 32.3, ambient
light not built yet). With MPO off, on and off are the same (42–43 ms): without overlays a
windowed swap chain cannot be scanned out directly and goes through the desktop compositor,
about 13 ms more; fullscreen still flips directly and matches spec #3's L1 (28.7 / 31.7).

Still to do:

- 1080p120: the PS5 cannot be forced to 120 Hz (its 120 Hz output is Automatic or Off; only a
  game with a 120 Hz mode sends it). Measured when such a game runs, on Windows or here.
- Run 3 (bubbles pinned, controls hidden, research R9): done on Windows at 1080p60; CachyOS still to do.
- Windows: 1080p120 (see above).
