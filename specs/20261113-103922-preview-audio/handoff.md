# Handoff: Preview Audio

**Updated**: 2026-11-13 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

## State

Spec, plan and tasks committed. Phase 1 (T001–T004) done and committed (`build: add audio
crate`): empty `crates/audio` wired into the app and the boundary test, `pipewire` 0.10 and the
WASAPI features in the workspace, PipeWire build packages in CI. Not pushed.

Phase 2 validations (2026-11-13, committed in `test: add the audio probe examples`):

- T005 written: `crates/platform/examples/pipewire_probe.rs`. **Not compiled**: this PC has no
  Linux toolchain (WSL Ubuntu has no Rust or PipeWire headers), so the first build is on
  CachyOS. Written against the `pipewire` 0.10.1 sources; it needs the crate feature `v0_3_49`
  (`target.object`, a playback buffer's `requested` size), now set in the workspace Cargo.toml.
  Signals: the main thread's own PipeWire loop holds the `SIGINT`/`SIGTERM` sources (spa blocks
  the signal in the adding thread, and `add_signal_local` asserts the thread is named `main`)
  and is set up before the `pipewire` thread starts, so that thread inherits the blocked mask.
- T007 done: `crates/platform/examples/audio_endpoints.rs`, run here (T008's Windows half,
  recorded in measurements/pairing.md): the ezcap and the C920 each show one container id on
  video and audio. Finding: virtual endpoints all report the null container id
  `{00000000-0000-0000-ffff-ffffffffffff}`; research R5 and data-model.md "Pairing" now say it
  is no link.

Phase 2 pure crate (2026-11-13):

- T009 done: `crates/audio/src/resample.rs`. `Format`, `interpolate(samples, channels,
  position, out)` (Catmull-Rom on a contiguous interleaved slice; needs one frame before and two
  after the position, so T011's ring hands it a contiguous window), `ChannelMap::new(in, out)
  .apply(frame, frame)`. Rules not in the task text, chosen here: input channels up to the
  output's count go 1:1 (so 5.1 → 7.1 keeps its order), and a mono output gets the average of the
  stereo mix-down. −3 dB is `1/√2`.
- T010 done: `crates/audio/src/volume.rs`. `gain`, `icon` → `VolumeIcon`, `wheel`, `Ramp::new(gain,
  rate)` with `set_target`, `advance()` (gain for the next output frame), `current`, `target`.
- T014 done: `crates/audio/src/choice.rs`. `AudioChoice::parse` / `Display`,
  `OutputChoice::parse` / `Display`, `resolve(&choice, pair, &inputs) -> Resolved`,
  `output_for(&choice, &outputs) -> (Option<String>, bool)` (`None` = the system default). The
  task's `default` argument was dropped: no rule uses it (contract and task updated).
- T011 done: `crates/audio/src/bridge.rs`. Two rules changed from the spike, in research R4,
  data-model.md "Bridge" and the task text: a stall's backlog is cut down to one input period +
  margin (as priming), because down to the margin alone gave a 5 ms gap in the burst test; after
  any silence the gain ramps from 0 again (no click when the sound returns). Ring capped at 200 ms
  of input while nobody pulls (counted as dropped). `drop_ms` counts priming, cuts and the cap.
- T012 done: `crates/audio/tests/drift.rs` (4 cases, 30 simulated minutes each, ~14 s in
  debug). Criterion changed in the task text and research R15: the waiting audio is compared with
  its value at second 10, not second 1, because priming leaves one input period above the margin
  and the follower takes it away in about 8 s (median 8.3 → 6.8 ms at +300 ppm, 8.0 → 5.6 ms at
  −300 ppm). Settled: the median moves less than 0.1 ms over 30 min, `rate_ppm` within ±20 ppm
  of the offset; the +300/−300 levels differ by ~1.2 ms, the follower's proportional error
  (300 ppm × 2 s ≈ 0.6 ms each way).

Settings (2026-11-13):

- T021 done: `crates/settings/src/model.rs`. `Settings.audio: Audio { output, volume, muted }`,
  `DeviceChoices.audio`, `Settings.audio_devices: Vec<(id, AudioDevice { name, exclusive })>`
  most recently used first, with `audio_device(id)` and `touch_audio_device(id, name)` (as
  `touch_device`). `output` and `audio` are kept as text, as `mode`: the settings crate does not
  depend on `crates/audio`, the app parses them with `OutputChoice::parse` / `AudioChoice::parse`.
  Rules chosen here: a non-text or empty `output`/`audio` takes `"default"`/`"auto"`; a float
  `volume` is rounded then clamped (NaN takes 80); `DeviceChoices::default().audio` is `"auto"`, so
  `Default` is now written by hand. Entries with `exclusive = false` stay in memory but are not
  written.

Privacy (2026-11-13):

- T016 done: `crates/platform/src/privacy.rs` with `Capability::{Camera, Microphone}`,
  `access(Capability)`, `open_privacy(Capability)`; `windows/privacy.rs` builds the consent store
  keys (`webcam` / `microphone`, plus `NonPackaged`) and the Settings page per capability;
  `linux/privacy.rs` returns `Unknown`. Camera callers in app.rs and ui.rs pass
  `Capability::Camera`; camera behavior unchanged. This PC reads Allow for both. research R7's
  `Blocked` row now names `platform::access(Capability::Microphone)`. Linux build not checked
  here (trivial module).

Platform interface (2026-11-13):

- T015 done: `crates/platform/src/audio.rs` with the types of contracts/platform.md; stubs in
  `windows/audio.rs` and `linux/audio.rs` (`Monitor::start` errs, sessions post
  `Error(Failed("not implemented"))` for both sides, `close` posts `Closed`). Signatures fixed
  here (task text and contract updated): `Monitor::start` returns `Result<Monitor, AudioError>`;
  the session takes `Arc<dyn Samples>` (trait with `on_input` and `fill`, called on the audio
  threads) instead of two closures; `Endpoint` has `base_name` and `numbered` (per flow) as video
  devices, so the app renumbers after `Added`/`Removed`; `Format` carries the sample format
  (`48000/2/i32`); `reported_latency_ms` is an `Option`; `Session` closes on drop. T017 turns
  `windows/audio.rs` into `windows/audio/mod.rs`; T019/T020 put the Linux code in
  `linux/pipewire.rs` behind `linux/audio.rs`. Linux build not checked here.

Windows endpoints (2026-11-13):

- T017 done: `crates/platform/src/windows/audio/{mod.rs, endpoints.rs, container.rs, format.rs}`.
  Monitor thread `audio-monitor` (COM multithreaded): notifications registered before the first
  list; it keeps the ids and the default output it reported, so nothing is reported twice
  (Windows calls `OnDefaultDeviceChanged` twice per change, measured). Mix format read from an
  activated, not initialized, `IAudioClient`; `format.rs` maps `WAVEFORMATEX`/`EXTENSIBLE` to
  `Format` (T018 extends it). `video_link` = T007's `CM_*` path; the null container id is no
  link. `windows-core` is now a direct dependency (the `implement` macro names it). Checked live
  on this PC: ezcap and C920 video and audio links equal (same values as T008); unplugging the
  C920 gave `Removed` then `Added`; switching the default output gave `DefaultOutput`.

Windows session (2026-11-13):

- T018 done: `crates/platform/src/windows/audio/stream.rs` (threads `audio-in`/`audio-out`,
  one `Stream` per open; reopen requests through a channel, the newest wins; the last thread to
  end posts `Closed`) and `format.rs` (`bytes`, `wave_format`, `mask_of`, `standard_mask`,
  `read`/`write` with round-trip tests; integers scaled by 2^15/2^23/2^31 and clamped, so they
  round-trip exactly). The `Com` guard moved to `windows/audio/mod.rs`. Exclusive output tries
  the input's opened rate first (an atomic per side; 0 until that side opens).
- New example `crates/platform/examples/audio_session.rs` (passthrough without the bridge,
  prints events, delivered rates, a reopen halfway, the close time). Kept (maintainer,
  2026-11-13): it checks a session without the app, here before T023 and for T020 on CachyOS.
- Measured here with the ezcap: shared opens in ~25 ms (ezcap 10 ms period, Voicemeeter output
  2.67 ms through `IAudioClient3`), 48000 frames/s both sides; ezcap exclusive i16 3 ms, C24
  monitor exclusive i24in32 3 ms, Voicemeeter Input exclusive i24 2 ms; reopen ~15 ms; `Closed`
  5–100 ms after close. `GetStreamLatency` is 0 for shared through `IAudioClient3`: reported as
  `None`.
- **Decision changed (maintainer, 2026-11-13)**: an exclusive open *takes* a device other apps
  use shared (they get `AUDCLNT_E_DEVICE_INVALIDATED`, the endpoint stays active), and
  `AUDCLNT_E_DEVICE_IN_USE` only comes when another app already holds it exclusive (a shared open
  is refused too). Now: those two → `InUse`, `Gone` only when the endpoint left; new app state
  `Held` (tag In use, `Audio device in use` / `Try again`, manual retry). Updated: spec (US4
  scenarios 3 and 6, FR-015, FR-017, the states table), data-model, research R7,
  contracts/platform.md, tasks T018, T038, T041. The G533 here is held exclusive by another app
  (Voicemeeter, probably): CapturaX gets `InUse` on it.

## Done in the last sessions

- PR #9 (device preview) squash-merged into `main` (2f4beea); this branch starts from it.
- spec.md and checklists/requirements.md; docs/roadmap.md #4 in progress with the spec link.
- Mockup page for the UI decisions (private Artifact):
  https://claude.ai/artifact/DWT9wMk8mY64QXU9woPy2n (version 2).
- Plan (2026-11-13), main decisions:
  - New pure crate `crates/audio` (bridge: ring, spike's rate follower with 2 ms margin, cubic
    interpolation, channel map, volume ramp; pairing; choices). Device streams in
    `crates/platform` (research R1, R13).
  - Windows: the spike's WASAPI engine, now in any device format (shared via `IAudioClient3`
    with the mix format, exclusive probing formats) (R2).
  - Linux: PipeWire directly with the `pipewire` 0.10 crate, one `pipewire` thread, both streams
    in one `node.group` (as module-loopback), `node.latency` 128/48000, format held explicitly;
    no PulseAudio layer unless the probe fails (R3).
  - Pairing by the system's link: Windows container id, Linux USB bus path without the interface
    suffix; name fallback; never across different links (R5).
  - Volume in the app's bridge, cubic law, 10 ms ramp; OS stream volume untouched (R8).
  - Nothing waits for an audio thread; quit waits at most 1 s for the audio close (R9).
  - Linux measurement: null sink `capturax-measure` with the card on the left and the app on the
    right, plus the output's reported latency; `analyze.py audio --drift` for the 30-minute runs
    (R10).
  - Constitution amended to 1.2.0 (maintainer, 2026-11-13): principle III's preview audio
    exception now names PipeWire on Linux next to WASAPI, so the plan has no deviation.

## Decisions

Spec decisions (maintainer, 2026-11-13) are in spec.md. The maintainer accepted the plan's
engines (WASAPI on Windows, PipeWire directly on Linux) and the constitution amendment
(2026-11-13). `--no-audio` (contracts/cli.md) is kept unless the maintainer removes it.

## Open questions

- Validated during implementation, before the code that depends on them: the Linux engine probe
  and the pairing keys (`--list-devices`) on both systems (quickstart).

## Next step

1. While the ezcap is on this Windows PC: nothing else of T006/T008 needs it here.
2. On CachyOS (ezcap and C920 moved there): `cargo build` (needs `pipewire` headers and
   `clang`), fix the probe if it does not compile (fold into the commit), then T006 (quickstart
   "Linux engine probe") and T008's Linux half (commands in measurements/pairing.md).
3. T018 is done; Phase 2 has no Windows-only task left (T022 needs T019). Next on Windows only
   after CachyOS: T013 (after T008's Linux half), then T022.
