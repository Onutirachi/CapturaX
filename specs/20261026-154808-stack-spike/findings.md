# Findings: Stack Spike

Status: **in progress**. Sections are filled as the spike advances; decisions are taken only at the
end (T040).

## Environment

See [measurements/environment.md](measurements/environment.md).

## Method (as run)

- Source: [spike/tools/flash.html](../../spike/tools/flash.html) served from the Windows PC
  (`live-server`) and opened in the PS5 browser through a link sent by e-mail (the browser has no
  address bar, and the PS5 media gallery did not list MP4 files from the USB drive). The page
  flashes white for 50 ms once per second.
- Reference: 27G2G4 on the card's HDMI passthrough. Preview: C24FG7x, 1920x1080 @ 144 Hz,
  DisplayPort on the RTX 5070 Ti, fullscreen.
- Camera: Galaxy S23 FE at 240 fps (one camera frame ≈ 4.17 ms).
- Counting: **by hand**, camera frames between the flash on the reference and on the preview,
  in After Effects, across several clips per path.

## Video latency — preliminary (Windows, 1080p, PS5 at 60 Hz)

Delay of the preview relative to the passthrough reference.

| Path | 1080p60 | 1080p120 | Notes |
|---|---|---|---|
| native (`d3d12videosink`) | 6 frames ≈ **25 ms** | 6 frames ≈ **25 ms** | |
| slint-cpu (bare) | 12 frames ≈ **50 ms** | 12 frames ≈ **50 ms** | |
| OBS (preview) | 12–13 frames ≈ **50–54 ms** | — | |
| browser | not measured yet | | |
| slint, other renderers | 10–12 frames ≈ **42–50 ms** | — | T014 step 1, below |
| **slint-layered, DX12** (Immediate or Mailbox) | 5–6 frames ≈ **21–25 ms** | — | T047, below |
| slint-layered, Vulkan Immediate | 10 frames ≈ **42 ms** | — | |
| slint-layered, Vulkan Mailbox | 9–10 frames ≈ **38–42 ms** | — | |

The maintainer reports consistent values across several clips per path. These results are
**preliminary** until the analysis script (T016–T018) produces at least 20 samples per path and
mode with median and 95th percentile (FR-002).

### Reading against the success criteria (preliminary)

- **SC-002** (Slint within one frame of native): **fails** for the CPU-copy variant: ≈ 25 ms more
  than native, 1.5 frames at 60 fps. **Passes** for slint-layered on DX12 (T047), on par with
  native.
- **SC-003** (chosen path no more than one frame slower than OBS): slint-cpu is on par with OBS;
  native is ≈ 25 ms faster than OBS.
- 60 and 120 fps capture give the same delay because the source runs at 60 Hz; the mode only
  matters with a 120 Hz source (a PS5 game with 120 Hz output).

### Where slint-cpu spends its time (T046)

Median per frame, 1080p60, measured inside the app:

| Stage | ms |
|---|---|
| capture → appsink (GPU color conversion and download) | 1.1 |
| copy into the Slint image | 1.9 |
| wait for the UI thread | 0.0 |
| UI update → render done | 8.3 |
| **total inside the app** | **≈ 11.4** |

The copies cost only ≈ 3 ms, so moving frames through the GPU alone cannot close a ≈ 25 ms gap.
Inferred, not measured directly: ≈ 8 ms is the frame waiting for Slint's next render (it renders
about 120 times per second for 60 frames), and ≈ 15 ms comes after rendering, in the presentation
queue (one or two refreshes at 144 Hz). T014 was redirected to these two stages.

### T014 step 1 — Slint renderers (1080p60, counted by hand, 240 fps camera)

| Renderer | Presents through | Camera frames | ≈ ms | In-app pacing (UI → drawn) |
|---|---|---|---|---|
| femtovg (default) | OpenGL | 12 | 50 | 8.4 ms, 120 draws for 60 frames |
| femtovg-wgpu | Direct3D 12 (wgpu) | 11 | 46 | 8.6 ms, 120 draws |
| skia | Direct3D | 12 | 50 | not reported (no draw notifications) |
| skia-opengl | OpenGL | not filmed (same path as femtovg) | — | 8.7 ms, 120 draws |
| skia-wgpu | — | fails: "Unsupported WGPU backend for use with Skia: vulkan" | — | — |
| software | CPU drawing, copied to the window | 10 | 42 | not available (no rendering notifier) |

The renderer changes at most ≈ 8 ms; ≈ 17 ms remain versus native. The in-app pacing is the same
for every GPU renderer, so it comes from Slint's core and not from the renderer. The software
renderer, which does not go through the GPU's vsync queue, is the fastest, which supports the
inference that much of the gap is in presentation.

### T047 — own render loop (slint-layered)

winit window and wgpu swap chain presenting each video frame on arrival; the Slint UI rendered by
the software renderer into a transparent layer and composited over the video on the GPU
(research R13). 1080p60, fullscreen, counted by hand at 240 fps.

| Backend | Present mode | Camera frames | ≈ ms |
|---|---|---|---|
| Vulkan | Immediate | 10 | 42 |
| Vulkan | Mailbox | 9–10 | 38–42 |
| Direct3D 12 | Immediate | 5–6 | 21–25 |
| Direct3D 12 | Mailbox | 5–6 | 21–25 |

- Inside the app, frame ready → presented takes ≈ 0.9 ms (median), so the backend difference is
  after presentation.
- The DWM hypothesis holds: a Vulkan swap chain on Windows is composed by DWM, while a DXGI
  flip-model swap chain (Direct3D 12) in fullscreen can get independent flip and skip composition,
  like the native sink.
- Mailbox is as fast as Immediate on Direct3D 12 at this measurement resolution and does not tear.
  The maintainer saw occasional tearing with Immediate; whether it is noticeable while playing is
  still to be judged.
- The UI layer works: text and button draw over the video and the button takes clicks.
- YUY2 instead of NV12 (Direct3D 12, Mailbox, 1080p60): 7 camera frames, against 5–6 with NV12.
  No gain on this card; at most one camera frame worse, within manual counting noise. NV12 stays
  first in the format preference.
- On Linux there is no Direct3D: wgpu uses Vulkan and the result depends on the compositor
  (direct scanout and tearing control on Wayland). To be measured (T021, T038).

### T013 — encoder load (slint-layered, Direct3D 12, Mailbox, 1080p60)

A `tee` branch encodes the video with `nvh264enc` at 12 Mbps into a discarding sink, as a
recording would; its queue does not drop frames. The preview goes through the same `tee` with and
without the encoder.

| Run | Camera frames | Encoded | Preview dropped | Frame ready → presented (median) |
|---|---|---|---|---|
| without encoder | 5–6 | — | 0 | ≈ 1.1 ms |
| with encoder | 5–6 | 60 fps | 0 | ≈ 1.1 ms |

**SC-004 passes** (preliminary, counted by hand): no measurable added latency and no dropped
frames.

### Remaining latency levers (not yet tested)

At 60 Hz the card delivers a frame only after receiving all of it (≈ 16.7 ms), so 21–25 ms is
within a few milliseconds of the floor. What is left, largest first:

1. A 120 Hz source halves that wait (≈ 8 ms less). Testing needs a 120 Hz source with the flash
   page, such as the PC's HDMI output into the card, since the PS5 browser runs at 60 Hz.
2. VRR on the preview monitor removes the wait for the next refresh without tearing. The
   C24FG7x range likely starts above 60 Hz, so it may only help with a 120 Hz source.
3. PresentMon can confirm independent flip and measure the time to display with millisecond
   resolution, below the camera's 4.17 ms.
4. Card modes (720p60; YUY2 tested, no gain), GPU power settings and the monitor's low-lag mode: small, cheap
   checks.
5. Zero-copy upload: under 1 ms; left to the real implementation.

A 120 Hz source only helps if the app captures at 120 Hz. The spike's automatic choice (largest
resolution at about 60 fps) would pick 1440p60 on this card and drop half of a 1080p120 signal.
docs/product.md F3 now lists an **Auto source** mode (follow the incoming signal, when the device
reports it) next to Auto max resolution and Auto max FPS; how to read the incoming signal per
platform is for the capture spec.

The card is UVC only, so delivery before a frame is complete (as some professional cards do) is
not available.

## Audio — preliminary (Windows, PS5 menu clicks, via Voicemeeter)

Method: research R3, "Method used in practice". Delay of the preview relative to the line-input
reference, measured by hand in FL Studio over several clicks.

| Path | Delay | Notes |
|---|---|---|
| Voicemeeter reading the card directly (WDM) | ≈ 0 | the two clicks are hard to tell apart |
| audio-preview, pipeline clock (`sync=true`) | silent | continuous clock-skew correction |
| audio-preview `--free-run` (200 ms output buffer) | 300–350 ms | |
| `--free-run --buffer 20` | ≈ 120 ms | |
| `--free-run --buffer 20` with the 20 ms dropping queue | **100–110 ms** | no clicks or dropouts; chosen |
| same, `--exclusive both` | lower | heavily distorted and crackling; exclusive mode also locks the device |
| previous CapturaX (raw WASAPI, shared mode, 15 ms output queue) | 160 ms | |

- The card adds almost nothing: what remains is the path from an app into the Voicemeeter virtual
  input through the Windows mixer, which the previous app paid too. Not measured yet without
  Voicemeeter (output straight to the headset).
- **A/V offset**: audio ≈ 105 ms against video ≈ 21–25 ms, so sound is ≈ 80 ms behind the image,
  within the 125 ms limit of SC-005 (preliminary). Playing with `layered-preview --audio`, the
  maintainer noticed no offset; the difference is audible only when the passthrough sound plays at
  the same time.
- Drift over 30 minutes: not measured yet (T025).

## Image

- The ezcap declares limited range (16–235, colorimetry `2:3:0:1`) but sends full range: on a PS5
  home screen about 8% of the luma values were below 16. Read as limited, those are clipped to
  black and the preview looks darker than the passthrough. With `--range full` the maintainer
  found it matching the passthrough on the same monitor (27G2G4, switching HDMI and DisplayPort
  inputs). The app needs a color range setting (Auto, Full, Limited); Auto can infer full range
  from values below 16.

## Observations

- The previous CapturaX showed video with `getUserMedia` in WebView2 and measured 6 camera frames,
  on par with native: an indication for the browser reference (T015).
- Playing with `layered-preview --audio` (Direct3D 12, Mailbox, 1080p60), the maintainer noticed no
  difference from the passthrough by eye.

- Dragging the slint-layered window stutters the video, sometimes for a long moment, with or
  without the encoder, while the pipeline keeps delivering 60 fps. On Windows the system holds the
  app's event loop during a window move, and this prototype presents from that loop. The real app
  must present video on a thread of its own. Fullscreen is not affected.

- In the 1080p120 mode with a 60 Hz source, the card delivers about 111 fps on every path; the
  limit is the source, not the path.
- The in-pipeline latency query reports 0 ms with `mfvideosrc`; it cannot be used as a secondary
  measurement in this form.
- The Slint window drew about twice per delivered frame at 60 fps (120 draws for 60 frames):
  redundant rendering to investigate.
- Judged by eye, the maintainer could not reliably tell which path was faster; measurement was
  needed.

## Capabilities

Spec User Story 4, one row per capability (T039). Draft: the Linux column is filled in T038. The
sections below record how each capability was built and checked.

| Capability | Windows | Linux | Notes |
|---|---|---|---|
| Ambient light around the video, in its colors | working | not checked (T038) | GPU passes in the render loop, ≈ 0.1 ms CPU side; GPU time not measured (T034) |
| Info overlay as separate bubbles: pin, order, corner; only pinned ones with the chrome hidden | working | not checked (T038) | Positions computed by hand (Slint layouts do not animate); placements not saved between runs (T035). The detached floating window was dropped; on Wayland it would have been limited (no always on top or placement) |
| Resolution badges (HD, Full HD, QHD, 4K) in distinct colors | working | not checked (T038) | `Badge` with the previous app's colors, in the mode picker and the resolution bubble (T028) |
| Source picker with groups and badges | working | not checked (T038) | Dropdown with group headers, detail and badge per item (T028); the preview's device picker lists devices only (screens and windows belong to recording and stream sources), so groups are seen in the mode picker (Automatic, Modes) |
| Depth effects (layered shadows, raised and sunken surfaces) | limited | not checked (T038) | Nine-slice shadow images and tone steps; the software renderer draws no `drop-shadow`, no gradients on rounded shapes, no rounded clip, and Slint has no blur. Shadows need a @2x set above 100% scale (T048) |
| Custom fonts and the brand logo | working | not checked (T038) | Bundled Inter; SVG logos must be sized in their own aspect ratio, and the dark theme uses a light variant (T048) |
| Language and theme switching without restart | working | not checked (T038) | Bundled gettext catalogs and `@tr`; text built in Rust is translated in Slint (T036) |
| Frameless window: move, resize, maximize, fullscreen | limited | not checked (T038) | All work, snapping included; no snap layouts flyout on the maximize button (needs `HTMAXBUTTON`, not reachable through winit) (T037) |
| Smooth video while the window moves or resizes | limited | not checked (T038) | No stall since the video has its own thread; fast moves drop to 40–50 fps, likely DWM, not confirmed (T052) |

### What the software renderer draws (T048, Slint 1.18.1, Windows)

Test screen `layered-preview --ui render-test`, over the video, checked from screenshots.

| Feature | Result |
|---|---|
| Bundled font (Inter, variable) | Drawn; weights 400, 600 and 800 distinct; system fonts also available. The maintainer liked Inter: it is the font for the directions |
| SVG logo | Drawn anti-aliased, **but with 2 px teeth when the element's box does not have the SVG's aspect ratio**: Slint rasterizes the SVG keeping its aspect (90×87 for a 90×90 box) and then stretches the result without filtering, repeating one row in every ~29. Sizing the image by one side only (`height: 90px`) removes them. The CapturaX wordmark has dark letters: the dark theme needs a light variant |
| Rounded rectangle (solid color) | Drawn, smooth corners |
| Linear and radial gradients | Drawn, but **the border radius is ignored**: square corners |
| Gradient fill on a rounded `Path` | **Drawn as a solid color**: not a workaround |
| Opacity | Drawn, blends with the video |
| `drop-shadow-*` | **Not drawn** (empty in the renderer source) |
| Clip with a border radius | **Square**: the radius is ignored |
| Nine-slice image | Drawn; a pre-blurred shadow (tools/make_shadow.py, two layers: soft and tight) looks soft, with no seams at 3× zoom |
| Overlapping menu | A menu with shadows opens over other elements with an opacity and position animation (160 ms). Captured mid-fade and open while semi-transparent: the content shows through it without the shadow darkening it (hollow shadows). Menus are now opaque (maintainer's decision); the fade still applies |
| Blur | Not available in Slint in any renderer. Dropped (maintainer's decision): only panels are semi-transparent, without blur |

Outcome: depth with nine-slice shadows and rounded corners works, overlapping menus included
(research R14). Gradients and blur are dropped (maintainer's decision); only panels are semi-transparent. Anything round over the video (its
own corners, a clipped thumbnail) is drawn by our shader. The shadows are images: their corner
radius and blur are fixed per image, and screens above 100% scale need a @2x set.

### UI cost on the video loop (T049, first look)

`layered-preview --ui render-test --stats`, 1280×720 window, 100% scale, 1080p60 video. UI time is
the software render plus the upload of the whole UI texture.

| State | UI redraws per second | UI time median / max | Frame ready → presented |
|---|---|---|---|
| Idle | 0 | — | 0.8–0.9 ms |
| Menu opening and closing twice a second | 25–32 | 0.7 / 0.9 ms | 0.8–1.1 ms |
| First draw at start (fonts, SVG parsing) | 1 | 4–41 ms | — |

An animating UI does not delay the video measurably at this size. Fullscreen at 1080p and 1440p
with the real directions is T051.

### UI cost in fullscreen (T051)

`--ui float --stats`, ezcap 1080p60 NV12, Direct3D 12 + Mailbox, fullscreen (F11) on a 1080p
144 Hz monitor at 100% scale. The maintainer has no 1440p monitor: 1440p is the same monitor at
2560×1440 through NVIDIA DSR (1.78x DL), which gives the same UI texture size. Animations redraw at
144 Hz, so a 300 ms chrome animation is about 43 redraws and a 160 ms dropdown animation about 23.

| Screen | State | UI redraws per second | UI time median / max | Frame ready → presented |
|---|---|---|---|---|
| 1080p | Idle, chrome hidden | 0 | — | 0.8–1.0 ms |
| 1080p | Chrome showing | 45 | 1.4 / 1.8 ms | 0.9 ms |
| 1080p | Chrome hiding | 41 | 0.7 / 2.1 ms | 0.9 ms |
| 1080p | A dropdown opening or closing every second | 21–22 | 2.0–2.8 / 3.4 ms | 0.8–0.9 ms |
| 1080p | Pointer moving to the other dropdown | 53 | 0.7 / 2.9 ms | 1.1 ms |
| 1440p (DSR) | Idle, chrome hidden | 0 | — | 0.8–0.9 ms |
| 1440p (DSR) | Chrome showing, pointer moving over it | 85 | 1.5 / 2.6 ms | 1.0 ms |
| 1440p (DSR) | Chrome hiding | 39 | 1.1 / 2.6 ms | 1.0 ms |

Outcome: the video kept 60 fps and its per-second median stayed at 0.8–1.1 ms in every state, at
both sizes. The UI never took more than 3.4 ms per redraw. The open dropdown is the costliest UI
(2.0–2.8 ms median, more for the longer list), and 1440p costs only a little more than 1080p
(1.5 against 1.4 ms showing the chrome), far less than its 1.78 times the pixels. The per-second
median hides single frames: in this prototype a video frame that arrives during a redraw waits up
to its length (3.4 ms at most), which the real app avoids by rendering the UI on its own thread.
Two seconds at the start of the 1080p run showed a 9 ms median with no UI redraw; they did not
come back at 1440p and look like the switch to fullscreen, not the UI (cause not confirmed).

Decision for the real app (maintainer, 2026-10-28), whatever T051 shows: the video never waits
for the UI. The UI is rendered on its own thread and the compositor always uses the last finished
UI image; in this prototype a video frame that arrives while the UI changes waits for the redraw.

### Ambient light (T034)

Built on the GPU in the render loop, as the previous app's AmbientLight.tsx: each new video frame
is shrunk to 96×54 (16 taps per texel, saturation 1.4), blurred in two passes (sigma 6 texels) and
blended into the previous glow (35% of a new frame at 30 Hz, scaled to the real frame interval).
The glow is drawn stretched over the video area at strength 0.9, before the video, so it shows in
the letterbox bars. The dock's button toggles it.

`--ui float --stats`, `--test-video` 1080p60, Direct3D 12 + Mailbox, 1280×720 window:

| Ambient light | Presented | Frame ready → presented |
|---|---|---|
| Off | 60–61 fps | 0.8–0.9 ms |
| On | 60–61 fps | 0.9–1.0 ms |

Outcome: working on Windows. The maintainer checked it with live video and a widened window and
liked the result. The difference is about 0.1 ms on the CPU side; the GPU time of the three small
passes is not measured. Linux is checked in T038.

### Info bubbles (T035)

The capability "overlay detached as a floating window that stays on top" was built and dropped:
the maintainer found it not useful. It worked as a second borderless winit window with its own
Slint software window (the platform hands out one window adapter per component). On Windows it
needed its own wgpu device: only a DirectComposition swap chain (`DxgiFromVisual`) lets a window
be transparent, and the main window keeps its HWND swap chain. On Wayland, always on top and
window placement are not available to apps, so it would have been limited there.

What replaces it, all in Slint over the video: each bubble (status, resolution, FPS, latency) is
arranged on its own. Hovering a bubble for 120 ms with the chrome open shows a menu (move left,
pin, corner, move right), one menu at a time; the corner button opens a 2 × 2 menu of the four
corners. Each corner holds its own group, packed against it. When the chrome hides, the unpinned
bubbles slide off the nearest screen edge, then the pinned ones close up against their corner;
on showing, the pinned ones make room first, then the unpinned ones slide back vertically (at
once when none is pinned). Positions
are computed by hand (Slint layouts do not animate), with the per-bubble state in arrays of the
component.

Outcome: working on Windows; reviewed and approved by the maintainer with `--test-video`. The
FPS bubble shows the selected mode and the latency bubble a sample value, not measurements.
Placements are not saved between runs.

### Language switching (T036)

Slint's bundled translations: build.rs compiles `lang/<language>/LC_MESSAGES/layered-preview.po`
(gettext, no translation context) into the binary, and the strings in the `.slint` files use
`@tr`, with English as the source. `slint::select_bundled_translation` switches the language
while running; every translated text updates at once, with no restart and no rebuild of the
pickers. At startup Slint picks the system language; `--lang en|pt-BR` overrides it. The
toolbar's language button names the other language in itself ("Português" / "English").

`@tr` takes literals only, so text built in Rust (the pickers' automatic entries, groups, None,
Test pattern) is passed as English keys and translated in Slint by a lookup function
(`Texts.label`); all translations stay in one catalog. The running language is itself a
translatable string (`@tr("en")`, "pt-BR" in the catalog), so the UI knows it without Rust.
Strings are written by hand here; the real app would extract them with `slint-tr-extractor`.

Outcome: working on Windows; reviewed and approved by the maintainer with `--test-video`
(English and Portuguese). The review shortened "Stream to friends" to "Stream".

### Frameless window (T037)

The Float direction opens a winit window without decorations (`with_decorations(false)`, plus
`with_undecorated_shadow` on Windows for the system shadow and Windows 11's rounded corners); the
other UIs keep the system frame, since they have no title bar of their own. The title bar from
T028 asks for window actions through the `WindowState` global (drag, minimize, toggle maximize,
close; `maximized` comes back from Rust on each resize). A press on the bar starts the system move
(`drag_window`) only once the pointer moves more than 4 px, so a double click still reaches the
bar and toggles maximize; after the move the loop sends Slint a pointer exit, since the system
move swallows the release. Resizing is handled in Rust, before Slint: a 6 px band along the edges
and corners sets the resize cursor and a left press there calls `drag_resize_window` (off when
maximized or fullscreen). While the left button is held on the UI the edges are not checked, so
a fast drag of the title bar toward the top edge keeps the normal cursor (the move starts only
after 4 px, and the pointer could reach the resize band first).

Checked by the maintainer on Windows 11: move by the bar (a maximized window restores under the
pointer), double click to maximize and restore, resize from the four edges and corners, minimize,
maximize, close, F11 and the dock's fullscreen, snapping by dragging to the screen's edges and
Win+arrows. Not available: the snap layouts flyout on the maximize button. Windows shows it only
when the window answers `WM_NCHITTEST` with `HTMAXBUTTON` over that button, which winit does not
let an app do; it would take subclassing the window procedure.

Review fixes: the minimum window is 800 × 520, the smallest where Float fits without overlaps
(the toolbar's three dropdowns across; the right dock between the toolbar and the bottom
bubbles); a compact layout for smaller windows is left to the main screen's spec (docs/product.md
F8). The spike-only language and theme switches hide when the toolbar is too narrow for them; the
real app keeps both in the settings (F39, F46). The info bubbles animate their place relative to
their corner, so a window resize moves them at once (animating the absolute place made them chase
the new corner for 300 ms).

**The video stalls while the window is moved**, with the system frame (`--ui overlay`) as much as
without it, and it did so before T037. Measured with `--test-video` at 1080p60 (`--stats` now also
prints the longest gap between two presented frames and the window moves per second): capture
steady at 60 fps with nothing dropped, while presenting falls to 34–57 fps with gaps of 50–130 ms;
frame ready → presented median 2–23 ms (0.9 ms at rest). Holding the window still mid-drag
presents one frame per second (gaps of 1005 ms). Cause: on Windows, moving or resizing a window
runs the system's modal loop on the thread that owns it; the spike presents the video from that
thread, on winit user events, which the modal loop delivers late. Reading the monitor's refresh
rate on every move event was also slow and now happens only when the monitor changes; it was not
the cause. The fix to prove is presenting the video from its own thread (T052).

### Video thread (T052)

Research R15 as built: a `video` thread owns the GPU (`src/gpu.rs`: surface, layers, ambient
light, composite, present). The appsink puts each frame in a shared inbox and wakes it; the main
thread draws the UI with Slint and, when it changed, puts a copy of the image there with the scene
(backdrop, video area, ambient light) and the window size. The inbox keeps only the newest of
each; the video thread presents when a frame or a UI image arrives and prints the `[loop]` stats.
The surface is created on the main thread and moved; the thread stops before the window closes.
The UI cost in `--stats` is now the Slint draw plus that copy (≈ 0.4–1.0 ms median); the upload
moved to the video thread.

Measured by the maintainer with `--test-video` at 1080p60, Float (frameless) and overlay (system
frame) alike:

| | Before (T037) | After |
|---|---|---|
| At rest | 60 fps, median 0.9 ms | 60–61 fps, median 1.1 ms |
| Window moving | 34–57 fps, gaps 50–130 ms | 40–61 fps, gaps 30–115 ms; median 1–30 ms |
| Held still mid-drag | 1 fps, gaps 1005 ms | no stall |

The stall is gone; the maintainer sees a short hitch when a move starts, then smooth video.
Fast moves still drop to 40–50 fps with a 20–30 ms median, with or without the system frame.
The video thread no longer waits for the window's, so this is likely Windows composing the
moving window (DWM), not the app; not confirmed. UI animations pause during a move, as expected.
Capture never dropped a frame.

## Visual directions

The maintainer chose Float, laid out as the previous app, while reviewing the components (T028);
Frame and Rail were dropped (FR-006 revised).

Theme (T027): one cool neutral in four tone steps, orange accent, Inter, depth by tone and
nine-slice shadows; approved by the maintainer, then adjusted so every text pair reaches 4.5:1 in
both themes: dark ink on the orange instead of white, a deeper orange (`accent-text`) for orange
text in light, info bubbles black at 55% (30% left white text at 2.1:1 over a white frame), a
2px focus ring as the only outline. The same identity is kept outside the repository as a design
system artifact (tokens, rules, component previews) for other projects.

Components (T028), arranged as the previous app after the maintainer's review: a thin opaque
title bar with the wordmark and rounded window controls; a toolbar over the top of the video with
fixed-width dropdowns (video device, audio, mode; devices only, screens and windows belong to the
output's source); a left dock (fullscreen, ambient light, volume with a slider on hover) and a
right dock (preview, screenshot, record, stream, settings); capture info at the bottom right with a
pin; tooltips after 500 ms. Toolbar, docks and info hide 2 s after the pointer leaves and return
when it reaches the top of the video. Toggled tools sit on a soft orange chip. The ground around
the video takes the theme's deepest tone (the compositor reads it from the UI each frame). All
drawn by the software renderer over the video in both themes. Slint notes: an `Image` inside a layout asks for
the image's natural width even when only its height is set, so logos need both sides (in the
SVG's aspect ratio); icons are single-ink SVGs tinted with `colorize`.

Video area (T050): the UI exposes where the video goes (`video-area`: position, size, corner
radius) and the loop letterboxes the video inside it; the shader rounds the corners. The video
starts below the title bar and never sits behind it; in fullscreen the title bar hides and the
video takes the whole screen. Keys: F11 toggles fullscreen, Esc leaves it and never quits
(docs/product.md F8).

Review (T033, maintainer, 2026-10-29): the Float direction with live video from the ezcap (PS5,
1080p60) in both themes, windowed and fullscreen, chrome shown and hidden and each dropdown open
(video, audio, mode). The maintainer is satisfied and found nothing wrong or not working.
Screenshots: [measurements/screens/](measurements/screens/) (`<theme>-<windowed|fullscreen>-<state>.jpg`).
The latency in the info overlay is still a sample value.

## Decisions

Pending (T040).
