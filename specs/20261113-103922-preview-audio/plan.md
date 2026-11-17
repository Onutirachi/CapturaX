# Implementation Plan: Preview Audio

**Branch**: `feat/20261113-103922-preview-audio` | **Date**: 2026-11-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261113-103922-preview-audio/spec.md`

## Summary

The preview's sound, outside GStreamer on both systems. `crates/platform` opens the two device
streams with the OS's own API: on Windows the stack spike's WASAPI engine (shared with the
shortest mixer period, or exclusive per device), on Linux PipeWire directly through its client
library, both streams in one node group so they run in the same graph cycle. A new pure crate,
`crates/audio`, holds the bridge between them: about 2 ms of spare audio held by the spike's rate
follower (no cutting, no drift), rate and channel mapping for any device format, and the volume
with a click-free ramp; it is unit tested with a simulated 30-minute drift. The platform layer
also lists audio devices with hot plug and the default output, and gives each device the system's
physical link (Windows container id, Linux USB path), so each video device pairs with its own
input. An audio state machine in the app follows the video session, the per-device input choice,
the output choice and the exclusive flags, and shows every audio state. The UI gains the Audio
dropdown, the volume button and popover, and a Windows-only Audio settings category. The latency
tools gain a drift report and a PipeWire measurement script for the Linux path.

## Technical Context

**Language/Version**: Rust 1.99, edition 2024 (app shell research R1)

**Primary Dependencies**: as the device preview (Slint 1.18, winit 0.30, wgpu 30, gstreamer-rs
0.25); `pipewire` 0.10 in `crates/platform` on Linux; more `windows`/`windows-sys` features on
Windows (research R14)

**Storage**: `capturax.toml`: new `[audio]` and `[audio_devices."<id>"]` tables, `audio` key in
`[devices."<id>"]` (contracts/settings-file.md)

**Testing**: `cargo test`: the bridge (drift simulation, formats, ramps), pairing, choices and the
audio state machine with a fake platform; platform audio, latency, drift and acceptance by hand
(quickstart.md)

**Target Platform**: Windows 11 (WASAPI) and Linux (PipeWire; KDE Plasma Wayland checked)

**Project Type**: desktop app (Cargo workspace)

**Performance Goals**: against the monitor's audio on Windows median ≤ 62 / 50 / 22 ms (shared /
input exclusive / both exclusive), Linux ≤ 25 ms in the PC; drift ≤ 10 ms in 30 minutes; A/V
within ITU-R BT.1359; sound within 1 s of the first frame; volume within 100 ms (SC-001 to
SC-006)

**Constraints**: no audio call on the main or video thread; closing never hangs (FR-006); no OS
call outside `crates/platform`; the video's budgets unchanged (SC-007); every text in the catalogs

**Scale/Scope**: one audio session at a time; a handful of endpoints; one toolbar dropdown, one
dock button with a popover, one settings category, seven audio states

No NEEDS CLARIFICATION remains. The handoff's open points are answered in research.md: pairing
per platform (R5), the Linux engine (R3: PipeWire directly, decided by a probe that repeats the
spike's hang and starvation cases), the 30-minute drift runs and the Linux measurement (R10).

## Constitution Check

*Constitution 1.2.0. Checked before research and again after design: passes, no deviations.
Version 1.2.0 (maintainer, 2026-11-13) extends principle III's preview audio exception to
PipeWire on Linux, which a first draft of this plan had to record as a deviation.*

| Principle | How the plan complies |
|---|---|
| I. Preview latency | Budget, method and reference in spec SC-001 to SC-004 and quickstart; shortest periods on both systems, exclusive mode on Windows; 2 ms margin held by rate, no shared clock with the video; video budgets re-measured with sound (SC-007) |
| II. Simple by default | Plays with no setup: Auto pairing and the system default output (FR-007, FR-009); exclusive is opt-in per device; every audio problem shown with its next action (data-model.md "Audio state") |
| III. One native pipeline | One process; the preview's audio goes through WASAPI on Windows and PipeWire on Linux directly, as the constitution allows, justified by the spike's measurements (research R2, R3); video capture stays in GStreamer, and recording and streaming audio will too (spec #8) |
| IV. Platform layer | WASAPI, MMDevice, `CM_*`, the microphone privacy switch and PipeWire only in `crates/platform` (contracts/platform.md); `crates/audio` pure and covered by the boundary test; Linux has no exclusive mode and no privacy switch, stated in the spec |
| V. Private and safe | No network; nothing written to the system; the OS stream volume is not touched |
| VI. Portable | Choices in the portable settings file; `libpipewire` is the sound server's own client library on the system, nothing for the user to install (R14) |
| VII. Spec-driven, tested | Spec approved 2026-11-13; pure logic unit tested; manual checks listed; one new dependency justified (R14); the spike's engine adopted only as research R2 names it |
| Technology constraints | Rust; Slint over the app's own swap chain; texts in both catalogs; icons from the existing set |
| Workflow | English; Conventional Commits without AI trailers; CI on both systems (Linux job gains `libpipewire-0.3-dev` and `clang`) |

## Design

### Threads and data flow

```text
Windows                                        Linux
 audio-in thread (MMCSS)                        pipewire thread (one per process)
   WASAPI capture event → f32 ─┐                  registry, default metadata ──► AudioEvent
                               │                  capture stream process ─┐
 audio-out thread (MMCSS)      │                  playback stream process │ (same cycle, node.group)
   WASAPI render event ◄─ pull ┤                                          │
 MMNotificationClient ─► AudioEvent             ◄─ pull ── Bridge ◄─ push ┘
                               ▼
                         Bridge (crates/audio): ring, follower, interpolation, channels, gain
                               │ atomics
                               ▼
 app main thread: audio.rs state machine ◄── AudioEvent / SessionEvent (EventLoopProxy)
   follows preview.rs's session, settings, UI     stats printer reads BridgeStats once per second
```

- The main thread only sends requests (open, reopen a side, close, gain) and receives events;
  it never waits for an audio thread (research R9).
- The audio session opens when the video session opens (preview states Opening, Playing, No
  signal) and closes with it; its number ties its events to it.
- Gain changes go straight to the bridge (an atomic target), so volume is heard within one output
  period plus the 10 ms ramp.

### Startup and quit

Startup gains, after the video monitor: start the audio monitor (`--no-audio` skips it). The
first `Listed` lets the audio state machine resolve the open video device's input. Quit gains,
right after closing the video session: close the audio session and wait for `Closed` at most 1 s,
stop the audio monitor; then the shell's sequence. A stuck audio close is logged and the process
exits after saving settings (research R9).

### App (`app/src/`)

- `audio.rs` (new): state machine of data-model.md "Audio state": inputs from the platform
  monitor, the preview's session events, settings and UI picks; outputs: platform requests,
  menu items, the volume button state, the settings category rows.
- `sound.rs` (new): runs the audio commands (monitor, session, bridge); `app.rs` hands it the
  video session's open and close from the preview's `Open` and `Close`; `preview.rs` unchanged.
- `stats.rs`: `[audio-source]` and `[audio]` lines.
- `cli.rs`: `--list-devices` additions, `--no-audio`.
- `ui.rs`: the Audio dropdown, volume button and popover, Audio category bridge.

### UI (`app/ui/`)

- `components/toolbar.slint`: the Audio select after the color range select (spec #3 FR-002a
  already counts it in the toolbar's width rules); menu with Input and Output groups, state header,
  tags.
- `components/dock.slint`: the volume button between ambient light and settings.
- `components/volume-popover.slint` (new): slider with value (FR-011 as changed 2026-11-18); hover,
  click, right click, wheel, touch and keyboard behavior of FR-011 (research R12).
- `components/checkbox.slint` (new) and `components/settings-panel.slint`: category `audio`
  (Windows only) with the Exclusive mode section.
- New icons (`volume`, `volume_1`, `volume_2`, `volume_off`, `microphone`, `headphones`), texts in
  `texts.slint` and the pt-BR catalog (FR-022).

### Investigations inside the plan

- **Linux engine probe** (research R3, quickstart): before the Linux session is built on it; a
  failure stops the work for the maintainer's decision.
- **Pairing keys** (research R5): `--list-devices` on both systems with the ezcap and the C920
  before the pairing rule is final.

## Project Structure

### Documentation (this feature)

```text
specs/20261113-103922-preview-audio/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── audio.md         # new crate
│   ├── platform.md      # additions
│   ├── settings-file.md # additions
│   ├── stats.md         # additions
│   └── cli.md           # additions
├── checklists/requirements.md
├── handoff.md
├── measurements/        # pairing.md, latency.md, drift.md, acceptance.md (during implementation)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                 # workspace: + crates/audio; pipewire; windows/windows-sys features
.github/workflows/ci.yml   # Linux: libpipewire-0.3-dev, clang
crates/audio/              # NEW, pure (contracts/audio.md)
├── src/
│   ├── lib.rs
│   ├── bridge.rs          # ring, follower, priming, cut, stats (R4)
│   ├── resample.rs        # cubic interpolation, channel map, mix-down (R4)
│   ├── volume.rs          # gain law, ramp, icon, wheel (R8)
│   ├── pairing.rs         # links, clean names (R5)
│   └── choice.rs          # AudioChoice, OutputChoice, resolve (data-model.md)
└── tests/
    └── drift.rs           # simulated clocks apart over 30 minutes, stall, priming

crates/platform/src/
├── audio.rs               # interface: Monitor, Session, Endpoint, events (contracts/platform.md)
├── privacy.rs             # camera.rs generalized: Capability::{Camera, Microphone}
├── windows/audio/{mod.rs, endpoints.rs, stream.rs, format.rs, container.rs}   # R2, R5, R6
├── windows/privacy.rs     # camera.rs + microphone consent
└── linux/pipewire.rs      # thread, registry, metadata, streams (R3, R6)
crates/platform/examples/pipewire_probe.rs   # quickstart "Linux engine probe"

app/src/
├── audio.rs               # NEW: audio state machine, menus, volume state
├── sound.rs               # NEW: runs the audio commands (monitor, session, bridge)
├── stats.rs, cli.rs, ui.rs, app.rs, main.rs   # wiring
app/ui/components/{volume-popover, checkbox}.slint          # NEW
app/ui/components/{toolbar, dock, menu, settings-panel}.slint, texts.slint, main.slint
app/ui/assets/icons/{volume, volume_1, volume_2, volume_off, microphone, headphones}.svg
app/lang/pt-BR/LC_MESSAGES/capturax.po
app/tests/platform_boundary.rs   # + crates/audio

crates/settings/src/model.rs     # [audio], [audio_devices], devices.audio

tools/latency/analyze.py         # audio --drift
tools/latency/pipewire-measure.sh   # NEW: null sink, links, pw-record (R10)
tools/latency/README.md
```

**Structure Decision**: a fifth workspace crate, `crates/audio` (research R13), pure and std-only,
for everything that decides the delay, the drift and the sound's quality; the device streams in
`crates/platform` behind the interface of contracts/platform.md; the state machine in the app
beside `preview.rs`. The spike's `spike/audio-preview/src/wasapi.rs` (tag `spike/stack-20261026`)
is the starting point for `windows/audio/` and `bridge.rs`, rewritten to this split.

## Complexity Tracking

No constitution violations to justify.
