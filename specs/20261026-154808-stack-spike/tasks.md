---
description: "Tasks for the stack spike"
---

# Tasks: Stack Spike

**Input**: Design documents from `specs/20261026-154808-stack-spike/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: no automated tests (throwaway prototypes, constitution VII). The analysis script is
validated against hand-counted frames (T018).

**Organization**: grouped by user story. Tasks marked **(maintainer)** need the maintainer's hands
(installing, filming, judging); the agent prepares them and processes the results.

**Commits**: at each checkpoint the agent shows what changed and commits only after the
maintainer validates it. Nothing is pushed until Phase 7. At the end of every working session the
agent updates handoff.md (constitution, Development Workflow).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4 from spec.md

---

## Phase 1: Setup

**Purpose**: tools installed, workspace created, card modes known.

- [x] T001 (maintainer) Install GStreamer 1.28.8 `gstreamer-1.0-msvc-x86_64-1.28.8.exe` with the development components; the agent then checks `GSTREAMER_1_0_ROOT_MSVC_X86_64`, the `bin` folder on `PATH`, `pkg-config` and `PKG_CONFIG_PATH`, and fixes what is missing (research R1)
- [x] T002 List the card's video and audio modes with `gst-device-monitor-1.0 Video/Source Audio/Source` and record them, with the Windows environment fields from data-model.md, in specs/20261026-154808-stack-spike/measurements/environment.md
- [x] T003 Create the Cargo workspace spike/Cargo.toml with members `capture`, `native-preview`, `slint-preview`, `audio-preview` (gstreamer-rs 0.25, Slint 1.18)
- [x] T004 [P] Create the crate skeletons spike/capture/, spike/native-preview/, spike/slint-preview/, spike/audio-preview/ (Cargo.toml and src/main.rs or src/lib.rs each)
- [x] T005 [P] Create spike/tools/analyze.py with uv inline dependencies (opencv-python, numpy, imageio-ffmpeg) and the `video`, `audio`, `summary` subcommands as stubs (contracts/cli.md)

**Checkpoint**: `cargo build` succeeds in spike/; maintainer validates; commit.

---

## Phase 2: Foundational

**Purpose**: capture code shared by every binary.

- [x] T006 Implement device and mode listing (`--list`) in spike/capture/src/devices.rs
- [x] T007 Implement `--mode WxH@FPS[:FORMAT]` parsing and Auto selection ("largest resolution with ≥ 59 fps") in spike/capture/src/mode.rs
- [x] T008 Implement the video source builder: `mfvideosrc` on Windows, `v4l2src` on Linux, native caps, GPU MJPEG decode when the mode is MJPEG only (research R6) in spike/capture/src/video.rs
- [x] T009 [P] Implement `--stats` (print only) and `--record` (print and append; official runs only): rendered and dropped frames from the sink and in-pipeline latency, printed every second and, with `--record`, appended to specs/20261026-154808-stack-spike/measurements/pipeline-stats.csv (`run,path,variant,mode,platform,second,rendered,dropped,pipeline_latency_ms`), in spike/capture/src/stats.rs
- [x] T010 [P] Implement the shared options (`--list`, `--device`, `--mode`, `--stats`) in spike/capture/src/cli.rs

**Checkpoint**: `native-preview --list` shows the card with 1080p60 and its highest frame-rate mode; maintainer validates; commit.

---

## Phase 3: User Story 1 — Know whether the preview is fast enough to play (P1) 🎯 MVP

**Goal**: latency of native, Slint (CPU and GPU), browser and OBS paths, with and without the encoder, on Windows and Linux.

**Independent Test**: every path and mode has ≥ 20 samples with median and p95 in measurements/summary.md.

- [x] T011 [US1] Implement native-preview: sinks `d3d12` (default on Windows), `d3d11`, `gl` (default on Linux), `wayland`, `vulkan`, all with `sync=false`, and `--fullscreen` in spike/native-preview/src/main.rs
- [x] T012 [US1] Implement slint-preview `--bare` with `--transfer cpu`: `appsink` (`sync=false`, `max-buffers=1`, `drop=true`) → RGBA → `slint::Image`, following Slint's gstreamer-player example, in spike/slint-preview/src/video_cpu.rs, spike/slint-preview/ui/bare.slint and spike/slint-preview/src/main.rs
- [ ] T013 [US1] Implement `--encoder`: a `tee` branch with the NVIDIA hardware H.264 encoder at 1080p60, about 12 Mbps, into a discarding sink; if that encoder is not available, fall back to another hardware encoder or a software one and print which encoder was used; in spike/capture/src/encoder.rs
- [ ] T014 [US1] **Timebox: one working session.** Implement `--transfer gpu`: Linux via EGL as in the gstreamer-player example; Windows via Slint's wgpu texture import or the OpenGL underlay; if it does not work in the timebox, stop and record what failed and why in specs/20261026-154808-stack-spike/findings.md, in spike/slint-preview/src/video_gpu.rs
- [ ] T015 [P] [US1] Create the browser reference page: device and mode picker, `getUserMedia` with `resizeMode: "none"`, fullscreen, measured FPS via `requestVideoFrameCallback`, in spike/browser/index.html
- [ ] T045 [P] [US1] Create the sync test sources (research R2): a 60 fps video with a full-screen white flash and a 1 kHz beep once per second, played from a USB drive in the PS5 media gallery (spike/tools/make_sync_video.py, output in clips/), and an equivalent full-screen web page for the PS5 browser (spike/tools/flash.html)
- [ ] T016 [P] [US1] Implement `analyze.py video`: mark TV and preview regions once per clip (saved next to the clip), detect sudden image changes (white flashes or menu screen changes, research R2) by frame difference, pair them, append rows to measurements/video-samples.csv; rules from data-model.md: "a clip with fewer than 5 detected flash pairs is rejected; delays above 500 ms are flagged as detection errors and excluded", `delay_ms` must be ≥ 0; in spike/tools/analyze.py
- [ ] T017 [US1] Implement `analyze.py summary`: n, median, p95, min, max per path, variant, mode and platform, plus the dropped-frame percentage from pipeline-stats.csv; a path and mode that could not be opened is shown as "n/a", never omitted; output specs/20261026-154808-stack-spike/measurements/summary.md (contracts/measurements.md)
- [ ] T018 [US1] (maintainer) Validate the analyzer: count 5 flashes of one clip by hand and compare with the script (difference ≤ one camera frame, about 4 ms)
- [ ] T019 [US1] (maintainer) Set up OBS: Video Capture Device in the same mode, fullscreen projector on the monitor (research R9)
- [ ] T020 [US1] (maintainer) Record 25 s clips at 240 fps for native, slint-cpu, slint-gpu (if T014 succeeded), browser and OBS in 1080p60 and the highest frame-rate mode, plus slint-cpu with `--encoder`, one path at a time (the card has a single owner) with nothing else changed between clips; a clip during which the card disconnected is discarded and noted; the agent analyzes each clip into measurements/video-samples.csv
- [ ] T021 [US1] (maintainer) Linux: install GStreamer on CachyOS (base, good, bad, PipeWire plugin, NVIDIA codecs), build spike/, record the native path and the fastest Slint variant in 1080p60; the agent analyzes the clips and records the session type in measurements/environment.md
- [ ] T022 [US1] Write the video and encoder results, and the SC-002 to SC-004 evaluation, in specs/20261026-154808-stack-spike/findings.md

**Checkpoint**: SC-002 decision (Slint confirmed or alternatives listed); maintainer reviews; commit.

---

## Phase 4: User Story 2 — Hear the card with the lowest delay (P2)

**Goal**: audio delay, A/V offset and drift.

**Independent Test**: measurements/summary.md has the audio table and the offset is compared with the ITU-R BT.1359 limits.

- [ ] T023 [US2] Implement audio-preview: `wasapi2src` → `wasapi2sink` with `low-latency=true` on Windows, `pipewiresrc` → `pipewiresink` on Linux, `--input`, `--output`, `--list-audio`, input paired with the video device when detectable, in spike/audio-preview/src/main.rs
- [ ] T024 [P] [US2] Implement `analyze.py audio`: detect sound onset pairs (beeps or PS5 menu clicks; TV then PC), append rows to measurements/audio-samples.csv, and add the audio table to the summary, in spike/tools/analyze.py
- [ ] T025 [US2] (maintainer) Record the minute-0 and minute-30 audio clips (quickstart step 3) and play with the headset, noting whether any offset is noticeable and any audible glitches (clicks, dropouts)
- [ ] T026 [US2] Write the audio results and the SC-005 evaluation in specs/20261026-154808-stack-spike/findings.md

**Checkpoint**: maintainer reviews; commit.

---

## Phase 5: User Story 3 — Compare visual directions with live video (P3)

**Goal**: Float, Frame and Rail directions, dark and light, with live video.

**Independent Test**: switch directions and themes at runtime; every direction shows title bar, video, info overlay, source picker and dock.

- [ ] T027 [US3] Create theme tokens for dark and light (accent `#FF7304`, depth by tone and shadow, no outlines) and load one custom font family bundled in spike/slint-preview/ui/assets/fonts/, in spike/slint-preview/ui/theme.slint
- [ ] T028 [P] [US3] Copy the logo (old/capturax-react/brand/X.svg and CapturaX.svg) into spike/slint-preview/ui/assets/ and create shared components (title bar with window controls, dock, source picker with groups and resolution badges, info overlay), with visible labels of at most three words and longer text only in tooltips, in spike/slint-preview/ui/components/
- [ ] T029 [P] [US3] Create the Float direction in spike/slint-preview/ui/directions/float.slint
- [ ] T030 [P] [US3] Create the Frame direction in spike/slint-preview/ui/directions/frame.slint
- [ ] T031 [P] [US3] Create the Rail direction in spike/slint-preview/ui/directions/rail.slint
- [ ] T032 [US3] Add runtime switching of direction and theme, `--direction`, `--theme` and `--test-video`; when no capture device is found, show a clear message with the next action (connect a device or use `--test-video`) instead of an empty video area; in spike/slint-preview/src/main.rs and spike/slint-preview/ui/app.slint
- [ ] T033 [US3] (maintainer) Review the directions with live video; the agent saves screenshots in specs/20261026-154808-stack-spike/measurements/screens/ and records the choice and reasons in findings.md

**Checkpoint**: maintainer chooses a direction; commit.

---

## Phase 6: User Story 4 — Confirm the interface can do what the product needs (P4)

**Goal**: an outcome for every capability in spec User Story 4.

**Independent Test**: the capability table in findings.md has no empty row.

- [ ] T034 [P] [US4] Ambient light: a small downscaled copy of the video, blurred and stretched behind it, in spike/slint-preview/src/ambient.rs and spike/slint-preview/ui/components/ambient.slint
- [ ] T035 [P] [US4] Info overlay pinnable over the video, and detachable as a floating always-on-top window, in spike/slint-preview/ui/components/overlay.slint and spike/slint-preview/src/overlay_window.rs
- [ ] T036 [P] [US4] Language switching without restart (English and Portuguese, a few strings with `@tr`) in spike/slint-preview/ui/ and spike/slint-preview/lang/
- [ ] T037 [US4] Frameless window with own title bar: move, resize, maximize, fullscreen, following Slint's custom-titlebar example, in spike/slint-preview/src/window.rs
- [ ] T038 [US4] (maintainer) Linux: run the chosen direction on CachyOS and check every capability there
- [ ] T039 [US4] Fill the capability table (working, limited, not possible; platform; notes) in specs/20261026-154808-stack-spike/findings.md

**Checkpoint**: maintainer reviews; commit.

---

## Phase 7: Decisions and wrap-up

- [ ] T040 Complete findings.md: environment, method, decisions (Slint status, chosen preview path, chosen direction), open questions for later specs
- [ ] T041 Amend the constitution with the Slint decision (version bump, Sync Impact note) in .specify/memory/constitution.md, and update the stack spike row and any affected later specs in docs/roadmap.md
- [ ] T042 Update specs/20261026-154808-stack-spike/handoff.md to the final state
- [ ] T043 (maintainer approves) Tag the branch head as `spike/stack-20261026` and push the tag, then delete `spike/` in a commit (research R11)
- [ ] T044 (maintainer approves) Push the branch, open the pull request, squash merge

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (US1).
- US2 (Phase 4) needs only Phase 2; it can run before or after US1.
- US3 (Phase 5) needs T012 (video inside Slint); US4 (Phase 6) needs Phase 5's components.
- Phase 7 needs all stories.
- If US1 rejects Slint (SC-002), stop before Phase 5 and decide with the maintainer how to continue.

## Parallel Opportunities

- T004 and T005; T009 and T010.
- In US1: T015 (browser page) and T016 (analyzer) while T011–T014 are built.
- In US3: T029, T030 and T031 once T027 and T028 exist.
- In US4: T034, T035 and T036.

## Implementation Strategy

1. **MVP = US1**: it alone answers whether the stack holds. Stop after Phase 3 if Slint fails.
2. Then US2 (small), US3 and US4.
3. Measure on Windows first; do the Linux checks in two visits (T021 together with T038 if convenient).
