# Acceptance: Device Preview

Quickstart "First checks" and "Manual acceptance" ([quickstart.md](../quickstart.md)) per user
story. The latency budget is in [latency.md](latency.md).

## US1 (T027)

"First checks" 2–3 and "Manual acceptance" 1. Not checkable yet, recorded as not checked: the
device select with its badge and detail line (T031, US2) and `lat_ms` in the `[loop]` line
(T036, US3).

### Windows 11 — 2026-11-07

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, preview monitor C24FG7x 144 Hz
G-SYNC, DX12, release build of `feat: hold the gpu performance request while previewing`.
Devices: Logitech HD Pro Webcam C920 and the phone's Windows virtual camera ("S23 FE de Carlos
(Câmera Virtual do Windows)"). Checked by the maintainer by hand; the agent read the log. The
ezcap was not at the Windows PC.

| Check | Result | Notes |
|---|---|---|
| First check 2: start without `[preview]` → the card in Auto max FPS | Pass (rule as designed) | No capture card connected. The virtual camera has no "cam" in its name, so the start order (data-model.md "Start order") took it as a capture card and opened it over the C920, four starts in a row: `1280x720@30 NV12 bt709 range=limited(auto)`, first frame 26–69 ms after the choice. The heuristic was kept (maintainer, 2026-11-07): a first-start guess, the user's choice is remembered. Not saved as the last used device (fallback, not a user choice), as designed. |
| First check 3: `[source]` line with `--stats` | Pass | `[source] device="HD Pro Webcam C920" … mode=1920x1080@30 format=NV12 matrix=bt601 range=full(auto)` |
| US1 1: restart → same device and mode | Pass | No device menu yet (T031) and the virtual camera is hard to turn off, so the maintainer pasted the C920's id from `--list-devices` into `[preview] device`: `start: last used device`, 1080p30 NV12, first frame 365 ms after the choice |
| US1 1: color range Auto / Full / Limited | Pass | Maintainer; switches are not logged |
| US1 1: fullscreen and windowed | Pass | Maintainer |
| US1 1: image matches the passthrough | Not checked | Needs the ezcap and the PS5 |
| 150 %, English | Not checked | |

GPU request: "gpu performance requested (adapter 0000:01:00.0)" after the first frame and
"released" on quit in every run (no-op on Windows, contracts/platform.md). In the C920 run the
camera delivered 15 fps (privacy shutter closed, auto exposure): `recv=15`, `gap_ms` ≈ 80, `ready_ms` 0.8 median,
≤ 1.4 max after the first second.

### CachyOS KDE Plasma Wayland — 2026-11-07

**Environment**: CachyOS, KDE Plasma 6 Wayland, NVIDIA GeForce RTX 3050, one 1080p 120 Hz
monitor, Vulkan, release build of `feat: hold the gpu performance request while previewing`
copied to `~/capx/t027` with no settings file. Device: ezcap Live Gamer Ultra. Driven by the agent
over SSH (`ydotool`, `spectacle` screenshots, `nvidia-smi` once per second); raw logs in
[raw/t027/](raw/t027/).

| Check | Result | Notes |
|---|---|---|
| First check 2: start without `[preview]` → the card in Auto max FPS | Pass | `start: first capture card`, 1920x1080@120 NV12, first frame 392–393 ms after the choice |
| First check 3: `[source]` line with `--stats` | Pass | `range=limited(auto)` from the declared range at `Playing`, then `full(auto)` once the detector ran |
| US1 1: restart → same device and mode | Pass | Same card and mode (start order, the only device); a stored range (`full`) applied on restart |
| US1 1: color range Auto / Full / Limited | Pass | Button `Auto (Completa)`; Limitada and Completa apply at once (`[source] range=limited`, `range=full`, no new `Playing`), saved in `devices."<id>".color_range`; back to Auto → `full(auto)` |
| US1 1: fullscreen and windowed | Pass | F11 and Esc, toolbar in both |
| GPU request (T026) | Pass | P8 210 MHz 5.8 W → P0 1777/7001 MHz 24 W within a second of the first frame, held while previewing; hide to tray → "gpu performance released", "PowerMizer back to Adaptive", P5 then P8 within 2–4 s |

The first runs had no HDMI input (the card's own "Please Connect a Video Source" image, pure
black and white, where the range barely shows). With the PS5 on a PlayStation Store page (bright
colors, dark gray background), screenshots of the windowed preview:

| Choice | `[source]` | Dark background (left margin, luma min / mean) | White button | Yellow frame |
|---|---|---|---|---|
| Auto | `full(auto)` | as Full | | |
| Full | `full` | 5 / 9.7 | (253, 255, 255) | (246, 221, 35) |
| Limited | `limited` | 0 / 0.0 | (255, 255, 255) | (255, 239, 27) |

Limited crushes the dark gray to black and clips the yellow, as expected for a full-range source
read as limited; Auto picks Full. The maintainer checked by eye that the preview matches the
passthrough in Auto (and the screenshots above): Pass.

Not checked on CachyOS: 150 %, English.

The ezcap on Windows (T027 asks for it "when it is at the Windows PC") is left for the next time
the card is there; the range behaviour on Windows was confirmed by the maintainer with the C920.

## US3 (T039)

"Manual acceptance" 3, with the agent comparing the bubbles with `--stats` (SC-007, first two
parts). Logs in [raw/t039/](raw/t039/): `run2.txt` (the checks), `run3.txt` (after the restart).

### Windows 11 — 2026-11-08

**Environment**: as US1 on Windows (RTX 5070 Ti, preview on the C24FG7x 144 Hz G-SYNC, windowed,
DX12), release build of `feat: feed the info bubbles` (d28c777). Device: ezcap Live GAMER ULTRA.
Source: the CachyOS PC's HDMI output into the ezcap, 1920 × 1080 at 60 Hz, then 119.88 Hz (KDE
display settings), instead of the PS5: any 60 Hz source serves the "111 / 120" check, and the
PC can also hold 120 Hz. Checked by the maintainer by hand, with screenshots; the agent read the
log at the screenshot times (clock of the log file's start).

| Source → card mode | Frame rate bubble | Latency bubble | `[loop]` at the same seconds | Result |
|---|---|---|---|---|
| 60 Hz → 1080p60 | `60 / 60 FPS` | `2 ms + ~42 ms` | `pres=60`, `lat_ms=2.2–2.3` median | Pass |
| 60 Hz → Auto max FPS (1080p120) | `112 / 120 FPS` | `2 ms + ~25 ms` | `pres=111–112`, `lat_ms=2.1` | Pass |
| 119.88 Hz → 1080p120 | `111 / 120 FPS` | `2 ms + ~25 ms` | `pres=111–112`, `lat_ms=2.1–2.2` | Pass |

The estimate matches the formula with `C_WINDOWS` = 5 ms (before T040) and the 144 Hz monitor:
2 × 16.7 + 5 + 3.5 = 41.8 ms at 60 FPS, 2 × 8.3 + 5 + 3.5 = 25.2 ms at 120 FPS.

| Check | Result | Notes |
|---|---|---|
| Four bubbles: source, mode, frame rate, latency | Pass | Bottom right of the video area, with the chrome |
| Latency displays Measured / Measured + estimated / Total, tooltip | Pass | Maintainer: every choice marked, applied at once |
| Frame rate displays Presented / Presented / nominal / Nominal | Pass | Maintainer |
| Mode displays (resolution, + name, + aspect ratio, + both) | Pass | Maintainer |
| Choices after a restart | Pass | Saved as `[overlay] latency = "total"`, `fps = "nominal"`, `mode = "size+aspect"`; after quit and restart: `~27 ms` (`lat_ms` 2.2 + 25.2), `120 FPS`, `1920 × 1080 [16:9]` |
| Presented within ±1 of `pres`, measured within 1 ms of `lat_ms` | Pass | See the table above |
| 150 %, English | Not checked | |

**Seen during the run, outside US3** (maintainer: stutters, 120 Hz does not feel like 120 Hz,
the chrome not smooth):

- In the 1080p120 mode the ezcap delivers 111–113 frames a second on Windows (`recv`) with a
  60 Hz and with a 119.88 Hz source, as in T029 (latency.md, L3 and L3b); the app presents what
  it receives (`pres` ≈ `recv`). Eight missing frames a second at 120 Hz are visible as judder.
  On CachyOS the same card gives 111–120 (latency.md, T064). So the quickstart's "about 111 /
  120" with a 60 Hz source also holds with a 120 Hz source on Windows.
- Present stalls: in 31 of 396 seconds `ready_ms` max was 17–40 ms (normally ≤ 1.6), some with
  replaced frames (`repl` 1–3), for example t=115, 136–139, 226, 229–230 and eight seconds in a
  row at t=294–301; some with UI activity, some without. These are SC-006's Windows stalls,
  investigated at T052.
