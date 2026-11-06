# CapturaX — Roadmap

The order in which specs are written and built. Each entry lists the features it covers (IDs
from [product.md](product.md)), what it depends on, and the command that starts it in a new
conversation. Update the status and the spec link when a spec starts and when it is merged.

Status: **next**, **in progress**, **done**, or blank (not started).

| # | Spec | Covers | Depends on | Status |
|---|---|---|---|---|
| 1 | [Stack spike](#1-stack-spike) ([spec](../specs/20261026-154808-stack-spike/spec.md)) | — | — | done |
| 2 | [App shell](#2-app-shell) ([spec](../specs/20261104-112852-app-shell/spec.md)) | F8, F11 (framework), F39, F40, F41 (settings storage), F46, F48, F49 | 1 | done |
| 3 | [Device preview](#3-device-preview) ([spec](../specs/20261106-170507-device-preview/spec.md)) | F1–F4, F6, F7, F11, F43, F52 | 2 | in progress |
| 4 | [Preview audio](#4-preview-audio) | F12–F14, F47 | 3 | |
| 5 | [Screenshot](#5-screenshot) | F19 | 3 | |
| 6 | [Preview extras](#6-preview-extras) | F9, F10, F42 | 3 | |
| 7 | [Screen and window capture](#7-screen-and-window-capture) | F5 | 3 | |
| 8 | [Output audio](#8-output-audio) | F15–F17 | 4 | |
| 9 | [Recording](#9-recording) | F20–F27, F48 (Record in the tray), F50 (framework, record, screenshot), F51 (recording) | 7, 8 | |
| 10 | [Direct link](#10-direct-link) | F28–F33, F38, F48 (Live in the tray), F50 (stream), F51 (streaming), F53 | 9 | |
| 11 | [Server](#11-server) | F34–F37, F53 (server side) | 10 | |
| 12 | [Peer-to-peer live](#12-peer-to-peer-live) | F34, F36, F37, F53 (app side) | 11 | |
| 13 | [Packaging and updates](#13-packaging-and-updates) | F41 (installer, firewall), F44, F45 | 2 | |
| 14 | [Compact windows](#14-compact-windows) | F25 (compact windows), F48 (Settings in the tray) | 10 | |

Linux is part of every spec, not a final step. Specs 5, 6 and 7 can be done in any order;
spec 13 can start any time after spec 2.

## Specs

### 1. Stack spike

Validates Rust + GStreamer + Slint before any feature work. The findings go into the plan as
decisions; the spike code is not merged.

Done: [findings](../specs/20261026-154808-stack-spike/findings.md). Slint confirmed with the
app's own render loop; the "Open questions for later specs" section there lists what each later
spec inherits. The spike code is kept under the tag `spike/stack-20261026`.

```text
/speckit-specify Stack spike, before any feature work (see docs/roadmap.md #1). Two goals:
(1) Preview latency: open a capture card in its native mode and measure glass-to-glass latency
on three paths: GStreamer to a native window (D3D11/D3D12 on Windows, Vulkan or GL on Linux),
GStreamer to a texture inside a Slint UI, and a browser getUserMedia preview as reference. Also
play the card's audio on a chosen output with the lowest latency and measure it.
(2) Slint UI: build 2-3 visual directions of the main screen (video taking most of the window,
minimal chrome, capture info overlay, source picker, dock) to compare look and feel.
Constraints: no outlines, very little text; Ninjoy prints in old/_refs/ninjoy-2026-10-23 are
inspiration only; brand orange #FF7304 and logo from old/capturax-react/brand. Windows first,
Linux checked too. Old implementation for reference: old/capturax-react.
```

### 2. App shell

Done: [handoff](../specs/20261104-112852-app-shell/handoff.md),
[latency](../specs/20261104-112852-app-shell/measurements/latency.md) and
[acceptance](../specs/20261104-112852-app-shell/measurements/acceptance.md). Carried to spec #3:
keeping the GPU awake on Linux (SC-001 not met there at idle clocks), the Windows stalls at
presentation mode changes and language switches. Leaving fullscreen back to a snapped place
on Windows goes to the next spec that touches the window.

```text
/speckit-specify App shell (see docs/roadmap.md #2): the Rust + Slint application skeleton
chosen by the stack spike: own winit + wgpu render loop with the video presented from its own
thread and Slint drawn as a layer over it (stack spike findings, research R13 and R15), the Float
direction's frameless window and chrome. Covers F8, F39, F40, F46, F48, F49, the settings storage part of F41, and a common
way to show empty and error states (F11). Includes the theme tokens from the spike's chosen
visual direction, i18n infrastructure (English and Portuguese), the portable settings file with
the per-user fallback, and CI (format, lint, tests, build on Windows and Linux).
```

### 3. Device preview

```text
/speckit-specify Device preview (see docs/roadmap.md #3): covers F1-F4, F6, F7, F11, F43 and F52
from docs/product.md. Device list with hot plug, native modes, the automatic modes (source, max resolution, max FPS), reconnect,
latency and FPS measurement, explicit empty and error states (including a hint to turn off
HDCP on the console when the image is black while a signal is present). Must state a latency budget and
how it is measured (constitution principle I); the stack spike measured 30.4 ms median at 1080p60
on Windows with Direct3D 12 + Mailbox, and lists the open questions this spec inherits. Also
investigate the Windows stalls at presentation mode changes the app shell measured (focus change
229-401 ms, entering fullscreen 51-73 ms; app shell research R4).
```

### 4. Preview audio

```text
/speckit-specify Preview audio (see docs/roadmap.md #4): covers F12-F14 from docs/product.md.
Lowest-latency playback of an audio input on a chosen output, paired automatically with the
video device, with volume and mute, and F47 (exclusive mode per device). On Windows through WASAPI
directly, as the stack spike's engine; it must measure the 30-minute drift the spike left open,
and the Linux path. Must state a latency budget and how it is measured.
```

### 5. Screenshot

```text
/speckit-specify Screenshot (see docs/roadmap.md #5): covers F19 from docs/product.md.
```

### 6. Preview extras

```text
/speckit-specify Preview extras (see docs/roadmap.md #6): covers F9, F10 and F42 from
docs/product.md: ambient light, hiding the preview, and the capture info overlay (bubbles that
can be pinned, reordered and anchored to any corner; the detached window was dropped in the
stack spike).
```

### 7. Screen and window capture

```text
/speckit-specify Screen and window capture (see docs/roadmap.md #7): covers F5 from
docs/product.md, on Windows (own list) and Linux (Wayland portal picker, X11 if supported).
```

### 8. Output audio

```text
/speckit-specify Output audio (see docs/roadmap.md #8): covers F15-F17 from docs/product.md.
Audio sources list per output (up to 8), PC sound captured per app with exclusions (Windows process
loopback, Linux PipeWire), mix on track 1 and optional separate tracks.
```

### 9. Recording

```text
/speckit-specify Recording (see docs/roadmap.md #9): covers F20-F27 from docs/product.md,
including the Record panel (the Stream panel is added by spec 10), the global
hotkeys of F50 for recording and screenshot (the streaming hotkey is added by spec 10), and the
recording status of F51 on the tray icon and the taskbar button. The tray's Record entry and
Ctrl+F10 open the Record panel in the main window (the compact window comes in spec 14).
```

### 10. Direct link

```text
/speckit-specify Direct link live (see docs/roadmap.md #10): covers F28-F33 and the viewer page
F38 from docs/product.md, the Stream panel (F25, F27), the F50 global hotkey (opens the Stream
panel, stops a live), the tray's Live entry (F48), the streaming status of F51 and the optional
live password (F53). The PC serves the stream; automatic network setup; hardened server.
```

### 11. Server

```text
/speckit-specify Self-hosted server (see docs/roadmap.md #11): covers the server side of
F34-F37 and F53 from docs/product.md: signaling, viewer page, TURN fallback, passwords (server
and optional live password) and link names,
as a container image with a compose file and one data volume. Replaces the old relay in
old/capturax-react/relay. Deployed by the maintainer on an ARM (Oracle Ampere) VPS that hosts
other projects behind a shared reverse proxy.
```

### 12. Peer-to-peer live

```text
/speckit-specify Peer-to-peer live (see docs/roadmap.md #12): covers the app side of F34, F36
and F37 from docs/product.md: streaming through the self-hosted server with WebRTC, server
settings and password status in the app, link names, and the optional live password (F53).
```

### 13. Packaging and updates

```text
/speckit-specify Packaging and updates (see docs/roadmap.md #13): covers the installer and
firewall parts of F41, F44 and F45 from docs/product.md: Windows installer, Linux AppImage,
bundled GStreamer, Configure firewall action, update check with automatic install.
```

### 14. Compact windows

```text
/speckit-specify Compact windows (see docs/roadmap.md #14): covers the compact windows of F25 and
the Settings entry of F48 from docs/product.md. When the main window is not visible, the tray's
Live, Record and Settings entries and the Ctrl+F9 / Ctrl+F10 hotkeys open that panel as a compact
window of its own on the desktop (the same panel, same size and look, without the preview and
without opening the preview device); `--live` and `--record` start the app that way (a running
instance opens them). Live and Record can be open at the same time. Reuses the panels built in
specs 9 and 10.
```
