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

## Observations

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

Not started (T034–T039).

## Visual directions

Not started (T027–T033).

## Decisions

Pending (T040).
