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
