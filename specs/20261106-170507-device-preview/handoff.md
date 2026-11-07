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
  (product.md F54, roadmap #15, last spec). Spec, research R5, data-model, contracts, quickstart,
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

## Open questions

- R7 is untested on the CachyOS PC: if the NVML request does not hold the clocks, the fallback
  (small keep-alive GPU work) needs the maintainer's decision.
- Windows error mapping for "in use" vs "failed" is a heuristic (R6), tested in acceptance
  (quickstart US4: card open in another app); if it misreads, Windows shows the generic Failed
  state instead (maintainer, 2026-11-06), no Media Foundation code of our own.
- The ezcap's Windows identity (USB serial or port-based) is unknown until it is plugged into the
  Windows PC (`Get-PnpDevice`); without a serial its choices only follow it on the same port
  (accepted by the maintainer).

## Next step

1. T064 (research R16) first, as the maintainer asked (2026-11-07): the stack spike is built in
   `target/spike-stack` (git worktree of the tag, `git worktree remove target/spike-stack` when
   done) with `target/spike-c920.bat` and `target/spike-test-video.bat` to compare; compare in
   release (`cargo build --release`), debug draws the UI much slower.
2. `/speckit-implement` from T023 (Phase 3, US1). Linux GPU power (R7) is validated at T025 and
   system notifications (R15) at T061, each before it is wired in.
