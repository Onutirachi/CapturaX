# Research: Stack Spike

Decisions needed before building the spike prototypes. Each entry: decision, rationale,
alternatives considered.

## R1. Versions

- **Decision**: GStreamer 1.28.x (1.28.8 is the current stable, 1.30 expected in Q4 2026);
  gstreamer-rs 0.25; Slint 1.18; Rust stable 1.99 (installed, MSVC toolchain).
- **Rationale**: latest stable releases; gstreamer-rs 0.25 targets the 1.28 API.
- **Windows build**: the **MSVC** x86_64 package, matching the Rust MSVC toolchain and Visual
  Studio 2022 (MinGW binaries cannot be linked by the MSVC toolchain). Since 1.28 there is a single
  `.exe` installer for runtime and development files; the development components are selected in the
  installer (or with `/TYPE=devel`).
- **End users**: never install GStreamer; it ships inside the app folder (constitution VI). Only
  developers install it to build.
- **Alternatives**: waiting for GStreamer 1.30 — rejected, the spike must not wait for a release
  and the elements used here exist in 1.28.

## R2. Glass-to-glass video latency method

- **Decision**: the PS5 plays an A/V sync test video (a white flash with a beep, once per second)
  through the YouTube app. The reference display ("TV" below: the 27G2G4 monitor on the card's HDMI
  passthrough) and the PC monitor (preview, C24FG7x, never the monitor on the integrated GPU) are
  filmed side by side with the Galaxy S23 FE at 240 fps. A script finds each flash in the TV region
  and in the preview region of the clip and reports the time difference per flash.
- **Rationale**: one 25-second clip yields over 20 samples; at 240 fps the resolution is about 4 ms,
  enough to resolve one 60 fps frame (16.7 ms). Automating the frame search makes 20+ samples per
  path and mode cheap and removes counting errors. The TV adds its own latency, so results are
  relative to the TV; this is fair across paths, which is what the decision rule needs.
- **Alternatives**: counting frames by hand — slow and error-prone; an HDMI output of the PC fed
  into the card with an on-screen timer — needs a second output and measures a desktop loop, not
  the console case; in-pipeline timestamps only (GStreamer latency tracer, driver timestamps) —
  kept as a secondary measurement because it excludes display and capture-card delay.

- **HDCP**: the PS5's HDCP setting must be off (Settings > System > HDMI), otherwise the card
  captures a black image. Some PS5 video apps refuse to play with HDCP off; if the YouTube app is one
  of them, the sync test video is played from a USB drive in the PS5 media gallery instead. CapturaX
  never tries to bypass HDCP.
- **Source used in practice**: the PS5 media gallery did not list MP4 files from the maintainer's USB
  drive, so the video test runs from a page (spike/tools/flash.html) opened in the PS5 browser,
  flashing white for 50 ms once per second, without sound. Source timing jitter does not affect the
  result, since both displays show the same console frame. Audio uses the menu clicks below.
- **Fallback source**: if no test video can be played, the maintainer moves back and forth between
  PS5 menu screens; each screen change is a visual event and each move plays a UI click. The
  analyzer therefore detects sudden image changes (frame-difference spikes) in each region, not
  only white flashes, and sudden sound onsets, not only beeps.

## R3. Audio latency and A/V offset method

- **Decision**: same test video. The phone records normal video with sound (slow motion records no
  audio) while the TV speaker and the PC speaker play; each beep is heard twice. A script finds the
  beep pairs and reports the delay of the PC sound relative to the TV. The A/V offset of the preview
  is (audio delay) − (video delay) from R2, measured in the same session.
- **Rationale**: same reference (the TV) for audio and video, so the offset is directly comparable
  to the ITU-R BT.1359 limits in SC-005.
- **Alternatives**: measuring with the headset — cannot be recorded; loopback cable into the PC —
  measures the PC's own input path, not what the player hears.

- **Method used in practice**: the passthrough monitor's headphone output goes by cable into the PC's
  line input; Voicemeeter mixes that reference with the preview output (the system default output
  is a Voicemeeter virtual input) into the headset. Each PS5 menu click is heard twice. The
  maintainer records the mix and measures the gap between the two clicks in FL Studio. The result
  is the preview relative to the line-input path, both through Voicemeeter.

## R4. Native-window path (reference for the fastest possible preview)

- **Decision**: Windows: `mfvideosrc` (Media Foundation) → GPU upload → `d3d12videosink`, with
  `d3d11videosink` as a variant. Linux: `v4l2src` → `glimagesink`, with `waylandsink` and
  `vulkansink` as variants if available on CachyOS with the NVIDIA driver. All sinks with
  `sync=false` (show each frame as soon as it arrives).
- **Rationale**: these are GStreamer's native presentation sinks; the fastest of them is the floor
  the Slint path is compared against.
- **Alternatives**: writing a custom swap chain — not justified for a spike if a stock sink reaches
  the floor.

## R5. Video inside Slint

- **Decision**: two variants.
  1. **CPU copy** (required): `appsink` (`sync=false`, `max-buffers=1`, `drop=true`) delivering RGBA
     frames that become a `slint::Image`, as in Slint's official `gstreamer-player` example. Works on
     Windows and Linux.
  2. **GPU, no copy** (timeboxed): Linux via EGL, as in the same example; Windows by importing a GPU
     texture through Slint's wgpu integration (`wgpu_texture` example) or rendering the video under
     the UI (`opengl_underlay` example). If the Windows GPU variant does not work within its
     timebox, the report says so and why.
- **Rationale**: the official example uses CPU copy on Windows and EGL zero-copy on Linux, so the key
  question (is video inside Slint fast enough?) has different answers per platform. The CPU variant
  tells whether a simple, portable path is already good enough; the GPU variant tells what is
  possible if it is not.
- **Alternatives**: overlay UI in a separate transparent window above a native video window —
  measured only if both Slint variants fail SC-002, because it complicates windowing.

## R13. Own render loop with Slint as a layer (decided 2026-10-27)

- **Context**: native ≈ 25 ms, slint-cpu 42–50 ms whatever the renderer, OBS ≈ 50 ms (findings.md).
  The extra time is in Slint's render pacing and presentation, not in copies.
- **Decision**: try an own render loop. The app owns the window (winit) and the swap chain (wgpu,
  present mode Immediate, else Mailbox, else Fifo) and presents as soon as a video frame arrives.
  Slint keeps drawing the whole UI with the same `.slint` files, through its public software
  renderer into a premultiplied RGBA layer that is redrawn only when the UI changes; the loop
  composites video + UI layer on the GPU in one frame. Window, mouse and keyboard events are
  forwarded to Slint through its platform API.
- **Rationale**: overlays are real (one image), one window only (works on Wayland), Slint's
  customization is kept, and presentation is under our control.
- **Alternatives**: B, native video child window with overlays in extra transparent windows —
  kept as fallback; rejected first because Wayland does not let apps position windows, so the
  overlay windows would not work on Linux. egui instead of Slint — less suited to the visual goals.
  Finishing T014 steps 2 and 3 — expected to stay at ≈ 35–40 ms because presentation stays in Slint.
- **Floor**: frame-based capture costs about one source frame (16.7 ms at 60 Hz); native is close
  to the realistic floor (≈ 18–20 ms at 60 Hz). Further gains come from a 120 Hz source and
  tearing presentation, which apply to any option.

## R14. What the software renderer draws (2026-10-27)

- **Context**: under R13 the whole UI goes through Slint's software renderer. Its 1.18.1 source
  (`i-slint-renderer-software`) leaves `draw_box_shadow` empty and ignores the radius when
  clipping; rectangles with rounded corners, borders, linear, radial and conic gradients, images,
  paths (SVG), text with bundled fonts and opacity are drawn. T048 confirmed it and found two more
  limits: gradients ignore the border radius, and a `Path` filled with a gradient is drawn in a
  solid color.
- **Decision**: confirm this on a test screen (T048) and try a shadow substitute: pre-blurred
  shadow images stretched as nine-slice under raised surfaces. If no substitute looks right, drop
  shadows and give depth with tone steps and gradients only. The video corners are rounded in our
  shader, not by Slint clipping. Result (T048): the nine-slice shadows look right and are kept.
  The maintainer dropped gradients and blur (2026-10-27): surfaces are solid tones with rounded
  corners and nine-slice shadows; only panels (dock, title bar, info overlay) may be
  semi-transparent over the video, while menus, dropdowns, buttons and icons are opaque. Slint has
  no blur in any renderer (only the shadow's own blur). Shadows are hollow (the shape is cut out) and carry their
  offset inside the image, because the renderer applies opacity to each element separately: a
  filled shadow would show through a semi-transparent surface.
- **Later option, not in the spike**: surfaces drawn by our shader from geometry reported by the
  UI (signed distance field), which scales perfectly and could blur the video behind a panel
  (considered and dropped by the maintainer).
  Limits: everything it draws sits under the whole Slint layer, so overlapping elements (menus,
  tooltips) cannot use it; positions must follow the UI's animations; part of the look leaves the
  `.slint` files.
- **Rationale**: the Ninjoy prints are a starting point, not a target (maintainer, 2026-10-27);
  depth that stays inside the `.slint` files without a second renderer is worth more than
  matching them.
- **Alternatives**: shadows drawn by our GPU compositor from element geometry — more code, kept in
  case the images look poor at different sizes; Slint's Skia or FemtoVG renderer into a texture —
  brings back a GPU context shared with Slint, which R13 avoided.
- **Cost**: the UI is redrawn only when it changes (partial redraws with a reused buffer), but a
  full redraw at 1440p is a CPU cost on the video thread; T049 and T051 measure it.

## R6. Capture modes

- **Decision**: list the card's modes with `gst-device-monitor-1.0` first, then test 1080p60 and the
  highest frame-rate mode (expected 1080p120). Prefer uncompressed formats (NV12, YUY2); if a mode is
  only offered as MJPEG, decode it on the GPU when possible and note the decoder used.
- **Rationale**: the spec requires native modes; the format determines whether decoding adds
  latency.
- **Alternatives**: testing every mode — unnecessary for a decision.

## R7. Preview audio

- **Decision**: Windows: `wasapi2src` → `wasapi2sink`, both with `low-latency=true`. Linux:
  `pipewiresrc` → `pipewiresink`. The output device is chosen by the user (the headset).
- **Rationale**: WASAPI low-latency mode reached about 10–20 ms in the previous app; PipeWire is
  CachyOS's audio server.
- **Alternatives**: PulseAudio elements on Linux — PipeWire native elements avoid the compatibility
  layer.
- **Revised in practice (T023)**: playing on the pipeline clock (`sync=true`) stays silent: the
  output keeps correcting a ≈ 170 ms clock skew. The chosen setting free-runs the output
  (`sync=false`, as the previous app did) with a 20 ms output buffer, after a queue that drops its
  oldest audio beyond 20 ms, so a delay at start-up cannot stay in the buffers.

## R8. Encoder load test

- **Decision**: add a `tee` branch to the Slint path with the NVIDIA hardware H.264 encoder
  (`nvh264enc` or the D3D12 variant on Windows) at the recording preset "High" (1080p60, about
  12 Mbps) into a discarding sink. Measure preview latency (R2) and dropped frames (sink statistics)
  with and without it.
- **Rationale**: proves early that one pipeline can record without hurting the preview (SC-004).
- **Alternatives**: writing real files — not needed to measure the load.

## R9. Browser and OBS references

- **Decision**: browser: a single HTML page using `getUserMedia` with `resizeMode: "none"` and the
  same mode, opened in Edge (Chromium, same engine as WebView2). OBS: a Video Capture Device source
  in the same mode, shown fullscreen-projected on the monitor.
- **Rationale**: the browser path is what the previous app used; OBS is the reference users compare
  against.

## R10. Prototype UI directions

- **Decision**: three directions in one Slint app, switchable at runtime, each with dark and light
  themes:
  - **Float**: full-bleed video; title bar, source picker and dock float over it as raised
    surfaces with soft depth, hiding when idle.
  - **Frame**: the video sits in a sunken well; chrome lives around it (top bar, bottom dock) and
    never covers the video.
  - **Rail**: full-height video with a slim vertical rail for the dock and source picker; the info
    overlay is a compact strip.
- **Rationale**: three distinct answers to "where does the chrome go relative to the video", which is
  the main layout question for a video-first app.
- **Alternatives**: color-only variations of one layout — would not inform the layout decision.

## R11. Keeping spike code out of `main`

- **Decision**: spike code lives in `spike/` on the spike branch. Before the pull request, a tag
  `spike/stack-20261026` is pushed to keep the code reachable, then `spike/` is deleted in a commit,
  so the squash merge brings only the documentation.
- **Rationale**: satisfies constitution principle VII while keeping the code available for later
  specs.

## R12. Analysis tooling

- **Decision**: one Python script run with `uv run` (inline dependencies: OpenCV, NumPy,
  imageio-ffmpeg) that analyzes the slow-motion clips (R2) and the audio clips (R3) and writes CSV
  rows plus a summary.
- **Rationale**: `uv` and Python are installed; the script is a measurement tool, never shipped.
