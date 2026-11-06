# Latency Budget (T041, T050)

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

## Windows 11, settings states (T050) — 2026-11-06

**Environment**: as above (same machine, monitor, G-SYNC setting, release build of
`feat: add language and theme settings` plus the catalog and tests after it). Every run focused,
with the settings panel open. Header in every run: `backend=dx12 mode=mailbox` (FR-003).

- **Menu**: a select's menu opened and closed every 2–3 s, nothing chosen.
- **Language**: English ↔ Português from the panel every 1–3 s.
- **Theme**: Dark ↔ Light from the panel every 1–3 s.

| Run | Seconds | Switches | `ready_ms` median | Max > 3 ms | `pres` 59–61 | Stalls ≥ 100 ms (`ready_ms` max) |
|---|---|---|---|---|---|---|
| 1080 window menu | 56 | — | 1.0–1.1 | 0 | 56 | |
| 1080 window language | 63 | 15 | 1.1–9.9 | 10 | 58 | 643, 600, 962, 339 ms |
| 1080 window language (repeat) | 68 | 55 | 1.1–9.8 | 8 | 63 | 729, 623, 779 ms |
| 1080 window theme | 64 | 64 | 1.1–1.2 | 2 | 64 | |
| 1080 fullscreen menu | 68 | — | 1.0–1.2 | 2 | 68 | |
| 1080 fullscreen language | 69 | 22 | 1.1–9.7 | 8 | 65 | 171, 388, 358, 696 ms |
| 1080 fullscreen theme | 73 | 71 | 1.1–1.2 | 0 | 73 | |
| 1440 window menu | 73 | — | 1.1–1.2 | 2 | 73 | 313 ms |
| 1440 window language | 90 | 35 | 1.1–1.3 | 3 | 90 | 111, 316, 126 ms |
| 1440 window theme | 70 | 69 | 1.2–1.3 | 2 | 70 | 494 ms |
| 1440 fullscreen menu | 72 | — | 1.2–1.5 | 0 | 72 | |
| 1440 fullscreen language | 82 | 73 | 1.1–1.4 | 7 | 80 | 298, 610, 815, 526 ms |
| 1440 fullscreen theme | 109 | 117 | 1.2–1.4 | 5 | 109 | 182 ms |
| **Menu** | **269** | | **all ≤ 1.5** | **4 (1.5%)** | **269 (100%)** | 1 |
| **Theme** | **316** | **321** | **all ≤ 1.5** | **9 (2.8%)** | **316 (100%)** | 2 |
| **Language** | **372** | **200** | **362 of 372 ≤ 1.5** | **36 (9.7%)** | **356 (95.7%)** | 19 |

**Language switch stalls.** About one switch in ten stalls the video for 110–960 ms. The language
was already on screen (the `language switch` line, 1.1–4.2 ms) before each stall. The main thread
stops too: the maintainer's clicks during a stall were taken about 2 s apart instead of about
1 s. Every size and mode does it; switching to either language does it. Ruled out with isolated
runs, the app focused and untouched, nothing in the UI changing:
- the window title: `SetWindowTextW` on the window every 3 s, 10 times
  (`raw/win-1080-win-title-test.txt`): no stall;
- the settings save on the HDD that holds this folder: a 300-byte file written and flushed in
  the same folder every 3 s, 10 times, 50–72 ms each (`raw/win-1080-win-flush-test.txt`): no
  stall. Theme switches also save the file, and stall far less often.
Cause unknown. It looks like the Windows stalls of T041 (focus changes, the 625 ms stall with the
panel), which a theme switch (2 in 321) and the menu (1 in 269 s) also showed here.

**SC-005** (from the log): language switch 1.1–4.2 ms (203 switches, check 7 included), theme
switch 2.9–27.8 ms from the panel (321) and 34.6–37.0 ms following a Windows theme change. Met.

### Verdicts (settings states)

- **SC-001**: met for the menu and theme states (median ≤ 1.5 ms in every second, max ≤ 3 ms in
  98.5% and 97.2%). Not met while switching the language (90.3% of the seconds within 3 ms). The
  maintainer accepts it (2026-11-06): the video does not matter while the language is being
  changed; what matters is no stall during real use.
- **SC-002**: met for the menu and theme states (100%); 95.7% while switching the language.
- **SC-005**: met.

### Quickstart checks 1 and 7 (Windows)

- **1**: window shown 396 ms after start, in pt-BR (system language) and dark (system theme,
  setting Automatic). The "no source" state does not exist yet (T057): checked in T059.
- **7**: language and theme switch at once, with an open menu and a visible tooltip included;
  with the theme on Automatic, the app follows Windows to light and back. Checked by the
  maintainer.

## CachyOS KDE Plasma Wayland — 2026-11-06

**Environment**: CachyOS (kernel 7.3.1-1-cachyos), KDE Plasma Wayland, NVIDIA GeForce RTX 3050
(driver 615.78.08), release build of `fix: build the Linux tray without tokio`, test video
`1080p60`. One monitor, 1920 × 1080 at 120 Hz, scale 100%, no variable refresh (`Vrr:
incapable`), so the unfocused runs of the variable refresh case do not apply and there is no
1440p run (DSR is Windows only). Header in every run: `[present] backend=vulkan mode=mailbox
adapter="NVIDIA GeForce RTX 3050"` (FR-003). Window 1280 × 720 at 320,157.

The runs were driven over SSH by the agent: `ydotool` for pointer and keys (real input through
uinput, flat acceleration), KWin scripting to read the window's geometry, `spectacle` for
screenshots (none during the measured runs). Same actions as on Windows: chrome shown from the
top of the video and left every 5 s, the panel opened with the gear and closed with Esc every
3 s, a select's menu opened and closed every 2.5 s, language English ↔ Português and theme
Claro ↔ Escuro every 2 s. GPU clock state from `nvidia-smi -q -d PERFORMANCE` before and after
each run: P8 or P5 (idle clocks) in every run; the driver never raised the clocks for this load.

### Interface states, window still

| Run | Seconds | UI active | `ready_ms` median | Max > 3 ms | `pres` 59–61 | Highest max | GPU |
|---|---|---|---|---|---|---|---|
| 1080 window idle | 37 | 0 | 3.1–6.2 | 37 | 37 | 7.0 | P8 → P8 |
| 1080 window chrome | 38 | 20 | 1.7–6.3 | 38 | 38 | 8.8 | P5 → P8 |
| 1080 window panel | 37 | 26 | 3.1–6.3 | 37 | 37 | 8.1 | P8 → P8 |
| 1080 window menu | 37 | 29 | 3.2–6.3 | 37 | 37 | 7.2 | P8 → P8 |
| 1080 window language | 38 | 33 | 3.1–6.3 | 38 | 38 | 8.4 | P8 → P8 |
| 1080 window theme | 37 | 33 | 1.7–6.2 | 37 | 37 | 8.4 | P8 → P5 |
| 1080 window unfocused | 38 | 1 | 1.7–6.2 | 38 | 38 | 6.9 | P8 → P8 |
| 1080 fullscreen idle | 37 | 0 | 3.3–6.2 | 37 | 36 | 38.6 | P8 → P5 |
| 1080 fullscreen chrome | 37 | 20 | 3.1–35.4 | 37 | 28 | 43.8 | P8 → P8 |
| 1080 fullscreen panel | 37 | 27 | 1.7–34.5 | 34 | 24 | 42.9 | P8 → P5 |
| 1080 fullscreen menu | 37 | 28 | 3.2–20.0 | 37 | 37 | 20.2 | P8 → P8 |
| 1080 fullscreen language | 37 | 31 | 3.3–34.3 | 37 | 29 | 42.7 | P5 → P5 |
| 1080 fullscreen theme | 37 | 31 | 1.7–25.6 | 37 | 31 | 42.3 | P8 → P5 |
| **Window** | **262** | | **0 ≤ 1.5** | **262 (100%)** | **262 (100%)** | | |
| **Fullscreen** | **222** | | **0 ≤ 1.5** | **219 (98.6%)** | **185 (83.3%)** | | |

"Unfocused": a click on the desktop at t≈8 took the focus, the app stayed visible: no stall at
the focus change (highest max 6.9 ms), unlike Windows.

**`ready_ms` follows the GPU's idle clocks.** The median steps between about 1.7, 3.1 and 6.0 ms
within a run with nothing changing, the steps of the GPU's clock levels. In fullscreen there are
also stretches of 3–5 s at about 40 presents a second (`repl` about 20, median 27–35 ms, gaps of
25 ms), 37 seconds in all. Three runs repeated with the graphics clock locked
(`sudo nvidia-smi -lgc 1800,1800`, reset with `-rgc` afterwards):

| Run, clock locked | Seconds | `ready_ms` median | Max > 3 ms | `pres` 59–61 | Highest max |
|---|---|---|---|---|---|
| 1080 window idle | 37 | 3.1 | 37 | 37 | 3.3 |
| 1080 fullscreen chrome | 38 | 1.6–1.7 | 36 | 38 | 4.3 |
| 1080 fullscreen panel | 37 | 1.6–1.7 | 33 | 37 | 5.7 |

With the clock locked the 40-presents stretches are gone and the fullscreen median is
1.6–1.7 ms; windowed stays at 3.1 ms, about 1.4 ms more than fullscreen. The memory clock was not
locked (P5 during these runs). Keeping the GPU awake is planned for spec #3 (quickstart); the
windowed difference is new and goes with it.

### Moving and resizing (SC-003)

Real drags with `ydotool` (pointer held down on the title bar or the bottom-right corner), checked
in KWin's window geometry:

| Run | Held still mid-drag | Slow | Fast | Longest gap |
|---|---|---|---|---|
| 1080 window move | `pres` 60, median 6.0–6.2 ms | `pres` 60 (t=4–8) | `pres` 60–61 (t=14–17, 10 strokes of 400–500 px) | 19.7 ms |
| 1080 window resize | — | `pres` 60–61, 1280 × 720 ↔ 983 × 546 (t=5–14) | `pres` 60 (t=14–17) | 19.6 ms |

`moves` stays 0 on Wayland: the compositor moves the window without telling the client.

### Settings states (T050)

- **Language** (SC-005): `language switch` 16.7–21.6 ms windowed (15 switches), 17.2–29.2 ms
  fullscreen (15). No stall at a switch: the Windows language stalls do not happen here.
- **Theme** (SC-005): `theme switch` 20.3–80.4 ms windowed (14), 23.9–72.7 ms fullscreen (14)
  from the panel; 8.0–64.2 ms following a Plasma color scheme change (6, T045 below).

### Theme Automatic follows Plasma (T045)

Theme and language on Automatic, settings panel open; `plasma-apply-colorscheme BreezeLight` then
`BreezeDark`, three times 2 s apart: the portal's `color-scheme` changes, the app logs `system
theme Some(Light)` / `Some(Dark)` (two or three times per change: the portal signals it more than
once, one switch follows) and `theme switch light|dark` each time, and the panel is light then
dark in the screenshots (`raw/linux-1080-win-system-theme-log.txt`). Before the run the session
had no scheme set explicitly (`plasma-apply-colorscheme` called BreezeLight current while the
portal said dark, value 1); it ended on BreezeDark, the same colors.

### Quickstart checks 1 and 7 (Linux)

- **1**: window shown 250 ms after start, in pt-BR (system language) and dark (system theme),
  "Nenhuma fonte" state without a source.
- **7**: language and theme switch at once from the panel's menus (each switch made through an
  open menu); Automatic follows Plasma (T045). The tooltip case was not driven.

### Verdicts (Linux)

- **SC-001**: not met. No second has a median ≤ 1.5 ms: with the GPU at idle clocks (P8/P5) the
  median is 1.7–6.3 ms windowed and up to 35 ms in fullscreen's 40-presents stretches. With the
  graphics clock locked, fullscreen gets 1.6–1.7 ms and windowed 3.1 ms. The cause is the GPU's
  power management, which the quickstart leaves to spec #3. Accepted for the shell
  (2026-11-06, the maintainer left the call to the agent): recorded here, fixed with keeping the
  GPU awake in spec #3, together with the windowed 3.1 ms.
- **SC-002**: met windowed (100%), not met in fullscreen (83.3%): the 40-presents stretches,
  gone with the clock locked.
- **SC-003**: met. The video keeps 60 presents while moving and resizing, slow, fast and held
  still; the longest gap is 19.7 ms.
- **SC-005**: met (all switches under 100 ms).
