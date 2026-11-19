---
description: "Tasks for the preview audio"
---

# Tasks: Preview Audio

**Input**: Design documents from `specs/20261113-103922-preview-audio/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: unit tests for every pure module (in the module's own `#[cfg(test)] mod tests`), the
integration test `crates/audio/tests/drift.rs`, the audio state machine's tests with a fake
platform in `app/src/audio.rs`, and the extended `app/tests/platform_boundary.rs`. Audio devices,
latency, drift and the UI are manual acceptance (quickstart.md): hosted CI has no audio device.

**Spike code**: "port from the spike" means read the file with
`git show spike/stack-20261026:spike/<path>` and rewrite it into the named file, to the structure
of plan.md; nothing imports from the spike. The engine is `spike/audio-preview/src/wasapi.rs`.

**Texts**: every task that adds a user-facing text writes it with `@tr("...")` in a `.slint` file
(texts Rust needs go in `app/ui/texts.slint`), exactly as spec FR-022 gives it, and adds its
Portuguese (Brazil) translation from FR-022 to `app/lang/pt-BR/LC_MESSAGES/capturax.po` in the
same task; entries FR-022 marks `*` already exist and are reused, not duplicated;
`app/tests/i18n.rs` must keep passing.

**Icons**: new icons are Tabler (MIT, 24 px stroke, as the existing ones), added to
`app/ui/assets/icons/` in the task that first uses them.

**Tasks marked (maintainer)** need the maintainer's hands or machines; the agent prepares them,
gives the exact commands, and records the results in `measurements/`. Hardware batching: before
the ezcap or the C920 moves between PCs, every pending (maintainer) task that needs it on the PC
where it is now is done first.

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

**Purpose**: the new crate exists and is guarded by the boundary test; the platform crate and CI
can build PipeWire and WASAPI code.

- [x] T001 Add the audio crate (research R13): in the workspace Cargo.toml add `crates/audio` to `members`; create crates/audio/Cargo.toml (package `capturax-audio`, description "CapturaX preview audio: the bridge between input and output, pairing and choices", workspace package fields, **no dependencies**) and crates/audio/src/lib.rs with a module doc pointing to contracts/audio.md and the modules `bridge`, `resample`, `volume`, `pairing`, `choice` (empty files); add `audio = { package = "capturax-audio", path = "../crates/audio" }` to app/Cargo.toml; `cargo build` passes
- [x] T002 [P] Extend app/tests/platform_boundary.rs: add `"crates/audio"` to `CHECKED_CRATES` and `"pipewire"`, `"libspa"` to `OS_CRATES`; update the module doc ("`app`, `settings`, `capture` and `audio`"); the test passes
- [x] T003 [P] Platform dependencies (research R14): in the workspace Cargo.toml add `pipewire = "0.10"` under the `# OS crates` group with the comment `# PipeWire client for the preview audio on Linux (spec 20261113-103922-preview-audio, research R3)`; add to `windows` the features `Win32_Media_Audio`, `Win32_Media_KernelStreaming`, `Win32_Media_Multimedia`, `Win32_System_Com`, `Win32_System_Com_StructuredStorage`, `Win32_System_Variant`, `Win32_System_Threading`, `Win32_Devices_FunctionDiscovery`, `Win32_UI_Shell_PropertiesSystem`, `Win32_Foundation`, `Win32_Security`, and to `windows-sys` `Win32_Devices_DeviceAndDriverInstallation`, `Win32_Devices_Properties`; add `pipewire.workspace = true` under the Linux dependencies of crates/platform/Cargo.toml; `cargo build` passes on Windows; `cargo deny check licenses bans` passes
- [x] T004 [P] In .github/workflows/ci.yml add `libpipewire-0.3-dev clang` to the Ubuntu `apt-get install` list, with a comment saying the `pipewire` crate builds its bindings with bindgen (preview audio research R14); the Linux job builds (checked on the PR)

**Checkpoint**: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` pass on
Windows; commit `build: add audio crate`.

---

## Phase 2: Foundational

**Purpose**: the validations that decide the design (Linux engine probe, pairing keys), the pure
bridge, the platform audio interface and streams on both systems, and the settings. Nothing in
the app plays yet.

### Validations first

- [ ] T005 Write the Linux engine probe crates/platform/examples/pipewire_probe.rs (research R3, quickstart "Linux engine probe"): a `pipewire` main loop on its own thread; finds the first `Audio/Source` node whose `node.description` contains the `--input` argument (default `ezcap`) and the default sink; creates a capture stream (`target.object` = the node's `node.name`) and a playback stream (no target), both with `media.type=Audio`, `media.category=Capture`/`Playback`, `media.role=Game`, `node.latency=<--latency, default 128>/48000`, `node.group=capturax.preview`, format F32 interleaved held explicitly to 2 channels FL FR at 48000 Hz; the capture `process` copies into a `Mutex<VecDeque<f32>>`, the playback `process` takes what it needs (silence and a gap count when short); every second prints `delivered_ppm` (input frames against wall time), `gap_ms`, `wait_ms`; on SIGINT disconnects both streams, quits the loop and exits; with a 1 s watchdog that logs and calls `std::process::exit(2)` if quitting takes longer; Linux only (`#![cfg(target_os = "linux")]` with a `fn main` stub elsewhere); builds on Linux
- [x] T006 (maintainer) Run the probe on CachyOS with the ezcap and the PS5 (quickstart "Linux engine probe" steps 1–4: 10 × `timeout -s INT -k 5 3`, a 60 s run, `pw-dump` of the capture stream's format, `pw-top` for the shared driver) and record each result in specs/20261113-103922-preview-audio/measurements/probe.md with the date, PipeWire version (`pipewire --version`) and kernel. **Gate**: if any step fails, stop the Linux tasks (T019, T020) and decide the fallback with the maintainer (research R3); record the decision in research.md R3 before continuing
- [x] T007 Pairing keys probe: add to crates/platform/examples/audio_endpoints.rs a listing of every audio input and output with name, id and the raw link sources (Windows: `PKEY_Device_ContainerId` from each endpoint's property store, and for every Media Foundation video device its container id via `CM_Get_Device_Interface_PropertyW(DEVPKEY_Device_InstanceId)` → `CM_Locate_DevNodeW` → `CM_Get_DevNode_PropertyW(DEVPKEY_Device_ContainerId)` from its symbolic link; Linux: each audio node's parent device `device.bus-path`, and for every V4L2 device the `device.bus_path` GStreamer reports — print it with `gst-device-monitor-1.0 Video/Source` instructions in the example's doc instead of linking GStreamer here)
- [x] T008 (maintainer) Run T007's example on Windows (ezcap and C920) and on CachyOS (ezcap and C920), record the printed keys in specs/20261113-103922-preview-audio/measurements/pairing.md, and check research R5's rule: Windows container ids equal per physical device; Linux bus paths equal after removing a trailing `:<n>.<n>`. If a key differs from the rule, update research R5 and data-model.md "Pairing" before T013

### Pure crate (`crates/audio`)

- [X] T009 [P] crates/audio/src/resample.rs (research R4): `Format { rate: u32, channels: u16 }`; 4-point cubic (Catmull-Rom) interpolation of an interleaved frame at a fractional position; channel map: mono → every output channel, stereo → output channels 0 and 1 (others silent), more input than output channels mixed down (channels 0/1 kept, center and LFE (2, 3 in the WAVEFORMATEXTENSIBLE/PipeWire order FL FR FC LFE …) added at −3 dB to both sides, the rest added to their side at −3 dB, by even/odd index), then clamped to ±1. Tests: a 1 kHz sine at 48 kHz read at a 48000/44100 step keeps 1 kHz (zero crossings) and its level within 0.1 dB, distortion below −60 dB (residual after removing the fitted sine); mono → stereo equal channels; 5.1 → stereo of a center-only signal gives equal sides at −3 dB
- [X] T010 [P] crates/audio/src/volume.rs (research R8, data-model.md "Volume"): `gain(level: u8, muted: bool) -> f32` = `(level/100)³`, 0 when muted; `icon(level, muted) -> VolumeIcon` with "muted or 0 → `Off`; ≤ 33 → `Low`; ≤ 66 → `Medium`; else `High`"; `wheel(level, notches: i32) -> u8` "±5 per notch, clamped to 0–100"; `Ramp` that moves a current gain linearly to a target over 10 ms of output frames at a given rate. Tests for each rule, and that no step of a ramp from 0 to 1 at 48 kHz exceeds 1/480
- [X] T011 crates/audio/src/bridge.rs (research R4, contracts/audio.md "Bridge"), ported from the spike's `Follower` and the render loop of `wasapi.rs`: `Bridge::new(input: Format, output: Format)`, `push(&self, &[f32])`, `pull(&self, &mut [f32])`, `set_input_period(frames)`, `set_gain(f32)` (atomic target, `Ramp` from T010), `take_stats() -> BridgeStats { wait_ms_median, wait_ms_max, rate_ppm, gap_ms, drop_ms }`; constants verbatim: margin **2 ms**, control every **500 ms**, correction in **2 s**, never beyond **±1000 ppm**, cut above **40 ms** of spare audio down to one input period + margin (as priming; down to the margin alone gaps at the next output event); priming "drops down to one input period + margin"; an empty ring plays silence counted as a gap and primes again; the read step is `ratio × input.rate / output.rate` through T009's interpolation and channel map; time for the 500 ms control is counted in output frames (not `Instant`), so tests are deterministic; the `Mutex` is held only for the copy; no allocation in `push`/`pull` after the buffers reach their size. Unit tests: priming, gap counting, a 40 ms burst dropped once, gain ramp applied
- [X] T012 [P] crates/audio/tests/drift.rs (research R15): simulate 30 minutes with the input at +300 ppm and at −300 ppm against the output, 3 ms input packets and 2.67 ms output packets (118 frames at 44.1 kHz), a 48 kHz → 48 kHz and a 48 kHz → 44.1 kHz case: after the first second no gap and no drop; from second 10 on (priming leaves one input period above the margin, which the follower takes away in about 8 s) the waiting audio within 1 ms of its value at second 10 and `rate_ppm` within ±50 ppm of the simulated offset
- [X] T013 [P] crates/audio/src/pairing.rs (research R5, data-model.md "Pairing"): `Endpoint` (id, name without the app's numbering, link: Option<String>), `VideoKey { link: Option<String>, name: String }`, `clean_name` (lowercase; the same rules on both systems, told apart by the name's form, so the tests run anywhere: a name that ends with `(…)` is a Windows endpoint and the text inside its last parentheses is taken, unless a profile word comes before them (`… Digital Stereo (IEC958)`); otherwise the profile words `analog stereo`, `digital stereo`, `estéreo analógico`, `estéreo digital`, `mono`, `analog surround …`, `multichannel` and what follows them are removed (whole words, after the device's name), then the text after `: `), `pair(&VideoKey, &[Endpoint]) -> Option<&str>` with the rules verbatim: "Inputs whose `link` equals the video's: one → it; several → the name match among them, else the first by id. No input with the video's link: inputs without a link, or with no link on the video side, matched by cleaned name. An input whose link is present and different is never paired"; names match when equal or one is a prefix of the other at a word boundary; of several name matches, the first by id. Tests with the names and keys recorded in measurements/pairing.md (T008), the spike's `ezcap Live GAMER ULTRA Estéreo analógico` against `ezcap Live GAMER ULTRA: ezcap L`, `Digital Audio Interface (ezcap Live Gamer Ultra)` against `ezcap Live Gamer Ultra`, and two identical cards with different links
- [X] T014 [P] crates/audio/src/choice.rs (data-model.md "Audio choice", "Output choice", contracts/audio.md): `AudioChoice::{Auto, None, Input(String)}` with settings texts `"auto"`, `"none"`, the id; `OutputChoice::{Default, Device(String)}` with `"default"`, the id; unknown or empty text → the default; `resolve(choice, pair, inputs) -> Resolved::{Play(id), NoPair, Missing(id), Off}`; `output_for(choice, outputs) -> (Option<id>, fallback: bool)` (`None` opens the system default; `Default` → `(None, false)`; a `Device` id not present → `(None, true)`). Tests for every row of the data-model tables

### Platform layer (`crates/platform`)

- [X] T015 crates/platform/src/audio.rs: the interface of contracts/platform.md "Audio devices" and "Audio session": `Endpoint` (id, name, base_name, flow, link, format), `Format { rate, channels, sample: SampleFormat }` (printed `rate/channels/sample`), `numbered(&mut [Endpoint])` (" (2)" per flow, as video devices), `AudioEvent::{Listed { inputs, outputs, default_output }, Added, Removed, DefaultOutput}`, `Monitor::start(on_event) -> Result<Monitor, AudioError>`, `video_link(video_id: &str) -> Option<String>`, `SessionRequest { number, input, output: Option<String>, input_exclusive, output_exclusive }`, `Session::open(request, samples: Arc<dyn Samples>, on_event)`, `reopen_input`, `reopen_output`, `close` (also on drop), `SessionEvent::{Opened { number, side, mode, format, period_ms, reported_latency_ms: Option }, Error { number, side, error }, Closed { number }}`, `Side::{Input, Output}`, `StreamMode::{Shared, Exclusive, SharedAfterInUse}` (printed as the stats), `AudioError::{Gone, InUse, Blocked, Format, Failed(String)}`. The platform crate does not depend on `crates/audio`: the trait `Samples` has the two callbacks (`on_input(&[f32], Format)`, `fill(&mut [f32], Format)`) that the app connects to the bridge. OS functions forward to `os::audio` with stub implementations that report `Failed("not implemented")`; module doc says every call returns at once (research R9)
- [X] T016 [P] Generalize crates/platform/src/camera.rs into crates/platform/src/privacy.rs (contracts/platform.md "Privacy", research R7): `Capability::{Camera, Microphone}`, `access(Capability) -> Access`, `open_privacy(Capability)`; Windows reads the `microphone` consent store (`...\ConsentStore\microphone` and `microphone\NonPackaged`, HKCU and HKLM, any `Deny` → Denied) as the webcam's and opens `ms-settings:privacy-microphone`; rename crates/platform/src/windows/camera.rs to windows/privacy.rs and the Linux module likewise; update the callers in app/src/app.rs and app/src/ui.rs (`platform::access(Capability::Camera)`, `platform::open_privacy(Capability::Camera)`); behavior for the camera unchanged; tests still pass
- [X] T017 Windows endpoints and pairing link, crates/platform/src/windows/audio/{mod.rs, endpoints.rs, container.rs, format.rs} (research R5, R6): enumeration of active endpoints per flow with id (`GetId`), friendly name, `PKEY_Device_ContainerId` as link, mix format (`IAudioClient::GetMixFormat` on an activated, not initialized, client; `format.rs` maps `WAVEFORMATEX`/`EXTENSIBLE` to `Format`, T018 extends it); `windows-core` a direct dependency (the `implement` macro names it); the default render endpoint (`eConsole`); an `IMMNotificationClient` implementation (`windows::core::implement`) whose callbacks only post `AudioEvent`s through the callback (no blocking calls inside, per Microsoft's rules) for `OnDeviceStateChanged`, `OnDeviceAdded`, `OnDeviceRemoved`, `OnDefaultDeviceChanged(eRender, eConsole)`; on a state change the endpoint is read on the monitor's own thread, not in the callback; `video_link` from the `mf:` id's symbolic link via the `CM_*` calls of T007; container ids formatted lowercase without braces, the null container id as no link; the monitor thread `audio-monitor` registers the notifications before the first list and keeps the ids and the default output it reported, so a change already reported is not reported twice (Windows calls `OnDefaultDeviceChanged` twice for one change)
- [X] T018 Windows session, crates/platform/src/windows/audio/{stream.rs, format.rs} (research R2, R7), ported from the spike's `wasapi.rs`: `format.rs` = the spike's `Sample` (read/write of `f32, i32, i24in32, i24, i16`) generalized to any channel count and rate, with unit tests of round trips; `stream.rs`: threads `audio-in` and `audio-out`, COM multithreaded per thread, MMCSS "Pro Audio", event wait with a **100 ms** timeout checking an atomic stop flag; shared open: `IAudioClient3::InitializeSharedAudioStream` with the endpoint's mix format and `GetSharedModeEnginePeriod`'s minimum, else `Initialize` shared with the mix format and a **20 ms** buffer (no `AUTOCONVERTPCM`); exclusive open: first format accepted by `IsFormatSupported(EXCLUSIVE)` trying `f32, i32, i24in32, i24, i16` × rates (the other side's rate, 48000, 44100, 96000) × channels (2, the mix format's), minimum device period, event mode, the `AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED` retry; exclusive refused with `AUDCLNT_E_DEVICE_IN_USE`, `AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED` or every format unsupported → the same side reopened shared with `mode = SharedAfterInUse`, and when the shared open is refused too, `Error(InUse)`; shared output keeps **1.5 periods** queued, exclusive writes one period per event; error mapping of research R7 (endpoint not found or not active → `Gone`; `AUDCLNT_E_DEVICE_IN_USE`, or `AUDCLNT_E_DEVICE_INVALIDATED` while the endpoint is still active (taken by an exclusive app, measured) → `InUse`; `E_ACCESSDENIED` on an input → `Blocked`; else `Failed(text)`, WASAPI's codes named); `GetStreamLatency` of 0 (shared through `IAudioClient3`, measured) is `None`; `Opened` carries `GetStreamLatency` as `reported_latency_ms`; `close` sets the stop flag and returns (threads `Stop` and release on their own, then post `Closed`); `reopen_*` handled on the side's thread; events carry the request's `number`
- [x] T019 Linux monitor, crates/platform/src/linux/pipewire.rs (the thread and the registry) behind crates/platform/src/linux/audio.rs (`Monitor`, `video_link`) (research R3, R6), after T006 passes: one `pipewire` thread per process started lazily by the first `Monitor::start` or `Session::open`, commands through `pipewire::channel`; registry: `Audio/Source` and `Audio/Sink` nodes (monitor sources excluded), id `node.name`, name `node.description`, link = the parent device's `device.bus-path` (from the bound device's info; the registry global does not carry it) with a USB path's trailing `:<n>.<n>` removed, format from the node's first raw `EnumFormat` (open properties at their default); the `default` metadata's `default.audio.sink` (its JSON `name`) → `DefaultOutput`; changes reported after the server answers a `sync` (removals, additions, the default output); connection lost → every endpoint `Removed`, then the thread connects again (one try a second) and reports the new endpoints `Added`; `video_link` from the V4L2 id's `@<bus path>` part with the same suffix removed
- [x] T020 Linux session in crates/platform/src/linux/pipewire.rs, after T006 passes, from the probe (T005): both streams with the properties of research R3 verbatim (`node.latency=128/48000`, `node.group=capturax.preview`, `media.role=Game`, format F32 interleaved at the node's channels and positions with the rate left to the graph, held explicitly), output `target.object` only when an output id is given; a side opens after the connection's first list and only on a listed node (an id that is not there → `Gone`); `process` callbacks call `on_input`/`fill` with the stream's format (the graph's rate); `Opened` at the stream's first cycle (100 ms later when the stream reports no delay yet), with the device node's format, that cycle's size as the period and `reported_latency_ms` from `pw_stream_get_time`'s delay; stream state `Error` → `Failed(text)` or `Format` before any format was negotiated; node removed → `Gone`; stream closed by the server → `Failed`; connection lost → `Failed` for each open side, the session kept so `reopen_*` opens it on the new connection; `close` disconnects and destroys on the `pipewire` thread and returns at once; exclusive flags ignored
- [X] T021 [P] Settings, crates/settings/src/model.rs (contracts/settings-file.md): `[audio]` with `output` (`"default"` or an id), `volume` (0–100, default **80**, "A `volume` outside 0–100 is clamped; a non-number takes 80"), `muted` (default false); `audio` key in `DeviceChoices` (`"auto"` default, `"none"` or an id; "A missing or unknown value takes its default"); `[audio_devices."<id>"]` with `name` and `exclusive`, "written only when `exclusive` is true; at most 32 are kept, the least recently used dropped on save"; unknown keys kept; tests for each rule and a round trip
- [x] T022 CLI, app/src/cli.rs and app/src/main.rs (contracts/cli.md): `--list-devices` prints, after the video devices, the audio inputs and outputs (name, id, link, format, `default` mark), then each video device's link (`platform::video_link`) and the input `audio::pair` gives (or `none`); `--no-audio` skips the audio monitor and sessions; `--test-video` never opens audio

**Checkpoint**: `cargo test -p capturax-audio` passes (drift included); `--list-devices` shows
correct pairs on both systems (maintainer, quickstart "Pairing"); commit per group
(`feat(audio): add the bridge`, `feat(platform): add audio devices and streams`, settings, CLI).

---

## Phase 3: User Story 1 - Hear the console while playing (Priority: P1) 🎯 MVP

**Goal**: the paired input plays on the system default output while the video device is open,
within budget, stable for 30 minutes.

**Independent Test**: PS5 on the ezcap, default settings: sound with the image, latency (SC-001
shared, SC-002), A/V (SC-004), drift (SC-003), continuity (SC-005), video unchanged (SC-007).

- [x] T023 [US1] app/src/audio.rs (new; data-model.md "Audio session", "Audio state"): `AudioController` driven by `handle(event, &Settings) -> Vec<Command>` as `preview.rs` is: inputs `Monitor(AudioEvent)`, `Session(SessionEvent)`, `VideoOpened { session, device: VideoDevice { id, base_name, link } }` (the app reads `platform::audio::video_link`, so the controller stays pure), `VideoClosed { session }`; commands `Open(SessionRequest)`, `ReopenInput`, `ReopenOutput`, `Close`, `SetGain`; this task covers `Off`, `Opening`, `Playing`, `NoPair` with Auto and the system default output: opens when the video session opens (preview states Opening, Playing, No signal), closes when it closes, numbers sessions and ignores events of older numbers ("no sound plays after the video is gone"). Tests with event sequences: open/close with the video, a video switch while the audio is opening, NoPair, a late `Opened` of an old session ignored
- [x] T024 [US1] Wiring in app/src/app.rs, app/src/sound.rs (new) and app/src/main.rs (plan "Startup and quit"): the app gives the controller `VideoOpened`/`VideoClosed` when it runs the preview's `Open` and closes the video session (`preview.rs` unchanged: every session open and close already goes through those commands); start `platform::audio::Monitor` after the video monitor (not with `--no-audio`), its events and session events as winit user events through the `EventLoopProxy`; `Command`s executed without waiting (the session object lives in `sound.rs`, its `Samples` feed an `Arc<audio::Bridge>` made with the formats of the `Samples` calls once both sides have called, not from `Opened` (T020); the bridge is made again when a side's format changes or the input reopens); gain from settings at open; quit: close the audio session right after the video session, wait for `Closed` at most **1 s**, log and continue if it does not come, then the shell's sequence and `std::process::exit` after saving settings if an audio thread is still alive
- [x] T025 [P] [US1] app/src/stats.rs (contracts/stats.md): the `[audio-source]` line on every `Opened` (session, input and output names, `in_mode`/`out_mode` as `shared`, `exclusive`, `shared-in-use`, formats `rate/channels/sample`, periods, `out_latency_ms`), and the `[audio] t= wait_ms= rate_ppm= gap_ms= drop_ms=` line once per second from `Bridge::take_stats()` while a session is open; existing lines unchanged
- [x] T026 [P] [US1] tools/latency/analyze.py: `audio --drift` (research R10): the pairs' delays with their times grouped per minute (`--drift` replaces `--minute`); prints and appends to the measurements (`audio-drift.csv`, one row per clip, also a table in `summary.md`) the medians of minute 0 and of the last minute with at least 5 pairs, their difference, and the least-squares slope in ms per minute; tools/latency/README.md documents it
- [x] T027 [P] [US1] tools/latency/pipewire-measure.sh (research R10, quickstart "Linux"): run while the app plays; `start` creates a null sink `capturax-measure` (`pw-cli create-node adapter` with `factory.name=support.null-audio-sink`, `media.class=Audio/Sink`, `audio.position=[FL FR]`, `object.linger=true`), finds the app's streams in `pw-dump` by `media.name` (`Preview audio input`, `Preview audio`; both have `node.name` `capturax`), and with `pw-link` adds the left channel of the node feeding the app's input to the sink's FL and the app's playback left channel to the sink's FR, leaving the app's own links alone (it keeps playing on the real output); `record <file> <seconds>` runs `pw-record --target capturax-measure --channels 2` reading the sink's monitor; `stop` removes the sink and so its links; usage and the meaning of left and right in tools/latency/README.md
- [ ] T028 [US1] (maintainer) Windows, ezcap on the Windows PC: quickstart "Latency budget" shared run (SC-001 ≤ 62 ms), A/V with spec #3's glass to glass in the same session (SC-004), SC-007 with and without `--no-audio`, the 30-minute shared drift run with `--stats` logged (SC-003, SC-005), and one 30-minute run on the G533 directly by ear and stats; results in measurements/latency.md and measurements/drift.md
- [x] T029 [US1] (maintainer) Linux, ezcap on CachyOS: SC-002 with pipewire-measure.sh plus `out_latency_ms` on the real output (median ≤ 25 ms, p95 ≤ 30), the variants `node.latency` 64 and 256 (a temporary environment variable `CAPTURAX_PW_LATENCY` read only in debug builds, removed in T046), SC-004 by ear, SC-007, the 30-minute drift run; results in measurements/latency.md and measurements/drift.md; the maintainer picks the default from the variants (research R3; 128 kept on 2026-11-17)

**Checkpoint**: the console is heard on both systems within budget; commit
`feat: play the preview audio`.

---

## Phase 4: User Story 2 - Volume and mute (Priority: P1)

**Goal**: the volume button and popover change only the preview's sound, without clicks, and are
remembered.

**Independent Test**: change volume and mute while playing; the system mixer and other apps
unchanged; restart keeps both.

- [X] T030 [P] [US2] Icons from Tabler saved in app/ui/assets/icons/: `volume` (two waves) as `volume.svg`, `volume-2` (one wave) as `volume_2.svg`, `volume-2` without its wave as `volume_1.svg` (Tabler has no low volume icon; `volume-3` is the speaker with an X) and `volume-3` as `volume_off.svg` (the muted speaker of the mockup), noted in the folder's LICENSE; the volume button in app/ui/components/dock.slint between ambient light and settings, its icon from `audio::icon`, tooltip `Volume {}% · Muted` (FR-022)
- [X] T031 [US2] app/ui/components/volume-popover.slint (new; FR-011, research R12): horizontal slider 0–100 with its value, nothing else (maintainer, 2026-11-18: no mute button, no Output select); anchored beside the left dock inside the window; behavior: click on the button toggles mute; resting the pointer **300 ms** on the button opens it; it stays open while the pointer is on the button or the popover and closes **400 ms** after it leaves both; right click opens it; the wheel over the button changes the volume ±5 per notch without opening; touch: a tap opens it, a tap outside closes it, holding the button 500 ms toggles mute (`IconButton` gained `takes-hold`/`held`) (no touch reached the UI until now: app/src/ui.rs forwards winit's touches to Slint as pointer events of the first finger and sets `Input.touch`, since Slint's public pointer events carry no finger); keyboard: the button is one focus stop; its focus shows the popover, which takes no focus; Enter or Space toggles mute, the arrows move the level ±5, Home/End 0/100 (`IconButton` gained `key`), and the popover closes when the focus leaves the button; the popover is a child of the volume button inside the chrome, and keeps the chrome shown while open; moving the slider or turning the wheel while muted unmutes
- [X] T032 [US2] app/src/ui.rs and app/src/audio.rs: the button and popover callbacks update `[audio] volume` and `muted` in the settings (saved as other settings), send `SetGain(audio::gain(level, muted))` to the open bridge at once (no reopen; the controller's `Event::Volume` gives it while a session is open or opening), and feed the icon and tooltip; at start the saved values are shown and used
- [ ] T033 [US2] (maintainer) Check US2 on both systems: the system mixer and another app's sound unchanged while moving and muting; no click; heard within 100 ms (SC-006); restart keeps level and mute; mouse, touch (if a touch screen is available) and keyboard behaviors of FR-011 (done 2026-11-17; the mouse and keyboard behaviors again after FR-011's change of 2026-11-18: CachyOS done 2026-11-18, Windows open)

**Checkpoint**: commit `feat: add the preview volume`.

---

## Phase 5: User Story 3 - Choose the audio input and the output (Priority: P2)

**Goal**: per-device input choice and an app-wide output choice, both remembered, with the lists
following the system.

**Independent Test**: with the ezcap and the C920, pick each input per video device and another
output, restart, every choice kept and playing; plug and unplug a headset.

- [x] T034 [US3] app/src/audio.rs: input choice per video device (`[devices."<id>"] audio`, `AudioChoice`), output choice (`[audio] output`, `OutputChoice`), `Missing(id)` shown as Disconnected, the output fallback flag (data-model.md "Output fallback": set when a `Device(id)` output is missing or fails, cleared when the output reopens on it), `DefaultOutput` → reopen the output side only when the choice is Default (Windows; on Linux PipeWire moves it, only the name changes), and on both systems when it names an output while the output side playing on the system default is failed (Linux after PipeWire restarts, research R7; a test for it), `Added`/`Removed` keep the lists and reopen what returned; a pick reopens only the side it touches ("output change → output side; input change → both sides (the bridge restarts empty)"); menu items: Input group (`Auto ({})` / `Auto (none)`, every input numbered as video devices, `No audio`), Output group (`System default ({})`, every output), current marked. Tests for each transition
- [X] T035 [US3] app/ui/components/toolbar.slint, menu.slint and app/src/ui.rs: the Audio select after the color range select, icon `volume` (`volume_off` for No audio), title `Audio`, value the input; menu groups `Input` · `Output`; in the narrow toolbar the icon look as the other selects (spec #3 FR-002a); texts of FR-022 for this task: `Audio`, `Input`, `Output`, `Auto ({})`, `Auto (none)`, `No audio`, `System default ({})`
- [ ] T036 [US3] (maintainer) Check US3 on both systems with the ezcap and the C920: every input per video device, another output, restart keeps them; change the system default output and plug a headset that becomes default (sound moves within 3 s, SC-006); pick the G533 and unplug it (plays on the default and says so) and plug it back (moves back); devices appear and leave the menu within 3 s; the C920's microphone paired by Auto (accepted, spec edge case) and `No audio` remembered for it (CachyOS half done 2026-11-18 without a headset there: the headset and G533 steps are on Windows; the input unplug moved to T042)

**Checkpoint**: commit `feat: choose the preview audio input and output`.

---

## Phase 6: User Story 4 - Exclusive mode for the lowest delay (Priority: P2)

**Goal**: per-device exclusive mode on Windows, falling back to shared when refused.

**Independent Test**: mark the ezcap input and the output exclusive, measure, and check another
app cannot play on the exclusive output.

- [ ] T037 [P] [US4] app/ui/components/checkbox.slint (new, styled like the shell's toggle: focus ring, hover, whole-row click) and the `audio` category in app/ui/components/settings-panel.slint, shown on Windows only (`platform::IS_WINDOWS` passed from Rust): section `Exclusive mode`, the line `Lowest delay. No other app can use the device while the preview does.`, groups `Inputs` · `Outputs`, one row per endpoint with the `Exclusive` checkbox and the `In use by the preview` mark; category title `Audio` (FR-014, FR-022)
- [ ] T038 [US4] app/src/audio.rs and app/src/ui.rs: the checkboxes read and write `[audio_devices."<id>"] exclusive`; a change on a device in use reopens that side only (FR-005); the session request carries the flags; `Opened` with `SharedAfterInUse` → state `InUse` (tag `In use`, header `Audio device in use` / `Exclusive mode unavailable`), exclusive tried again at the next open; `Error(InUse)` on a side → state `Held` (tag `In use`, header `Audio device in use` / `Try again`, action Try again; an output on a `Device(id)` falls back to the default instead, as data-model.md says); an exclusive side shows the `Exclusive` tag; texts of FR-022 for these; tests for the flag change and the In use transition
- [ ] T039 [US4] (maintainer) Windows, ezcap on the Windows PC (batch with T028 when possible): SC-001 input exclusive (≤ 50 ms) and both exclusive (≤ 22 ms), A/V both exclusive (SC-004), the two 30-minute drift runs (SC-003, SC-005); another app cannot play on the exclusive output and gets it back at once when the preview stops using it; results in measurements/latency.md and measurements/drift.md

**Checkpoint**: commit `feat: add the exclusive audio mode`.

---

## Phase 7: User Story 5 - Clear states for the sound (Priority: P3)

**Goal**: every reason the sound does not play is shown with its next action.

**Independent Test**: force each state as in quickstart "States and closing".

- [x] T040 [US5] app/src/audio.rs: `Disconnected(name)` (an input `Gone` or missing; returns on `Added` with the same id), `Blocked` (input `Blocked`, or `platform::access(Capability::Microphone)` = Denied before opening; read again every 3 s while Blocked, as the camera's), `Failed(name)` (any other error, or a shared open refused; **Try again** reopens), output `Gone` or `Failed` → fallback to the default with the flag; video states win: no audio state while the video device is not open (FR-018). Tests for each row of data-model.md "Audio state" and its transitions
- [x] T041 [US5] app/ui/components/toolbar.slint, menu.slint and app/src/ui.rs: the state's tag before the dropdown's value and its title, detail and action at the top of the menu, as the video device's states; actions `Open privacy settings` → `platform::open_privacy(Capability::Microphone)` and `Try again`; output fallback header `{} disconnected` / `Playing on the system default`; tags `Disconnected`, `Blocked`, `Failed`, `In use` (both `InUse` and `Held`); texts of FR-022 (reusing the `*` entries)
- [ ] T042 [US5] (maintainer) quickstart "States and closing" on both systems: every state forced in English and Portuguese with its action; 100 closes in a row with sound playing on each system, none over 1 s, no hang of the app or the system's sound (SC-008); results in measurements/acceptance.md (Linux half done 2026-11-18: No paired audio, Disconnected and Output fallback in both languages, 101 closes, max 15 ms)

**Checkpoint**: commit `feat: show the preview audio states`.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T043 [P] README.md (audio requirements: PipeWire on Linux; build needs `libpipewire-0.3-dev` and `clang`), tools/latency/README.md (drift and PipeWire measurement), docs/roadmap.md #4 status, AGENTS.md "Status" (the preview audio built)
- [ ] T044 (maintainer) SC-006 responsiveness on both systems from the logs (quickstart "Responsiveness"): sound within 1 s of the first frame, input/output/exclusive changes within 1 s, devices in the menu within 3 s, reconnect and default change within 3 s; results in measurements/acceptance.md (Linux done 2026-11-18: sound vs first frame, picks, reconnect, output fallback, volume and mute, default change, menu; Windows left)
- [ ] T045 (maintainer) SC-009 acceptance on Windows 11 and CachyOS KDE Plasma Wayland with the ezcap and the C920 through User Stories 1–5; results in measurements/acceptance.md
- [ ] T046 Cleanup: remove the debug-only `CAPTURAX_PW_LATENCY` (T029) unless the maintainer keeps it as a documented developer option; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on both systems; handoff.md updated

---

## Dependencies & Execution Order

- **Phase 1** → **Phase 2** → stories.
- In Phase 2: T005 → T006 (gate) → T019, T020. T007 → T008 → T013. T009, T010 → T011 → T012.
  T015 → T017, T018, T019, T020. T016, T021 any time after Phase 1. T022 after T013, T015, T017,
  T019.
- **US1** (T023–T029) needs all of Phase 2. **US2** needs T023–T024. **US3** needs US1. **US4** needs US1 and T018 (Windows only). **US5** needs US3 (the
  menu header and the fallback flag).
- Maintainer tasks batch by hardware: ezcap on Windows → T028, T039 (and T033, T036, T042, T044 on
  Windows); ezcap on CachyOS → T006, T029 (and the Linux halves of T033, T036, T042, T044); T008
  on whichever PC has the devices, both before T013 is final.

### Parallel opportunities

- Phase 1: T002, T003, T004 together after T001.
- Phase 2: T009, T010, T013, T014, T016, T021 together; T017/T018 (Windows) alongside T019/T020
  (Linux) on different machines.
- US1: T025, T026, T027 together while T023/T024 are built.
- US2: T030 alongside T031.
- US4: T037 alongside T038's state work.

## Implementation Strategy

1. **MVP = US1**: Phases 1–3. The console is heard on both systems, measured; stop and validate.
2. **US2** next (also P1): volume and mute make the MVP usable day to day.
3. **US3**, **US4** (Windows), **US5**, each committed at its checkpoint after the maintainer's
   check.
4. Polish, acceptance on both systems, then the pull request when the maintainer asks.
