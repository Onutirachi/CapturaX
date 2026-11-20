# Handoff: Preview Audio

**Updated**: 2026-11-20 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

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
  The popover is a child of the button inside the chrome, not in the top layer; it holds only
  the slider and its value and takes no focus (FR-011 as changed 2026-11-18, see below); it keeps
  the chrome shown and closes when the chrome hides or the settings open. `IconButton` gained
  `takes-wheel`/`wheel`, `right-pressed`, `takes-hold`/`held` (a 500 ms press; its release clicks
  nothing), `key` (keys other than Enter and Space), `label` (the screen reader name, the tooltip
  by default), and an empty tooltip hides the shown one (the button's tooltip is empty while the
  popover shows).
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
  audio; then `Output`: System default with the default's name, every output), `value()` for
  the select's value. A missing chosen endpoint is not
  offered and nothing in its group is marked. Input `Error`s are still only logged (T040).
- Checked on Windows: fmt, clippy (app, all targets), 191 app tests (19 new in `audio.rs`).

Audio select (2026-11-18, T035 done, committed in `feat: add the audio select`):

- Toolbar: the fourth select `Audio` (icon `volume`, `volume_off` for No audio); its value is
  `AudioController::value` shown through the select's `shown`. Its `current` is the input row in
  use, -1 without a video device, so the button shows no value then; the select stays enabled
  without a video device and its menu holds only the `Output` group (System default, every
  output). The narrow toolbar's icon look counts four selects.
- `MenuItem` gained `checked`: a menu with a choice per group marks both the input and the output
  in use (`Menus.current` marks one row only). `ui::audio_item`/`audio_label` turn
  `audio::Item`/`Label` into rows.
- `app.rs`: `show_audio()` (called by `show_menus`, after every `UserEvent::Audio`, and after a
  pick) keeps `AudioMenus` (menu, output rows, value) and sets the Slint models when they change.
  A pick writes `[devices."<id>"] audio` (`touch_device` with the video device's base name) or
  `[audio] output` and then sends `Event::Input`/`Event::Output`. `AudioController::video()` and
  `Sound::controller()` added; `default_output()` is now test-only.
- **Chosen here**: `System default` without a name when the system has no output (a text not in
  FR-022; pt-BR `Padrão do sistema`). Texts added to the catalog: `Audio`, `Input`, `Output`,
  `Auto ({})`, `Auto (none)`, `No audio`, `System default ({})`, `System default`.
- Checked on Windows: fmt, clippy (app, all targets), 191 app tests. The app (debug, no video
  device usable here: NVIDIA Broadcast's camera was in use) showed the Audio select in the icon
  look, its menu with `Saída` and the default checked, and a pick of the G533 written to
  `[audio] output` and back to `default`. The Input group was not seen (needs the ezcap or the
  C920: T036).

Volume popover and output change (maintainer, 2026-11-18, during T036; folded into `feat: add
the preview volume` and `feat: add the audio select`, the spec updated first):

- The output is one for the app and is chosen in the Audio menu only (FR-009): the popover's
  Output select is gone (`OutputPicked`, the `outputs` rows of `AudioMenus` and `Select.expanded`
  with it; `output_items()` is private to the menu).
- The popover holds the slider and its value only (no mute button: two speaker icons side by side
  looked wrong). The button does everything: click mutes; touch tap opens, a 500 ms hold mutes;
  the keyboard stops on the button once (its focus shows the popover, which takes no focus),
  Enter/Space mutes, arrows ±5 and Home/End move the level (FR-011). Texts `Mute` and `Unmute`
  left the catalog.
- T033 is open again for the mouse and keyboard behaviors (no touch screen on either PC).
- Found in T036 (Linux, 2026-11-18): the C920 picked as the ezcap's input and unplugged gives the
  input `Gone`, which is only logged; the session stays "Playing", so the C920's `Added` reopens
  nothing and the select still names it. This is T040/T041's work (`Disconnected`, reopen on
  `Added`, the tag); the input unplug is checked in T042, not T036.
- Maintainer, 2026-11-18 on CachyOS (ezcap and C920, release with this change): the new popover
  with mouse and keyboard (T033's Linux recheck) and T036's Linux half are ok. No headset on that
  PC: plugging a headset that becomes the default and the G533 unplug are checked on Windows.

Audio states (2026-11-18, T040 and T041 done, committed in `feat: add the audio states`):

- `AudioState` gained `Blocked`, `Held(name)` and `Failed(name)`. **Chosen here**: `Held` comes
  now, not with T038, because a side's `InUse` needs a state and T041 already shows `In use`;
  T038 keeps the exclusive flags and `InUse` (`SharedAfterInUse`).
- An input `Error` closes the session: `Gone` → `Disconnected(name)` (the input leaves the list at
  once; a later `Added` with its id opens it, other inputs and a list read again change nothing, so
  Auto's pair that left stays Disconnected, not No paired audio), `Blocked` → `Blocked`, `InUse` →
  `Held`, `Format`/`Failed` → `Failed`. This fixes T036's C920 unplug finding.
- Microphone access is read before opening (`Denied` → `Blocked`, nothing opened) and every 3 s
  while Blocked (app tick, as the camera); it opens once a read went off and then no longer is.
  **Changed in the spec first**: data-model.md said "when the window regains focus, as the
  camera's", but the camera reads every 3 s; data-model.md and T040's text now say 3 s.
- Output on the system default: `Failed`/`Format`/`Blocked` → `Failed(default name)`, `InUse` →
  `Held(default name)`, the input side kept; it reopens when `DefaultOutput` names an output
  (state back to Opening) or on Try again (new session). `Gone` → Opening without a tag (Windows
  names the next default at once). A chosen output's errors still fall back with the flag.
- Try again works in `Held`/`Failed` only (close + resolve). Nothing retries by itself (the
  monitor's list changes nothing in these states).
- Menu: `Label::Notice` (kind `state`: title bold, detail below, on a `top` well; 54 px with a
  detail, 36 without) and `Label::Action` (kind `action`: a soft accent chip, 40 px, picked like
  any row so the keyboard reaches it; `Pick::Action` → `Event::TryAgain` or
  `platform::open_privacy(Microphone)`). The fallback notice shows while Playing.
- Select: `state-tag`/`state-tag-kind` (`warn`: Disconnected, In use; `bad`: Blocked, Failed) shows
  the tag before the value, turns the icon into `alert_triangle` in that tone (also the icon look)
  and starts the tooltip with it. `Tag` gained the `warn` and `bad` kinds (tone on a soft fill).
- FR-018: `show_audio` drops the notice and tag unless the preview is Opening, Playing or No
  signal (an input that leaves a moment before its video), and `show_preview_state` calls it.
- Texts added (pt-BR in the catalog): `Disconnected`, `Blocked`, `Failed`, `No paired audio`,
  `Choose an input`, `Audio device in use`, `Playing on the system default`; reused `In use`,
  `{} disconnected`, `Waiting to reconnect`, `Access blocked`, `Desktop app permission is off`,
  `Cannot open {}`, `Try again`, `Open privacy settings`. `Failed` of a default without a name
  says `Cannot open System default`.
- Checked on Windows: fmt, clippy (app, all targets), 205 app tests (14 new).
- Maintainer, 2026-11-18 on CachyOS (release with this change): the C920 picked as the ezcap's
  input and unplugged shows the `Disconnected` tag and header, and its sound returns by itself when
  plugged back. The C920 under Auto pairs its own microphone (accepted), so No paired audio needs a
  camera without one (NVIDIA Broadcast on Windows). The CachyOS clone was moved to this branch's
  head; its old state is in `stash@{0}` and the branch `backup/07b88fe` (nothing new in either).

States and closing on Linux (2026-11-18, T042's Linux half, T044 partly; folded into `feat: add
the audio states`):

- `audio session N closed in X ms` is now logged when a session's `Closed` comes after a switch
  (before, only the quit was timed); quickstart names the lines SC-008 reads.
- Run over SSH on CachyOS (clone wiped and cloned again from a bundle at `43a5eaa`; `ydotool`,
  screenshots in `~/capx/out`, the run script `~/capx/t042.sh`): No paired audio (v4l2loopback),
  Disconnected (C920 `unbind`/`bind`) and Output fallback (null sink `capx-test`) in English and
  Portuguese; 101 closes, max 15 ms, none missed. Results in measurements/acceptance.md.
- Found: the C920 resets on USB when it resumes from autosuspend (2 s), so it is gone ~3 s at the
  app's start after being idle; the app recovers (Disconnected, then Playing on Added). Not an app
  bug. The ezcap is PCIe: quickstart's "unplug the ezcap" row now uses the C920.
- `pactl unload-module module-null-sink` (by name) removed VBAN's sink too; restored with
  `systemctl --user restart vban`. Quickstart now says to unload by id.

Responsiveness on Linux (2026-11-18, T044's Linux half done; results in
measurements/acceptance.md, method in quickstart "Responsiveness"):

- Over SSH on CachyOS (release `43a5eaa`): mute 40–41 ms to digital zero, unmute 29–30 ms, wheel
  volume steps 0–7 ms (recorded from the app's own playback node with `pw-record`); a changed
  system default followed in 104–120 ms (WirePlumber moves the stream, no reopen; the menu names
  the new default); a null sink appears in and leaves the open Audio menu in under 0.9 s.
- The first run of ten mute clicks changed nothing; the repeat toggled every time. Recorded as is;
  not seen by hand.
- Scripts on the test PC: `~/capx/t044vol.sh`/`.py`, `t044wheel.sh`/`.py`, `t044def.sh`.

Windows session (2026-11-19, ezcap and C920 on the Windows PC):

- Volume law changed to the square (`gain = v²`, commit `feat: use the square law for the preview
  volume`): the maintainer heard almost no difference from 0 to 20 % with the cubic law (20 % =
  −42 dB). research R8, data-model.md and T010's text changed first. Not heard yet with the new
  law.
- T033 done: the maintainer checked the popover with mouse and keyboard on Windows; the restart
  (level 64 and muted kept) checked here from the settings and a screenshot.
- T036 Windows, by the maintainer: changing the system default moves the sound at once with
  System default selected; a chosen output unplugged plays on the default and returns when plugged
  back. The select keeps naming the chosen output meanwhile (by design: data-model "Device(id)").
  Not confirmed yet: that the `Disconnected` tag and `Playing on the system default` header showed.
  The G533 cannot be unplugged (it is behind Voicemeeter): use the ARZOPA monitor's speakers
  (easy to unplug) or disable a Voicemeeter virtual endpoint instead. Headset plugged as the new
  default: not done.
- T042 Windows partial (measurements/acceptance.md): Blocked and In use in both languages, 100
  quits (3–13 ms). Found: an input held exclusive by another app gives In use, not Failed;
  quickstart's table changed (In use is forced with `audio_session --exclusive-in`, no T038
  needed). Failed has no way found to force it without administrator rights. The NVIDIA Broadcast
  camera says `In use` unless the Broadcast app runs (starting it from here did not keep it up).
- Found: the Blocked detail in Portuguese is cut in the menu (`Permissão para aplicativos da área
  de trabalh…`); the maintainer decides between a shorter text and wrapping.
- Afternoon (commits `37a6118`, `c434a91`, maintainer's decisions of 2026-11-19, spec, data-model
  and settings contract changed first):
  - A missing chosen input or output stays in the Audio menu, marked, with `Disconnected`, until it
    returns or another is picked; Auto keeps the name of its paired input that left.
  - The notice is a card: generic title (`Device disconnected`, `Device in use`, `Cannot open`),
    the device's name line, the detail (wraps), the action inside the card as a neutral button;
    tinted warn (yellow) or bad (red), neutral for No paired audio; height follows its lines (Slint
    measures it, `Menus.notice-height` feeds Rust's `item_top`). The light theme's notice colors
    and the English texts were not looked at on screen.
  - `[audio_devices."<id>"] name` is now written for chosen endpoints too, so one missing at start
    is named (not its id).
  - Failed accepted by the unit tests (maintainer). No paired audio forced by disabling the ezcap's
    recording endpoint in `mmsys.cpl` (the NVIDIA Broadcast camera pairs its own microphone by name
    while Broadcast runs).
- Found for later (not this spec):
  - Every dropdown's bottom touches the window's edge: give menus the docks' margin from the
    window (maintainer, 2026-11-19).
  - Dropdowns animate when opening but not when closing (maintainer, 2026-11-19).
  - The Audio menu's `No audio` is the last row of the Input group; it should come first, next
    to Auto (maintainer, 2026-11-20: fix later; this spec's menu).
  - Names cut with "…" inside the open menus scroll so they can be read whole: a pause at the
    start, a slide to the end, a pause there, back to the start, again (maintainer, 2026-11-20:
    later; only inside menus and lists, not in a closed dropdown; only the row under the pointer
    scrolls, everywhere; the others keep their "…"). It replaces the wrapped names of the
    settings' Audio list (one line again).
  - Smooth scrolling everywhere something scrolls (menus, settings panel): Slint's `Flickable`
    jumps per wheel notch; a wheel handler that eases `content-y` toward a target would do it
    (maintainer, 2026-11-20: later, with the other cosmetic fixes).
  - Device preview (spec #3): the NVIDIA Broadcast camera at its largest mode (3840×2160) fails to
    start and shows the video state `In use`, though nothing holds it; at 1080p it plays. A mode
    that fails to start is reported as in use.
- Driving the app on Windows: scratchpad scripts (not kept) used `SetCursorPos`/`mouse_event` and
  `Graphics.CopyFromScreen`; the window is the process's `MainWindowHandle` only once its title is
  `CapturaX` (before that it is winit's hidden `Winit Thread Event Target`). The settings and
  `capturax.log` (UTC times) are next to the exe in `target/release`. Windows refuses to bring the
  window forward while the maintainer types in VS Code: press Alt before `SetForegroundWindow`,
  check `GetForegroundWindow` before any click, and `ShowWindow(SW_MAXIMIZE)` if it was restored.

Windows, second session (2026-11-19 evening, release of `c434a91`; results in
measurements/acceptance.md):

- T042 done: the C920 unplugged as the ezcap's input (Disconnected card, back 87 ms after its
  endpoint returned, same id on another USB port), Output fallback in English (ARZOPA, 43–45 ms),
  the light theme's red and yellow cards. The C920's microphone cannot be held exclusive here
  (another app, likely NVIDIA Broadcast, has it open); In use is forced on the ezcap's input.
- T036 done: no other headset to plug (maintainer); covered by the default change and the ARZOPA
  appearing in and leaving the open menu.
- T044 Windows partial: sound before the first frame (130–438 ms), input picks 7–83 ms, output pick
  69 ms, reconnect 87 ms, fallback 43–45 ms, default change 28–56 ms (12 switches by
  `IPolicyConfig::SetDefaultEndpoint`, maintainer allowed), menu updated in place. Left: volume and
  mute (≤ 100 ms) with a steady sound (PS5) and the exclusive box (T039).
- The C920's microphone sounded very late to the maintainer: measured (acceptance.md, T044
  Windows) the app adds ~24 ms on a real endpoint; on `Voicemeeter Input` 111 ms against 144 ms
  for Windows' own "Listen to this device" (the maintainer heard both alike); the C920 itself
  60–68 ms. Not an app fault; where the ~87 ms of the virtual endpoint go is not separated (the
  maintainer trusts Voicemeeter's engine). Method: `tools/latency/keylat.py` (global key hook + C920
  via `sounddevice` + loopback of the app's output via `soundcard` (it cannot open the C920),
  cross-correlation). The G533 cannot be looped back (Voicemeeter holds it exclusive). **Before
  T028**: the quickstart plays the app on a Voicemeeter virtual input; check what of Voicemeeter's
  buffering the recording includes.
- Found: a monitor change (the ARZOPA is a monitor) holds the main thread 0.5–3.4 s (render log
  behind, Windows moves the window); the audio is not affected. Not this spec's work.
- Driving notes: the toolbar shows when the pointer is over the top of the video (y ≈ 60), not
  anywhere; `SetCursorPos` plus a small `mouse_event` move makes Slint see it. **After a script
  brings the app forward, the maintainer's keys and clicks go to it**: ask them to click VS Code
  before typing (a stray click and Enter re-picked rows in an open menu). Editing `capturax.toml`
  by hand: a broken line makes the app keep it as `.damaged-*` and start from defaults.
- `target/release/capturax.toml` is set to the ezcap, English, output System default, C920's
  microphone not chosen (Auto); the original is in the scratchpad only (theme dark, language auto,
  muted, C920 as device).

Windows, third session (2026-11-19 evening, release of `c434a91`, PS5 on the ezcap; results in
measurements/latency.md and acceptance.md):

- **Measurement method changed (maintainer)**: the app plays on `Voicemeeter In 5`, which
  Voicemeeter reads as a hardware input strip; bus B3 carries the line input (left) and the app
  (right) and is recorded with `sounddevice` from `Voicemeeter Out B3` (scratchpad `rec.py`, not
  kept: `sd.rec` on the WASAPI device, 25 s, 48 kHz stereo). Why: on the same session the app and
  the spike gave 52/53 ms on `In 5`, 68/66 ms on VAIO3 and 99 ms (app) on `Voicemeeter Input`; the
  reference (Voicemeeter reading the ezcap by WDM) is −28 ms (spike −26). So a virtual input adds
  its own buffer, and the spike's 59 ms of October was on a virtual input. spec ("Latency budget
  and how it is measured", SC-001), research R10 and quickstart changed first; limits unchanged.
  The maintainer's everyday output stays a virtual input (VAIO3): its extra delay is recorded next
  to each result. The spike's engine is measured in the same session: built from
  `backup/before-linux-fixups` (46b66bb) in a scratchpad worktree, `spike/` workspace,
  `cargo build --release -p audio-preview`, then `audio-preview --engine wasapi --input ezcap
  --output "In 5" --stats` (the worktree is still registered: `git worktree remove` it when done).
- T028 shared done except the drift runs: SC-001 52 ms (p95 54); glass to glass with sound 29.0
  ms and `--no-audio` 28.9 ms (SC-007 passes); A/V computed 23 ms behind (VAIO3 39 ms) and in sync
  by ear (SC-004 shared passes).
- T044 Windows: volume and mute done (loopback of `In 5` calibrated with beeps: the loopback tap
  itself is ~100–110 ms late on VAIO endpoints; the app's change is about 0–20 ms). Wheel fine
  with real spacing; 10 notches in the same instant lose two (script only, not worth a change).
  Left: the exclusive box (T039).
- The C920's late sound (previous session) explained: camera 60–68 ms, app ~24 ms,
  `Voicemeeter Input` ~47 ms more than `In 5`, plus Voicemeeter to the G533.
- Found for later (not this spec): **fullscreen entered from a maximized window leaves the
  taskbar on top** (maintainer, Windows): in fullscreen the window still had `WS_MAXIMIZE` and
  `WS_CAPTION` (style 0x17CF0000, rect covering the monitor), so Windows does not treat it as
  fullscreen. From a restored window F11 is fine. Likely winit's borderless fullscreen keeping the
  maximized state; `app.rs` `apply_fullscreen`.
- Driving: scratchpad scripts (not kept) `shot.py` (restore with `SW_RESTORE` if minimized, Alt +
  `SetForegroundWindow`, hover y=60, screenshot), volume button at window (52, 535) with the window
  maximized on DISPLAY1 (screen −1876, 527). Minimize the app at the end of a script so the
  maintainer sees the chat again (they cannot tell when a script is done otherwise).
- `target/release/capturax.toml`: ezcap, audio Auto, output `Voicemeeter In 5`, volume 80, English
  (original in the scratchpad only). Voicemeeter: B3 = line input L + `In 5` R; VAIO3 and the
  ezcap WDM strip were also routed to B3 during the session (check before recording).

Windows, fourth session (2026-11-19 night; results in measurements/drift.md):

- **T028's drift found a delay that grows after the app**: about 0.8 ms per minute on `In 5` and
  on VAIO3, linear, no reset of its own, while `wait_ms` stayed steady; reopening only the output
  (Audio menu) brought it back (85 → 58 ms). The maintainer doubts it is Voicemeeter (never
  noticed drift in other apps over days); untested lead: the `IAudioClient3` short period, which
  other apps do not use. **Palliative (maintainer)**: spec FR-003a, research R16, T047
  (`crates/audio/src/resync.rs`) and T048 (`stream.rs`): a shared Windows output reopens itself
  at the first quiet moment (−50 dBFS for 300 ms) after 5 minutes. With it: 62 → 41–46 ms and
  steady, nothing heard at the reopens; the maintainer played on it "working very well". SC-003
  and SC-005 Windows recorded as passing (drift.md). The cause is investigated after the app is
  done.
- **Measurement output changed (maintainer)**: Windows runs use VAIO3 (the everyday output), not
  `In 5` (spec "Latency budget"; memory). SC-001 shared stays as measured on `In 5`.
- `crates/platform` now depends on `crates/audio` (Windows only) for `Resync`.
- Recorder: scratchpad `recstream.py` (not kept): `sounddevice` WASAPI input streamed to a 16-bit
  WAV, prints peaks every 10 s. Check Voicemeeter's B3 first: `Voicemeeter Input` (the Windows
  default) was routed to B3 too and spoiled one check.
- `target/release/capturax.toml`: output VAIO3, ezcap, English.
- Found: each output open starts at a different delay (62–67 ms fresh, 42–58 ms after a reopen);
  part of R16's open question. Short input gaps (8–16 ms) appear while a game plays, not heard.

Windows, fifth session (2026-11-19 night; results in measurements/drift.md "T028: the G533
directly"):

- **The G533 popped** for 30–60 s at a time with the shared queue at 1.5 periods: it runs on a
  10 ms period (no short period), and in stretches Windows takes nothing at one wake and 20 ms at
  the next, more than the 15 ms queued. Found with a diagnostic build (per-second wakes, empty
  queues, frames written, padding range, largest wake interval). **Decision (maintainer)**: spec
  FR-003b, research R17, T049: a shared output on a 10 ms period or longer keeps its whole buffer
  queued (22 ms on the G533, about 7 ms more); short periods unchanged. Run with T049: 30 minutes,
  no pop heard. T028 done.
- The diagnostic code (per-second output wakes, `CAPTURAX_EXP_MARGIN_MS`) was dropped with its
  branch (maintainer); a copy may remain in the session scratchpad as `exp-g533-diag.patch`.
- Left as seen (research R17): short bridge gaps of 10–20 ms when Windows takes 22 ms at once, and
  a ~100 ms stop of the whole system once; none heard.
- Driving: an earlier app instance (PID from a previous run) kept running, so a new start only
  showed it (single instance) and exited at once: check `tasklist` before starting a run.
  `taskkill` without `/F` closes the app cleanly.
- `target/release/capturax.toml`: output back to VAIO3, ezcap, English.

Exclusive switches (2026-11-20, T037 and T038 done, not committed yet):

- **Redesign (maintainer, same day, after seeing a first version with checkboxes and groups)**:
  tabs `Inputs` · `Outputs`, column headers `Device` · `Exclusive`, the shell's toggle instead of a
  checkbox (spec FR-014/FR-022, plan, research R12 and T037 changed first). A mockup with a radio
  to choose the devices there too was published (https://claude.ai/artifact/5c5eScpvnQKrUuWiD5LzWV)
  and **rejected**: no device choice in settings, so the category stays Windows only. The
  checkbox component and its icon were dropped.
- `toggle.slint` gained `held`: a held switch only asks (`toggled(!checked)`), Rust saves and sets
  the row (`ui::sync_rows` changes a same-length model in place, so a switch keeps the keyboard
  focus). The whole row flips it. Its accessible label is the endpoint's name.
- Settings panel: category `Audio` (icon `volume`) between Preview and About, only when
  `Prefs.exclusive-available` (`platform::IS_WINDOWS`); `Heading` (name + line), `Tabs` (new local
  component: accent bar under the chosen one, one Tab stop, Left/Right/Home/End), `ColumnHeads`,
  one `EndpointRow` per endpoint of the tab in the system's order, an outlined `Preview` tag under
  the name (whole, wrapped; maintainer: room for several tags) for the sides opened now (maintainer: a tag per user of the device,
  recording and streaming add theirs later; not in the Audio menu for now). The tab resets to
  Inputs each time the panel opens. Texts in the catalog (`Device`, `Preview` already there).
- Audio menu: an orange `Exclusive` tag (`Tag` kind `accent`) after the name of every row whose
  device has the switch on (Auto and System default for the device they name), also in the closed
  dropdown as the other dropdowns' tags; the state tag `Exclusive` was dropped (maintainer).
- After a design review (`design:design-critique` skill, maintainer asked to apply it): the rows
  in use come first (`AudioController::exclusive_rows`, stable sort), the heading, tabs and
  headers sit above the panel's `Flickable` (`audio-head`; the `Flickable` starts under it), the
  switch is described as `Exclusive` to screen readers (`Toggle.description`), another tab starts
  at the top, and the menus' position indicator shows at the panel's right whenever its rows scroll
  (any category; maintainer: the list did not look scrollable). Left out: a count on the tabs and
  splitting the names.
- Maintainer, 2026-11-20 (ezcap as a Voicemeeter WDM input): with the switch off the app and
  Voicemeeter both hear the card (the sound doubles), so Voicemeeter's WDM is shared; with it on the
  app takes the card from Voicemeeter, as research R2 measured and the settings line says. Whether
  Voicemeeter gets it back by itself when the app lets go is T039's check.
- Another app taking the card (scratchpad `steal.py`, not kept: `uv run`, `sounddevice` WASAPI
  input opened exclusive, int16 48 kHz stereo, held N s): with the app exclusive the script is
  refused (PortAudio `Invalid device`, -9996); with the app shared the script takes the card and
  the app goes `Held("HDMI (ezcap Live GAMER ULTRA)")` at once (`Error { side: Input, error:
  InUse }`), card `Device in use` / name / `Try again`, alert icon and `In use` tag; after the
  script let go, Try again reopened the session shared and Playing (15:27:57).
- Checked on screen after the redesign (release, pt-BR, maximized): tabs, headers, switch on and
  off from a row click with the saved value shown, the log line for each change.
- `audio.rs`: `Event::Exclusive(id)`; the request carries both flags; a change reopens only the
  side on that endpoint (`ReopenInput`/`ReopenOutput`), nothing when no session or another
  endpoint. **Chosen here** (data-model.md changed): the system default output uses the box of the
  endpoint it names; `AudioState::InUse(name)` (the refused side's name, input first) with notice
  `Device in use` / {} / `Exclusive mode unavailable`, no action; in `InUse` the menu shows that card,
  not the output fallback's. No state tag while Playing exclusive (the menu rows' tag says it);
  `select.slint` shows the alert icon only for `warn`/`bad`.
- Checked on screen here (release): the category, the row hover, a click saves
  `exclusive = true` and unchecking drops the entry. Then, with the old app closed (ezcap, PS5 off,
  VAIO3 output): checking the ezcap's input reopened only the input, `Exclusive` (I16, 3 ms period)
  19 ms after the click; checking VAIO3 reopened only the output, `Exclusive` (I24, 2 ms) in 12 ms
  (**VAIO3 does take exclusive mode**, contrary to an earlier note); a restart opens both exclusive
  at once. `In use by the preview` marks both rows; the `Exclusive` tag shows in the wide toolbar
  (a 960 px window uses the icon look, which shows the icon alone, as R12 says). The C920's
  microphone checked and picked: `exclusive refused (no usable format); shared`, state
  `InUse("Microfone (HD Pro Webcam C920)")`, yellow alert icon and the card `Device in use` / name /
  `Exclusive mode unavailable`. Unchecking a box of an endpoint not in use reopens nothing. pt-BR
  texts checked (Áudio, Modo exclusivo and its line, Entradas, Saídas, Exclusivo, Em uso pelo
  preview). Not heard by ear yet (maintainer).
- `target/release/capturax.toml` after this run: language `pt-BR`, every endpoint shared (the
  maintainer turned them off). The S23 camera's stored name was already double-encoded
  (`CÃ¢mera`) before this session.
- Driving: scratchpad `shot.py` (Tab reveals the docks; settings button at window (43, 364) with
  the 960 × 511 window, (52, 589) maximized; `wheel x y n` scrolls); `grab.ps1` captures with
  `-ExecutionPolicy Bypass` (no PIL here).

T039 and T050 (2026-11-20; results in measurements/latency.md, drift.md, acceptance.md):

- T037/T038 committed (`5813666`) before this session.
- **Output (maintainer)**: "both exclusive" means the ezcap's HDMI input and the output; measured
  on VAIO3 only (real use), no `In 5`. Release of `5813666`, then of T050.
- Input exclusive, VAIO3 shared: 20 ms at the start (SC-001 ≤ 50 passes). Its 31-minute run had
  jumps of −27 and +55 ms with no event in the app and the preview *ahead* of the line input for
  minutes (no pairs there; found with a per-window cross-correlation, scratchpad `xcorr.py`, not
  kept). Both exclusive: 26–34 ms at each open (over 22 ms) and +0.51 ms/min linear over 30
  minutes, no jump, `wait_ms` steady at ~6 ms. So the growth is after the app with or without the
  mixer and the short period; likely Voicemeeter's VAIO buffer (research R16, not proven).
- **Decision (maintainer)**: no more 30-minute runs; the exclusive output resyncs too (spec
  FR-003a, research R16, T050: `stream.rs` feeds `Resync` on both modes). Checked: three resyncs
  reopened the exclusive output in 8 ms each, nothing heard; picture and sound in sync by ear
  (SC-004 exclusive). SC-001 both exclusive (≤ 22 ms) recorded as not met, cause after the app.
- Another app playing on VAIO3 (scratchpad `play.py`, not kept: `sounddevice` WASAPI shared, 1 s
  tone) was refused (`Invalid device`) while the preview held it exclusive and played at once after
  the app quit. T044's exclusive box recorded from the T037/T038 checks (12–19 ms); T044 done.
- Recorder: scratchpad `recstream.py` (not kept), as the fourth session's. Raw logs
  `raw/win-2026-11-20-t039-*.log` and `t050-resync-excl.log`.
- `target/release/capturax.toml`: output VAIO3, ezcap's input and VAIO3 exclusive, pt-BR (the
  morning's copy in the scratchpad only: output `Voicemeeter Input`).
- Found for later (maintainer, 2026-11-20; not this spec, likely the device preview's or the
  shell's): **Windows turns the screens off for inactivity while the preview is showing**. The app
  should keep the display awake while a preview is visible (Windows
  `SetThreadExecutionState(ES_DISPLAY_REQUIRED | ES_CONTINUOUS)`, Linux an idle inhibit), and let it
  sleep when the window is minimized or in the tray, or the preview is off (future).

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

- Windows: why Windows sometimes takes two periods at once on the G533 (research R17); not
  needed for the fix.
- Windows: why the delay grows 0.5–0.9 ms per minute after the app's output (research R16);
  after the app is done. Exclusive grows too, so the short period is not it. First test: another
  app (Windows' "Listen to this device") playing the ezcap on VAIO3 for 30 minutes.
- Validated during implementation, before the code that depends on them: the Linux engine probe
  and the pairing keys (`--list-devices`) on both systems (quickstart).

## Next step

1. Commit after the maintainer validates: `feat: resync the exclusive output too` (stream.rs,
   spec, research, tasks T050) and `docs: record the exclusive runs` (measurements, tasks T039 and
   T044, handoff).
2. Left: T043 (docs), T045 (acceptance through User Stories 1–5 on both systems), T046 (cleanup),
   T005's Linux build (the probe, not compiled yet).
