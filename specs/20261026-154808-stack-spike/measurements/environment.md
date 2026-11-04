# Environment

## Windows

| Field | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26300 (build 26300) |
| GPUs | NVIDIA GeForce RTX 5070 Ti, driver 617.14; AMD Radeon integrated graphics, driver 32.0.21042.62 |
| Monitors connected | 27G2G4 (main, 144 Hz, DP, RTX); C24FG7x (144 Hz, DP, RTX); S2-TEK TV projector (60 Hz, HDMI, RTX, usually disabled); Philips 221EL (60 Hz, DP→DVI, RTX); ARZOPA portable (144 Hz, integrated AMD GPU) |
| Monitor used for the preview | C24FG7x, 1920x1080 @ 144 Hz, DisplayPort on the RTX 5070 Ti |
| Reference display (passthrough) | 27G2G4 on its HDMI input, fed by the card's passthrough; next to the C24FG7x |
| GStreamer | 1.28.8, MSVC x86_64 (`C:\Program Files\gstreamer\1.0\msvc_x86_64`) |
| Rust | 1.99.0, stable-x86_64-pc-windows-msvc |
| OBS | OBS Studio 32.2.2 (read on 2026-11-04, after the clips) |
| Browser | Microsoft Edge 156.0.4314.58 (read on 2026-11-04, after the clips) |
| Camera | Galaxy S23 FE, 240 fps slow motion |
| Source | PlayStation 5, video output locked to 1080p (the 27G2G4 shows no passthrough above 1080p), HDR off, HDCP off |

### Capture card

`ezcap Live GAMER ULTRA` — internal PCIe card that presents itself as a USB video device
(UVC, `vid_32ed&pid_3201`). Seen by `mfvideosrc` (Media Foundation) and `ksvideosrc`.

Video modes reported:

| Format | Modes |
|---|---|
| NV12 | 1920x1080 @ 60–120; 3840x2160 @ 30; 2560x1440 @ 60; 1280x720 @ 60 |
| YUY2 | 1920x1080 @ 60; 2560x1440 @ 50; 1280x720 @ 60 |
| P010 (10-bit) | 1920x1080 @ 60; 1280x720 @ 60 |
| BGRx | 1920x1080 @ 30; 1280x720 @ 60 |

- Highest frame-rate mode: **1920x1080 @ 120 NV12**.
- Auto (largest resolution with ≥ 59 fps): **2560x1440 @ 60 NV12**.
- All modes are uncompressed, so no MJPEG decoding is needed (research R6).

Audio input: `HDMI (ezcap Live GAMER ULTRA)`, stereo, 48 kHz (F32LE, S32LE, S24LE, S16LE).

### GStreamer elements checked

`mfvideosrc`, `d3d12videosink`, `d3d11videosink`, `wasapi2src`, `nvh264enc`, `nvd3d11h264enc`:
all present.

## Linux

| Field | Value |
|---|---|
| OS | CachyOS (rolling), kernel 7.3.1-1-cachyos (updated from 7.2.11 just before T021) |
| Session | KDE Plasma 6.8.2, **Wayland** (KWin 6.8.2) |
| CPU | Intel Core i3-10105 |
| GPU | NVIDIA GeForce RTX 3050 (GA106), proprietary driver 615.78.08 |
| Monitor used for the preview | the only output, HDMI-A-1, 1920x1080 @ 120 Hz, no VRR (C24FG7x, the same monitor as on Windows, on the RTX 3050) |
| Reference display (passthrough) | 27G2G4 on its HDMI input, fed by the card's passthrough, as on Windows |
| GStreamer | 1.28.8 (Arch packages: base, good, bad, ugly, libav), PipeWire plugin 1.6.9 |
| PipeWire | 1.6.10 |
| Rust | 1.99.1, stable-x86_64-unknown-linux-gnu |
| Camera | Galaxy S23 FE, 240 fps slow motion, hand-held position different from the Windows clips (no tripod) |

### Capture card

The same card, seen by `v4l2src` as `ezcap Live GAMER ULTRA: ezcap L` (`/dev/video0`). The
PipeWire device provider hides the V4L2 one in GStreamer's device monitor and lists the card as a
PipeWire camera (`pipewiresrc`), whose `device.api` is also `v4l2`; the spike asks for every
provider and takes the V4L2 device (findings.md, "Linux").

Video modes reported by `v4l2src`: the same NV12, YUY2 and BGRx modes as on Windows (no P010), each
also offered as DMA-BUF (`DMA_DRM`). Auto picks 2560x1440 @ 60 NV12, as on Windows.

Audio input (PipeWire): `ezcap Live GAMER ULTRA Estéreo analógico`, S16LE, stereo, 48 kHz.
Default output: the motherboard's S/PDIF (`iec958-stereo`).

### GStreamer elements checked

`v4l2src` (needed `gst-plugins-good`, missing at first), `glimagesink`, `waylandsink`, `vulkansink`,
`pipewiresrc`, `pipewiresink`, `nvh264enc` and the other NVENC/NVDEC elements: all present.
