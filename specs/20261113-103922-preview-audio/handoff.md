# Handoff: Preview Audio

**Updated**: 2026-11-17 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

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

Linux probe (2026-11-16, on CachyOS over SSH; committed in `docs: record the Linux engine probe
and pairing keys`):

- T006 done, **gate passed** (measurements/probe.md): the probe compiled unchanged on the first
  build (PipeWire 1.6.10, kernel 7.3.1); 10 of 10 runs closed in 0 ms, 60 s without a gap or a
  drop, format F32 FL FR held, the ezcap's node drives the output and both streams at 128/48000.
  The Linux engine is PipeWire directly; T019 and T020 can start. Recorded in research R3.
- Run twice: with OBS open on the ezcap's node (the maintainer had left it open), then with it
  closed. Both pass. Only with OBS open, two things happen in the first seconds after the
  streams start: the whole graph runs up to 0.8 % fast for about 4 s (about 20 ms of input
  caught up, no gap), and PipeWire's journal logs the IEC958 output's `resync`. With OBS closed
  neither appears. T029 should also listen to a start with another app open on the card.
- Not covered: only the internal IEC958 output was tried; nobody listened.
- T008 done (measurements/pairing.md): on Linux the ezcap (video `…-usb-0:3:1.0`, audio
  `…-usb-0:3:1.2`) and the C920 (`…-usb-0:1:1.0`, `…-usb-0:1:1.2`) each give one key; research
  R5's rule holds on both systems. The C920 is now plugged into the CachyOS PC.
- `audio_endpoints.rs` changed (folded into `test: add the audio probe examples`): the
  registry's global has no `device.bus-path`, so the example binds each device and reads its
  info; T019's text and research R5 now say so. Its doc points to `capturax --list-devices` for
  the video path (`gst-device-monitor-1.0` shows PipeWire's entry, without `device.bus_path`).
- The CachyOS clone is on this branch, at the same commit as here (bundle of 2026-11-16).

Linux monitor (2026-11-16, built and run on CachyOS over SSH; committed in `feat: add the Linux
audio monitor`):

- T019 done: `crates/platform/src/linux/pipewire.rs` and `linux/audio.rs` (`Monitor` = a
  watcher of the thread, `usb_key`, `video_link`; the session is still the stub). In
  `pipewire.rs`: `send(Command)` starts the `pipewire` thread when needed and waits only for the
  connection, `post` sends only when it runs; `Connection` is one connection with its listeners;
  `State` is what that connection's server says (nodes, devices, the `default` metadata);
  `Reports` is what the watchers were told and outlives a connection.
- Nothing is reported as it arrives: each change asks for a `sync`, and the answer to the latest
  one sends the difference from what was reported (research R6).
- **Decision (maintainer, 2026-11-16)**: when the connection to PipeWire is lost the thread
  reports every endpoint `Removed`, connects again (one try a second, commands served meanwhile)
  and reports the new endpoints `Added`. The thread never ends once set up. In research R6 and
  the task text.
- For T020: its commands go in `Command`, its streams belong to a `Connection` (they die with
  it, so a lost connection is the session's `Failed`, research R7, and the app's retry opens them
  on the new one). The node table has no channel positions yet: read them from the same
  `EnumFormat` in `raw_format`.
- Checked on CachyOS with a temporary example (not kept): `Listed` 2 ms after start with the
  ezcap `48000/2/i16`, the internal input and IEC958 output `48000/2/i32`, the C920
  `32000/2/i16`, default output the IEC958; links `pci-0000:03:00.0-usb-0:3` (ezcap) and
  `pci-0000:00:14.0-usb-0:1` (C920), equal to `video_link` of their V4L2 ids; a null sink loaded
  and unloaded gave `Added` (link none, `48000/2/f32`) and `Removed`; `pactl set-default-sink`
  gave `DefaultOutput`; the C920's card profile off and back gave `Removed` then `Added`; a
  second monitor started later got its own `Listed`. fmt, clippy (workspace) and the platform
  tests pass there.
- Reconnection checked: `systemctl --user restart pipewire` gave the removals, the connection
  back 20 ms later and every endpoint `Added` within 0.7 s (WirePlumber's `auto_null` sink shows
  up and leaves on the way); with PipeWire, its sockets and WirePlumber stopped for 6 s, a
  monitor started meanwhile got an empty `Listed`, and both monitors got the endpoints and the
  default output when the services came back.
- Not checked: a real unplug (the profile switch stood in for it).
- The maintainer allows restarting services on the CachyOS PC (a test machine, not in daily use).
- The CachyOS clone is on this branch at the same commit as here (bundle of 2026-11-16, after
  this commit).

Linux session (2026-11-16, built and run on CachyOS over SSH; committed in `feat: add the Linux
audio session`):

- T020 done: the session's side of `crates/platform/src/linux/pipewire.rs` and the `Session` of
  `linux/audio.rs`. `Command::{Open, Reopen, Close, Running}`; `Sessions` (kept through every
  connection, as `Reports`) holds each session's `Samples`, callback and two `Wanted` sides; a
  side's `Stream` is created on the current connection and dropped with it.
- Chosen here (task text, research R3 and R7, contracts/platform.md updated):
  - The format's rate is left out, so the streams run at the graph's rate. `Opened.format` is
    the device node's own format (the C920: `32000/2/i16`), while `Samples` is called with the
    stream's (`48000/2/f32`): **T023 must take the bridge's rates from the `Samples` calls, not
    from `Opened`** (doc of `SessionEvent::Opened` and `Samples` in `crates/platform/src/audio.rs`).
  - A side opens after the connection's first list and only on a listed node; an id that is not
    there, or a node that leaves, is `Gone`. Without this PipeWire's session manager would give
    the stream another device (a missing capture card would become the microphone).
  - `Opened` is reported at the stream's first cycle, told to the `pipewire` thread through its
    own command channel (`Running`). The delay comes with the link's latency, which in about one
    start of ten came after the first cycle (seen as `Some(0.0)` in a release build): a stream
    that says 0 is asked again 100 ms later (`SETTLE_MS`).
  - A stream that ends inside its own callback is kept in `Sessions::ended` and destroyed at the
    next flush.
  - Stream names (shown by mixers as the media name): `Preview audio` and `Preview audio input`.
- Checked on CachyOS with `audio_session` (ezcap, IEC958 output): both sides open 15–95 ms after
  the call, `48000/2/i16` in and `48000/2/i32` out, period 2.67 ms, reported latency 4.0 ms in
  and 2.67 ms out, 48000 frames/s each way with no silence; a reopen of the output takes ~12 ms;
  `Closed` 0.3 ms after `close`, 10 of 10 runs. Unknown input id → `Gone` at once and the output
  still opens. C920 as the input with its card profile switched off → `Gone` in 15 ms. A null
  sink as the named output, unloaded → `Gone`. Default output moved to a null sink and back with
  `output = None`: PipeWire relinks the stream itself, no event, no lost frame count.
  `systemctl --user restart pipewire` while playing: the input reports `Gone` (its node is
  removed before the connection closes) and the default output `Failed("PipeWire closed the
  stream")`; a `reopen_output` 3 s later opens on the new connection. fmt, clippy (workspace)
  and the platform tests pass there; fmt and clippy of the platform crate pass on Windows.
- **Decision (maintainer, 2026-11-16)**: after a PipeWire restart the input comes back by the
  usual `Removed`/`Added` path, but the default output is left in `Failed`; the app reopens an
  output side that failed while playing on the system default when `DefaultOutput` names an
  output again, without **Try again**. In research R7, data-model.md "Transitions" and T034.
- Not checked: nobody listened (T029); a real unplug; a device with more than two channels.
- The CachyOS clone is on this branch at the same commit as here (bundle of 2026-11-16, after
  this commit).

Pairing (2026-11-16, committed in `feat: add the audio pairing`):

- T013 done: `crates/audio/src/pairing.rs`. `Endpoint { id, name, link }`, `VideoKey { link,
  name }`, `pair(&video, &inputs) -> Option<&str>`, `clean_name(name) -> String`.
- Chosen here (task text updated): `clean_name` has no per-system switch, so both systems' names
  are tested on any PC; the name's form decides. A name ending with `(…)` gives its device part
  (inner parentheses kept: `Realtek(R) Audio`), unless a profile word comes before the
  parentheses (`… Digital Stereo (IEC958)`). Profile words count only as whole words after the
  device's name (`Monoprice Capture`, `Mono Capture` stay whole). Of several name matches, the
  first by id.
- For T022 and T023: pass the endpoint's `base_name`, not the numbered `name` (a trailing ` (2)`
  would be read as a device part).
- Known limit: a video name that itself ends with `(…)` (`UVC Camera (046d:0825)`) is cleaned to
  the text in the parentheses, so it only pairs by link. Not seen on the maintainer's devices.

Command line (2026-11-16, committed in `feat: add the audio command line options`):

- T022 done: `app/src/cli.rs` (`--no-audio`, `StartupOptions::no_audio` and `audio()`),
  `app/src/main.rs` (`list_audio`, `print_endpoints`), README options table.
- `StartupOptions::audio()` is false with `--no-audio` or `--test-video`: **T023 asks it before
  starting the audio monitor or a session**. Until then its only use is a log line at start
  (`preview audio off for this start`).
- `--list-devices` output after the video devices: `Audio inputs:` and `Audio outputs:` (name,
  `id:`, `link:` or `none`, `format:` or `unknown`; the system default output's name ends with
  ` [default]`), then `Pairing:` with each video device's `link:` and `audio: <name> (<id>)` or
  `none`. Audio that cannot be listed goes to stderr and the exit code stays 0. With no video
  device the `Pairing:` part is left out.
- Checked on CachyOS (ezcap and C920 there): both pair by link (`pci-0000:03:00.0-usb-0:3`,
  `pci-0000:00:14.0-usb-0:1`), the IEC958 output is marked default; fmt, clippy and the CLI
  tests pass. Checked on Windows: fmt, clippy, CLI tests; the devices here now are an Elgato 4K
  Pro (no link on video or audio: paired by name with `Elgato 4K Pro Audio (Elgato 4K Pro)`)
  and the NVIDIA Broadcast camera (`none`). The ezcap and C920 on Windows were checked by T008's
  example, not by `--list-devices`.
- Seen, not changed: on Linux the internal sound card's endpoints carry a link
  (`pci-0000:00:1f.3`), not only USB devices.
- The CachyOS clone is clean at `3a1b506` (bundle of 2026-11-16); it does not have T022.

Audio controller (2026-11-17):

- T023 done: `app/src/audio.rs`. `AudioController::handle(Event, &Settings) -> Vec<Command>`,
  `state()`, `session()` (the audio session number open or opening), `default_output()`;
  `Event::reason()` for the state-change log line, as the preview's.
- Chosen here:
  - `VideoOpened` carries `VideoDevice { id, base_name, link }`: the app (T024) reads
    `platform::audio::video_link` when the video session opens, so the controller stays pure.
  - `Command::Open` does not close by itself: a session still open gets an explicit `Close`
    first (a `VideoOpened` without a `VideoClosed` gives `[Close, Open]`).
  - A video opened before the monitor's `Listed` shows `Opening`; the list resolves it.
  - `NoPair` resolves again on every `Added`, so an input that pairs and comes later opens.
  - `VideoClosed` of a session other than the open one is ignored (late close).
  - `Error` events are only logged for now (states come with T034, T038, T041); `Settings` is
    not read yet (Auto and the default output; choices come with T034, exclusive with T038).
  - `ReopenInput`, `ReopenOutput` and `SetGain` are defined, not emitted yet.
- The module is declared with `#[allow(dead_code, reason = "wired into the app by T024")]` in
  `main.rs`; **T024 removes it**. Inside `main.rs` the crate is now `::audio::pairing` (the
  module `audio` shadows the crate there).
- fmt, clippy (app, all targets) and the 7 tests pass on Windows. Linux build not checked here
  (no platform code changed).

Wiring (2026-11-17, committed in `feat: wire the preview audio into the app`):

- T024 done: `app/src/sound.rs` (new) runs the controller's commands; `app.rs` gives it
  `VideoOpened` (with `platform::audio::video_link` read there) when it runs the preview's `Open`,
  and `VideoClosed` when it closes the video session; `preview.rs` unchanged (every video open and
  close goes through those commands; task text and plan updated). `UserEvent::Audio(audio::Event)`
  carries monitor and session events. The audio monitor starts after the video monitor; `App.sound`
  is None when `StartupOptions::audio()` is false. The dead_code allow on `mod audio` is gone;
  `ReopenInput`, `ReopenOutput`, `SetGain`, `session()` and `default_output()` carry an `expect`
  naming the task that uses them (T025, T032, T034).
- The bridge: `Feed` (the session's `Samples`) makes it from the formats of the `Samples` calls
  once both sides have called (not from `Opened`, as T020 asked), again when a side's format
  changes or `ReopenInput` runs; until then input is dropped and the output plays zeros. Gain from
  `[audio] volume`/`muted` at open, kept for each new bridge. **For T025**: the bridge is in
  `Feed.formats.bridge` (`sound.rs`); the stats need a getter there.
- Quit: the session's own callback sets a `Closed` flag (the event loop does not run during
  `exiting`), the quit waits for it at most 1 s, then drops the monitor. `main` already ends with
  `std::process::exit`, so a stuck audio thread cannot hold the process.
- Checked: fmt, clippy (app, all targets) and the app tests (3 new in `sound.rs`) on Windows and
  CachyOS. On CachyOS (ezcap): paired by link, `Opening` → `Playing` 79 ms after the video open,
  before the first video frame; the window closed by KWin: audio closed 1 ms into the quit. On
  Windows only the NVIDIA Broadcast camera is here: `Off` → `NoPair` → `Off` with its video
  session. Not checked: sound through the bridge (the ezcap's input was silent; nobody listened).

Stats lines (2026-11-17, committed in `feat: add the audio stats lines`):

- T025 done: `app/src/stats.rs` (`AudioSide`, `audio_source`, `audio_line`, pure, tested),
  `app/src/sound.rs` (each side's `Opened` kept per session; `StatsClock`; `print_stats` takes the
  playing bridge's `take_stats()` once a second), `app/src/audio.rs` (`endpoint_name(id)`, None
  names the default output), `app/src/app.rs` (`about_to_wait` wakes for the `[audio]` line).
- Chosen here (contracts/stats.md updated): `[audio-source]` waits until both sides opened, then
  again at each `Opened` (a reopen); names from the monitor's list (an unknown id prints the id);
  `out_latency_ms=-` when the output does not say; while no bridge plays, `wait_ms=-/-` and the
  other fields 0; `[audio] t` counts from the video thread's start, so it matches `[loop] t`. A
  bridge replaced mid-second loses that second's counts (only at a format change or input reopen).
- Checked: fmt, clippy (app, all targets), 170 app tests on Windows. On CachyOS (ezcap, IEC958
  output, release): `[audio-source] session=1 input="ezcap Live GAMER ULTRA Estéreo analógico"
  in_mode=shared in_format=48000/2/i16 in_period_ms=2.67 output="Áudio interno Estéreo digital
  (IEC958)" out_mode=shared out_format=48000/2/i32 out_period_ms=2.67 out_latency_ms=2.7`;
  `[audio]` from t=1 with the same `t` as `[loop]`: second 1 `gap_ms=8.0` (before priming), then
  0; `wait_ms` 7.4 → 4.9 ms over 6 s with `rate_ppm` +921 → +105 (the follower taking away the
  priming's extra period, as research R15). Windows: no paired device here (only NVIDIA Broadcast).

Drift analysis (2026-11-17, committed in `feat: add the audio drift analysis`):

- T026 done: `tools/latency/analyze.py audio --drift` (in place of `--minute`): each pair's
  minute is the reference onset's time in the clip; prints each minute's pairs, median and p95,
  then the first and last minutes with at least 5 pairs, their medians' difference and the
  least-squares slope over every pair (ms/min). New `audio-drift.csv` (one row per clip, replaced
  on re-run) and a "Drift runs" table in `summary.md`. README documents it.
- Fixed in quickstart.md: the audio commands used `--path`/`--variant`, which `audio` never had;
  now `--output <run>` with `--minute 0` or `--drift`. The drift run is now 31 minutes so minute
  30 (SC-003) has pairs.
- Checked on Windows with a synthetic 30.5-minute stereo WAV (right 20 ms behind, +0.1 ms/min):
  1830 pairs, minute 0 20.0 ms, minute 30 23.0 ms, difference +3.0, slope +0.093 ms/min, a few
  seconds to run. A 25 s clip with `--drift` is rejected (one minute); `--minute` mode unchanged.

Linux measurement script (2026-11-17, committed in `feat: add the PipeWire measurement script`):

- T027 done: `tools/latency/pipewire-measure.sh` (`start`, `record <file> <seconds>`, `stop`;
  needs jq), README section "Preview audio inside the PC (Linux)".
- Method changed (research R10, quickstart "Linux", T027 updated first): the app is no longer
  moved to the null sink. `start` finds the app's streams in `pw-dump` and adds two links: the
  device feeding the app's input, FL → sink FL; the app's playback FL → sink FR. The app keeps
  WirePlumber's links and plays on the real output, so `out_latency_ms` comes from the same run
  and T034's output menu is not needed. `stop` destroys the sink (its links go with it).
  `record` refuses when the sink has fewer than 2 input links.
- Fixed on CachyOS: the streams are found by `media.name` (`Preview audio input`, `Preview
  audio`); both have `node.name` `capturax`, so the first version found nothing.
- Checked on CachyOS (ezcap, PS5, release, IEC958 output): `start` adds both links, a second
  `start` says "already linked"; `pw-top` shows the app's streams and `capturax-measure` under
  the ezcap's driver (one cycle); a 25 s `record` gives a 48 kHz stereo WAV. `analyze.py audio
  --stereo` read it but found 0 onsets: the PS5 gave continuous sound (background −27 dBFS), so
  **T029 needs distinct sounds a few seconds apart** (e.g. menu clicks), not game music.
  A cross-correlation of that WAV: right (app) = left (card) 2.06 ms later, correlation 0.999, in
  every 4 s window; level ratio 0.512 = 0.8³ (the app's volume at 80 on the cubic law). `stop`
  leaves no `capturax-measure` node or link and the app's own links untouched; `stop` again,
  `record` without the sink, `record` after the app quit, `start` without the app: each a clear
  message and exit 1 (bad arguments: usage, exit 2). shellcheck not run (not installed).
- Over SSH, an app started with `&` ignores SIGINT (non-interactive shell); stop it with SIGTERM.
Debug latency variable (committed in `test: read the PipeWire latency from the environment in
debug builds`): `CAPTURAX_PW_LATENCY=<frames>` sets `node.latency` in debug builds; T046 removes it.

Linux measurements (2026-11-17, T029 done; committed in `docs: record the Linux preview audio
latency`, `docs: record the Linux SC-007 runs`, `docs: record the Linux A/V offset` and the T029
drift commit), results in measurements/latency.md and drift.md:

- Sound source that works: PS5 **home menu with its background music off**, a navigation sound
  every 1–2 s (background −83 dBFS). Game menus, even paused, keep music under the clicks and give
  0 onsets. Clips are in `clips/` (ignored); `--stats` logs in `measurements/raw/t029/` (ignored,
  `*.log`; the older unrecorded `sc007-*` logs moved to `raw/t029/old/`).
- SC-002: 64 / 128 / 256 → 3.3 / 4.7 / 7.3 ms median with `out_latency_ms`; the measured part is
  always the bridge's 2 ms margin. **128 kept** (maintainer); research R3 now says the default
  balances delay against the graph's CPU cost, T029's wording changed with it.
- SC-007: four 60 s runs alternating sound and `--no-audio`, spec #3's SC-003 holds; `lat_ms`
  about 0.25 ms higher with sound.
- SC-004: computed sound ≈ 25 ms ahead of the image (4.7 − spec #3's ~30 ms glass to glass); by
  ear through VBAN (the PC has no speakers): no offset noticed. Accepted by the maintainer; the
  direct listening is Windows' (T028).
- SC-003/SC-005: 31-minute run, minute 0 and minute 30 both 2.0 ms (+0.0), `wait_ms` 4.8/4.8 from
  second 8 to the end, no gap or drop in 1940 s.
- How the runs were done: the screen blanks during long runs, `kde-inhibit --power --screenSaver
  sleep <s>` keeps it on; to listen from Windows, `systemctl --user start vban.service` (stream
  `LinuxAudio` to Voicemeeter, `vban_out` becomes the default sink; stop it afterwards).
- Seen, for T034: when PipeWire moved the playback to `vban_out` (default change), the
  `[audio-source]` line kept naming the IEC958 output (the name update on Linux is T034's).
- The CachyOS clone is at `0bf962c` (fast-forwarded from a bundle), release and debug built there.

Volume (2026-11-17, T030–T032 done; committed in `feat: add the preview volume`):

- T030: icons `volume`, `volume_2`, `volume_1`, `volume_off`. Tabler has no low volume icon and
  its `volume-3` is the speaker with an X: `volume_1` is `volume-2` without its wave, `volume_off`
  is `volume-3` (the mockup's muted speaker); noted in the icons' LICENSE, T030 and research R12.
- T031: `app/ui/components/volume-popover.slint` (`VolumeButton` with its popover, `VolumeSlider`).
  The popover is a child of the button inside the chrome, not in the top layer, so Tab goes from
  the button into the popover (mute, slider) and then to settings; arrows ±5, Home/End on the
  slider; the popover keeps the chrome shown and closes when the chrome hides or the settings
  open. `IconButton` gained `takes-wheel`/`wheel`, `right-pressed`, and an empty tooltip hides the
  shown one (the button's tooltip is empty while the popover shows).
- Touch: no touch reached the UI before (winit handles Windows' pointer messages itself, so
  Windows makes no mouse events from touches; Wayland neither). `ui.rs` now forwards the first
  finger as the left button and sets the new `Input.touch` (Slint's public pointer events carry
  no finger); a mouse move or press clears it. This makes every control tappable app-wide.
- T032: `SettingChange::Volume` stores `[audio] volume`/`muted` (saved as other settings) and
  sends the controller's new `Event::Volume`, which gives `SetGain` while a session is open or
  opening; the icon name comes from `audio::icon_name` (`audio::volume::icon`). The wheel:
  `UserEvent::VolumeWheel` → `audio::Wheel` (60 px a notch, touchpad remainder kept) →
  `audio::volume::wheel`; **chosen here**: the wheel unmutes, as the slider (T031 text updated).
  The saved values are shown at start.
- Checked on Windows: fmt, clippy (app, all targets), 173 app tests; the app shows the button
  between fullscreen and settings and the popover on hover (80 %). The maintainer tried the
  Windows UI (2026-11-17) and approved it.
- T033 done (maintainer, 2026-11-17, Windows and CachyOS): no click, change heard at once,
  restart keeps level and mute, mouse and keyboard behave as FR-011 (no touch screen on either
  PC). On CachyOS (ezcap, release at `cd030f1`, playback through VBAN) a watcher polled
  `wpctl get-volume` every 200 ms for the default sink, the app's own playback stream and another
  app's stream (kded6): all stayed at 1.00 for the whole test, so the OS volumes are untouched.

Input and output choice (2026-11-17, T034 done, committed in `feat: add the audio input and output choice`; nothing to see until T035 wires the picks):

- T034: `app/src/audio.rs` reads the input choice from `[devices."<id>"] audio` and the output
  from `[audio] output` at each open; the app writes the settings and then sends the new
  `Event::Input(AudioChoice)` / `Event::Output(OutputChoice)` (not sent yet: T035 wires the
  picks). An input pick opens a new session (Close + Open: both sides, the bridge empty) unless
  the choice resolves to the input already playing; No audio closes it (state Off). An output pick
  sends `ReopenOutput` only when the output to open changes.
- New state `AudioState::Disconnected(name)` for a chosen input that is missing; it opens on its
  `Added`. The name is the last one seen since start (a `names` cache filled by `Listed` and
  `Added`), else the endpoint id (an input chosen in an earlier run and never seen in this one
  shows its id; settings keep no name for it).
- Output fallback: `fallback: Option<id>`, set when the chosen output is missing at open or its
  side reports any `Error` (the side reopens on the default), cleared by the output side's
  `Opened` on it or a pick of System default; `AudioController::fallback()` gives the name for
  T041's header. The chosen output's `Added` moves the sound back.
- `DefaultOutput`: reopens an output on the default when the default changes on Windows
  (`reopens_on_default = platform::IS_WINDOWS`; tests build both), and on both systems when that
  output side had failed (Linux after PipeWire restarts). Removed endpoints do nothing by
  themselves: the open side's own `Error` acts.
- Menu model: `audio::Item { text: Label, current, pick: Option<Pick> }`, `menu()` (`Input`
  group only while a video device is open: Auto with the pair's name or None, every input, No
  audio; then `Output`: System default with the default's name, every output), `output_items()`
  for the popover's select, `value()` for the select's value. A missing chosen endpoint is not
  offered and nothing in its group is marked. Input `Error`s are still only logged (T040).
- Checked on Windows: fmt, clippy (app, all targets), 191 app tests (19 new in `audio.rs`).

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

1. Phase 5 (US3, input and output choice): T035 (the Audio select and the picks' settings
   writes in `app.rs`, using `AudioController::menu`/`output_items`/`value`). The CachyOS clone is at `cd030f1`,
   release built there.
2. With the ezcap still on CachyOS: the Linux halves of T036, T042, T044 as their code lands;
   T028 and T039 when the ezcap moves to Windows.
