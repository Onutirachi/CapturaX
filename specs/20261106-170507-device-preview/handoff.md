# Handoff: Device Preview

**Updated**: 2026-11-07 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261106-170507-device-preview`

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

## Open questions


- Windows error mapping for "in use" vs "failed" is a heuristic (R6), tested in acceptance
  (quickstart US4: card open in another app); if it misreads, Windows shows the generic Failed
  state instead (maintainer, 2026-11-06), no Media Foundation code of our own.
- The ezcap's Windows identity (USB serial or port-based) is unknown until it is plugged into the
  Windows PC (`Get-PnpDevice`); without a serial its choices only follow it on the same port
  (accepted by the maintainer).

## Next step

1. T065 (default mode 1080p at about 60 fps, FR-008 changed after T029): code in
   `crates/capture/src/mode.rs` (`default_choice(signal, &[Mode])`) and `app/src/preview.rs`
   (`mode_to_open`), after the maintainer approves it; smoke run with the ezcap on Windows
   (`target/t028/capturax.toml` without its `mode` line). Then Phase 4 (US2, T030 onward).
   System notifications (R15) are validated at T061 before they are wired in.

Linux PC notes: `ydotoold` does not survive a reboot; start it as in the app-shell handoff before
any GUI automation (started again for T027, 2026-11-07). The T027 copy is `~/capx/t027` (its own
`capturax.toml`, keep_in_tray on; may be left running). The clone is on a detached `8f191a7`. In the windowed preview the
toolbar shows only with the pointer near the top of the window (`glide 960 200`), not over the
middle of the video. The before/after binaries are in `~/capx/t064-before` and
`~/capx/t064-after` (can be deleted). The spike worktree `target/spike-stack` was removed (its
commit stays on `feat/20261026-154808-stack-spike`).
