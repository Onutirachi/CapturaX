# Implementation Plan: Device Preview

**Branch**: `feat/20261106-170507-device-preview` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261106-170507-device-preview/spec.md`

## Summary

Real devices in the app shell's render path. A new `crates/capture` lists capture cards and
webcams through GStreamer's device monitor (hot plug on both platforms), computes native and
automatic modes, and runs one pipeline per open device that hands the device's own YUV planes to
the video thread; the composite shader converts them with the right matrix and color range, so
the range switches live and fewer bytes are uploaded than the spike's RGBA. An app-side state
machine opens the remembered device, reconnects it and shows every device state with its action.
The toolbar gets device, mode and color range selects; the first info bubbles show source, mode,
frame rate and latency (measured plus an estimate), each with its display choice. On Linux the
app asks the NVIDIA driver for its performance mode while previewing, so idle clocks no longer
cost latency. A notice says when a device is connected or disconnected, in the app when focused and as a
system notification otherwise, with **Open** for a new device. A Video settings category adds
the Follow the monitor presentation. The Windows
stalls at presentation mode changes and the language switch stalls are investigated with
PresentMon and a present log, and fixed if the cause is the app's.

## Technical Context

**Language/Version**: Rust 1.99, edition 2024 (app shell research R1)

**Primary Dependencies**: as the app shell (Slint 1.18, winit 0.30, wgpu 30, gstreamer-rs 0.25),
plus `libloading` 0.8 in `crates/platform` on Linux (research R14)

**Storage**: the app shell's `capturax.toml`, new tables `[preview]`, `[overlay]`, `[devices."<id>"]`
(contracts/settings-file.md)

**Testing**: `cargo test`: pure unit tests (modes, identity, detectors, estimate, state machine,
settings), pipeline tests with `videotestsrc`; manual acceptance, glass-to-glass latency and the
stall investigation in quickstart.md

**Target Platform**: Windows 11 (Media Foundation capture, Direct3D 12) and Linux (V4L2 capture,
Vulkan; KDE Plasma Wayland checked)

**Project Type**: desktop app (Cargo workspace)

**Performance Goals**: glass to glass at 1080p60 median ≤ 32 ms / p95 ≤ 35 ms on Windows, ≤ 35 /
≤ 40 ms on Linux with default power settings; frame ready → presented as the app shell (median
≤ 1.5 ms, max ≤ 3 ms in 95% of seconds); open or switch ≤ 2 s; hot plug and reconnect ≤ 3 s
(SC-001 to SC-004)

**Constraints**: the video never waits for the UI or the window; measurement and analysis never
on the video thread (FR-015); no OS call outside `crates/platform`; no root or system-wide
setting for GPU power; every user-facing text in the catalogs

**Scale/Scope**: one open device at a time; up to a handful of devices listed; three toolbar
selects, four bubbles, one settings category, nine preview states

No NEEDS CLARIFICATION remains. The spec's inherited questions are answered in research.md:
GPU power on Linux (R7, with R4), V4L2 against PipeWire (R4: V4L2; PipeWire only for a sandbox,
not in this spec), the 120 Hz mismatch and reading the incoming signal (R3: no UVC device reports
it, so Auto source is not shown on them; the default is Auto max FPS), latency levers (R3, R4: optional, recorded), color range (R5),
the device list's detail line (spec FR-004), the Windows stalls (R8, investigation), F52 (R9),
fullscreen from a snapped window (R10), device notices (R15, validated first).

## Constitution Check

*Constitution 1.1.1. Checked before research and again after design: passes, no deviations.*

| Principle | How the plan complies |
|---|---|
| I. Preview latency | Budget stated with method and reference (spec SC-001, SC-002; quickstart "Latency budget"); native modes, no software scaling (the GPU fits the image when presenting); conversion removed from the pipeline (R4); GPU power state handled on Linux (R7); analysis off the video thread (R5) |
| II. Simple by default | Opens the right device and mode with no configuration (FR-005, FR-008); options only where the maintainer asked (color range, bubble displays, presentation); every device problem shown with its next action (data-model.md "Preview state") |
| III. One native pipeline | One process; the device is opened once by `capture::Session`, which later outputs attach to; GStreamer for capture and decode, the app's own GPU presenter for the preview, as the constitution allows |
| IV. Platform layer | Capture through GStreamer providers chosen from device properties, not `cfg`; registry, privacy page, NVML, QPC, system notifications and window rectangles in `crates/platform` (contracts/platform.md); boundary test extended to `crates/capture`; Linux has no camera privacy switch and no snap restore, stated |
| V. Private and safe | No network; NVML request is per-process and undone at exit; nothing written to the system |
| VI. Portable | Device choices in the portable settings file; no installed component needed (NVML is the driver's own library, optional); on Windows the notification app id is registered for the current user like the autostart entry, display only, no state (R15) |
| VII. Spec-driven, tested | Spec approved 2026-11-06; pure logic unit tested, pipeline tests with test sources, manual acceptance listed; one new dependency justified (R14); spike capture code adopted only as this plan names it (R3) |
| Technology constraints | Rust, GStreamer, Slint over the app's own swap chain; texts in both catalogs; accent and theme from the shell |
| Workflow | English; Conventional Commits without AI trailers; CI on both platforms |

## Design

### Threads and data flow

```text
GStreamer streaming thread (per session)          device monitor thread
  appsink callback:                                 DEVICE_ADDED / REMOVED
    planes → pooled Frame (captured, ready)              │
    every 500 ms: luma sample → range                    ▼
    │ Frame                    │ SessionEvent      app main thread: preview.rs
    ▼                          └──────────────────► state machine, settings, UI bridge
  Inbox (newest frame) ──► video thread                │ Scene, UI image, presentation
                           upload planes, YUV→RGB      ▼
                           composite UI, present  ◄─ Inbox
                           holds PerformanceGuard (Linux NVIDIA)
```

- Events from the monitor and sessions reach the main thread as winit user events through the
  shell's `EventLoopProxy`, as tray and theme events already do.
- Opening runs on a short-lived worker (building and setting a pipeline to Playing can block on
  Windows); the main thread never waits for it (FR-013).
- A session is closed before another opens; frames of a closing session are ignored by a session
  number carried in each frame.

### Startup and quit

Startup gains, after the window and the video thread exist: start the device monitor; on its
first list, choose the device (data-model.md "Start order") and open it with its remembered mode
and range, or show `NoDevices`. `--test-video` skips the monitor's choice. Quit gains, first:
close the session, stop the monitor; the rest is the shell's sequence.

### Render changes (`app/src/render/`)

- `VideoFrame` becomes the capture crate's `Frame` (planes, format, matrix, range, captured,
  ready).
- `gpu.rs` keeps one texture set per format (recreated on size or format change) and a uniform
  with the matrix, range and format; `composite.wgsl` samples the planes and converts (R4).
- The loop has two drivers (R9): frame-driven (Mailbox, today's loop) and refresh-driven (Fifo,
  acquire first, then the newest frame); switching reconfigures the surface on the video thread.
- Stats add `lat_ms` and `rep`, and the `[source]` line (contracts/stats.md); `--present-log`
  writes QPC lines (R8).

### UI (`app/ui/`)

- `components/toolbar.slint`: the toolbar panel in the shell's slot with the device, mode and
  color range selects (R13), in the spike's dropdown look (title inside); compact layout keeps
  them in the toolbar's row, narrower.
- `components/menu.slint`: `MenuItem` gains `kind`, `detail` (second line), `badge`,
  `disabled`; group headers are not selectable; keyboard navigation skips them.
- `components/badge.slint` (from the spike), `components/info-bubbles.slint` (fixed top left;
  hover settings buttons in the top layer).
- `main.slint`: the state message shows the preview state's icon, title, detail and actions;
  `source-open` becomes the preview state.
- `components/notice.slint`: the in-app device notice, one balloon showing the latest message
  (top layer, bottom right of the video area, about 5 s, × button, closes on click, **Open** for
  a connected device); no toast library.
- `components/settings-panel.slint`: category `video` with the Presentation select and tooltip.
- New texts in `texts.slint` and the pt-BR catalog.

### Investigations inside the plan

- **Linux GPU power** (R7): validate the NVML call first (quickstart), then build
  `gpu_performance`; if it does not hold the clocks, stop and decide the fallback with the
  maintainer.
- **Windows stalls** (R8): after the device preview works on Windows, run the matrix and record
  measurements/stalls.md; any fix needs the cause first; a G-SYNC driver profile is a maintainer
  decision.
- **Language switch stalls**: same session, stage lines.
- **System notifications** (R15): validate the Windows toast from an unpacked build and the KDE
  notification first; if the Windows toast does not work, decide the fallback with the
  maintainer.

## Project Structure

### Documentation (this feature)

```text
specs/20261106-170507-device-preview/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── capture.md
│   ├── platform.md      # additions
│   ├── settings-file.md # additions
│   ├── stats.md         # additions
│   └── cli.md           # additions
├── checklists/requirements.md
├── handoff.md
├── measurements/        # latency.md, stalls.md, acceptance.md (during implementation)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                 # workspace: + crates/capture; libloading
crates/capture/            # NEW: GStreamer devices, modes, session (contracts/capture.md)
├── src/
│   ├── lib.rs
│   ├── monitor.rs         # device monitor thread, events, preferred provider (R1)
│   ├── device.rs          # Device, identity, numbering (R2)
│   ├── mode.rs            # modes from caps, format rank, automatic modes, badges (R3, pure)
│   ├── session.rs         # pipeline, appsink, errors, no-signal timer (R4, R6)
│   ├── frame.rs           # Frame, plane pool, captured time (R11)
│   ├── analysis.rs        # range detector (R5, pure)
│   └── test_source.rs     # videotestsrc in NV12 (replaces app/src/test_video.rs)
└── tests/
    ├── modes.rs           # recorded caps of the ezcap and the C920
    └── pipeline.rs        # videotestsrc: frames, range switch, no signal

app/src/
├── preview.rs             # NEW: state machine, start order, reconnect, settings (data-model.md)
├── latency.rs             # NEW: estimate and bubble texts (pure)
├── render/{mod.rs, gpu.rs, composite.wgsl}   # YUV planes, refresh-driven loop
├── stats.rs               # lat_ms, rep, [source]
├── ui.rs                  # toolbar, bubbles, states, Video category bridge
├── cli.rs                 # --list-devices, --present-log
└── app.rs, main.rs        # wiring; arranged rect around fullscreen (R10)
app/ui/components/{toolbar, badge, info-bubbles, notice}.slint   # NEW
app/ui/components/{menu, select, settings-panel, state-message}.slint, main.slint, texts.slint
app/tests/platform_boundary.rs   # + crates/capture

crates/platform/src/
├── camera.rs, windows/camera.rs, linux/camera.rs   # camera_access, open_camera_privacy
├── gpu.rs, linux/nvml.rs                            # gpu_performance (R7)
├── notify.rs, windows/notify.rs, linux/notify.rs   # notify (R15)
└── windows/{mod.rs, snap.rs}                        # qpc, arranged_rect, set_outer_rect

crates/settings/src/model.rs   # preview, overlay, devices tables
```

**Structure Decision**: a fourth workspace crate, `crates/capture` (research R12): pure mode and
analysis logic plus the GStreamer session, testable without a device and free of OS calls. The
spike's `capture/src/{devices,mode,video}.rs` (tag `spike/stack-20261026`) are the starting point
for `monitor.rs`, `mode.rs` and `session.rs`, rewritten to this structure; the spike's
`info-overlay.slint` and `Badge` are the starting point for the bubbles and badges.

## Complexity Tracking

No constitution violations to justify.
