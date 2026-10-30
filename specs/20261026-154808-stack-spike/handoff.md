# Handoff: Stack Spike

**Updated**: 2026-10-30 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Implementation in progress. US1 (video latency) is settled on Windows: the own render loop
(`spike/layered-preview`, research R13) with Direct3D 12 and Mailbox matches native (5–6 camera
frames ≈ 21–25 ms) with a Slint UI layer. US2 (audio): on Windows the preview audio moves through
WASAPI directly (`audio-preview --engine wasapi`, research R7), shared or exclusive per device;
measured, except a 30-minute drift run with the new engine.
US3 is done: Float approved with live video (T033), UI cost measured in fullscreen (T051). US4
started: T034 (ambient light), T035 (info bubbles arranged per bubble, replacing the detached
window), T036 (language switching), T037 (frameless window) and T052 (video presented from its own
thread, added after T037) are done and approved. The measurement tools are written (T015, T016,
T017, T024) and the capability table is drafted (T039, Windows only); these, the formal
measurements and the audio work await the maintainer's validation of the proposed commits and
are not committed. `brand/` is not committed either; nothing pushed.

## Done in the last sessions

- T053 done (findings.md "Audio", research R7, tasks.md): the GStreamer audio path grew 2.63 ms per
  minute over 31.5 minutes (108 → 187 ms) with its own queue empty, so the growth was in the
  Voicemeeter virtual input. The previous app's method was found in this project's 2026-10-27
  session and in CapturaX-old (branch feat/hibrido, src-tauri/src/audio) and ported as
  `spike/audio-preview/src/wasapi.rs` (`--engine wasapi`, `--exclusive input|output|both`,
  `--period`, `--margin`), then improved: shared sides take the shortest Windows mixer period
  (IAudioClient3), and the playback rate follows the audio waiting in the app (2 ms spare)
  instead of cutting at 40 ms. Measured against the monitor's audio (reference panned left,
  preview right in the Voicemeeter recorder): 59 ms shared (was 112), 46 ms card exclusive (91),
  18 ms both exclusive (33); Voicemeeter reading the card is −26 ms, so the reference itself lags.
  `analyze.py audio --stereo` measures such recordings; the run-4 clips are in clips/ and in
  audio-samples.csv, summary regenerated against slint-layered 1080p60.
  Launchers 14–18 in spike/target/release (not in git); logs and recordings in clips/ and the
  maintainer's Voicemeeter folder. The maintainer stopped testing here: a hardware output cannot be
  recorded on a separate channel with their equipment.

- Formal Windows clips recorded by the maintainer (T020, T025; 11 video clips at 240 fps, audio at
  minutes 0 and 30, all in clips/) and analyzed into measurements/ (video-samples.csv,
  audio-samples.csv, summary.md; not yet written into findings.md, T022/T026). Regions for every
  clip in `clips/<clip>.regions.json` (TV right, preview left). The analyzer changed while doing
  it: changes are timed at the halfway brightness crossing (sub-frame), and a pair whose reference
  switches within one camera frame is excluded as a camera skip (data-model.md). Results (median
  ms, 1080p60): native 28.6, slint-layered 30.4, with encoder 31.5, browser 28.5, OBS 39.7,
  slint-cpu 53.2; at 1080p120 every path is 3–5 ms slower. Audio: 119 ms at minute 0, 182 ms at
  minute 30: **drift of +63 ms** (≈ 2 ms per minute, seen inside a single clip too), failing
  SC-005; cause not found (the 20 ms dropping queue does not bound it). The browser page
  opened the exact modes (120 reads 120.00048, resizeMode none). T018: the maintainer counted 8
  frames on 5-layered-60 (reference at half, preview at its first lit frame, since that monitor
  switches within one camera frame); the script gives 6.4–8.1, within one camera frame. Audio drift:
  T053 added (`audio-preview --stats` drift report, `14-audio-drift.bat` in target/release writes
  clips/audio-drift.log); the sink's ring buffer and the dropping queue bound what waits inside the
  app to about 40 ms, so the growth is likely after the app (Windows mixer or Voicemeeter), not
  confirmed. The maintainer asked whether an HTML UI would be better than Slint, since the browser
  preview matched native, and then decided to keep Slint (see Decisions).

- Measurement tools, not yet reviewed or committed: `spike/tools/analyze.py` `video` (frame
  difference per region, events merged within 300 ms, each TV event paired with the next preview
  event; prints pairs in camera frames for T018; `--fps 240` when a clip stores its playback
  rate), `audio` (onset pairs in any recording where each sound is heard twice, e.g. the
  Voicemeeter mix) and `summary` (one table per platform and mode, every path listed;
  `--av-reference` picks the video run for the A/V offset). Checked only with synthetic clips of
  known delay (exact) and then on all of the maintainer's clips (above). `spike/browser/index.html`
  (T015) is served on localhost (from disk Edge hides the device names) and opened the card's
  exact modes. contracts/cli.md, contracts/measurements.md (summary per platform and
  mode) and quickstart.md updated. findings.md has the capability table with the Windows column.

- T052 done and approved (spec US4, research R15, findings.md): moving the window stalled the
  video, with or without the system frame, because Windows holds the window's thread during a
  move and the video was presented from it. Now a `video` thread owns the GPU (src/gpu.rs) and
  is woken by the appsink directly; the main thread keeps winit and Slint and hands over the
  newest UI image, scene and size. The stall is gone; fast moves still drop to 40–50 fps (likely
  DWM, not confirmed). `--stats` prints the longest gap between presented frames and the window
  moves per second.
- T037 done and approved (findings.md): frameless window for `--ui float` only (src/window.rs),
  title bar actions through the `WindowState` global, move after 4 px so double click maximizes,
  6 px resize band handled in Rust (not checked while the left button is held on the UI). All
  checked on Windows 11 except the snap layouts flyout on the maximize button (needs
  `HTMAXBUTTON`, not available through winit). Minimum window 800 × 520; the compact layout for
  small windows is docs/product.md F8 (main screen spec). The spike-only language and theme
  switches hide when narrow; in the real app they live in the settings (F39, new F46 in the App
  shell spec). The bubbles follow a window resize at once.

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

- Audio preview on Windows (maintainer, 2026-10-30): WASAPI directly, not the GStreamer
  elements. Every audio input and output gets an **Exclusive mode** checkbox in the settings, off
  by default (docs/product.md F47); both modes stay and both are improved. Nothing specific to
  Voicemeeter or virtual devices: ordinary users must get the best result, and those with more
  than one output can reserve one. Tips for Voicemeeter users may come later (the product idea of
  letting Voicemeeter read the card stays).

- Slint stays (maintainer, 2026-10-29): the browser preview matching native latency does not
  reopen the UI choice, neither a browser preview (principle III) nor an HTML UI over the native
  video. Not to be raised again; T040 records it.

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
- Audio: 30-minute drift with the WASAPI engine and audio without Voicemeeter not measured; the
  spike's `layered-preview --audio` still uses the GStreamer path.

## Next step

1. Maintainer validates the proposed commits (tools, measurements, WASAPI audio engine, audio
   results, F47).
2. Optional: check the no-device message (T032) running. Choosing a picker item does not reopen
   the capture; say if the review needs that.
3. US4: Linux (T038, also check moving the window there), then the Linux column of T039.
4. Findings for video and audio (T022, T026) and wrap-up (T040–T044).
