# Handoff: Stack Spike

**Updated**: 2026-10-29 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Implementation in progress. US1 (video latency) is settled on Windows: the own render loop
(`spike/layered-preview`, research R13) with Direct3D 12 and Mailbox matches native (5–6 camera
frames ≈ 21–25 ms) with a Slint UI layer. US2 (audio) works and is measured except for drift.
US3 is mostly done: theme, components and the Float layout are approved and the video sits in a
UI-defined area. US4 has not started. All work is committed except the logos (below); nothing
pushed.

## Done in the last sessions

- US3/US4 tasks rewritten for `spike/layered-preview` (T048–T051 added).
- T048 software renderer check (research R14): nine-slice shadows kept; no gradients, no blur,
  no `drop-shadow`; SVGs must be sized in their own aspect ratio.
- T049 `--stats` reports the UI cost (≈ 0.7 ms per redraw on the shared thread).
- T027 theme (dark and light, WCAG contrast, Inter) and T028 components in the previous app's
  layout, shown with `--ui components`. The same identity is a design system artifact for other
  projects: https://claude.ai/artifact/Q5DGmM2hywuyy6T1hV6fW1 (private).
- T050 video area from the UI: video below the title bar, title bar hidden in fullscreen,
  ground in the theme's deepest tone. F11 toggles fullscreen; Esc only leaves it (product F8).
- T029 done as the components gallery; T030 and T031 (Frame, Rail) dropped, FR-006 revised.

Command (from `spike/`):
`.\target\release\layered-preview.exe --device ezcap --mode 1920x1080@60:NV12 --backend dx12 --present mailbox --ui components --range full`
(add `--audio`, `--stats` as needed).

## Decisions

- Option D (research R13): own winit + wgpu loop presents video on arrival; Slint draws the UI
  with its software renderer into a layer composited on the GPU. Direct3D 12 + Mailbox on
  Windows.
- The video never waits for the UI in the real app: the UI renders on its own thread and the
  compositor uses the last finished UI image.
- Visual rules: only panels may be semi-transparent; menus, dropdowns, buttons and icons are
  opaque; info bubbles are black at 55% with opaque text, fixed to the window's bottom right.
  The preview's device dropdown lists devices only (screens and windows are recording/stream
  sources). Dropdowns have a fixed width.
- Logo font: Urbanist ExtraBold (800), slightly tight tracking.

## Do not commit yet

- `brand/` (the maintainer's new logos) and the spike copies `spike/layered-preview/ui/assets/`
  `CapturaX.svg`, `X.svg`, `CapturaX-light.svg`. They are committed when the brand work starts.
  Note: committed spike code already references `CapturaX-light.svg`.

## Open questions

- The maintainer will compare the image with the passthrough to check whether it is darker even
  with `--range full`.
- Proposal awaiting the maintainer: the device dropdown detail shows the last mode used for that
  device, else the default automatic mode. If accepted, record it in docs/product.md F2/F3.
- Linux latency (T021, T038): wgpu uses Vulkan there; depends on KWin direct scanout.
- Audio drift (T025) and audio without Voicemeeter not measured.

## Next step

1. T032: move the gallery into `ui/directions/float.slint`; `--theme`, runtime theme switch,
   `--test-video`; real devices in the dropdowns; forward wheel, right button and keyboard to
   Slint (Esc closes menus first); clear message when no device is found.
2. T051 and T033 with the maintainer (UI cost in fullscreen at 1080p and 1440p, review).
3. US4: T034 ambient light on the GPU, T035 detachable info overlay, T036 language switching,
   T037 frameless window, then Linux (T038) and the capability table (T039).
4. At the end, formal measurement (T015–T022, T024–T026) and wrap-up (T040–T044).
