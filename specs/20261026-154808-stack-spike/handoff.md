# Handoff: Stack Spike

**Updated**: 2026-10-27 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Implementation in progress. US1 (video latency) is settled on Windows: the own render loop
(`spike/layered-preview`, research R13) with Direct3D 12 and Mailbox matches native (5–6 camera
frames ≈ 21–25 ms) with a Slint UI layer, and the encoder adds nothing. US2 (audio) works and is
measured except for drift. US3 and US4 have not started. All work is committed; nothing pushed.

## Done in the last session

- T047 own render loop, T013 encoder load test, T023 audio preview: see tasks.md and findings.md.
- Manual measurements recorded in findings.md: renderers, layered backends, YUY2 (no gain),
  encoder, audio paths, the ezcap's wrong color range.
- `layered-preview --audio` is the usable "play through the spike" command; the maintainer played
  with it and noticed no difference from the passthrough:
  `.\target\release\layered-preview.exe --device ezcap --mode 1920x1080@60:NV12 --backend dx12 --present mailbox --audio --range full`
- docs/product.md: three automatic modes in F3 (Auto source, Auto max resolution, Auto max FPS),
  a color range setting in F2, a Voicemeeter idea in Future ideas.

## Decisions

- Option D (research R13): our own winit + wgpu loop presents video on arrival; Slint draws the UI
  with its software renderer into a layer composited on the GPU. On Windows, Direct3D 12 is
  required (Vulkan goes through DWM composition, ≈ 4 frames slower). Mailbox: as fast as
  Immediate, no tearing.
- Audio: free-running output (`sync=false`), 20 ms output buffer, a queue dropping audio beyond
  20 ms (research R7, revised). Through the Voicemeeter virtual input it measures ≈ 100–110 ms;
  the previous app measured 160 ms on the same path.
- Audio measurement method changed (research R3, "Method used in practice"): passthrough monitor
  headphone out → PC line in, mixed with the preview in Voicemeeter, gap measured in FL Studio.
- The real app must present video on its own thread (the window stutters while dragged) and needs
  a color range setting (the ezcap declares limited range but sends full).
- The maintainer counts video frames by hand in After Effects; the analyzer (T016–T018) is still
  wanted for the formal 20-sample run.

## Open questions

- Linux latency (T021, T038): wgpu uses Vulkan there; depends on KWin direct scanout.
- Audio without Voicemeeter (straight to the headset) not measured.
- Remaining latency levers not tested: 120 Hz source through the PC's HDMI, VRR (findings.md).

## Next step

1. Maintainer, when convenient: T025 audio drift, measuring at minute 0 and minute 30 with
   `layered-preview --audio`, noting any clicks.
2. Rewrite US3 and US4 tasks (T027–T039) for `spike/layered-preview` instead of
   `spike/slint-preview`, before any code: the UI is drawn by Slint's software renderer, so
   measure its CPU cost at 1080p and 1440p; ambient light becomes a GPU effect in our own shader;
   the frameless title bar uses the winit window. Get the maintainer's approval, then implement.
3. At the end, formal measurement in one session: analyzer (T016–T018), browser (T015), all paths
   at 1080p60 and 1080p120 (T019–T020), Linux (T021).
