---
description: "Tasks for the device preview"
---

# Tasks: Device Preview

**Input**: Design documents from `specs/20261106-170507-device-preview/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: unit tests for every pure module (in the module's own `#[cfg(test)] mod tests`), the
integration tests the plan names (`crates/capture/tests/modes.rs` with recorded caps,
`crates/capture/tests/pipeline.rs` with `videotestsrc`) and the extended
`app/tests/platform_boundary.rs`. Devices, latency, stalls and window behavior are manual
acceptance (quickstart.md), since hosted CI has no capture device and no GPU.

**Spike code**: "port from the spike" means read the file with
`git show spike/stack-20261026:spike/<path>` and rewrite it into the named file, to the structure
of plan.md; nothing imports from the spike.

**Texts**: every task that adds a user-facing text writes it with `@tr("...")` in a `.slint` file
(texts Rust needs go in `app/ui/texts.slint`) and adds its Portuguese (Brazil) translation to
`app/lang/pt-BR/LC_MESSAGES/capturax.po` in the same task; `app/tests/i18n.rs` must keep passing.

**Icons**: new icons are Tabler (MIT, 24 px stroke, as the shell's), added to
`app/ui/assets/icons/` in the task that first uses them.

**Tasks marked (maintainer)** need the maintainer's hands or machines; the agent prepares them,
gives the exact commands, and records the results in `measurements/`.

**Commits**: at each checkpoint the agent shows what changed and commits only after the
maintainer validates it (Conventional Commits, no AI trailers). Pure or unwired tasks may be
committed after `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`
pass, without waiting for a manual test. Nothing is pushed unless asked. At the end of every
working session the agent updates handoff.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: the new crate exists, is guarded by the boundary test and CI can run its pipeline
tests; recorded caps are available for the mode tests.

- [X] T001 Add the capture crate: in the workspace Cargo.toml add `crates/capture` to `members` and `libloading = "0.8"` to `[workspace.dependencies]` (research R14), and move the `# Media` comment to say the capture crate uses it; create crates/capture/Cargo.toml (package `capturax-capture`, workspace package fields, dependencies `gst`, `gst-app`, `gst-video`, `log`) and crates/capture/src/lib.rs declaring the modules `monitor`, `device`, `mode`, `session`, `frame`, `analysis`, `test_source` (empty files for now); add `capture = { package = "capturax-capture", path = "../crates/capture" }` to app/Cargo.toml; add `libloading.workspace = true` under `[target.'cfg(target_os = "linux")'.dependencies]` in crates/platform/Cargo.toml; `cargo build` passes
- [X] T002 [P] Extend app/tests/platform_boundary.rs to `crates/capture` (research R12): parse crates/capture/Cargo.toml with the same forbidden crate list, and scan `crates/capture/src/` for `cfg(windows)`, `cfg(unix)`, `cfg(target_os` (except `#[cfg(test)]`); also add `libloading` to the forbidden list for every crate but `crates/platform`
- [X] T003 [P] In .github/workflows/ci.yml add the runtime package `gstreamer1.0-plugins-base` to the Ubuntu `apt-get install` list (`videotestsrc`, `videoconvert`, `appsink` for the pipeline tests); the Windows installer already ships them; add `{ crate = "libloading", wrappers = ["capturax-platform"] }` to `[bans] deny` in deny.toml only if `cargo deny check bans` passes with it (wgpu or another dependency may already pull libloading; if so, leave the ban out and say why in the commit)
- [X] T004 [P] Record caps as test data in crates/capture/tests/data/: `c920-windows.txt` (run `gst-device-monitor-1.0 Video/Source` on the Windows PC with the C920, keep the C920's block: name, `caps`, `properties`), `ezcap-linux.txt` (same on the CachyOS PC over SSH, the `v4l2deviceprovider` block of the ezcap) and, when the ezcap is plugged into the Windows PC, `ezcap-windows.txt`; a README.md in that folder says how each file was produced and on which date

**Checkpoint**: `cargo build`, `cargo test` and `cargo clippy --all-targets -- -D warnings` pass; commit `build: add capture crate`.

---

## Phase 2: Foundational

**Purpose**: the capture crate's pure model (frames, modes, devices, range detector), the device
monitor, a session that delivers YUV frames, the render path drawing them, the settings
additions, and the test video moved onto this path. Every story builds on it.

**⚠️ No user story starts before this phase is done.**

- [X] T005 [P] Implement crates/capture/src/frame.rs (data-model.md "Frame"): `PixelFormat` (`Nv12`, `Yuy2`, `Uyvy`, `I420`, `Bgrx`, `Rgba`), `Matrix` (`Bt601`, `Bt709`), `Range` (`Full`, `Limited`); `Frame { width, height, format, planes: [Plane; up to 3] (bytes, stride = row length, rows packed), matrix, range, captured: Instant, ready: Instant, session: u64 }`; `matrix_for(colorimetry: Option<Matrix>, height) -> Matrix` ("unknown → BT.709 at 720p and above, BT.601 below"); `captured_at(now: Instant, clock_time, pts, base_time) -> Instant` = `now − (clock_time − (pts + base_time))`, saturating when the difference is negative (research R11); a `PlanePool` reusing byte buffers (no allocation per frame once warm, "buffers come from a reuse pool"); unit tests for `matrix_for`, `captured_at` (normal, negative, zero) and the pool reusing a returned buffer
- [X] T006 [P] Implement crates/capture/src/mode.rs (pure, research R3), ported from the spike's `capture/src/mode.rs`: `Mode { width, height, fps: (num, den), format: Format }` with `Format` = `Nv12`, `Yuy2`, `Uyvy`, `I420`, `Bgrx`, `Rgba`, `Mjpg`, `Other(String)`; frame rates "compared within 0.5", 59.94 kept as 60000/1001; `modes_from_caps(&gst::Caps, declared_full_nv12: bool) -> Vec<Mode>` with "one per resolution × frame rate, best format", rank "NV12, YUY2, UYVY, I420, BGRx, MJPG (decoded), then any other raw format", ranges giving their ends plus common steps, and the Windows exception (MJPG and NV12 at exactly the same sizes with NV12 declared full range → YUY2 before NV12 at that size when offered); sorted "by resolution then frame rate, largest first"; `Mode::label()` (`1920 × 1080 · 60`, `59.94` when fractional); `ModeChoice` (`AutoSource`, `AutoMaxResolution`, `AutoMaxFps`, `Manual { width, height, fps }`) with `parse`/`Display` for `auto-source`, `auto-max-resolution`, `auto-max-fps`, `1920x1080@60`, `1920x1080@59.94` (unknown text → `None`); `resolve(choice, &[Mode], signal: Option<Signal>) -> Option<Mode>` per data-model.md "Mode choice" ("Largest resolution at 58–62 fps, highest such rate; none at 58+ → largest resolution at the device's highest rate"; "Highest rate at 1080p; else at the largest resolution below 1080p; else at the smallest above"; Manual missing → `None`); `shown_automatic(&[Mode], signal) -> Vec<(ModeChoice, Mode)>` in the order Auto source, Auto max resolution, Auto max FPS, "skipping one that resolves to the same mode as an earlier one", Auto source only with a signal; `default_choice(signal)` ("`AutoSource` when shown, else `AutoMaxFps`"); `Badge` (`Uhd4k`, `Qhd`, `FullHd`, `Hd`) and `badge(height)` ("≥ 2160 4K, ≥ 1440 QHD, ≥ 1080 Full HD, ≥ 720 HD, none below"); unit tests for every rule with hand-built mode lists
- [X] T007 Create crates/capture/tests/modes.rs (depends on T004, T006): parse the recorded caps of crates/capture/tests/data/ with `gst::Caps::from_str` and check the modes, the format chosen per mode (ezcap: NV12 at 1080p120 and 1080p60; C920: YUY2 before the Frame Server's NV12 where both exist), Auto max resolution (C920: 1080p30, not 2304 × 1536 at 2 fps), Auto max FPS (ezcap: 1080p120), the shown automatic list without duplicates and no Auto source
- [X] T008 [P] Implement crates/capture/src/device.rs (research R1, R2): `Device { id, name, modes, signal: Option<Signal> /* always None in this spec */, gst: gst::Device }`; `is_preferred(&gst::Device) -> bool` ("`device.api == "mediafoundation"` or a device type named `GstV4l2Device`"); `identity(props) -> Option<String>` as a pure function over the property values ("`mf:<device.path lowercased>` or `v4l2:<vid>:<pid>:<serial>@<bus_path>`", `/dev/videoN` never used); `numbered(&mut [Device])` ("numbered " (2)", " (3)" when repeated", in list order); `find<'a>(id, &'a [Device]) -> Option<&'a Device>` (exact id only); `start_order(devices, last: Option<&str>, recent: &[String]) -> Option<&Device>` per data-model.md "Start order" ("the last used device if listed; else the most recently used listed device; else the first capture card; else the first webcam", names containing "cam" last, case-insensitive, the choice logged); unit tests with the property values of research R2 (C920 path, ezcap Linux bus path), duplicate names and every start order branch
- [X] T009 [P] Implement crates/capture/src/analysis.rs (pure, research R5): `sample_luma(plane, stride, width, height, format) -> Vec<u8>` taking "every 8th pixel of every 8th row" (for YUY2/UYVY the Y bytes of the packed plane); `RangeDetector` (chosen `Auto`/`Full`/`Limited`, declared range; Auto starts at the declared range, "Limited when undeclared"; "at least 0.1% of samples in one sampling are below 16" → Full until `reset()` on device or mode change; `in_use()`); no black-image detection (spec FR-020 removed); unit tests with synthetic luma for each threshold
- [X] T010 Implement crates/capture/src/monitor.rs (depends on T006, T008; research R1), ported from the spike's `capture/src/devices.rs`: `Monitor::start(on_event: impl Fn(MonitorEvent) + Send + 'static) -> Result<Monitor, Unavailable>` running a `gst::DeviceMonitor` on `Video/Source` with `show_all_devices(true)` on its own thread, keeping only preferred devices, building each `Device` (id, name, modes from its caps, numbered); events `Listed(Vec<Device>)` once at start, `Added(Device)`, `Removed(id)`; duplicate adds of the same id (Windows re-probes) are dropped; `devices()` re-reads `monitor.devices()` (for the 3 s re-read, US4); `stop()` joins the thread, also on drop; every device seen is logged with its id
- [X] T011 Implement crates/capture/src/session.rs and crates/capture/src/test_source.rs (depends on T005, T006, T009; research R4), ported from the spike's `capture/src/video.rs`: `Session::open(device, mode, range: RangeChoice, session_id, on_frame, on_event) -> Session` building `source ! capsfilter(mode) [! decoder] [! videoconvert] ! appsink` on a short-lived worker thread so the call returns at once; the appsink asks for the mode's raw format, `sync=false max-buffers=1 drop=true`; MJPG decoders in the order `nvjpegdec`, `d3d12mjpegdec`, `jpegdec` (first one found); formats outside `PixelFormat` (P010 included) get `videoconvert` to NV12, logged; the appsink callback copies the planes into a pooled `Frame` with its matrix (from the caps' colorimetry), the range in use, `captured` (T005, `do-timestamp` stays false; on Linux the v4l2src warning about bad timestamps is logged) and `ready`, and calls `on_frame`; events `Playing { format, matrix }`, `FirstFrame`, `Range(in_use)`, `Error(DeviceError::Failed(text))` (the detailed mapping comes in US4, T042); every GStreamer error with its debug text logged; `close()` sets Null, idempotent and on drop; `Session::test_video(width, height, fps, session_id, on_frame, on_event)` in test_source.rs with `videotestsrc is-live=true pattern=ball` in NV12 through the same appsink code
- [X] T012 Create crates/capture/tests/pipeline.rs (depends on T011): with `Session::test_video` at 640 × 360 @ 30, NV12 frames reach `on_frame` with two planes of the right sizes, `captured ≤ ready` for every frame, `Playing` and `FirstFrame` arrive in order, `close()` twice is fine; skip with a message (not fail) when `gst::init` fails
- [X] T013 [P] Add the settings of data-model.md "Settings additions" to crates/settings/src/model.rs per contracts/settings-file.md: `Preview { device: Option<String>, recent: Vec<String> /* most recent first, at most 8 */, presentation: Presentation /* lowest-latency | follow-monitor, default lowest-latency */ }`, `Overlay { latency: LatencyDisplay /* measured | measured+estimated | total, default measured+estimated */, fps: FpsDisplay /* presented | presented/nominal | nominal, default presented/nominal */ }`, `devices: Vec<(String, DeviceChoices { name: String, mode: String, color_range: ColorRange /* auto | full | limited, default auto */ })>` ordered most recently used first; `mode` kept as text (the capture crate parses it; an unknown value is read as the default automatic mode by the app); unknown values take their defaults (rule 1); "at most 32 device entries are kept (the least recently used is dropped)" on write (rule 3); `Settings::touch_device(id, name)` moves an entry to the front; version stays 1; unit tests: empty tables, every invalid value, 33 devices → 32 with the oldest dropped, round trip keeping unknown keys, the device id with backslashes and `#` from contracts/settings-file.md as a quoted table key
- [X] T014 Render YUV planes in app/src/render/mod.rs, app/src/render/gpu.rs and app/src/render/composite.wgsl (depends on T005; research R4): the inbox carries `capture::Frame` instead of `VideoFrame` (remove `VideoFrame`); `gpu.rs` keeps one texture set for the current format and size, recreated when they change: NV12 = R8 + RG8 (half size), YUY2 and UYVY = RGBA8 at half width unpacked in the shader, I420 = three R8, BGRx and RGBA as is; a uniform with format, matrix (BT.601 or BT.709 coefficients) and range (full, or limited 16–235 / 16–240); `composite.wgsl` samples the planes and converts to RGB before the existing letterbox and rounded corners; the shader's conversion is a pure function mirrored in Rust for a unit test (`yuv_to_rgb` for black, white and a primary in both matrices and ranges, against reference values); existing render tests keep passing
- [X] T015 Move the test video onto the capture crate (depends on T011, T014): app/src/main.rs and app/src/app.rs use `capture::Session::test_video` with the `--test-video` presets (contracts/cli.md: "now plays through the capture crate in NV12"), frames go to the inbox through `on_frame`; delete app/src/test_video.rs; GStreamer missing → message on stderr and exit 69 as before
- [X] T016 Add `--list-devices` to app/src/cli.rs and app/src/main.rs (depends on T010; contracts/cli.md): starts the monitor, waits for `Listed`, prints for each device its name and id, its modes (label and format), the shown automatic modes and what they resolve to, stops the monitor and exits 0; exit 69 if GStreamer cannot start; unit test for the option parsing

**Checkpoint**: `capturax --test-video 1080p60 --stats` plays the ball through the NV12 path with `ready_ms` within the shell's budget on Windows (quick look, the full budget is T028); `capturax --list-devices` lists the C920 on Windows and the ezcap on Linux (quickstart "First checks" 1); `cargo test` passes on both platforms; maintainer validates; commit `feat: add capture crate and yuv render path`.

---

## Phase 3: User Story 1 - Play the console through the preview (Priority: P1) 🎯 MVP

**Goal**: at start the app opens the right device in its remembered or default mode and shows it
through the YUV path, with the color range dropdown, the GPU kept awake on Linux, and the latency
budget met on both platforms.

**Independent Test**: PS5 on the ezcap; start the app with no `[preview]` table → Auto max FPS;
restart → same device and mode; measure glass to glass (quickstart "Latency budget").

- [ ] T017 [P] [US1] Implement the pure preview state machine in app/src/preview.rs (data-model.md "Preview state"): `PreviewState` with all nine states (`NoDevices`, `Opening(id)`, `Playing`, `Blocked(id)`, `InUse(id)`, `Disconnected(id)`, `NoSignal(id)`, `Failed(id)`, `NoSource`); `Preview::handle(event) -> Vec<Command>` where events are monitor events, session events (tagged with the session number), user choices (device, mode, range, Try again), window hidden to tray / shown, and commands are `Open { device, mode, range, session }`, `Close`, `SaveChoice`, `ShowState`; this task covers start (devices? none → `NoDevices`; some → `Opening` of `start_order`, T008), `Opening → Playing` on `FirstFrame`, any session error → `Failed` (refined in US4), a stale session number ignored, hidden to tray → `Close` and shown → `Opening` again, minimized → nothing, "the last used device is saved when a device reaches `Playing` after a user choice, not when the app falls back to another"; the mode to open: the device's stored `mode` parsed with `ModeChoice::parse` and resolved, unknown or missing → `default_choice` (spec edge case, contracts/settings-file.md rule 2: the stored value is not replaced); unit tests with a fake device list for each transition above
- [ ] T018 [US1] Wire the preview into app/src/app.rs and app/src/main.rs (depends on T010, T011, T013, T017): after the window and the video thread exist, start the `Monitor` (its events and every session event reach the main thread as winit user events through the existing `EventLoopProxy`); run `Preview` commands: `Open` closes the current session first, then `Session::open` with a new session number and `on_frame` writing to the inbox (frames whose number is not the current one are dropped there); `--test-video` skips the device choice (contracts/cli.md); the quit sequence gains, first, close the session and stop the monitor; on `Playing` after a user choice, `store.update` sets `preview.device`, moves the id to the front of `preview.recent` (at most 8) and `touch_device`; hidden to tray / shown and minimized per T017; log each open with the time from the choice to `FirstFrame` (SC-004)
- [ ] T019 [US1] Show the opening state and the shell's `no-source` from the preview in app/ui/main.slint and app/src/ui.rs (depends on T018): replace `source-open` with a `preview-state` property (string id) plus `state-title`, `state-detail`; `Opening` shows the state message with a spinner (a rotating `loader_2` icon, Tabler) and the device's name, no old frame (the video thread clears the video layer on `Close`: add `Inbox::clear_video()` in app/src/render/mod.rs); `NoSource` keeps the shell's icon and title; `Playing` shows the video; the other states show a provisional generic text until US4 (T045) gives them their own
- [ ] T020 [P] [US1] Create app/ui/components/badge.slint, ported from the spike's `Badge` in `layered-preview/ui/components/basics.slint` (colors per badge as the spike, text `4K`, `QHD`, `Full HD`, `HD`), and extend `MenuItem` in app/ui/components/menu.slint with `kind` (`item`, `header`), `detail` (right-aligned text), `badge` (empty or a badge id) and `enabled`: headers are not selectable and keyboard navigation skips them and disabled items; the existing selects keep working with text only; add the badge colors to app/tests/contrast.rs (badge text on its fill at 4.5:1)
- [ ] T021 [US1] Create app/ui/components/toolbar.slint and place it in the toolbar slot of app/ui/main.slint (depends on T020; research R13): the panel the shell left empty, holding a row of selects in the Float order device, mode, color range (this task adds only **Color range**; T031 adds the other two in front of it); the compact layout keeps them in the toolbar's row with shorter labels and no overlaps (shell FR-009); keyboard focus order from the toolbar to the docks; Color range offers Auto, Full, Limited, its button showing `Auto (Full)` / `Auto (Limited)` with the range in use (data-model.md "Color range choice")
- [ ] T022 [US1] Wire the color range (depends on T011, T018, T021): `Session::set_range(choice)` in crates/capture/src/session.rs applies to the next frame without a restart (FR-010); the appsink callback runs the luma sampling and `RangeDetector` once every 500 ms on the streaming thread, never on the video thread (FR-015), and posts `Range(in_use)` on change; the detector resets on device or mode change; the choice is read from and saved to `devices."<id>".color_range` through `Preview`; app/src/ui.rs updates the dropdown label; extend crates/capture/tests/pipeline.rs: `set_range(Full)` changes the next frames' range without a new `Playing`
- [ ] T023 [US1] Add the `[source]` line to app/src/stats.rs and its printing in app/src/render/mod.rs (depends on T022; contracts/stats.md): `[source] device="…" id="…" mode=1920x1080@60 format=NV12 matrix=bt709 range=full(auto)`, printed with `--stats` to stdout and the log when a source starts playing or its range in use changes (`(auto)` when inferred); existing fields unchanged; unit test for the exact line
- [ ] T024 [P] [US1] Implement `gpu_performance(pci_bus_id: Option<&str>) -> PerformanceGuard` in crates/platform/src/gpu.rs (shared, re-exported from lib.rs), crates/platform/src/linux/nvml.rs and a Windows no-op (contracts/platform.md, research R7): on Linux load `libnvidia-ml.so.1` with `libloading`, `nvmlInit_v2`, find the device by PCI bus id (`nvmlDeviceGetHandleByPciBusId_v2`, else the first device), read the current PowerMizer mode, call `nvmlDeviceSetPowerMizerMode_v1(PREFER_MAXIMUM_PERFORMANCE)`, restore the previous mode and `nvmlShutdown` on drop; any missing library, symbol or error code → logged once, no-op guard; add crates/platform/examples/gpu_performance.rs holding the guard for 60 s and printing what happened
- [ ] T025 [US1] (maintainer) Validate research R7 on the CachyOS PC (depends on T024) before wiring it: run `cargo run --release -p capturax-platform --example gpu_performance` while `nvidia-smi --query-gpu=pstate,clocks.gr,clocks.mem,pcie.link.gen.current,power.draw --format=csv -lms 500` runs in another terminal; expected P0 with high clocks during the 60 s and back to idle after; the agent records the output in measurements/latency.md. **If the clocks do not hold, stop US1's Linux work here** and bring the fallback of research R7 (keep-alive GPU work) to the maintainer for a decision (spec and research first)
- [ ] T026 [US1] Hold the performance guard on the video thread in app/src/render/mod.rs (depends on T025, T018): acquire `platform::gpu_performance` with the adapter's PCI bus id (from wgpu's adapter info, when it gives one) when device frames start being presented, drop it when presenting stops (hidden, device closed, quit); not for the test video unless `--stats` is on (comparison runs); log acquire and release
- [ ] T027 [US1] (maintainer) Quickstart "First checks" 2–3 and US1 acceptance 1 on Windows (C920, then the ezcap when it is at the Windows PC) and on CachyOS (ezcap): start without `[preview]` → Auto max FPS (1080p120 on the ezcap), restart → same device and mode, color range Auto shows `Auto (Full)` with the PS5 home screen and matches the passthrough, Full and Limited switch at once, fullscreen and windowed; the agent records the results in measurements/acceptance.md
- [ ] T028 [US1] (maintainer) Latency runs L1, L2, L4, L5 and L6 of quickstart.md "Latency budget" (L4 with `nvidia-smi` showing P0 while previewing, and `power.draw` with and without the request for FR-012), with the `--stats` log of each clip; the agent runs `uv run tools/latency/analyze.py …` as in quickstart, records median, p95 and the SC-001 / SC-002 verdicts, and the SC-003 reading of `ready_ms` and `pres` across the shell's interface states, in measurements/latency.md
- [ ] T029 [US1] (maintainer) Latency runs L3 and L3b (Auto max FPS with a 60 Hz and a 120 Hz source); the agent records them in measurements/latency.md next to L1 (cost of the default mode with a 60 Hz source, research R3); optional levers of quickstart recorded if tried

**Checkpoint**: SC-001, SC-002 and SC-003 met; maintainer validates; commit `feat: open the capture device in the preview`. If a budget fails, fix the path before adding anything on top (plan, Investigations).

---

## Phase 4: User Story 2 - Pick a device and a mode (Priority: P1)

**Goal**: the device and mode selects in the toolbar, with badges, detail lines, automatic modes
and remembered choices per device.

**Independent Test**: ezcap and a webcam connected; open both menus, check them against
`--list-devices`, switch devices and modes (quickstart Manual acceptance 2).

- [ ] T030 [P] [US2] Build the menu models in app/src/preview.rs as pure functions (depends on T006, T008): `device_items(devices, open_id, stored_choices) -> Vec<Item>` (name, detail line = "the mode it will open in: its remembered mode, else its default automatic mode, with the resolved resolution and frame rate" (FR-004), badge of that mode, open one marked; empty list → one disabled "No devices" item, never an empty menu); `mode_items(device, choice) -> Vec<Item>` (header **Automatic**, the `shown_automatic` entries with their resolved mode as detail, header **Modes**, each mode's label and badge, the current choice marked); unit tests: C920 and ezcap lists from the recorded caps, duplicates removed, a stored mode the device lacks shows the default as detail
- [ ] T031 [US2] Add the **Device** and **Mode** selects in front of Color range in app/ui/components/toolbar.slint and their models in app/src/ui.rs (depends on T021, T030): items from T030 as Slint models, updated on every monitor event so a removed device disappears from an open menu and a gone device is never shown (spec edge case); the select buttons show the open device's name and the current choice (an automatic choice reads e.g. `Auto max FPS`); the Mode select is disabled with no open device; both keyboard usable
- [ ] T032 [US2] Handle the choices in app/src/preview.rs and app/src/app.rs (depends on T031): picking a device → `Opening` of that device with its stored mode and range, else its defaults (US2 scenario 4); picking a mode → `Opening` of the same device with it and `store.update` of `devices."<id>".mode` (FR-009); a stored manual mode the device no longer has is replaced only when the user picks again (contracts/settings-file.md rule 2); a remembered device plugged in while another one plays only appears in the menu, no switch (spec edge case); log each choice; unit tests in preview.rs for both flows
- [ ] T033 [US2] Make the `NoSource` and `NoDevices` actions open the toolbar's device menu (depends on T031; US2 scenario 5, data-model.md "Preview state" actions): `state-action("open-devices")` in app/src/ui.rs shows the chrome and opens the Device select's menu with keyboard focus on it
- [ ] T034 [US2] (maintainer) US2 acceptance (quickstart Manual acceptance 2) on Windows and CachyOS with the ezcap and the C920: menus, badges, no screens, automatic group without duplicates, modes largest first, each device keeps its own mode and range, video within 2 s of a switch (SC-004, read from the log's open → first frame times, which T018 logs); with the device menu open, plug and unplug the webcam: it appears and leaves within 3 s (SC-004, read from the monitor's log lines against the moment of plugging); two identical devices if available (quickstart 8), else recorded as not checked; the agent records the results in measurements/acceptance.md

**Checkpoint**: maintainer validates; commit `feat: add device and mode menus`.

---

## Phase 5: User Story 3 - See how the preview is doing (Priority: P2)

**Goal**: source, mode, frame rate and latency bubbles with their display choices; `lat_ms` and
`rep` in the statistics; the estimate calibrated against glass to glass.

**Independent Test**: preview at 1080p60 and at 1080p120 with a 60 Hz source, compare the bubbles
with `--stats` (quickstart Manual acceptance 3).

- [ ] T035 [P] [US3] Implement app/src/latency.rs (pure, data-model.md "Latency estimate" and "Bubble displays"): `estimate(platform: Platform, source_period: Duration, refresh_period: Duration, vrr: bool, c_platform: Duration) -> Duration` = `F + (Windows ? F : 0) + C_platform + (vrr ? 0 : R / 2)`; `C_WINDOWS` and `C_LINUX` constants starting at 5 ms (calibrated in T040); `latency_text(display, measured, estimated)` → "3 ms", "3 ms + ~24 ms", "~27 ms"; `fps_text(display, presented, nominal)` → "60 fps", "111 / 120 fps", "120 fps" (nominal rounded unless fractional, as `Mode::label`); the platform is passed in (no `cfg` in app, boundary test); unit tests for each formula branch and each text
- [ ] T036 [US3] Add `lat_ms` and `rep` to app/src/stats.rs and the video thread in app/src/render/mod.rs (depends on T014; contracts/stats.md): record `presented − captured` per presented frame, `lat_ms=<median>/<max>` (`-/-` without frames) and `rep=<n>` (0 until US5) appended at the end of the `[loop]` line, existing fields unchanged; the per-second summary (presented frames, measured latency median) is computed always, not only with `--stats`, and sent to the main thread as a user event for the bubbles (no extra work per frame beyond one `Instant` subtraction, FR-015); unit tests for the new fields and the exact line
- [ ] T037 [US3] Create app/ui/components/info-bubbles.slint and place it in app/ui/main.slint (depends on T036), ported from the spike's `layered-preview/ui/components/info-overlay.slint`: four bubbles in the video area's top left, shown with the chrome, in the spike's shape (black at 55%, opaque text): source name, mode label, frame rate text, latency text, updated once per second; the frame rate and latency bubbles show a settings button (`adjustments_horizontal` icon) on hover, drawn in the top layer, opening a menu with their three displays (Measured / Measured + estimated / Total; Presented / Presented / nominal / Nominal), the current one marked; the latency bubble's tooltip says what each part is (FR-016a); bubble texts on the bubble fill added to app/tests/contrast.rs against white and black video
- [ ] T038 [US3] Feed the bubbles from app/src/app.rs and app/src/ui.rs (depends on T035, T037): source and mode from the open session, nominal fps from the mode, the per-second summary from T036, the estimate with the source frame period, the current monitor's refresh period (winit `refresh_rate_millihertz`, 60 Hz when unknown) and `vrr = false` (no platform call reports variable refresh in this spec, data-model.md "else false"; the calibration absorbs it); display choices read from and saved to `[overlay]` (`store.update`), applied at once; bubbles hidden when no device is playing
- [ ] T039 [US3] (maintainer) US3 acceptance (quickstart Manual acceptance 3): bubbles at 1080p60 and 1080p120 with the PS5 at 60 Hz (about "111 / 120 fps"), each display choice, the choice after a restart; the agent compares bubble values with `--stats` (presented ±1 per second, measured latency within 1 ms, SC-007 first two parts) and records them in measurements/acceptance.md
- [ ] T040 [US3] Calibrate `C_WINDOWS` and `C_LINUX` in app/src/latency.rs from the L1 and L4 medians of T028 (data-model.md: "so that measured + estimated matches the camera's median"), record the derivation in measurements/latency.md, then (maintainer) check the Total display against the L1, L2, L4, L5 medians within 5 ms (SC-007); if it misses, record by how much and why before changing the formula (spec first)

**Checkpoint**: maintainer validates; commit `feat: add preview info bubbles`.

---

## Phase 6: User Story 4 - Every device problem says what to do (Priority: P2)

**Goal**: every state of FR-019 with its icon, text and action; reconnect; sleep and wake; device
notices in the app and as system notifications (FR-027).

**Independent Test**: force each state as in quickstart Manual acceptance 4.

- [ ] T041 [P] [US4] Implement `camera_access() -> Access` (`Allowed`, `Denied`, `Unknown`) and `open_camera_privacy()` in crates/platform/src/camera.rs (shared, re-exported from lib.rs), crates/platform/src/windows/camera.rs and crates/platform/src/linux/camera.rs (contracts/platform.md, research R6): Windows reads `Value` of `Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam` and `...\webcam\NonPackaged` under HKCU and HKLM ("any `Deny` → Denied", all missing → Unknown, else Allowed) and opens `ms-settings:privacy-webcam` with `ShellExecuteW`; Linux returns `Unknown` and `open_camera_privacy` does nothing; unit test for the Windows decision as a pure function over the four values
- [ ] T042 [US4] Map device errors in crates/capture/src/session.rs (depends on T011; research R6): `DeviceError` = `InUse`, `Blocked { node: Option<String> }`, `Gone`, `NoSignal`, `Failed(String)`; a pure `map_error(platform_hint, domain, code, debug_text, playing_since, still_listed, access: Access) -> DeviceError`: Linux `RESOURCE/BUSY` → InUse; `RESOURCE/OPEN_READ_WRITE` with "Permission denied" → Blocked naming the node; `RESOURCE/NOT_FOUND`, or `RESOURCE/READ` with the device no longer listed → Gone; Windows: access Denied → Blocked; else not listed → Gone; else `STREAM/FAILED` within 2 s of PLAYING → InUse; else Failed; the access value is passed in by `app` (no OS call in the crate); no frame within 5 s of `Playing` → `Error(NoSignal)` (a timer on the session's worker); unit tests for every branch of `map_error`; extend crates/capture/tests/pipeline.rs with a `videotestsrc num-buffers=0` source that never produces → `NoSignal` after 5 s
- [ ] T043 [US4] Complete the state machine in app/src/preview.rs (depends on T017, T042): session errors → `Blocked`, `InUse`, `Disconnected` (Gone, or a monitor `Removed` of the open id), `NoSignal`, `Failed`; **Try again** → `Opening` of the same device and mode; `Disconnected` waits for the same id and reopens it in the same mode when it is `Added` (F4), a different device added meanwhile only appears in the menu (spec edge case); while waiting (`NoDevices` or `Disconnected`) a 3 s timer asks for `Monitor::devices()` to catch a missed add (research R1); `NoDevices` on Windows first checks `platform::camera_access()` and shows `Blocked` when denied (research R6); an error while `Playing` with access now Denied → `Blocked` (spec edge case); sleep and wake: errors after resume → `Disconnected`, reopened when listed again; unit tests for each transition of data-model.md's diagram
- [ ] T044 [US4] Wire the timers, access and states in app/src/app.rs (depends on T041, T043): the 3 s re-read on the main loop's timers (`ControlFlow::WaitUntil`), `camera_access()` passed to the session error mapping, **Open privacy settings** → `platform::open_camera_privacy()`; log every state change with its reason
- [ ] T045 [US4] Give each state its icon, texts and actions in app/ui/main.slint, app/ui/texts.slint and app/src/ui.rs (depends on T019, T033, T044; research R13, data-model.md "Preview state"): `NoDevices` `device_tv_off` "No devices" + short hint, action opens the device menu; `Blocked` `lock` "Camera access blocked" (Windows) with **Open privacy settings** and **Try again**, or "Cannot open /dev/videoN" (Linux, the node from `Blocked { node }`) with **Try again**; `InUse` `player_pause` "In use by another app" + **Try again**; `Disconnected` `device_tv_off` "<name> disconnected", action opens the device menu; `NoSignal` `antenna_bars_off` "No signal" + **Try again**, the mode menu stays reachable; `Failed` `alert_triangle` "Cannot open <name>" + **Try again**; add the Tabler icons `lock`, `player_pause`, `antenna_bars_off` with their license line; texts short, in both catalogs
- [ ] T060 [P] [US4] Implement `notify(Notice) -> bool` in crates/platform/src/notify.rs (shared, re-exported from lib.rs), crates/platform/src/windows/notify.rs and crates/platform/src/linux/notify.rs (contracts/platform.md, research R15): `Notice { title, action: Option<String>, tag: String }`, actions reported through a callback given at init (`on_action(tag)`); Linux `org.freedesktop.Notifications.Notify` with `zbus`, app icon, `replaces_id` of the previous notice, `ActionInvoked` and `NotificationClosed` signals on a thread; Windows: add the `windows` crate (workspace dependency, version 0.62, features `UI_Notifications`, `Data_Xml_Dom`, `Win32_UI_Shell`) to crates/platform under `cfg(windows)` (research R14), `SetCurrentProcessExplicitAppUserModelID("CapturaX")`, the `HKCU\Software\Classes\AppUserModelId\CapturaX` key (`DisplayName`, `IconUri`) written on first use and rewritten when the path differs, the toast XML with one button when there is an action, `Activated` → `on_action(tag)`, the previous toast hidden; notifications unavailable or off → logged once, returns false; unit tests for the toast XML and the D-Bus arguments as pure functions; add crates/platform/examples/notify.rs showing a notice with **Open** and printing the action
- [ ] T061 [US4] (maintainer) Validate research R15 (depends on T060): `cargo run -p capturax-platform --example notify` on Windows 11 from the unpacked build (name CapturaX and its icon, **Open** prints the action) and on CachyOS KDE Plasma (same); the agent records the result in measurements/acceptance.md. **If the Windows toast does not show or its button does not reach the app**, stop and bring the fallback of research R15 to the maintainer (spec and research first)
- [ ] T062 [US4] Add the notices to the state machine in app/src/preview.rs (depends on T043; data-model.md "Device notice"): monitor `Added`/`Removed` after `Listed` → `Command::Notice { kind, device_id, name }`; `Reconnected` when the `Added` device is the one `Disconnected` is waiting for, `Connected` otherwise, `Disconnected` on `Removed`; user event `NoticeOpen(id)` → same flow as picking the device (T032); unit tests: no notice for the first list, each kind, open from a notice
- [ ] T063 [US4] Create app/ui/components/notice.slint and wire the notices in app/ui/main.slint, app/ui/texts.slint, app/src/ui.rs and app/src/app.rs (depends on T061, T062; research R15): in-app notice in the top layer, bottom right of the video area above the dock, icon (`plug_connected` / `plug_connected_x`, Tabler) and text "<name> connected" / "<name> reconnected" / "<name> disconnected", **Open** for connected, a close button (×, Tabler `x`, with an accessible label) and a click anywhere on the balloon closing it, **Open** opening the device and closing it, about 5 s (kept while hovered), one balloon showing only the latest message (a newer one replaces the text, no queue), never over the state's actions, no toast library; window focused → in-app, else `platform::notify` with the same texts in the current language, `on_action` → `UserEvent::NoticeOpen(id)` → show the window and open the device; texts in both catalogs; notice text on its fill added to app/tests/contrast.rs; log every notice and where it went
- [ ] T046 [US4] (maintainer) US4 acceptance (quickstart Manual acceptance 4 and 6) on Windows and CachyOS: no devices and plug in, camera privacy off and **Open privacy settings** (Windows), `chmod 000` on the node (Linux), card open in OBS or `gst-launch-1.0` (In use; if Windows misreads it, the in-use mapping is dropped on Windows per research R6 and the state becomes Failed: spec and research updated first), unplug while playing and plug back (video within 3 s, same mode, SC-004), card moved to another port, no HDMI signal, close to tray (webcam light off) and show, minimize and restore, sleep and wake, the notices of quickstart 4 (focused, another app focused, minimized, in the tray; FR-027); both languages; the agent records the results in measurements/acceptance.md (SC-005)

**Checkpoint**: maintainer validates; commit `feat: add device states and reconnect`.

---

## Phase 7: User Story 5 - Choose how the preview meets a variable refresh monitor (Priority: P3)

**Goal**: the Video settings category with Presentation; a refresh-driven loop for Follow the
monitor that switches without reopening the device.

**Independent Test**: on the 144 Hz G-SYNC monitor switch the setting while previewing, watch the
cursor and read `[present]` and `rep` (quickstart Manual acceptance 5).

- [ ] T047 [US5] Add the refresh-driven loop to app/src/render/mod.rs and app/src/render/gpu.rs (depends on T014, T036; research R9): `Inbox::set_presentation(LowestLatency | FollowMonitor)`; Follow the monitor reconfigures the surface on the video thread to `PresentMode::Fifo` with `desired_maximum_frame_latency = 1`, then loops: acquire (blocks until about the next refresh), take the newest frame if any (upload only then) or reuse the last one, composite, present, counting a reuse as `rep`; Lowest latency goes back to Mailbox and today's frame-driven loop; switching never touches the session; the `[present]` header is printed again with `mode=fifo-follow` or `mode=mailbox`; on Vulkan log the image count; the stats `rep` counter fed from here
- [ ] T048 [US5] Add the **Video** category to app/ui/components/settings-panel.slint and wire it in app/src/ui.rs and app/src/app.rs (depends on T047; FR-022, research R13): icon `device_tv` (Tabler), row **Presentation** with **Lowest latency** (default) and **Follow the monitor**, tooltip "Only matters with variable refresh (G-SYNC, FreeSync)"; the change goes through `store.update` of `preview.presentation` and `Inbox::set_presentation` at once; read at start
- [ ] T049 [US5] (maintainer) US5 acceptance (quickstart Manual acceptance 5) on Windows (C24FG7x G-SYNC) and on CachyOS: cursor smooth with Follow the monitor and the app focused, `mode=fifo-follow` and `rep` > 0, back to `mode=mailbox`, no Opening state while switching; the agent records it in measurements/acceptance.md

**Checkpoint**: maintainer validates; commit `feat: add presentation setting`.

---

## Phase 8: Windows presentation stalls and snapped fullscreen (FR-023 to FR-025)

**Purpose**: the investigation the spec requires, its fix if the cause is the app's, and the
snapped-window fullscreen return. Needs US1 working on Windows.

- [ ] T050 [P] Implement `qpc() -> i64`, `arranged_rect(&Window) -> Option<Rect>` and `set_outer_rect(&Window, Rect)` in crates/platform/src/lib.rs, crates/platform/src/windows/mod.rs, crates/platform/src/windows/snap.rs and crates/platform/src/linux/mod.rs (contracts/platform.md): Windows `QueryPerformanceCounter`, `IsWindowArranged` → `GetWindowRect`, `SetWindowPos` with `SWP_NOZORDER | SWP_NOACTIVATE`; Linux `CLOCK_MONOTONIC` in ns, none, no-op
- [ ] T051 Add `--present-log <FILE>` to app/src/cli.rs and the present log to app/src/render/mod.rs and app/src/app.rs (depends on T050; contracts/cli.md, contracts/stats.md "Present log"): tab-separated lines `acquire <qpc>`, `present <qpc> frame=<n>`, `event <qpc> focus=<bool>` / `fullscreen=<bool>`, written by a writer thread through a channel (no file I/O on the video thread); the language switch in app/src/ui.rs and app/src/main.rs logs `stage <qpc> language=<stage> <ms>` for catalog switch, Slint property update, UI draw, upload and present (FR-024), with `--stats` or `--present-log`; unit tests for option parsing and line format
- [ ] T052 (maintainer) Run the stall investigation of quickstart.md "Windows stalls" (research R8) with PresentMon 2.3.1 elevated and `--present-log`: ten focus changes and ten fullscreen entries and exits plus 30 s idle in fullscreen, on the native 1080p monitor with the ezcap at 1080p60, then with G-SYNC off for capturax.exe, with MPO off (`OverlayTestMode=5`, reboot), with `OverlayMinFPS=0`, with `WGPU_DX12_PRESENTATION_SYSTEM=visual`; ten language switches; the agent matches PresentMon's `CPUStartQPC` with the present log and records in measurements/stalls.md per run the stalls over 50 ms, the `PresentMode` around each, and the conclusion
- [ ] T053 Act on the finding of T052 (SC-006): if the cause is in the app (for example a surface reconfigure or a blocking acquire at focus change, or a language switch stage), update research R8 with the cause and the fix first, then fix it in the named file and re-run the ten tries each (maintainer) with no stall over 50 ms; if the cause is the driver or the system (for example a G-SYNC switch), record the finding and any user-side workaround in measurements/stalls.md and research R8 and bring it to the maintainer for acceptance (an NVIDIA driver profile is the maintainer's decision, not done here)
- [ ] T054 Return fullscreen to a snapped place on Windows in app/src/app.rs (depends on T050; FR-025, research R10): on entering fullscreen keep `platform::arranged_rect(window)`; after leaving, if one was kept, `platform::set_outer_rect` to it (after winit's own restore); then (maintainer) quickstart Manual acceptance 7: snap to the left half, F11, F11 → back in the left half

**Checkpoint**: SC-006 met or the finding accepted; maintainer validates; commit `fix: …` or `feat: …` per what was changed (`docs: record presentation stall investigation` if only measurements).

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T055 [P] Update README.md (devices, `--list-devices`, `--present-log`, the Linux NVIDIA performance request and how to see it) and the status line of AGENTS.md to the device preview's state
- [ ] T056 [P] Check dependencies (`cargo tree --depth 1 -e normal` per crate): every direct dependency of `crates/capture`, the new `libloading` and the `windows` crate in `crates/platform` match research R14 (`windows` at the version already in the tree, no second copy); `cargo deny check licenses bans` passes; remove anything unused
- [ ] T057 Complete app/lang/pt-BR/LC_MESSAGES/capturax.po for every new text and fix any failing contrast pair; `cargo test` passes on both platforms
- [ ] T058 (maintainer) Full manual acceptance of quickstart.md on Windows 11 and CachyOS KDE Plasma Wayland at 100% and 150%, in both languages, with the ezcap and the C920 (SC-008); the agent records the results and known limits (Auto source not available on UVC cards, Windows in-use heuristic, ezcap identity on Windows, system notifications hidden by the system while another app is fullscreen) in measurements/acceptance.md
- [ ] T059 Update handoff.md, docs/roadmap.md (status of #3) and docs/product.md if a feature's description changed; prepare the pull request description (what the preview delivers, latency and stall measurements, known limits)

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: after Setup; blocks every story.
- **US1 (Phase 3)**: after Foundational. The MVP; its latency runs (T028, T029) gate everything
  that adds load to the path.
- **US2 (Phase 4)**: after US1's T018 and T021 (preview wiring and toolbar).
- **US3 (Phase 5)**: after Foundational for T035, T036; T040 needs US1's latency runs (T028).
- **US4 (Phase 6)**: after US1's T017–T019 and T022; T045 actions that open the device menu
  need US2's T033.
- **US5 (Phase 7)**: after Foundational and US3's T036 (the `rep` field).
- **Phase 8**: after US1 works on Windows; T052 is best run after US5 so the matrix covers both
  presentation modes.
- **Polish (Phase 9)**: after every story.

### Within the phases

- Phase 2: T005, T006, T008, T009, T013 in parallel; T004, T006 → T007; T006, T008 → T010;
  T005, T006, T009 → T011 → T012; T005 → T014; T011, T014 → T015; T010 → T016.
- US1: T017 → T018 → T019; T020 → T021 → T022 → T023; T024 → T025 → T026; all → T027 → T028, T029.
- US2: T030 → T031 → T032, T033 → T034.
- US3: T035, T036 → T037 → T038 → T039; T028 → T040.
- US4: T041, T042 → T043 → T044 → T045; T060 → T061; T043 → T062; T061, T062 → T063; T045, T063 → T046.
  T060–T063 were added after the first numbering and run inside Phase 6, before T046.
- US5: T047 → T048 → T049.
- Phase 8: T050 → T051 → T052 → T053; T050 → T054.

## Parallel Example: Foundational

```text
T005 frame.rs     T006 mode.rs     T008 device.rs     T009 analysis.rs     T013 settings model
```

## Parallel Example: User Story 1

```text
T017 preview.rs state machine     T020 badge + menu items     T024 gpu_performance (Linux NVML)
```

## Implementation Strategy

### MVP first

1. Phase 1 and Phase 2 → the test video through the NV12 path; `--list-devices` works.
2. Phase 3 (US1) → validate R7 on Linux early (T025), then measure the budget (T028, T029).
   **Stop and validate**: if SC-001 to SC-003 fail, fix the path before adding anything on top.

### Incremental delivery

US1 → US2 → US3 → US4 → US5 → Phase 8 → Polish, one commit per checkpoint after the maintainer
validates it. The pull request is opened only when the maintainer asks.

## Notes

- Every task names its files; the agent does not touch files outside them without saying why.
- If a decision changes during implementation, update spec.md, plan.md or research.md first.
- Spike code is a starting point, not a dependency: nothing imports from `spike/`.
