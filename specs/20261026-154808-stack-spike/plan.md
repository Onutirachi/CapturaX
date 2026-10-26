# Implementation Plan: Stack Spike

**Branch**: `feat/20261026-154808-stack-spike` | **Date**: 2026-10-26 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261026-154808-stack-spike/spec.md`

## Summary

Build throwaway prototypes that measure preview latency on four paths (native window, video
inside Slint, browser, OBS), measure preview audio delay and A/V offset, and show three visual
directions of the main screen in Slint with live video. Measurements use a PS5 playing an A/V
sync test video, filmed at 240 fps next to the TV passthrough, analyzed by a script. The outcome is
a findings report that confirms or rejects Slint and picks a visual direction. Details of each
decision are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.99 (stable, MSVC on Windows); Python 3 via `uv` for the analysis
script; HTML + JavaScript for the browser reference page.

**Primary Dependencies**: GStreamer 1.28.x with gstreamer-rs 0.25 (`gstreamer`, `gstreamer-app`,
`gstreamer-video`, `gstreamer-gl` on Linux); Slint 1.18. Analysis: OpenCV, NumPy, imageio-ffmpeg.

**Storage**: CSV files and clips under the feature directory's `measurements/` folder (clips are not
committed; only CSV and summaries).

**Testing**: none automated for the prototypes (throwaway, constitution VII). The analysis script is
validated by checking its output against a few frames counted by hand.

**Target Platform**: Windows 11 (RTX 5070 Ti) first; CachyOS (RTX 3050) for the Linux checks.

**Project Type**: desktop prototypes (Cargo workspace under `spike/`).

**Performance Goals**: see spec SC-002 to SC-005: Slint path within one frame of the native path;
chosen path within one frame of OBS at 1080p60; encoder adds less than one frame and under 1%
dropped frames; audio offset within ITU-R BT.1359 limits, drift under 10 ms in 30 minutes.

**Constraints**: same card, mode, monitor and method for every path; at least 20 samples per path
and mode; Windows GPU-texture variant timeboxed to one working session.

**Scale/Scope**: 4 small binaries, 1 HTML page, 1 analysis script; 3 visual directions × 2 themes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Result |
|---|---|---|
| I. Preview latency | The spike's purpose; budget and method stated (spec SC-002–SC-005, research R2–R3). | Pass |
| II. Simple by default | Directions follow "no outlines, labels of at most three words" (spec FR-007). | Pass |
| III. One native pipeline | All prototypes use GStreamer; browser and OBS are references only, not candidates. | Pass |
| IV. Windows and Linux | Linux checks are required (FR-009); session type recorded. | Pass |
| V. Private and safe | No network features in the spike. | Pass (n/a) |
| VI. Portable | Not applicable to throwaway prototypes. | Pass (n/a) |
| VII. Spec-driven | Spike code stays on the branch; tag + delete before merge (research R11). | Pass |
| Licensing | gstreamer-rs (MIT/Apache-2.0), Slint (GPL-3.0 option), GStreamer (LGPL), analysis tools not shipped. | Pass |

Post-design re-check: no new dependency or abstraction beyond the above. Pass.

## Project Structure

### Documentation (this feature)

```text
specs/20261026-154808-stack-spike/
├── spec.md
├── plan.md              # this file
├── research.md          # decisions R1–R12
├── data-model.md        # measurement records and report structure
├── quickstart.md        # how to run every measurement
├── contracts/
│   ├── cli.md           # command-line interface of each prototype
│   └── measurements.md  # CSV formats produced by the analysis script
├── measurements/        # CSV results and summaries (clips stay local)
├── findings.md          # the report (written during the spike)
├── handoff.md
└── tasks.md             # created by /speckit-tasks
```

### Source Code (spike branch only, deleted before merge)

```text
spike/
├── Cargo.toml               # workspace
├── capture/                 # shared: list devices and modes, build capture + audio pipelines
├── native-preview/          # binary: capture → native video sink (path A)
├── slint-preview/           # binary: Slint app — bare video (path B) and the 3 visual directions
│   └── ui/                  # .slint files: directions, themes, components
├── audio-preview/           # binary: low-latency audio passthrough (WASAPI / PipeWire)
├── browser/index.html       # path C reference page
└── tools/analyze.py         # clip analysis (uv run)
```

**Structure Decision**: one Cargo workspace so the capture code is written once and shared by the
native and Slint binaries. The visual directions live in the Slint binary because they must run
with live video, and its bare mode (no chrome) is the measured path B. The encoder load test is a
flag of the Slint binary.

## Phases of the spike

1. **Setup**: install GStreamer (runtime + development) on Windows and CachyOS; list the card's
   modes.
2. **Paths A and B** (P1): native preview, then Slint bare preview with CPU copy; GPU variant
   timeboxed; encoder flag.
3. **References** (P1): browser page, OBS setup.
4. **Measure** (P1): record clips per path and mode, analyze, write results.
5. **Audio** (P2): audio passthrough, measurement, 30-minute drift run.
6. **Directions** (P3) and **capabilities** (P4).
7. **Linux** checks for the fastest path and the chosen direction.
8. **Report and decisions**: findings, constitution and roadmap updates, tag, delete `spike/`, PR.

## Complexity Tracking

No constitution violations to justify.
