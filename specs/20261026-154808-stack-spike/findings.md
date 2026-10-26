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
| slint-gpu | not built yet (T014) | | |

The maintainer reports consistent values across several clips per path. These results are
**preliminary** until the analysis script (T016–T018) produces at least 20 samples per path and
mode with median and 95th percentile (FR-002).

### Reading against the success criteria (preliminary)

- **SC-002** (Slint within one frame of native): **fails** for the CPU-copy variant: ≈ 25 ms more
  than native, 1.5 frames at 60 fps.
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

## Observations

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
