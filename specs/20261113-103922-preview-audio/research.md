# Research: Preview Audio

Decisions for [plan.md](plan.md). Each entry: decision, rationale, alternatives. Evidence comes
from the stack spike ([findings](../20261026-154808-stack-spike/findings.md) "Audio (Windows, via
Voicemeeter)" and "Linux"; research R3 and R7; its engine `spike/audio-preview/src/wasapi.rs` at
tag `spike/stack-20261026`), the device preview ([research](../20261106-170507-device-preview/research.md)),
and the `pipewire` 0.10.1 crate's documentation. "Inferred" marks what was not run; every inferred
point that the design depends on has a validation step in [quickstart.md](quickstart.md) before
the code that relies on it.

## R1. Engine shape: platform streams, a pure bridge

- **Decision**: the audio path has two layers.
  - `crates/platform` opens the two device streams with the OS API (WASAPI on Windows, PipeWire
    on Linux) and runs their real-time threads. Each stream speaks interleaved `f32` at the
    device's own rate and channel count: the input calls `on_input(&[f32])` for every packet it
    receives, the output calls `fill(&mut [f32])` for every packet it must send. Sample format
    conversion (WASAPI exclusive integer formats) stays inside the platform module.
  - A new pure crate, `crates/audio`, holds the **bridge** between them (research R4): the waiting
    audio, the rate follower, the channel and rate mapping, the volume ramp and the statistics.
    It has no OS call and no GStreamer, so all of it is unit tested, drift included (a simulated
    30-minute run with clocks apart).
- **Rationale**: constitution IV keeps OS calls in `crates/platform`; the logic that decides the
  delay, the drift and the clicks is the same on both systems and is the part worth testing
  without hardware. The spike's engine mixed both in one file.
- **Alternatives**: GStreamer elements (`wasapi2src/sink`, `pipewiresrc/sink`): replaced by the
  spike (hidden buffers, 108–121 ms, 2.6 ms/min drift on Windows; hang on stop and starvation on
  Linux). The bridge in `crates/platform`: possible, but then the pure logic is tested only where
  the platform module builds, and the platform crate grows a second responsibility.
  `cpal` for both systems: its WASAPI backend opens shared mode only with the default 10 ms
  period (no `IAudioClient3`, no exclusive), and its Linux backend is ALSA, not PipeWire.

## R2. Windows: WASAPI, from the spike

- **Decision**: port the spike's engine (research R7 "Revised again"), with these changes:
  - **Any device format**. The spike forced 48 kHz stereo and fell back to Windows' converter
    (and its 10 ms period) when the mix format was different. Now each side opens in its own
    format and the bridge maps channels and rate (R4):
    - Shared: `IAudioClient3::InitializeSharedAudioStream` with the device's mix format
      (`GetMixFormat`, always `f32` in practice; any rate and channel count) and the shortest
      period `GetSharedModeEnginePeriod` allows. If `IAudioClient3` is missing or refuses, the
      normal `Initialize` with the mix format and a 20 ms buffer (the spike's fallback, without
      `AUTOCONVERTPCM`).
    - Exclusive: the first format the device takes, trying sample formats `f32, i32, i24in32,
      i24, i16` × rates (the other side's rate, 48000, 44100, 96000) × channels (2, the mix
      format's count); minimum device period; event mode, one period per buffer; the
      `AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED` retry as the spike.
  - **Threads**: one thread per side (`audio-in`, `audio-out`), registered with MMCSS "Pro Audio",
    each waiting on its WASAPI event with a 100 ms timeout and checking a stop flag, so a stop
    request is seen within 100 ms. COM is initialized per thread (multithreaded apartment).
  - **Endpoints by id**: `IMMDevice::GetId` (stable endpoint id) instead of the friendly name.
  - **Output queue**: shared output keeps 1.5 periods queued in Windows (the spike's value) when
    the period is short, and its whole buffer when the period is Windows' normal 10 ms or more
    (R17); exclusive writes one period per event.
  - **Errors** (R7) come back as values; nothing panics on a device error.
- **Rationale**: the spike measured this engine at 59 ms shared, 46 ms with the card exclusive,
  18 ms with both exclusive, holding about 6–8 ms in the app with nothing cut (findings). The
  format change covers FR-004 (the C920's mono 32 kHz or 16 kHz microphone, a 44.1 kHz headset)
  without losing the short period on devices whose mix format is not 48 kHz stereo.
- **Alternatives**: Windows' `AUTOCONVERTPCM` for every format mismatch: forbids
  `IAudioClient3`'s short period, so a 44.1 kHz headset would pay 10 ms more. Automatic stream
  routing (activating the default render interface so Windows moves the stream when the default
  changes): fewer lines, but the app needs the default's name and the change event anyway (FR-009),
  and reopening is under 1 s (SC-006).

## R3. Linux: PipeWire directly (FR-016)

- **Decision**: the `pipewire` crate (0.10, bindings to the system's `libpipewire-0.3`), one
  thread per process running a PipeWire main loop (`pipewire` thread) that owns the connection,
  the registry listener (R6) and the two streams of the open audio session. Every call from the
  app is a message to that thread (`pipewire::channel`); the app never waits for it (R9).
  - **Streams**: a capture stream with `target.object` = the input node's `node.name`, a playback
    stream with `target.object` = the chosen output's `node.name` (or none: PipeWire's default,
    which it moves to the new default itself), both `media.type=Audio`,
    `media.category=Capture/Playback`, `media.role=Game`, `node.latency=128/48000`.
  - **Format held explicitly**: `F32` interleaved, the node's own channel count and positions from
    its `EnumFormat`, at the graph rate; never left to negotiation (the spike's `pipewiresrc` fell
    to mono). PipeWire's adapter converts what the device does not offer.
  - **One cycle**: both streams carry the same `node.group` (`capturax.preview`), as
    `libpipewire-module-loopback` does, so PipeWire schedules them under one driver, the capture
    `process` runs before the playback `process` in the same cycle, and PipeWire resamples the
    device that does not drive. The bridge's follower stays in place and should read close to
    0 ppm; the stats show it (validation in quickstart "Linux engine probe").
  - **No PulseAudio layer**: decided by the probe in quickstart, which repeats the spike's failing
    cases with the new engine: 10 start/stop runs each killed with `SIGINT` (the spike's hang: 4
    and 7 of 10), the input's delivered rate (the spike's starvation: 17–25 %), and the format.
    If the probe fails, implementation stops and the fallback (both sides through
    `pipewire-pulse`, as the spike) is decided with the maintainer.
    **Result (2026-11-16, measurements/probe.md)**: passed on CachyOS with PipeWire 1.6.10: 10
    of 10 runs closed at once, no gap in 60 s, format held, one driver for both streams, in two
    series (OBS open on the card, then closed). Only with another app already running the
    card's node at a larger quantum, two things happen in the first seconds after the streams
    start, left to the listening test (T029): the graph runs up to 0.8 % fast for about 4 s,
    and the output's ALSA node logs `resync` while it joins the card's cycle.
  - **In the session (T020, 2026-11-16)**: the format's rate is left out, so each stream runs at
    the graph's rate and PipeWire's adapter converts the device's (the C920's node is
    `32000/2/i16`, its samples come at 48000). A side is opened only after the connection's
    first list and only on a node that is listed, because PipeWire's session manager gives a
    stream whose target is missing another device. `Opened` is reported at the stream's first
    cycle: the cycle's size is the period and `pw_stream_get_time`'s delay the reported latency
    (ezcap input 192 frames, IEC958 output 128, the same 2 s later). The delay comes with the
    link's latency, which in about one start of ten arrives after the first cycle; a stream
    that says 0 there is asked again 100 ms later and reported then (a null sink's delay is 0).
- **Rationale**: the spike's hang and starvation were in GStreamer's PipeWire plugin (reproduced
  with `gst-launch-1.0`, not with `pulsesink`); the PipeWire client library itself is what every
  desktop app's sound goes through on CachyOS. The node group puts the delay at about one quantum
  plus the bridge's margin; at 128/48000 that is 2.7 ms per side, inside SC-002's 25 ms with room
  for the device's own buffer.
- **Alternatives**: loading `libpipewire-module-loopback` in the process (same scheduling, less
  code): no place for the bridge's gap, drop and rate statistics (FR-019) or for the app's volume
  ramp, and errors come back as module failures. Linking the card's ports straight to the
  output's (`pw-link`): no volume of the app's own (FR-011 would change the card's node volume,
  which other apps hear). `pipewire-pulse` (the spike's workaround): an extra server hop and a
  fragment size the app does not control. Smaller quanta (64): kept as a measurement variant;
  `node.latency` lowers the whole graph's quantum while the preview plays, which costs CPU for
  every app, so the default balances delay against that cost.
  **Result (T029, 2026-11-17, measurements/latency.md)**: 64, 128 and 256 all meet SC-002 (3.3,
  4.7 and 7.3 ms median with the output's latency, no gap). 128 stays (maintainer): 64 saves
  1.4 ms, which cannot be heard, at the cost above; 256 adds 2.6 ms for nothing on this PC.

## R4. The bridge (FR-002, FR-003, FR-004, FR-012)

- **Decision**: the spike's follower, generalized:
  - Waiting audio in a ring of input frames (input channel count), behind a `Mutex` locked for
    the copy only (both sides lock it once per packet, a few µs; the spike did the same).
  - **Rate follower** (the spike's constants): every 500 ms the lowest spare level seen at output
    events is compared with a **2 ms** margin; the read ratio moves to correct the error in about
    2 s, never beyond ±1000 ppm. Above 40 ms of spare audio (a stall, not drift) the oldest audio
    is dropped down to one input period plus the margin and counted (the spike dropped down to the
    margin alone, which in the bridge's tests left a 5 ms gap at the next output event when the
    next input packet had not arrived yet). At start, what piled up while the output opened is
    dropped down to one input period plus the margin before playing ("primed"); an empty ring
    plays silence, counted as a gap, and primes again; the sound then returns with the 10 ms gain
    ramp from 0, so it does not click.
  - **Rate and channels**: the read position advances by `ratio × input_rate / output_rate`; the
    sample is taken by 4-point cubic (Catmull-Rom) interpolation, which also serves the ±1000 ppm
    correction. Channels: mono to every output channel; stereo to the output's front left and
    right (other output channels silent); more input channels than the output mixed down
    (front pairs kept, center and LFE added at −3 dB to both sides, rears added to their side at
    −3 dB), then clamped.
  - **Volume** (R8) applied while writing the output, with a 10 ms linear ramp to every new gain,
    so moving or muting never clicks.
- **Rationale**: the follower is what kept the spike's delay flat (rate within ±170 ppm in its
  logs) without cutting; it covers FR-003 for both systems. Cubic interpolation is a few lines,
  has no latency of its own beyond one frame, and is clean enough for 48 ↔ 44.1 kHz (aliasing only
  above 20 kHz); the spike's linear interpolation is fine for ±1000 ppm, not for a 9 % rate
  change.
- **Alternatives**: `rubato` (a resampling crate with adjustable ratio): better stop-band, but a
  new dependency and a sinc filter's delay (about 1 ms) for a benefit above hearing range on a
  preview. Letting Windows convert (`AUTOCONVERTPCM`): R2. Dropping or repeating samples instead
  of the rate: audible clicks (FR-003 forbids it).

## R5. Pairing (FR-007)

- **Decision**: a pure rule in `crates/audio`: `pair(video, inputs) -> Option<input id>`.
  1. **System link**: each video device and each audio input carries a `link` key from the
     platform; a single input with the video's key is the pair; several (a card with two audio
     inputs) → the one whose name also matches (step 2), else the first by id.
     - Windows: the device's **container id** (`DEVPKEY_Device_ContainerId`, one GUID for every
       function of one physical device). Video: from the Media Foundation symbolic link (the
       device id after `mf:`) through `CM_Get_Device_Interface_PropertyW(DEVPKEY_Device_InstanceId)`
       → `CM_Locate_DevNodeW` → `CM_Get_DevNode_PropertyW(DEVPKEY_Device_ContainerId)`. Audio:
       `PKEY_Device_ContainerId` from the endpoint's property store. The null container id
       `{00000000-0000-0000-ffff-ffffffffffff}`, which every virtual endpoint reports (Voicemeeter,
       Steam's microphone; measurements/pairing.md), is no link.
     - Linux: the USB device's udev path. Video: `device.bus_path` from the V4L2 device's
       properties (part of its id), e.g. `pci-0000:00:14.0-usb-0:3:1.0`, with the interface
       suffix (`:1.0`) removed. Audio: the PipeWire device's `device.bus-path` (from udev
       `ID_PATH`), the same suffix removed; it comes with the bound device's info, the registry
       global does not carry it. Measured with the ezcap (measurements/pairing.md): both paths
       end at the interface (video `…-usb-0:3:1.0`, audio `…-usb-0:3:1.2`) and are equal without
       it; `--list-devices` prints both keys for the check (quickstart "Pairing").
  2. **Name**: when no input carries the video's key (no link from the system), names compared
     after lowercasing and removing the platform's decorations: on Linux the V4L2 suffix after
     `: ` and PipeWire's profile words (`Analog Stereo`, `Estéreo analógico`, `Digital Stereo`,
     `Mono`, …, the node's `device.profile.description` removed from its description); on
     Windows the endpoint's `(…)` device part is compared (`Digital Audio Interface (ezcap Live
     Gamer Ultra)` → `ezcap Live Gamer Ultra`). Equal, or one a prefix of the other at a word
     boundary, matches.
  3. Never across links: an input whose key is present and differs from the video's is never
     paired by name (two identical cards).
- **Rationale**: the system link is exact and survives identical names; the name rule is the
  spike's, kept as a fallback for devices without a link (virtual cameras, unusual drivers).
- **Alternatives**: names only (the spike): fails with two identical cards (spec edge case).
  GStreamer's device properties for the audio side: the app does not use GStreamer for preview
  audio (R1), and the Windows provider does not expose the container id.

## R6. Device lists and the default output (FR-009, FR-010)

- **Decision**: `platform::audio::Monitor` on each system, events to the main thread through the
  shell's `EventLoopProxy` as the video monitor does: `Listed { inputs, outputs, default_output }`
  once, then `Added`, `Removed`, `DefaultOutput(id)`.
  - Windows: `IMMDeviceEnumerator` for the list (active endpoints), `IMMNotificationClient` for
    `OnDeviceStateChanged`, `OnDeviceAdded/Removed` and `OnDefaultDeviceChanged(eRender,
    eConsole)`; callbacks only post events (Microsoft forbids blocking in them).
  - Linux: the registry on the `pipewire` thread: `Audio/Source` and `Audio/Sink` nodes (monitor
    sources excluded), `node.name` as id, `node.description` as name, the parent device's
    `device.bus-path` as link; the `default` metadata's `default.audio.sink` for the default.
    PipeWire answers in pieces (the global, the bound device's info, the node's `EnumFormat`, the
    metadata's properties), so every change asks the server for a `sync` and the difference from
    what was reported is sent when the latest one is answered: removals, additions, then the
    default output. The format is the node's first raw `EnumFormat`, open properties (several
    sample formats, a range of rates) read at their default. When the connection to PipeWire is
    lost (the server restarted), every endpoint is reported removed and the thread connects
    again, trying once a second; the new connection's endpoints are then reported added and the
    default output again, so the app's lists and the preview's device come back as after a
    replug (maintainer, 2026-11-16).
  - Ids: Windows endpoint id; Linux `node.name` (stable across restarts, built from udev).
- **Rationale**: the same API that plays the device lists it, so ids match without translation;
  GStreamer's device monitor has no default-change event and its Windows ids are not endpoint
  ids.
- **Alternatives**: GStreamer `Audio/Source` monitor (the spike): R1 already removes GStreamer
  from this path; one more provider would also share the video monitor's provider rules.

## R7. Errors and states (FR-015, FR-017, FR-018)

- **Decision**: platform errors are a small enum mapped to the audio states of data-model.md:

  | Platform error | Windows source | Linux source | State |
  |---|---|---|---|
  | `Gone` | endpoint not found or not active (`AUDCLNT_E_DEVICE_INVALIDATED` with the endpoint gone) | node removed, stream `Unconnected` with the node gone | input: Disconnected (waits for `Added` with the same id); output: falls back to the default |
  | `InUse` | `AUDCLNT_E_DEVICE_IN_USE` (any open); `AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED` (exclusive open); `AUDCLNT_E_DEVICE_INVALIDATED` while the endpoint is still active (another app took it in exclusive mode) | — | exclusive open → the platform reopens shared (`SharedAfterInUse`, state In use); still refused, or taken while playing → state Held, **Try again** |
  | `Blocked` | `E_ACCESSDENIED` on an input, or `platform::access(Capability::Microphone)` = Denied | — | Blocked, action opens `ms-settings:privacy-microphone` |
  | `Format` | `AUDCLNT_E_UNSUPPORTED_FORMAT` on every exclusive format | stream `Error` at format negotiation | exclusive → shared, tag In use; shared → `Failed` |
  | `Failed(text)` | anything else | stream `Error`, connection lost | Failed, **Try again** |

  - Measured on Windows (T018, 2026-11-13): an exclusive open **takes** a device other apps use
    shared (they get `AUDCLNT_E_DEVICE_INVALIDATED` while the endpoint stays active), and
    `AUDCLNT_E_DEVICE_IN_USE` comes only when another app already holds the device exclusive, in
    which case a shared open is refused too. So the shared fallback only helps when exclusive is
    not allowed or no format fits; a device held by another app is the Held state, retried by
    hand (Windows does not say when it is released). Maintainer, 2026-11-13.
  - Measured on Linux (T020, 2026-11-16): when PipeWire restarts, the server removes its nodes
    before the connection closes, so a side on a named node reports `Gone` and a side on the
    default output reports `Failed`; the monitor then reports every endpoint removed and, on the
    new connection, added again with the default output (R6). The input returns on its `Added`
    as after a replug; the output would wait for **Try again**, so an output side that failed
    while playing on the system default is reopened when `DefaultOutput` names an output again
    (a `reopen_output` after the restart opened on the new connection, measured; maintainer,
    2026-11-16).
  - The microphone privacy switch is read as the camera's is (device preview R6), from the
    `microphone` consent store (all apps and `NonPackaged`); `camera.rs` generalizes to a
    capability argument.
  - An output failure never stops the input: the output falls back to the system default and
    the state says so; the specific output returns on its `Added`.
  - Video states win (FR-018): when the video session closes or fails, the audio session closes
    and no audio state is shown.
- **Rationale**: the same shapes as the video's `DeviceError` (device preview R6), so the UI reuses
  the tag and menu-header pattern.
- **Alternatives**: none worth recording.

## R8. Volume (FR-011, FR-012)

- **Decision**: one app-side gain in the bridge, `gain = v²` for the slider value `v` in 0–1
  (20 % ≈ −28 dB, 50 % ≈ −12 dB, close to the Windows volume slider), 0 when muted, ramped over
  10 ms (R4). The wheel moves 5 % per notch. The OS stream volume (`ISimpleAudioVolume`,
  PipeWire's stream volume) is left at 100 %.
- **Rationale**: works the same in shared and exclusive mode (exclusive streams have no Windows
  mixer volume) and on both systems; never touches the system's or other apps' volume. The
  square law spreads the slider's low end: changed from the cubic law of PipeWire and PulseAudio
  by the maintainer on 2026-11-19, who heard almost no difference from 0 to 20 % with it (20 % ≈
  −42 dB, 10 % ≈ −60 dB).
- **Alternatives**: the OS per-app volume: no effect in exclusive mode, and on Windows it moves the
  app's slider in the system mixer, which the user may set separately. Linear gain: most of the
  slider's travel sounds loud. Cubic (PipeWire, PulseAudio): the bottom fifth is near silence.

## R9. Closing never hangs (FR-006, SC-008)

- **Decision**: no audio call runs on the main thread or the video thread.
  - Open, reopen and close are requests to the audio session's own threads (Windows) or the
    `pipewire` thread (Linux); results come back as events. A reopen is "close, then open" on
    those threads; requests that arrive meanwhile replace the pending one.
  - Each audio session carries a number, as video sessions do; events of an older session are
    ignored, so no sound plays after its video is gone (spec edge case).
  - Windows close: the stop flag, `IAudioClient::Stop`, then the threads release their COM
    objects and end; the app does not join them on the main thread.
  - Linux close: `pw_stream_disconnect` and destroy on the `pipewire` thread.
  - Quit: the audio session is closed first and the app waits for its acknowledgment at most
    1 s; past that it logs and exits anyway (`std::process::exit` after the settings are saved),
    so a stuck device can never keep the app alive or frozen.
- **Rationale**: the spike's Linux hang froze the desktop because the main thread waited for the
  PipeWire lock; here nothing the user can reach waits for an audio thread.
- **Alternatives**: joining threads with a timeout on the main thread: still blocks the UI for the
  timeout on every switch.

## R10. Measurement (FR-020, SC-001 to SC-005)

- **Decision**:
  - **Windows** (SC-001, SC-003, SC-004): the spike's method unchanged (research R3 "Method used
    in practice"): Voicemeeter records the monitor's audio from the line input (left) and the
    preview's output (right); PS5 menu sounds; `analyze.py audio --stereo`. Changed 2026-11-19
    (maintainer, T028): the preview plays on `Voicemeeter In 5`, read by Voicemeeter as a
    hardware input strip, not on a virtual input: with the same app and the spike's engine alike,
    `In 5` gave 52–53 ms, VAIO3 66–68 ms and `Voicemeeter Input` 99 ms, so a virtual input adds
    its own buffer to the figure. The spike's engine (`audio-preview --engine wasapi`) is
    measured in the same session on the same output, and the extra delay of the maintainer's
    everyday virtual input is recorded next to each result. Each setting (shared, input exclusive, both exclusive) gets a
    25 s clip for SC-001 and a 30-minute recording for SC-003.
  - **Linux** (SC-002, SC-003): inside the PC (maintainer, 2026-11-13). A script
    `tools/latency/pipewire-measure.sh` creates a null sink `capturax-measure` and, while the app
    plays on its usual output, adds two links with `pw-link`: the left channel of the node that
    feeds the app's capture stream (the card) to the sink's left, and the app's playback left
    channel to the sink's right. The app's own links stay as WirePlumber made them, so it keeps
    playing on the real output (heard, and its `out_latency_ms` printed in the same run) and no
    output choice in the app is needed (changed at T027, 2026-11-17: the app's output set to the
    sink needed T034's menu and a relink against WirePlumber's policy). `pw-record` records the
    sink's monitor. Every linked node runs under one driver, so both channels pass the same graph
    cycles and `analyze.py audio --stereo` gives the app's own delay. The latency the real output
    reports while the app plays on it (the stream's `pw_stream_get_time` delay, printed in the
    app's `[audio-source]` stats line, R11) is added to that figure. `stop` removes the sink,
    which removes its links.
  - **Drift** (SC-003): `analyze.py audio` gains `--drift`: the pairs' delays grouped per minute,
    the medians of minute 0 and the last minute, and the slope by least squares, in ms per
    minute; the stats log of the same run is kept next to the recording.
  - **A/V** (SC-004): audio delay minus spec #3's glass-to-glass video delay of the same session
    (Windows); Linux as the spec says, mainly by ear.
- **Rationale**: the spike's method keeps the numbers comparable with its 59/46/18 ms; Linux has
  no second recorder, and the null sink puts reference and preview in one file with one clock.
- **Alternatives**: a hardware loopback cable on the Linux PC: not available (spec Assumptions).
  `pw-top` latency columns only: no measured delay, only configured sizes.

## R11. Statistics (FR-019)

- **Decision**: with `--stats`, an `[audio]` line per second (contracts/stats.md): the waiting
  audio's median and maximum (ms), the follower's correction (ppm), gaps and drops in ms during
  that second; and an `[audio-source]` line whenever a session opens or a side reopens: input and
  output names, mode (shared, exclusive, shared after in use), format, period and the stream
  latency the device reports. Counters are atomics written by the audio threads and read by the
  existing stats printer; nothing prints from an audio thread.
- **Rationale**: SC-003 and SC-005 read the run's stability from these lines; the Linux figure of
  R10 needs the output's reported latency.
- **Alternatives**: a separate audio log file: one more option to document; the stats already go
  to the log.

## R12. Interface pieces

- **Decision**, following the mockup page (handoff link, version 2):
  - **Audio dropdown** in the toolbar (spec #3's select and menu): icon `volume` (or
    `volume_off` for No audio), title Audio, value the input; the menu has an **Input** group
    (Auto, every input, No audio) and an **Output** group (System default, every output); the
    state's title, detail and action in the menu header as the video device's states; tags
    `Exclusive`, `In use`, `Disconnected`, `Blocked`, `Failed` with the existing `Tag` component.
    In the icon look (narrow windows) the icon alone, as the other selects.
  - **Volume button** in the left dock between ambient light and settings; its icon by level
    (`volume_off` muted or 0, `volume_1` up to 33 %, `volume_2` up to 66 %, `volume` above);
    tooltip `Volume {}% · Muted`. A new `volume-popover.slint`: a horizontal slider with the
    value (no mute button, no Output select: maintainer, 2026-11-18); anchored beside the dock,
    inside the window. Hover opens after 300 ms and closes 400 ms after the pointer leaves both;
    right click opens; wheel changes the level; touch tap opens, a 500 ms hold mutes; focus
    shows it without giving it the focus, Enter/Space mutes, arrows and Home/End on the button
    move the level.
  - **Settings Audio category** (Windows only): an `Exclusive mode` section with the line of
    FR-022, tabs Inputs and Outputs, column headers Device and Exclusive, one row per device
    with the shell's toggle and a `Preview` tag while the preview uses it (maintainer, 2026-11-20: tabs
    and the toggle instead of groups and a checkbox); the rows in use first, only the rows
    scroll.
  - Icons from Tabler as the existing ones (`volume`, `volume_2`, `volume_1`, `volume_off`,
    `microphone`, `headphones`), same license file. Tabler has no low volume icon: `volume_1` is
    its `volume-2` without the wave, and `volume_off` is its `volume-3` (speaker with an X), as
    on the mockup.
- **Rationale**: the mockup was approved with the spec; every piece extends a component the
  device preview already has.
- **Alternatives**: none; the decisions are the maintainer's.

## R13. Where the code goes

- **Decision**:
  - `crates/audio` (new, pure): bridge, follower, interpolation, channel map, volume, pairing,
    choice resolution, statistics counters.
  - `crates/platform`: `audio.rs` (interface), `windows/audio/{mod,endpoints,stream,format}.rs`,
    `linux/pipewire.rs`; `camera.rs` becomes `privacy.rs` with a capability (camera, microphone).
  - `app/src/audio.rs`: the audio state machine beside `preview.rs` (follows the preview's
    session, choices, fallbacks, menus); `stats.rs`, `ui.rs`, `cli.rs`, `app.rs` wiring.
  - `app/ui/components/volume-popover.slint` new; toolbar, dock, menu, settings panel
    and texts extended.
  - `tools/latency/`: `analyze.py --drift`, `pipewire-measure.sh`.
- **Rationale**: same split as the video (capture crate pure of OS calls, state machine in the
  app); `crates/capture` stays GStreamer-only. The boundary test (`app/tests/platform_boundary.rs`)
  extends to `crates/audio`.
- **Alternatives**: the bridge inside `crates/capture`: that crate is GStreamer capture; audio
  for recording (spec #8) will be GStreamer and can live there later.

## R14. Dependencies

| Crate | Where | Purpose | License | Status |
|---|---|---|---|---|
| `pipewire` 0.10 (with `libspa` 0.10) | `crates/platform`, Linux | PipeWire client: registry, streams, metadata (R3, R6) | MIT | the PipeWire project's own bindings (gitlab.freedesktop.org), maintained |
| `windows` 0.62 | `crates/platform`, Windows | WASAPI, MMDevice, property store, COM (R2, R6); already a dependency, features added (`Win32_Media_Audio`, `Win32_Media_KernelStreaming`, `Win32_Media_Multimedia`, `Win32_System_Com`, `Win32_System_Com_StructuredStorage`, `Win32_System_Variant`, `Win32_Devices_FunctionDiscovery`, `Win32_UI_Shell_PropertiesSystem`) | MIT or Apache-2.0 | Microsoft |
| `windows-sys` 0.61 | `crates/platform`, Windows | container id (`Win32_Devices_DeviceAndDriverInstallation`, `Win32_Devices_Properties`) (R5); features added | MIT or Apache-2.0 | Microsoft |

- `pipewire` links the system's `libpipewire-0.3` (the sound server's own client library, part of
  every PipeWire system, as `libnvidia-ml` is part of the driver); building needs its headers and
  `clang` for bindgen: CI's Linux job adds `libpipewire-0.3-dev` and `clang`. Constitution VI is
  kept: nothing extra is installed by the user.
- No GStreamer element is added. No new dependency in `crates/audio` (std only).

## R16. A delay that grows after the app (FR-003a)

- **Finding** (2026-11-19, measurements/drift.md): on Windows, shared, the delay measured against
  the line input grew linearly, about 0.8 ms per minute (+24 ms in 30 minutes on `In 5`, +20 ms in
  24 minutes on VAIO3, +38 ms in 57 minutes), while the app's `wait_ms` stayed at 10–11 ms, the
  input was drained at every event and the output kept at most 1.5 periods queued. Reopening only
  the output (the app's Audio menu) brought it back from 85 to 58 ms; restarting the app did the
  same. So it grows after `GetCurrentPadding`, where the app cannot see it. Not known: whether it
  is Voicemeeter's VAIO driver, the `IAudioClient3` short period (most apps use the normal 10 ms
  period, and the maintainer has never noticed drift in other apps), or something else.
- **Decision** (maintainer, 2026-11-19: a palliative now, the cause after the app is done): the
  Windows output thread reopens its own stream on the same endpoint at the first quiet moment
  (peak below −50 dBFS for 300 ms of what it writes, after the volume) once the stream has played
  for 5 minutes. The decision is a pure `audio::Resync` fed with each block the output writes. The
  reopen is the same as a user's output change: a new `Opened`, the `[audio-source]` line again,
  the bridge kept. Shared output only (an exclusive stream has no mixer after it); not on Linux.
- **Rationale**: 0.8 ms per minute over 5 minutes is 4 ms, below anything heard; a quiet moment
  hides the reopen's short silence (tens of ms). The muted preview is quiet, so it reopens then,
  unheard.
- **Alternatives**: a reopen on a fixed timer (a cut over sound); measuring the hidden delay
  (`IAudioClock` reports the driver's position, which is the side that does not see the growth);
  the normal `Initialize` period (untested, about 7 ms more on every output; kept for the
  investigation).

## R17. A shared output on a 10 ms period (FR-003b)

- **Finding** (2026-11-19, measurements/drift.md "T028: the G533 directly"): the G533 (wireless,
  through its dongle) opens shared with a 10 ms period (`IAudioClient3` grants no shorter one) and
  a 1056-frame (22 ms) buffer, so 1.5 periods is 15 ms queued. The maintainer heard pops for
  30–60 s at a time, several times in 22 minutes. A diagnostic build (branch
  `exp/20261113-103922-g533-underrun`) logged each output wake: the app's thread woke every
  ~11.5 ms throughout, but in those stretches Windows took nothing at some wakes (720 frames still
  queued) and two periods (20 ms) at the next, more than the 15 ms queued: ~20 empty queues a
  second, each a short silence heard as a pop, and the time lost piled up in the bridge until it
  dropped 60–150 ms of input a second. With the queue at the whole buffer (22 ms, through an
  environment variable of that build), the same double takes came with no empty queue and no
  shortfall, and the maintainer heard no pop in 19 minutes. Other apps on the G533 never popped:
  Windows' usual queues are 20 ms or more.
- **Decision** (maintainer, 2026-11-19): a shared output whose period is 10 ms or more keeps its
  whole buffer queued; a shorter period (VAIO3's 2.67 ms) keeps 1.5 periods, as before.
- **Rationale**: the double take is Windows' or the device's, not the app's, so the queue must
  hold two periods; the buffer Windows gives is just over that (22 ms for 20 ms). It costs about
  7 ms on those outputs only; the short-period outputs, where no double take was seen, keep their
  delay.
- **Alternatives**: two periods plus a margin for every shared output (about 3 ms more on VAIO3,
  where nothing failed); a larger buffer than Windows gives (more delay for no measured gain).
- **Left as seen**: with the larger queue, the bridge had a few gaps of 10–20 ms in 19 minutes when
  Windows took 22 ms at once (the bridge held about 20 ms), none heard; and one stop of the whole
  system for ~100 ms (no output wake, a video frame late), which no queue covers.

## R15. Tests without devices

- **Decision**: `crates/audio` unit tests: follower holding the level with input and output
  clocks ±300 ppm apart over a simulated 30 minutes (no drop, no gap after priming, level within
  1 ms of its settled value once the follower has taken away the priming, about 8 s), a stall
  (40 ms burst dropped once), start-up priming, 48 → 44.1 kHz and mono → stereo of a 1 kHz sine (frequency and level kept, distortion below −60 dB), mix-down of
  5.1, volume ramps (no step larger than the ramp allows), pairing (links, names of the ezcap and
  the C920 on both systems, two identical cards), choice resolution. `app` tests: the audio state
  machine with a fake platform (event sequences of US1–US5). Platform audio is checked by hand
  (CI runners have no audio devices) through quickstart.
- **Rationale**: constitution VII; the bridge is where SC-003 and SC-005 are decided.
- **Alternatives**: PipeWire's null sink in CI: possible later; not needed for this spec.
