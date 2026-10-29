# Handoff: Stack Spike

**Updated**: 2026-10-29 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Implementation in progress. US1 (video latency) is settled on Windows: the own render loop
(`spike/layered-preview`, research R13) with Direct3D 12 and Mailbox matches native (5–6 camera
frames ≈ 21–25 ms) with a Slint UI layer. US2 (audio) works and is measured except for drift.
US3 is done: Float approved with live video (T033), UI cost measured in fullscreen (T051). US4
started: T034 (ambient light) and T035 (info bubbles arranged per bubble, replacing the
detached window) are done and approved. T036 (language switching) is done and approved. All work is committed except `brand/`; nothing
pushed.

## Done in the last sessions

- T036 done and approved (findings.md): build.rs bundles `lang/<language>/LC_MESSAGES/layered-preview.po`
  (gettext, no translation context) and the UI strings use `@tr`; only `pt-BR` exists, English is
  the source. The toolbar has a language button left of the theme button, named in the other
  language ("Português" / "English"), which calls `slint::select_bundled_translation`; the text
  changes without restart. Slint starts in the system language; `--lang en|pt-BR` overrides it.
  Picker text built in Rust (Automatic, Auto max FPS, None, Test pattern…) is passed as English
  keys and translated in Slint by `Texts.label` (ui/texts.slint), so all translations stay in one
  catalog; the running language is the translatable string `@tr("en")` ("pt-BR" in the catalog).
  The no-device message moved to Slint (`no-device` replaces `notice`); the paired audio item is
  now "Auto" with the input's name as its detail. Checked here: builds with no warnings and
  `--lang fr` lists `pt-BR` as available. The maintainer checked it running; the review shortened
  "Stream to friends" to "Stream" ("Transmitir").

- T035 done and approved (findings.md); the spec, F42 in docs/product.md and tasks.md were
  revised: the detached floating window was built, then dropped as not useful, and its code
  discarded. Instead each info bubble is arranged on its own (ui/components/info-overlay.slint):
  hovering it with the chrome open shows a menu `[<] [pin] [corner] [>]` above it (below in the
  top corners); the corner button opens a 2 × 2 menu of the four corners, current one
  highlighted. Each corner is a group packed against it; a bubble sent to another corner goes to
  the group's far end. Chrome open: every bubble; on hiding, the unpinned ones slide off the nearest
  screen edge first, then the pinned ones close up against their corner (maintainer's review). On
  showing, the pinned ones make room first, then the others slide in.
  One menu open at a time (the last bubble entered), opening after 120 ms of hover. Top corners sit below the toolbar while it shows. Default: all bottom right, none
  pinned. IconButton gained `enabled`; new icons chevron_left/right and corner_tl/tr/bl/br. The agent's
  sandbox shows no windows: the maintainer runs every visual check.

- T034 done, approved by the maintainer with live video (findings.md): `src/ambient.rs` and
  `src/ambient.wgsl` follow the previous app's AmbientLight.tsx (96 × 54, saturation 1.4, blur
  sigma 6 texels, 35% of each new frame at 30 Hz scaled to the real frame interval, strength
  0.9). Each new video frame is shrunk (16 taps per texel), blurred in two passes, and the last
  pass blends into the previous glow; the glow is drawn stretched over the video area (same
  rounded corners) before the video, so it shows in the letterbox bars. The dock's button
  toggles it (`ambient-light` on Float); `--ambient` starts it on, `--ambient-strength` sets the
  strength (no runtime control); `--stats` prints `ambient on|off`. The layer shader's padding
  became a per-layer opacity. With `--test-video` at 1080p60 here: 60 fps both ways, frame ready
  → presented median 0.9–1.0 ms on vs 0.8–0.9 ms off (CPU side; the GPU time is not measured).

- T051 measured by the maintainer and recorded in findings.md: in fullscreen at 1080p and 1440p
  (NVIDIA DSR, no 1440p monitor) the video keeps 60 fps and a 0.8–1.1 ms ready → presented
  median; the UI takes at most 3.4 ms per redraw (open dropdown costliest).
- T033: the maintainer reviewed Float with live video and approved it with no changes;
  screenshots in measurements/screens/. US3 is done.
- T032: gallery moved to `ui/directions/float.slint` (`--ui float`, replaces `--ui components`);
  `--theme dark|light`; `--test-video` (`videotestsrc` in `--mode`, default 1080p60 NV12, new
  `capture::video::test`); pickers filled with real devices, audio inputs and modes; info overlay
  shows the running mode (latency stays a sample value); wheel, right/middle button and keyboard
  forwarded to Slint; Esc closes an open menu first; with no device the window opens with a
  message (Float only, the other UIs still fail). Checked here with `--test-video` and the C920
  (the ezcap was not connected); the no-device message was not seen running.
- Dropdown menus taller than the room below them scroll (wheel or drag) with a thin indicator.
- Fixed UI animations stepping at the video's frame rate (they looked choppy with the C920 at
  15–30 fps): the loop checked `has_active_animations` after `update_timers_and_animations`,
  which clears it. Animations now redraw at the monitor's refresh rate (≈ 43 redraws for the
  300 ms hide at 144 Hz); a timer that changes the UI is drawn at once. The fix was folded into
  the render loop commit, and the spike logos into the commits that first use them (history
  rewritten; the old branch head is kept as `backup/stack-spike-before-rebase` until approved).
- Spike logos committed as `CapturaX-dark.svg`, `CapturaX-light.svg` and `X.svg` (named like
  `brand/`).

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
`.\target\release\layered-preview.exe --device ezcap --mode 1920x1080@60:NV12 --backend dx12 --present mailbox --ui float --range full`
(add `--audio`, `--stats` as needed).

## Decisions

- The FPS bubble keeps showing the selected mode in the spike: the measured frame rate is built
  with F7 in the real app, whose spec defines which rate is shown (maintainer, 2026-10-29).

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

- `brand/` (the maintainer's new logos): committed when the brand work starts. The spike copies
  in `spike/layered-preview/ui/assets/` are committed.

## Open questions

- The maintainer will compare the image with the passthrough to check whether it is darker even
  with `--range full`.
- Proposal awaiting the maintainer: the device dropdown detail shows the last mode used for that
  device, else the default automatic mode. If accepted, record it in docs/product.md F2/F3.
- Linux latency (T021, T038): wgpu uses Vulkan there; depends on KWin direct scanout.
- Audio drift (T025) and audio without Voicemeeter not measured.

## Next step

1. Optional: check the no-device message (T032) running; the ezcap was seen in T033. Choosing
   a picker item does not reopen the capture; say if the review needs that.
2. US4: T037 frameless window, then Linux (T038) and the capability table (T039).
3. At the end, formal measurement (T015–T022, T024–T026) and wrap-up (T040–T044).
