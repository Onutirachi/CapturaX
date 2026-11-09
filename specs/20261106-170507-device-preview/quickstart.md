# Quickstart: Device Preview

How to check that the device preview works and meets its budget. Results go in
`measurements/` (latency.md, stalls.md, acceptance.md). Build and run instructions are in the
[README](../../README.md).

## Prerequisites

- Windows 11 PC (RTX 5070 Ti, C24FG7x 144 Hz G-SYNC as preview monitor, 27G2G4 on the card's
  passthrough) and the CachyOS PC (RTX 3050, 1080p 120 Hz), as in the spike and the app shell.
- ezcap Live Gamer Ultra with the PS5; a UVC webcam (Logitech C920).
- 240 fps camera (Galaxy S23 FE), `tools/latency` with `uv`.
- Windows: PresentMon 2.3.1 (elevated prompt) for the stall investigation.
- Release builds for every measurement.

## Automated checks (CI)

`cargo fmt --check`, `cargo clippy`, `cargo test` on Windows and Linux. Covers: mode parsing,
format ranking, automatic modes, duplicate removal and badges (pure, from recorded caps of the
ezcap and the C920 kept as test data); device identity and numbering;
settings additions (lenient read, defaults, 32-entry cap); range detector on
synthetic luma; the latency estimate; the preview state machine with a fake session; pipeline
tests with `videotestsrc` (NV12 frames reach the sink, `captured` ≤ `ready`, `set_range`
without restart, no-signal after 5 s with a source that never produces); boundary test now also
over `crates/capture`.

## First checks

1. `capturax --list-devices`: the ezcap (and the webcam) with their modes; the shown automatic
   modes and what they resolve to; no metadata node on Linux; no Auto source.
2. Start the app with the card connected and no `[preview]` table: it opens the card in Auto max
   FPS (1080p120 on the ezcap); the device select shows it with its badge and the detail line.
3. `capturax --stats`: a `[source]` line (format NV12 on the ezcap, range `full(auto)` once the
   PS5 home screen shows), then `[loop]` lines with `lat_ms`.

## Manual acceptance (SC-005, SC-008)

On Windows and on Linux, at 100% and at 150%, in both languages:

1. **US1**: restart the app → same device and mode; play in fullscreen and windowed; color range
   Auto shows `Auto (Full)` and the image matches the passthrough; Full and Limited change it at
   once.
2. **US2**: device menu lists the card and the webcam with their name and aspect tags, no screens; mode menu shows
   Automatic (with resolved modes, no duplicates) and Modes largest first; switching modes and
   devices shows video within 2 s (SC-004); each device keeps its own mode and range; with the
   device menu open, plugging and unplugging the webcam adds and removes it within 3 s (SC-004).
3. **US3**: bubbles show source, mode, frame rate, latency; in the 1080p120 mode with the PS5 at
   60 Hz the frame rate reads about "111 / 120 FPS"; the hover settings button of each bubble
   offers its three displays; the choice survives a restart.
4. **US4**, one by one:
   - unplug every device → "No devices", the device menu shows the same state; plug the card →
     it opens;
   - Windows: Settings → Privacy → Camera → desktop apps off → "Access blocked", **Open
     privacy settings** opens the page, turning it back on and **Try again** plays;
   - Linux: `sudo chmod 000 /dev/video0` → blocked state naming the node (restore afterwards);
   - open the card in another app first (OBS or `gst-launch-1.0`) → "In use", **Try again** after
     closing it plays;
   - unplug the card while playing → "disconnected"; plug it back → video within 3 s, same mode
     (SC-004); moved to another USB port, a card without a serial opens with default choices;
   - card connected without HDMI signal → "No signal" after 5 s;
   - notices (FR-027): with the window focused, plug the webcam → in-app "C920 connected" with
     **Open**, which opens it; unplug it → "C920 disconnected", the × closes it and so does a
     click on the balloon; plug and unplug quickly → one balloon with the latest message; unplug and plug the open card →
     "disconnected", then "reconnected" with no button; repeat with another app focused, the
     window minimized and hidden to the tray → system notifications, **Open** shows the window
     and opens the device; each within 3 s (SC-005).
5. **US5**: Settings → Video → Presentation: Follow the monitor → on the G-SYNC monitor the
   cursor stays smooth with the app focused; `--stats` header `mode=fifo-follow`, `rep` > 0;
   back to Lowest latency → `mode=mailbox`; the device is not reopened (no Opening state).
6. Close to the tray → the webcam's light turns off; show the window → it reopens. Minimize and
   restore → video at once. Sleep and wake the PC → the preview comes back.
7. Windows: snap the window to the left half, F11, F11 → back in the left half (FR-025).
8. Two identical devices, if available: numbered names, separate modes. Otherwise record as not
   checked.

## Latency budget (SC-001 to SC-003, SC-007)

Glass to glass with `tools/latency` (README there), PS5 browser showing `flash.html` at 1080p60,
passthrough and preview filmed together at 240 fps, 25 s clips, ≥ 20 samples, chrome idle:

| Run | Platform | Mode | Window | Budget |
|---|---|---|---|---|
| L1 | Windows | 1920 × 1080 · 60 | fullscreen | median ≤ 32, p95 ≤ 35 (SC-001) |
| L2 | Windows | 1920 × 1080 · 60 | windowed | same |
| L3 | Windows | Auto max FPS (1080p120, the default until T029) with the PS5 at 60 Hz | fullscreen | recorded (cost of the 60 Hz signal in a 120 mode) |
| L3b | Windows | Auto max FPS with a 120 Hz source (PS5 at 120 Hz, or the PC's HDMI output) | fullscreen | recorded |
| L4 | Linux, default power settings | 1920 × 1080 · 60 | fullscreen | median ≤ 35, p95 ≤ 40 (SC-002) |
| L5 | Linux | 1920 × 1080 · 60 | windowed | same |
| L6 | Linux, PowerMizer request off (comparison) | 1920 × 1080 · 60 | fullscreen | recorded |

```sh
uv run tools/latency/analyze.py --measurements specs/20261106-170507-device-preview/measurements \
    video clips/<clip>.mp4 --path capturax --variant <run> --mode 1920x1080@60:NV12 --platform windows
```

- With each clip keep the `--stats` log: SC-003 reads `ready_ms` (median ≤ 1.5 every second, max ≤
  3 in 95%) and `pres` (59–61 in 99%) across the app shell's interface states (idle, chrome,
  menu, settings panel, language and theme switches), windowed and fullscreen, both platforms.
- SC-007: set `C_platform` (data-model.md "Latency estimate") from L1 and L4, then check that the
  latency bubble's Total is within 5 ms of the clip medians, and `lat_ms` within 1 ms of the
  bubble's measured value.
- Linux GPU (research R7), before L4: run the NVML call alone and watch `nvidia-smi
  --query-gpu=pstate,clocks.gr,clocks.mem,pcie.link.gen.current,power.draw -lms 500`; then with the
  app: P0 while previewing, back to the idle state after closing. Record `power.draw` previewing
  with and without the request (FR-012).
- Optional levers (research R3, R4): a 120 Hz source through the PC's HDMI output into the card,
  the preview monitor's variable refresh; recorded if tried.

## Windows stalls (SC-006)

Per research R8, on the native 1080p monitor with the card at 1080p60:

1. `capturax --stats --present-log pm-app.tsv` and PresentMon as in research R8, together.
2. Ten focus changes (click another window, click back) and ten fullscreen entries and exits,
   then wait 30 s in fullscreen idle.
3. Repeat with G-SYNC off for capturax.exe; with MPO off (`OverlayTestMode=5`, reboot); with
   `OverlayMinFPS=0`; with `WGPU_DX12_PRESENTATION_SYSTEM=visual`.
4. Ten language switches with the present log (stage lines).
5. Record in measurements/stalls.md: per run the stalls over 50 ms, the `PresentMode` around
   each, and the conclusion. SC-006 passes when the cause is identified and either fixed (no
   stall over 50 ms in 10 tries each) or accepted by the maintainer.
