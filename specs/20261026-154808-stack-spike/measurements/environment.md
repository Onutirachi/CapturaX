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
| OBS | installed (version to fill) |
| Camera | Galaxy S23 FE, 240 fps slow motion |
| Source | PlayStation 5 |

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

_To fill during T021: distribution version, kernel, session type (Wayland or X11), NVIDIA driver,
GStreamer version, monitor._
