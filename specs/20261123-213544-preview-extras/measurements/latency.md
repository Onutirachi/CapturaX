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

Still to do:

- 1080p120: the PS5 cannot be forced to 120 Hz (its 120 Hz output is Automatic or Off; only a
  game with a 120 Hz mode sends it). Measured when such a game runs, on Windows or here.
- Run 3 (bubbles pinned, controls hidden, research R9): after US3.
- Windows: 1080p120 (see above).
- Spec #3's glass-to-glass check once with the ambient light on (maintainer's phone).
