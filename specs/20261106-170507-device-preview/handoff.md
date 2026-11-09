# Handoff: Device Preview

**Updated**: 2026-11-08 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261106-170507-device-preview`

## State

Spec, plan and tasks approved and committed. Implementation started (2026-11-06):

- **Phase 1 (T001–T004)** committed: `build: add capture crate` (38c843d).
- **Phase 2 (T005–T016)** validated by the maintainer (test video and `--list-devices` on
  Windows) and committed: `feat: add capture crate and yuv render path`.
- **Phase 3**: T017 (pure preview state machine, `app/src/preview.rs`) committed after the
  checks (`feat: add the preview state machine`); not wired yet (`#[allow(dead_code)]` on the
  module in main.rs until T018).
- **T018–T019** validated by the maintainer (start device, remembered device from the file) and committed (`feat: open the start device in the preview`). The app
  starts the monitor, opens the start device through `Preview`, saves a user choice on
  `Playing`, closes on hide to tray (also when started hidden by the system) and reopens on
  show; the inbox drops frames of other sessions (`Inbox::set_session`) and clears the video
  layer on close (`Inbox::clear_video`); `preview-state`/`state-title`/`state-detail` replace
  `source-open` in main.slint, the opening state spins `loader_2`. Smoke run on Windows: the
  start order picked the phone's virtual camera ("cam" heuristic), first frame 43–47 ms after
  the choice.
- **T020–T022** validated by the maintainer and committed (`feat: add the toolbar and the color
  range choice`, 0bd3803): badge, rich menu items, toolbar with the **Faixa de cor** dropdown in
  the spike's look (maintainer asked for it: research R13 updated), color range wired.
- **New task T064** (research R16, maintainer 2026-11-07): UI presents between slow video
  frames. With the C920 (30 fps) the chrome animates at 30 fps and, with G-SYNC, the cursor moves
  at 30 Hz, because UI presents ride on video frames (app shell R3/T041); the spike stays smooth.
  Release vs debug and Vulkan vs DX12 were ruled out by the maintainer (lag only with the camera,
  also on Vulkan).
- **T064** implemented and validated on Windows by the maintainer (chrome and cursor much
  smoother on the C920 with G-SYNC, a few slight animation hitches, expected), committed
  (`feat: present ui changes between slow video frames`). Windows runs in
  measurements/latency.md: idle unchanged, `ready_ms` better while the chrome animates (C920
  13 → 2 seconds over 3 ms, test video 4 → 0). The ezcap runs on CachyOS (2026-11-07, 1080p120,
  no HDMI input) show no change, as expected at 120 fps; T064 is checked.
- T064 notes: `Inbox::set_session(Some((id, fps)))` takes the mode's rate; `Inbox::set_refresh`
  is sent from `App::update_refresh`; `Pace` keeps the median of the last 15 arrival intervals;
  `PRESENT_MARGIN` is 2 ms; a frame later than expected + refresh + margin counts as late and
  the UI change goes alone; without a period the old 50 ms `FLOWING` rule still applies.
  Measuring note: a copied exe in its own folder starts with an empty `capturax.toml`
  (portable), so copy the settings to keep the remembered device.
- **T023** done: `[source]` line with `--stats`. `stats::Source` (device, id, mode, and format
  and matrix from `SessionEvent::Playing`) is kept in `App::source` for the open session; the
  line is printed on each `SessionEvent::Range` of the current session (the session sends it
  right after `Playing` and on each change), through `render::print_stats` (stdout and log).
  `(auto)` when the range choice is Auto. Not printed for the test video (no device, its events
  only go to the log). Smoke run on Windows: `[source] device="HD Pro Webcam C920" id="mf:…"
  mode=1920x1080@30 format=NV12 matrix=bt601 range=full(auto)`: the C920 declares BT.601 at
  1080p (the declared matrix wins over the 720p rule, as designed).
- **T024** done and committed (`feat: add the gpu performance request`): `platform::gpu_performance(pci_bus_id)
  -> PerformanceGuard` in `crates/platform/src/gpu.rs`, NVML in `crates/platform/src/linux/nvml.rs`
  (loaded with `libloading`; struct `{currentMode, mode, supportedPowerMizerModes}` checked against
  nvidia-ml-py 13.615 and the symbols of driver 615), Windows no-op in `windows/mod.rs`, example
  `crates/platform/examples/gpu_performance.rs` (`[seconds [pci bus id]]`, default 60 s). Already
  preferring maximum performance → left alone, nothing restored; a PCI id not found → first GPU;
  failures logged once per run (info when the library or a symbol is missing, warn when refused).
  fmt, clippy and tests pass on Windows and CachyOS.
- **T025 run by the agent over SSH** (2026-11-07, recorded in measurements/latency.md, logs in
  raw/t025/): P8 210/405 MHz Gen1 6 W → P0 1777/7001 MHz Gen3 23 W within ~1 s, held 60 s, back to
  P8 right after release; no root. R7 holds, no fallback needed. The request is per process
  (nvidia-settings cannot override it, nor the app theirs; research R7), so dropping the guard just
  withdraws ours. Validated by the maintainer; T025 checked.
- **T026** validated by the maintainer and committed (`feat: hold the gpu performance request
  while previewing`):
  `render::Feed { session, fps, gpu_performance }` replaces the `(id, fps)` pair of
  `Inbox::set_session` (devices ask, the test video only with `--stats`); the video thread
  acquires `Performance` (wraps `platform::PerformanceGuard`, logs "gpu performance requested
  (adapter …)" / "gpu performance released") right after a video frame is presented, so the first
  frame is not delayed by loading NVML, and drops it outside the inbox lock when the session
  closes, the window hides or the thread stops; `Gpu::pci_bus_id()` from wgpu's
  `AdapterInfo::device_pci_bus_id` (empty → None). fmt, clippy, tests pass on Windows; smoke run
  on Windows (test video `--stats`, DX12): requested with adapter `0000:01:00.0`.
- **T027** validated by the maintainer and committed (`docs: record us1 acceptance checks`),
  results in measurements/acceptance.md, Linux logs in measurements/raw/t027/. Windows (C920,
  maintainer by hand): with the phone's virtual camera on, the start order opened it (no "cam" in
  its name; heuristic kept by the maintainer, 2026-11-07), so the C920's id was pasted into
  `[preview] device` (no device menu until T031); restart, range switches, fullscreen pass.
  CachyOS (ezcap, agent over SSH): first start → 1080p120 NV12 in 393 ms, `Auto (Completa)`,
  Full/Limited at once and stored per device, F11/Esc, P0 while previewing and P8 when hidden
  with the requested/released lines; with the PS5 on, Limited crushes the dark gray and clips the
  yellow, Auto picks Full and matches the passthrough (maintainer). Not checked: the device select
  with badge (T031), `lat_ms` (T036), 150 % and English, the ezcap on Windows. A SIGTERM ends the
  app without the "quit" / "released" lines (the driver drops the per-process request anyway).
- **T028** validated by the maintainer and committed (`docs: record the latency budget runs`).
  Windows (ezcap moved to the Windows PC, portable copy `target/t028` with its own
  `capturax.toml`): L1 28.7 / 31.7 ms, L2 29.7 / 32.3 ms (SC-001 pass), preview on the left
  Samsung at 143.98 Hz (FreeSync not taken as G-SYNC), passthrough on the right G-SYNC monitor.
  The Windows logs show stalls of 50–675 ms; a check (`raw/t028/focus.txt`) proved they are the
  focus-change stalls (none in 70 s alone, one per focus change), left to T052 (SC-006). The
  ezcap's Windows id is `mf:\\?\usb#vid_32ed&pid_3201&mi_00#9&37ee7830&0&0000#…` (USB serial
  `00000001`; whether the id follows it to another port is still unchecked).
  Linux half: L4 30.1 / 34.4 ms, L5 29.6 / 33.4 ms
  (SC-002 pass), L6 without the GPU request 37.9 / 46.0 ms; the request costs about +16.5 W while
  previewing (24.7 W at P0 vs 8.2 W at P8). Mode pinned with `mode = "1920x1080@60"` under the
  ezcap's `[devices."…"]` (no mode menu until T031). L6 used the cfed7fe build (T064, before the
  request) in `~/capx/t028-off`; L4/L5 the 8f191a7 build in `~/capx/t028`. Clips
  `clips/t028-l{4,5,6}.mp4` need `--fps 240` (the phone stores 29.1). SC-003: windowed passes in
  every interface state (`raw/t028/ui.txt`, maintainer clicking); fullscreen with UI animating
  reaches max ~13 ms, as in the app shell, plus ~4 s after F11: **accepted as a known limit by
  the maintainer**, investigate only if it becomes a problem (spec SC-003, latency.md).
  `flash.html` is also served from the Linux PC (`python3 -m http.server 8000` in `~/capx/t028`),
  but the PS5 uses `http://192.168.0.10/tools/latency/flash.html` (the Windows PC).
- **T029** done (2026-11-07): L3 (Auto max FPS 1080p120, PS5 at 60 Hz) 34.0 / 39.6 ms, +5.3 ms
  of median over L1; L3b (the CachyOS PC's HDMI output into the ezcap at 1080p 119.88 Hz,
  `flash.html` in Firefox `--kiosk`; the PS5's 120 Hz cannot be held) 31.5 / 36.1 ms, 17
  samples. **Default changed by the maintainer**: Auto source when shown, else 1920 × 1080 at
  58–62 fps when the device has it, else Auto max resolution (spec FR-008, research R3,
  data-model, product F3, new task T065). The ezcap stays on the Windows PC; it goes back to
  CachyOS at T034 (batch the Linux checks then). The CachyOS HDMI cable goes back to its monitor
  (KDE remembers each display's setup).
- **T065** done: `default_choice(signal, &[Mode])` (Auto source; else 1920 × 1080 at its highest
  rate within 58–62 fps, 60 before 59.94; else Auto max resolution), passed the modes by
  `mode_to_open`; tests on the hand-built and the recorded caps (ezcap → 1080p60 NV12, C920 →
  Auto max resolution). Smoke run on Windows (`target/t065`, no `mode` line): the ezcap opened
  at `mode=1920x1080@60 format=NV12`, nothing written to the settings file.
- **T030** done and committed after the checks (pure, not wired; `feat: add the device and mode
  menu models`): in app/src/preview.rs `device_items(devices, open, &Settings)` and
  `mode_items(device, &ModeChoice)` return `Item { text: Label, detail: Option<Detail>, badge,
  current, disabled, pick: Option<Pick> }`. Catalog texts are named, not written (`Label::
  NoDevices / Automatic / Modes / Auto(choice)`, `Label::Plain` for names and mode labels;
  `Detail { automatic, mode }` reads "Auto max resolution · 1920 × 1080 · 30"), so T031 maps
  them through texts.slint. `mode_choice(device, stored)` (stored choice if the device can open
  it, else the default) now backs `mode_to_open`; T031 passes it to `mode_items`. Marking: the
  exact choice, else the row opening the same mode in the same group (a hidden duplicate
  Auto max FPS marks Auto max resolution on the C920; a stored `@60` marks a 59.94 mode).
  New `capture::recorded::device(text)` reads a `gst-device-monitor-1.0` block (name, caps,
  identity, modes); the capture `modes.rs` test and the preview menu tests use it on the
  recorded C920 and ezcap. fmt, clippy, tests pass on Windows.
- **T031** implemented and validated by the maintainer (2026-11-07), committed: **Device** and **Mode** selects in front of Color range (toolbar.slint, min width
  96 px in the compact layout, 140 px in Float); `Preview::menus(&Settings)` gives both models
  (mode menu empty with no open device, from `mode_choice` of the stored text);
  `App::show_menus` runs after every preview event, compares with the last menus and only then
  sets the Slint models (`ui::menu_item` maps `Label`/`Detail` through new `Texts` entries:
  `modes-automatic`, `modes-native`, `mode-auto-*`, `mode-auto-detail` "{} · {}"; pt-BR "Auto
  fonte", "Auto resolução máxima", "Auto FPS máximo", "Modos", "Dispositivo", "Modo"); an open
  menu follows new items through `Menus.update` (Select `changed items`), closes when its select
  is disabled or left empty. Picks come back as row indexes (`UserEvent::DevicePicked` /
  `ModePicked`) mapped to `Item::pick`; a device pick already sends `Event::ChooseDevice` (choosing
  the open device again does nothing, new test); a mode pick is only logged until T032, and the
  menus are reset after each pick (the select marks the picked row itself). With no open device
  the Device button shows only its title (no placeholder). fmt, clippy, tests pass on Windows;
  smoke run opened the C920 with no new warnings.
- **T066** implemented and validated by the maintainer (2026-11-07), committed: `crates/capture/src/tags.rs` (55 standards, name tag rule, aspect snapped within 3%,
  ratio family; re-exported from `mode`); `Mode::label` replaced by `size_text` ("1920 × 1080")
  and `text` (settings text "1920x1080@60", used by `--list-devices`, the session log and the
  tests); `Badge`/`badge` removed. `Item` carries `rate` (number text) and `tags`; `MenuItem`
  has `rate`, `tag`, `tag-kind` (the family as "16:9" or "custom"), `aspect`; `badge.slint`
  became `tag.slint` (`Tag { value, kind }`, kind a family, `custom`, `aspect` or `rate` for the
  outlined tag; its property is `value` because `text` shadowed `accessible-role: text`). Menu
  rows: text then the rate tag (on the detail line when there is one), tags on the right; the
  select button: text, rate tag (when no detail line) and name tag. Theme tokens `tag-1-1` …
  `tag-32-9`, `tag-custom`, `tag-aspect` (contrast test); texts `rate` ("{} FPS") and
  `custom-resolution` ("Custom"); "4K", "QHD", "Full HD", "HD" left the catalog.
  `--list-devices`: `1920x1080@30    NV12  [Full HD] [16:9]`, aligned per device.
- **T067** implemented and validated by the maintainer (2026-11-07), committed: the video thread keeps a `Fade` (150 ms) and the session of the frame on the GPU;
  `Inbox::clear_video` now fades the last frame out, then clears it (`last_frame` reset, so the
  fade steps do not wait for frames that will not come); a frame of a new session fades in from
  0; fade steps without a video frame are presented one refresh apart, riding on video frames
  when one is near (the R16 rule); the opacity goes in the video layer's uniform (the shader
  already had it). Opening shows no `StateMessage`: an empty area over the video area and, after
  1 s (a Slint `Timer`), `ProgressRing` (components/progress-ring.slint, a `Path` arc with the
  mockup's spinner B motion); "Opening {}" and `loader_2.svg` removed. `ready_ms` unchanged:
  test video 1080p60, debug builds of HEAD and the working tree alternated twice, median 1.3–1.4
  and max 1.7–2.3 ms on both. The ezcap is on the Windows PC (the app opened it at 1080p60).
- **T068–T070** implemented and validated by the maintainer on Windows (2026-11-07, after a
  softer shadow, half on the light theme, and an accent-filled segment with 8/6 px corners; the
  shadow is now Always by default), committed (`feat: add the video backdrop settings`). T068: `segmented.slint` (`Segmented { options, current, label }`, radio group,
  arrows/Home/End, accent-text dot), the **Preview** category (icon `device_tv.svg`, Tabler) with
  Video background and Video shadow as segmented rows (stacked under the name when narrow) and
  Watermark as a switch row (an on/off setting, like General; tasks and research updated);
  `settings::Background`, `settings::Shadow`, `Preview.watermark` (default true, custom
  `Default`). T069: `build.rs` writes `watermark.a8` (1024² coverage of brand/X.svg); `Kind::Mask`
  (shader kind 6, tint in `to_r`); `Scene.watermark` (RGB, opacity 0.06; white on the dark theme
  and on black, black on the light theme); the watermark and shadow are drawn under a scissor of
  the video area; `Scene.backdrop` is black with Black; `StateMessage` has no fill and takes
  `on-black` (tokens `ground-black`, `on-black-text`, `on-black-muted`, also the progress ring);
  `App::apply_backdrop` on each change. T070: `Kind::Shadow` (shader kind 7, distance to the
  video's rounded rectangle, two Gaussian shadows as the mockup's CSS, in logical pixels times
  `Scene.shadow` = the display scale), faded with the video; only with Always for now. Checks:
  fmt, clippy, tests (run with `CARGO_TARGET_DIR=target/check`: the maintainer had the app open);
  the shader compiles (a run with black, watermark and Always); `ready_ms` with the backdrop on vs
  off, 1080p60 test video alternated twice: medians 1.1–1.4 vs 1.2–1.7 ms, no measurable cost.
- **T032** implemented and validated by the maintainer (2026-11-07), committed (`feat: open
  and store the picked mode`):
  `Event::ChooseMode(ModeChoice)` and `Command::SaveMode { id, name, choice }` in preview.rs;
  `Open` keeps the mode it opened in. A mode pick stores the choice when it differs from the
  stored text (so a stored mode the device lacks is replaced only by a pick, rule 2) and reopens
  the same device only when the choice resolves to another mode (a choice opening the mode
  already playing is stored without a reopen, no fade); the reopen keeps the open's `chosen`
  flag (picking a mode of a fallback start device does not remember the device).
  `open_device(id, chosen, mode, ..)` takes the picked mode; a device with no mode now also
  closes the previous session (it was left running). The app logs "mode … chosen" and writes
  `devices."<id>".mode`. Tests: device pick with stored mode/range and defaults, mode pick
  reopen + store + menu mark, same mode stored without reopen, stale stored mode, no device or
  unoffered mode, a pick while opening, the remembered device plugged in while another plays.
  fmt, clippy, tests pass on Windows (`CARGO_TARGET_DIR=target/check`).
- **T033** implemented and validated by the maintainer (2026-11-07, all cameras disabled in
  by turning off "Let desktop apps access your camera"), committed (`feat: open the device menu from the empty states`): the
  `no-source` and `no-devices` state messages show a primary action **Choose device**
  ("Escolher dispositivo"), id `open-devices`; `ui.rs` routes it to `AppWindow.open-device-menu()`,
  which (unless the settings panel is open) shows the chrome at once (`reveal-chrome`, no slide,
  so the menu anchors where the select stands) and calls `Toolbar.open-device-menu()` →
  `Select.open-menu()` (now public); the menu takes the keyboard and gives it back to the Device
  select when it closes. With no devices the menu shows the one disabled "No devices" row.
  Disconnected's action is T045's. fmt, clippy, tests pass on Windows
  (`CARGO_TARGET_DIR=target/check`). The only way the maintainer found to reach NoDevices on
  Windows was Privacy & security → Camera → "Let desktop apps access your camera" off (disabling
  in Settings → Cameras or Device Manager left the phone's virtual camera listed): with it off
  Media Foundation lists no device. After T043 that shows Blocked (NoDevices checks
  `camera_access()` first), so NoDevices on Windows will need the cameras really gone.
- **T035** committed after the checks (`feat: add the latency estimate and bubble texts`):
  `app/src/latency.rs` with `Platform { Windows, Linux }` (passed in; T038 gets it from the
  platform crate, no `cfg` in app), `C_WINDOWS`/`C_LINUX` = 5 ms (`Platform::calibration`),
  `estimate`, `latency_text` (whole ms rounded to the nearest; Total rounds the sum once) and
  `fps_text` (nominal through `mode::rate_text`). Not wired: `#[allow(dead_code)]` on the module
  in main.rs until T038.
- **T036** committed after the checks (`feat: add the measured latency to the stats`):
  `Stats::new(now, lines)` always runs on the video thread; `presented(captured, ready, now)`
  records `presented − captured`; without `--stats` only the presented count and the latency are
  kept (the other line fields are skipped). `report` returns `Report { summary: Summary {
  presented, latency_ms }, line: Option<String> }`; `[loop]` ends with `lat_ms=<median>/<max>
  rep=<n>` (`repeated` stays 0 until T047 counts it). `next_report()` is `None` without
  `--stats` while idle (no wakeups), so the summary goes while frames are presented and once
  more (empty) after they stop. `VideoThread::start(.., on_second: OnSecond)`: the app's closure
  sends `UserEvent::Second(Summary)`, stored in `App::second` (`#[allow(dead_code)]` until
  T038). Smoke run on Windows (test video 1080p60): `lat_ms=2.8/4.3 rep=0`; without `--stats`
  the app got one summary per second (60 frames, ~2.8 ms).
- **T037** validated by the maintainer and committed (`feat: add the info bubbles`):
  `app/ui/components/info-bubbles.slint` with `InfoBubbles` (source, mode with its name tag,
  frame rate, latency; a bubble with an empty text is left out; a row, or a column when the row
  does not fit; source at most 360 px, elided), placed in main.slint in the video area's bottom
  right, 20 px from the edges (the row packed against the right edge, a column right-aligned;
  **maintainer, 2026-11-07: the default corner is the spike's bottom right**, not the task's top
  left; spec FR-016, research, plan and T037 updated), fading with the chrome (`1 - hidden`), Tab
  order after the toolbar; `info.engaged` keeps the chrome shown while the pointer is on a bubble,
  its controls or its menu. Hover controls: global `BubbleControls` + `BubbleControlsLayer` in the
  top layer (under menus and tooltips), a 48 px raised panel 8 px above the bubble, aligned to its
  right edge, with the
  `adjustments_horizontal` button ("Display"), after 120 ms of hover, hidden 250 ms after the
  pointer leaves (the gap counts), as the spike. The menu has a header ("Latency", "Frame rate")
  then the three displays, the current one marked. Keyboard (not in the task, FR-020): the frame
  rate and latency bubbles take the focus and Enter, Space, Up or Down open the menu. The latency
  tooltip (500 ms hover or focus, on the left side so it does not cover the controls) follows the
  display: measured only, measured + estimated on two lines, or total. Theme tokens `info-bubble`
  (#0000008c) and `info-text` (#ffffff), same in both themes; contrast pair over white and black
  video. Main window properties for T038: `info-source`, `info-mode`, `info-mode-tag`,
  `info-mode-tag-kind`, `info-fps`, `info-latency`, `latency-display`/`fps-display` (0, 1, 2 in
  the settings' order, default 1) and their `-changed` callbacks. Preview
  `ui/previews/info-bubbles-preview.slint` (row and column over white/black). Nothing shows in
  the app until T038 feeds the texts. fmt, clippy, tests pass on Windows.
- **T038** validated by the maintainer and committed (`feat: feed the info bubbles`):
  `App::show_info` (after each preview state shown, each `UserEvent::Second`, each
  display change) sets the bubbles while the state is Playing, else empties them: name and mode
  (`size_text`, name tag as the menus map it) from `App::source`, frame rate and latency only
  once a second with frames came (`App::second` is reset on `close_session`). Estimate:
  `latency::Platform::current()` from the new `platform::IS_WINDOWS`, source period from the open
  mode, `App::monitor_refresh` (the monitor's reported rate, else 60 Hz; the pacing `refresh`
  keeps its 16 ms default), `vrr = false`. Displays: `latency::{latency,fps}_display/_index`
  (0, 1, 2, unknown → default, tested), set from `[overlay]` at start, `SettingChange::
  LatencyDisplay`/`FpsDisplay` store them and show at once. `InfoBubbles` keeps its own copy of
  the frame rate and latency texts, taken only while active (or when emptied): before that the
  hidden bubbles' changes drew one UI image a second (`ui=1` with the chrome hidden), now `ui=0`.
  Smoke run on Windows (C920, `--stats`, 16 s): 1080p30 NV12 at 24 FPS (low light), `lat_ms`
  ~3.6, no new warnings (the NDI SDK error is GStreamer's NDI plugin, unrelated). Not checked by
  hand yet: the look in the app, hover controls, menu, tooltip, saved choice after restart.
- **Maintainer's check of T037/T038 (2026-11-07)**: liked it; asked for (1) the bottom right
  corner (done, above), (2) the hover controls centered on the bubble, as the spike (done, kept
  inside the window), (3) mode bubble displays: **FR-017a** (spec, data-model, settings-file
  contract, T037/T038 updated): Resolution / + name / + aspect ratio / + name + aspect ratio,
  default all (maintainer's pick). `settings::ModeDisplay` (`size`, `size+name`, `size+aspect`,
  `size+name+aspect`) in `[overlay] mode`, read/write tests; `latency::mode_display/mode_index`
  (0–3); the mode bubble got the menu (header "Mode"), the aspect tag (`info-mode-aspect` from
  `Tags::aspect`) and `mode-display` (the bubble picks its tags itself; Rust only stores the
  choice). Checks pass; validated by the maintainer in the app and committed with T037/T038.
- **Latency tooltip reworded** (maintainer found it confusing, 2026-11-07; picked "with the
  numbers"): each part of the bubble with its value, left-aligned (`Tooltip` text is now
  left-aligned; single-line tooltips look the same): Measured "3 ms: inside CapturaX
  (measured)"; Measured + estimated adds "~24 ms: capture card and monitor (estimated)"; Total
  starts with "~27 ms in total" then both lines. `latency::LatencyParts` (measured, estimated,
  total texts; `latency_text` builds on it) → `info-latency-measured/-estimated/-total`.
  Validated by the maintainer and folded into both commits; `ui=0` with the chrome hidden.
- **T041** committed after the checks (`feat: read the camera privacy switch`):
  `platform::{Access, camera_access, open_camera_privacy}` in `crates/platform/src/camera.rs`,
  `windows/camera.rs` (four `Value`s of the webcam consent store read with `RegGetValueW`; pure
  `decide`: any `Deny` → Denied, none readable → Unknown, else Allowed; `ShellExecuteW` on
  `ms-settings:privacy-webcam`, a failure logged), `linux/camera.rs` (Unknown, no-op). This PC:
  three `Allow`, HKLM `NonPackaged` missing → Allowed. Not used by the app until T044; not built
  on Linux yet (trivial, checked in the Linux batch).
- **T042** committed after the checks (`feat: map device errors`): `session::map_error(windows,
  ErrorCode, message, debug, since_playing, still_listed, Access) -> DeviceError` (pure, R6
  branches; `ErrorCode::of` reads the GStreamer domain/code; `node_in` takes `/dev/...` from the
  message or debug text); `Session::open(.., context: ErrorContext { windows, access }, ..)`, the
  app passes `platform::IS_WINDOWS` and a closure over `platform::camera_access()` (so T044's
  "access passed to the mapping" is done); `since_playing` counts from the call to
  set_state(Playing); "still listed" is a fresh `gst::DeviceMonitor` probe at error time
  (`monitor::listed_now`), not the running monitor's list, which lagged in the T033 case. The
  mapped error and its inputs are logged. NoSignal: the open worker waits after Playing and sends
  it when no frame came within 5 s (`NO_SIGNAL_AFTER`). `Session::test_without_frames`
  (doc-hidden, `num-buffers=0`) for the pipeline test (5.5 s). Until T043 the preview still shows
  every error as Failed, so the one visible change is a device that never sends a frame: Failed
  after 5 s instead of the progress ring forever. Smoke run on Windows (phone's virtual camera)
  normal. Linux build not checked yet (no platform code in the change).
- **T043** committed after the checks (`feat: complete the preview states`,
  2026-11-08): `app/src/preview.rs` maps each `DeviceError` to its state (Blocked keeps the
  Linux node: `PreviewState::Blocked { device: Option<String>, node: Option<String> }`, `device:
  None` = no device listed while access is denied); `Again { device, mode, chosen }` keeps the
  device of an error or Disconnected state, so Try again and a return reopen it **in the same
  mode** and still save a user choice once it plays (`resume` for the tray uses the same struct,
  so the tray also keeps the mode now); a device of an error state that leaves → Disconnected;
  the open device removed while access is Denied → Blocked (waits for it like Disconnected);
  `Preview::with_access(reader)` (the app passes `camera_access`) is read only when no device is
  listed or one leaves; `Preview::waiting()` for T044's 3 s timer; `Event::Relisted { devices,
  timer }` adds/removes what the monitor missed, and only with `timer` reopens a Disconnected
  device that is still listed (the re-read right after an error must not loop); every session
  error sends `Command::Relist` (the app reads `Monitor::devices()` now; the T033 open question);
  `Event::Resumed` reopens the open device at once and, until it plays, its errors →
  Disconnected (data-model diagram updated). 20 new tests; fmt, clippy, all tests pass on
  Windows. Smoke run: with the ezcap open in another program the app showed **In use**
  (`mapped to InUse ... since playing 34 ms`: R6's heuristic right in a real case), and once
  freed it played (first frame 466 ms). T044 got the wake detection and the timer's re-read
  (tasks.md).
- **T044** implemented, checks pass (fmt, clippy, all tests on Windows), **not committed: waits
  for the maintainer's test** (no smoke run: a release `capturax.exe` was running, single
  instance). `Monitor::devices()` was the started monitor's cached list (GStreamer returns a
  started provider's cache), so it became `Monitor::relist(on_listed)`: a probe of its own
  (`listed_now`) on a `device-relist` thread, the monitor's `known` list takes the result
  (contracts/capture.md, research R1 updated). App: `UserEvent::Relisted { devices, timer }`,
  one re-read at a time (`relisting`, a request meanwhile queued in `relist_queued`); a 1 s tick
  in `about_to_wait` (`TICK`, `ControlFlow::WaitUntil`): a wall-clock gap over 10 s
  (`WAKE_GAP`; on Linux `Instant` excludes suspend, so the wall clock; the first tick only sets
  it, so a slow start is not a wake) → `Event::Resumed`; while `Preview::waiting()` a
  `Relisted { timer: true }` every 3 s, the first one 3 s after the state starts waiting (no
  reopen loop faster than 3 s). `Event::Resumed` now also reopens the device of an InUse,
  NoSignal or Failed state (its error came in the second before the tick noticed the wake;
  data-model's "any → wake → Opening"), new test. State changes are logged in
  `preview_event` as `preview state <before> -> <after> (<reason>)` (`Event::reason`). The
  state action `open-privacy` calls `platform::open_camera_privacy()` (the button comes with
  T045). Known limit: a wall-clock step over 10 s (manual change) or a main-thread stall that
  long reads as a wake and reopens the open device once.
- **T044 tests on Windows (2026-11-08)**: the running monitor stopped reporting adds and removes
  after the first session error. **Cause found**: GStreamer shares one provider per factory, a
  second `DeviceMonitor` takes the provider bus's sync handler and clears it when dropped
  (research R1). T042's "still listed" probe (`monitor::listed_now`) and the first T044 relist
  made a second monitor. Fixed: `listed_now` reads the running monitor (a static `RUNNING`),
  a monitor of its own only when none runs; `Monitor::relist` reads it on a thread. The GST log
  run confirmed it (first unplug reported, none after the first probe). **Retested and accepted
  by the maintainer (2026-11-08)**: three unplugs of the playing C920 (one 1.6 s after its first
  frame) each logged `device removed` / `device added`, went straight to Disconnected and played
  again by itself; with the ezcap playing, the C920 leaving and coming back was heard too.
  Committed as `feat: wire the device timers and wake`. Not tried on hardware yet: sleep and
  wake, and starting with no device then plugging one in.
- **Maintainer, 2026-11-08**: on Windows any error after Playing was asked for is mapped 1 s
  later (also within the 2 s In use window: an unplug there read as In use) (`LISTED_AFTER` in session.rs, on a `session-error` thread; the last frame stays), so an
  unplug goes straight to Disconnected instead of showing Failed for a second (research R6).
- **State icons** (maintainer, 2026-11-08, picked on
  https://claude.ai/artifact/8bpmiFnSGKFnXyReHEyTsc, private): as planned except Disconnected
  `plug_connected_x` (same as the disconnect notice); Blocked keeps `lock` on both platforms.
  Titles and detail lines chosen by the maintainer, listed in research R13 (English by the
  agent); T045 updated.
- **T045 implemented (2026-11-08), waiting for the maintainer's check**, not committed: each
  state has its icon (Tabler `lock`, `player_pause`, `antenna_bars_off`, `plug_connected_x`
  added; the folder's LICENSE covers them), title and detail of research R13 (`ui::state_message`
  takes `platform::IS_WINDOWS` for Blocked's texts; Linux names the node, else the device) and
  actions: No source / No devices / Disconnected **Choose device**; Blocked on Windows **Open
  privacy settings** + **Try again** (`privacy-settings` property); the other errors **Try
  again** (`try-again` → `AppWindow.try-again()` → `UserEvent::TryAgain` → `Event::TryAgain`).
  **Device select in an error state** (decided here, maintainer to confirm): the toolbar keeps
  the state's device (`Preview::shown_device`), the Device select marks it, and while it is
  listed the mode menu shows its modes (`modes-reachable`, separate from `device-open`, which
  still gates Color range); a mode picked in an error state opens the device in it and stores
  the choice. data-model "Preview state" texts and quickstart US4 brought in line with R13.
  New tests: an error state keeps its device in the menus, a mode picked in an error state opens
  it. fmt, clippy, tests pass on Windows (`CARGO_TARGET_DIR=target/check`).
- **T045 first check (2026-11-09)**: the maintainer tested a stale release build (from before
  T045; the states were mapped right, the texts were the provisional ones). The log showed three
  real bugs, fixed (release rebuilt 2026-11-09 17:24, waiting for a new check): (1) a flash of the
  shell's "No source" message before the first list: main.slint starts as `opening` and
  `show_preview_state` shows `opening` until `Preview::listed()` (`no-source` only when the
  monitor cannot start); (2) a device picked while camera access is off failed as Failed: the
  `set_state` failure no longer fails the session when the bus error is being mapped
  (`shared.mapping`), so Windows maps it to Blocked after reading access; (3) access turned back
  on left "No devices" until a restart: a monitor started while access was denied never lists
  the devices again, so on the 3 s tick, when access went from Denied to not denied and no device
  is open, the app drops the monitor and starts a new one (a new `Listed`, `Event::reason`
  "monitor list"). (2) and (3) need hardware to confirm.
- **T045 retested (2026-11-09)**: the maintainer found the states working. The Device select was
  blank while the open device was disconnected (the menu has only listed devices); the
  maintainer chose to name it in the button, muted, without a menu row: `Preview::missing_name`
  → `device-missing` → `Select.shown` + `shown-muted`. Release rebuilt 17:43; accepted by the
  maintainer and committed (`feat: give each preview state its texts and actions`).

Nothing pushed.

## Done in the last session

- Phase 1: crate skeleton, boundary test over `crates/capture` (+ `libloading` forbidden outside
  platform), CI installs `gstreamer1.0-plugins-base`; the libloading ban in deny.toml was left
  out (wgpu-hal, ash, glutin, dlib already depend on it). Caps recorded:
  `crates/capture/tests/data/c920-windows.txt`, `ezcap-linux.txt` (`ezcap-windows.txt` when the
  ezcap is at the Windows PC).
- Phase 2: `frame.rs` (planes, pool, `captured_at`), `mode.rs` (modes, automatic modes, choice
  text, badges), `device.rs` (identity, numbering, start order), `analysis.rs` (range detector),
  `monitor.rs`, `session.rs` + `test_source.rs` (appsink copying planes into pooled frames),
  settings `[preview]`, `[overlay]`, `[devices."<id>"]`, YUV composite (NV12, I420, YUY2, UYVY,
  BGRx, RGBA; matrix and range as uniform rows), the test video through the capture crate in
  NV12 (`app/src/test_video.rs` deleted, `app` no longer depends on the gst crates),
  `--list-devices`.
- Checks: `cargo fmt`, `clippy -D warnings`, `cargo test` pass on Windows and on CachyOS.
  `--list-devices` lists the C920 (Auto max resolution → 1080p30 NV12; YUY2 where it beats the
  Frame Server's NV12) and the ezcap on Linux (Auto max FPS → 1080p120 NV12). Test video
  1080p60: Windows `ready_ms` 0.6–0.7 median, max ≤ 1.1 after warm-up; Linux 0.7 at first, then
  2.6 after a few idle seconds (GPU clocks dropping: what T024–T026 address).
- Implementation notes (not spec changes): `modes_from_caps(caps, declared_full_nv12)` gets
  `device.api == "mediafoundation" && nv12_declared_full(caps)` from the monitor; `Playing` is
  sent when the appsink receives its caps (the device accepted the format), so the no-signal
  timer (T042) starts there; `Device` has `base_name` (for renumbering) and `gst:
  Option<gst::Device>` (`None` only in tests); a decoded MJPG mode goes through `videoconvert`
  (passthrough when the decoder gives an uploaded format).
- T017 notes: `Preview::handle(event, &Settings)` reads the stored device choices from the
  settings passed in (no copy kept); events cover monitor, session, device choice, Try again,
  hidden/shown (minimized sends nothing); the mode and range choice events come with their
  tasks (T022, T031) so no unused variant exists. Removing the open device already gives
  `Disconnected` + `Close` (reconnect is US4).
- T018–T019 notes: error states other than Disconnected show the provisional "Cannot open
  <name>" until T045; items kept for later tasks carry `#[allow(dead_code)]` naming the task;
  `Preview::name(id)` keeps the names of devices seen this run (a state names a device that
  left).
- T020–T022 notes: `MenuItem` has `disabled` instead of the plan's `enabled` (a Slint struct
  field left out defaults to false, so the existing menus stay enabled); keyboard skipping of
  headers and disabled items is the Rust callback `Menus.next-selectable` (`ui::next_selectable`,
  Slint functions cannot loop). Badge fills are palette tokens (same in both themes) so the
  contrast test checks them. Toolbar: a panel band with titled selects (titles hidden in the
  compact layout); pt-BR "Faixa de cor": Auto, Completa, Limitada. The session sends
  `Range(in_use)` right after `Playing` (the initial value for the button) and on each change;
  `Session::set_range` applies to the next frame; `Preview` stores the choice through
  `touch_device(..).color_range`.
- The Linux clone (`~/Projetos/CapturaX`) is on a detached commit made from the working tree
  (`refs/tmp/wip`); its old app-shell copies were stashed ("pre-device-preview copies").

## Decisions

- All recorded in spec.md and research.md. Main plan choices: new crate `crates/capture`; YUV
  planes uploaded as they come and converted in the shader (range switches live); Auto source not
  shown on UVC cards (none reports its input, R3) but kept for devices that do; the default is
  Auto max FPS (maintainer, 2026-11-06; 1080p120 on the ezcap, whose cost with a 60 Hz source is
  measured, L3); a card without a serial moved to another port is a new device (maintainer); Linux GPU kept awake through
  NVML PowerMizer, per process, no root (R7, to validate first); Windows stalls investigated with
  PresentMon and a present log before any fix (R8); a G-SYNC driver profile would be a maintainer
  decision.

- **No HDCP hint** (maintainer, 2026-11-06): FR-020 removed. Black video with a signal also comes
  from a console that is off, RCA adapters and VHS tapes, so it is not a reliable sign of HDCP; no
  black-image detection. HDCP, color range, modes and the bubbles go into a getting started guide
  (product.md F54, roadmap #16, last spec). Spec, research R5, data-model, contracts, quickstart,
  tasks, product F11 and roadmap #3 updated.

- **AMD and Intel GPUs** (maintainer, 2026-11-06): FR-012 is covered and measured on NVIDIA
  only; AMD and Intel are a known limit (no hardware, known controls need root), to be measured
  when the maintainer has one (spec FR-012 and Assumptions, research R7).

- **No switch back to the remembered device** (maintainer, 2026-11-06): when the app started on
  another device because the remembered one was missing, plugging the remembered one in only adds
  it to the menu; the user picks it. Reconnect of the open device (F4, FR-014) is unchanged.
  Spec edge case, data-model and T032 updated, with the other `/speckit-analyze` fixes (FR-018
  wording, US4 reference, T028/T033 references, SC-004 list timing in T034 and quickstart).

- **Device notices** (maintainer, 2026-11-06): reconnect (FR-014) kept; a notice says when a
  device is connected, reconnected or disconnected, so the user is not left waiting on a device
  that will not come back (for example the ezcap back on another USB port, a new identity). In
  the app when the window has focus, a system notification otherwise, **Open** on a newly
  connected device. FR-027, US4 scenario 6, SC-005, product F4, research R15 (Windows toast for a
  portable app via a per-user `AppUserModelId` key, validated first at T061; Linux freedesktop
  notifications through `zbus`), tasks T060–T063 inside Phase 6. New direct dependency: `windows`
  0.62 (already in the tree) in `crates/platform`. In-app notice: one balloon drawn by the app
  (no toast library), only the latest message, × button, closes on any click.

- **GPU request for the live and Accelerate GPU setting** (maintainer, 2026-11-07): not in this
  spec. Recorded as product.md F55 and roadmap #15 (the getting started guide moved to #16, still
  last), after a usable app with a working live: measure whether waking the GPU helps a live with
  the preview hidden, and an Accelerate GPU setting (Auto: whenever needed except on battery,
  Off: never, On: whenever needed). Research R7 points to it. Tooltips on the more complex
  settings go with the getting started guide (F54, roadmap #16).

- **Mode tags, "FPS" and the switch fade** (maintainer, 2026-11-07, chosen on a mockup page:
  https://claude.ai/artifact/XuSNprmZCbdSX3DeEpqo3P, private): mode text `1920 × 1080 • 60 FPS`,
  "FPS" upper case everywhere; two tags per mode, a name tag by the old CapturaX rule (nearest
  standard at or above, else `Custom` in both languages) over a completed standards list, and an
  aspect ratio tag (gcd, snapped to a listed ratio within 3%); the rate as an outlined tag next
  to the size, `1920 × 1080 [60 FPS]` (variant F1, no separator character), and `--list-devices`
  printing each mode's settings text (`1920x1080@60`) in aligned columns with the tags in brackets; our own fills per ratio family, all
  white text; a fade of about 150 ms out and in on device and mode switches and at start, the
  Fluent progress ring alone after 1 s of opening (no text). Spec FR-002, FR-007, FR-013, FR-017,
  data-model "Mode" (rules, fills, list), research R3, R13, new R17, product F43,
  contracts/capture.md, new tasks T066 (tags) and T067 (fade).

- **Video backdrop** (maintainer, 2026-11-07, chosen on a second mockup page:
  https://claude.ai/artifact/5vWgGWi4i6VvkMk4Vr8YiD, private): Video background Theme (default) or
  Black; Watermark on by default (the X, as tall as the video area, bottom right, cut 12% by the
  right edge, 6% soft, 12% over the ambient light); Video shadow Always (default since the check) / With ambient light
  / Off; a segmented control for these choices. Product F56–F58 (roadmap #3), F9 and F38
  notes (the live page gets the same backdrop, spec #10 decides who chooses), spec FR-028 to
  FR-031, data-model "Video backdrop" and settings, contracts/settings-file.md, research R18,
  tasks Phase 4b (T068–T071); T048 now only adds Presentation to the Preview category of T068. The category is named
  **Preview** in both languages (maintainer, 2026-11-07), not Video: recording and live will
  have video settings of their own.

- **Toolbar and docks at any size** (maintainer, 2026-11-09, option D of the mockup page
  https://claude.ai/artifact/SSttGFEnjq4oXXKe3wubc5): the toolbar becomes a dock (floating, top
  center); every toolbar dropdown gets an icon; they show icon + title + value while all fit
  (176 px up to a maximum, growing with the room), else all icon only; the side docks stay in
  the middle of the sides in the compact layout (no longer in the toolbar's row; app shell
  FR-009 revised); the bubbles keep one row, between the docks when they meet; dropdowns at
  most 305 px (the toolbar stops growing at 1280); settings moves to the left dock (left = adjustments, right = features, 4 + 4 in
  the future; right dock hidden while empty).
  Spec FR-002a, research R13, plan UI, task T072 (not implemented yet; spec changes
  committed in `docs: turn the toolbar into a dock and fit it at any size`).

## Open questions


- **For T063**: the device notice is planned at the video area's bottom right, above the dock,
  where the info bubbles now are (maintainer moved them there, 2026-11-07). Decide with the
  maintainer where the notice goes (or that it covers the bubbles for its ~5 s).
- Windows error mapping for "in use" vs "failed" is a heuristic (R6), tested in acceptance
  (quickstart US4: card open in another app); if it misreads, Windows shows the generic Failed
  state instead (maintainer, 2026-11-06), no Media Foundation code of our own.
- The ezcap's Windows identity (USB serial or port-based) is unknown until it is plugged into the
  Windows PC (`Get-PnpDevice`); without a serial its choices only follow it on the same port
  (accepted by the maintainer).
- **Found during T033 (2026-11-07)**: turning camera access off (most likely the desktop apps
  privacy switch, the only way that worked; to confirm) while the phone's virtual camera plays
  gives a session error ("Internal data stream error", `streaming
  stopped, reason error (-5)`) but the monitor never reports it removed: no `device removed`
  line, the device stays in the menu, and picking it fails at once ("Failed to start") back to
  Failed. If it was the privacy switch, this is T043's "error while Playing with access now
  Denied → Blocked" edge case and the device staying listed is expected. Not yet checked whether
  a USB device (C920, ezcap) unplugged while playing is reported removed. T042's Windows "not listed → Gone" and T043's 3 s re-read (now only for a missed add)
  depend on the list being right: re-read the list on a session error too, and drop a device the
  re-read no longer has. Also seen: in Failed the Device select shows nothing (`open` is None);
  whether it keeps showing the failed device is for T043/T045.

## Next step

- **T072** (toolbar and docks at any size, FR-002a) is approved in the spec and waits for its
  turn; it can go before T060 if the maintainer prefers.
0. **US4 on Windows**: T041–T044 done (unplug and replug accepted). Still to try on hardware
   when convenient: sleep and wake with a device playing (log `woke from sleep: N s`, the
   device reopens), and start with no device then plug one in. T045 is done and committed. Next:
   T060 (`platform::notify`), then T061 (maintainer validates the Windows toast and the KDE
   notification), T062, T063.
1. US2 is done on Windows; its acceptance T034 and T071's CachyOS half wait for the Linux
   batch. T039 (US3 acceptance, Windows) passed on 2026-11-08 with the CachyOS PC as the 60 and
   120 Hz source (measurements/acceptance.md "US3"). Seen there, outside US3: the ezcap gives
   about 112 FPS at 1080p120 on Windows even with a 120 Hz source (judder the maintainer
   noticed; known since T029), and present stalls of 17–40 ms in 31 of 396 s, for T052. Next:
   T040 on Windows (2026-11-08): the first estimate formula counted one frame
   period too many (Total 44 ms against L1's 28.7 ms; `C_WINDOWS` would have been −10.3 ms), so
   the maintainer approved the corrected formula `(Windows ? F : 0) + C + (vrr ? 0 : R/2)`
   (data-model.md, research R11, measurements/latency.md "T040"). `C_WINDOWS` = 6.3 ms from L1
   with T039's `lat_ms` 2.2; `C_LINUX` = 8.4 ms provisional. The maintainer saw "~28 ms" in
   the Total on Windows at 1080p60 (SC-007 passes on Windows); committed
   (`feat: calibrate the latency estimate`). T040 stays open for Linux. In the Linux batch (T034): a `--stats` run with the
   L4 setup to read `lat_ms` and set `C_LINUX` = 30.1 − `lat_ms` − 4.2, then the Total check
   against L4 and L5. Known: in the 1080p120 mode with a 60 Hz source the Total reads about
   14 ms low (L3), outside SC-007. T048 waits for
   T047 (US5); it adds Presentation as a segmented control. Recording T071's Windows check in
   measurements/acceptance.md goes with the Linux batch at T034.
   The ezcap stays on the Windows PC until T034. System notifications (R15) are validated at T061 before they are wired in.

Linux PC notes: `ydotoold` does not survive a reboot; start it as in the app-shell handoff before
any GUI automation (started again for T027, 2026-11-07). The T027 copy is `~/capx/t027` (its own
`capturax.toml`, keep_in_tray on; may be left running). The clone is on a detached `8f191a7`. In the windowed preview the
toolbar shows only with the pointer near the top of the window (`glide 960 200`), not over the
middle of the video. The before/after binaries are in `~/capx/t064-before` and
`~/capx/t064-after` (can be deleted). The spike worktree `target/spike-stack` was removed (its
commit stays on `feat/20261026-154808-stack-spike`).
