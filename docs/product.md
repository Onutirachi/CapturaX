# CapturaX — Product

What CapturaX does, feature by feature. Specs reference the IDs below ("covers F1–F4"), so
every feature can be traced to the spec that delivered it. The
[constitution](../.specify/memory/constitution.md) holds the principles; this document holds
the features. Change it by pull request when a decision changes, before changing a spec.

## Vision

A simple app to watch and hear a capture card with the lowest possible latency, good enough to
play a console through it, and to share what you are playing with friends by link. It also
captures screens and windows, takes screenshots and records. It is not a replacement for OBS.

**Main scenario:** a console is connected to a capture card. The user plays looking at the
CapturaX preview and listening through their headset, records the session, and sends a link so
friends can watch live in their browser.

## Out of scope

- More than one source in one output (scenes, compositing, picture-in-picture, webcam overlay).
  Recording and streaming are two outputs and may each use a different source.
- Streaming to third-party platforms (RTMP, Twitch, YouTube, Kick).
- Video editing or effects.

## Features

The **Spec** column is filled in when a spec covers the feature.

### A. Video sources

| ID | Feature | Spec |
|---|---|---|
| F1 | List capture cards and webcams; the list updates when a device is plugged in or removed. | |
| F2 | Open a device in a native mode (resolution × frame rate × format reported by the driver), with no software scaling. One **Mode** list shows only the modes the device really supports. A **color range** setting (Auto, Full, Limited) corrects devices that declare the wrong range; Auto infers it from the video levels. | |
| F3 | Automatic modes next to the manual ones: **Auto source** follows the signal the device is receiving (resolution and frame rate), shown only when the device reports it; **Auto max resolution**: the largest resolution at about 60 fps; **Auto max FPS**: the highest frame rate at 1080p (or the largest resolution below it). An automatic mode that resolves to the same mode as another is not shown. Default: Auto source when available, otherwise 1920 × 1080 at about 60 fps when the device has it, otherwise Auto max resolution: with a 60 Hz source, the common case, Auto max FPS at 120 was measured about 5 ms slower (maintainer, 2026-11-07; earlier default Auto max FPS). | |
| F4 | When a device disconnects, show it; when it comes back, reopen it automatically. A notice (in the app when focused, a system notification otherwise) says when a device is connected or disconnected, with **Open** for a newly connected one (maintainer, 2026-11-06). In the app it is a balloon of the info bubbles' family in a corner chosen in the settings (bottom left by default); the settings notices use the same balloon (maintainer, 2026-11-09). | |
| F5 | Screens and windows as a source for recording and streaming, with the cursor and without a capture border. On Windows the app lists them (screens named like "Screen 2 - 27G2G4"); on Linux Wayland the system picker chooses them. | |

The preview shows devices only. Screens and windows are sources of the outputs (recording and
streaming); **View recording** and **View stream** show that output's source in the preview.
With no device open, the preview shows a "no source" state; its action opens the app's source
picker, the same one the toolbar opens, not a picker of its own (maintainer, 2026-11-06).

### B. Preview

| ID | Feature | Spec |
|---|---|---|
| F6 | Preview the device with the lowest possible latency. | |
| F7 | Measured latency and frame rate, updated live. Its spec defines which frame rate is shown (what the source delivers, what reaches the app or what is presented) and whether dropped frames are shown. | |
| F8 | Frameless window with its own title bar; fullscreen with F11, or from the start with the **Start in fullscreen** setting (a command-line option overrides it for one start, for shortcuts). Esc closes an open menu first, then leaves fullscreen; it never quits the app. In small windows the main screen switches to a compact layout instead of overlapping its controls (the stack spike's Float needs 800 × 520 without it). | |
| F9 | Optional ambient light: the area around the video glows with the video's colors. It lights over the chosen video background (F56); the watermark (F57) shows clearer over it and the shadow (F58) sits on it. | |
| F56 | **Video background** setting: **Theme** (default, the theme's deepest tone) or **Black**, the ground around the video (bars of a capture that does not fill the window, between source switches, behind the states) (maintainer, 2026-11-07). | |
| F57 | **Watermark** setting, on by default: the CapturaX X in the video background, as tall as the video area, at its bottom right with a part cut off by the right edge; it shows between source switches, behind the states and in the bars. Soft over the background, clearer over the ambient light (F9). Off leaves the bars plain (maintainer, 2026-11-07). | |
| F58 | **Video shadow** setting: **Always** (default), **With ambient light** or **Off**: a soft shadow around the video, over the background and the ambient light (maintainer, 2026-11-07). | |
| F10 | Hide the preview to save resources; recording and streaming continue. Hiding it releases the device (capture stops, a webcam's light turns off) unless recording or streaming uses it: the old app kept capturing while hidden. The video area then shows a "preview hidden" state (F11's component, icon `preview_off`) with a **Show preview** action, and says that recording and streaming continue when they do. Whether hiding also stops the preview's audio (C) is open (see To reconsider). | |
| F11 | Explicit empty and error states with the next action: no devices, access blocked by the OS, device in use, device disconnected, no signal. Never an empty list. A black image while a signal is present gets no hint: it is not a reliable sign of HDCP (a console that is off, an RCA to HDMI adapter, a VHS tape being converted), so HDCP is explained in the getting started guide (F54) instead (maintainer, 2026-11-06). | |
| F42 | Capture info overlay (source, mode, measured FPS and latency, recording/live status) over the video, as bubbles: each can be pinned, moved in the order and anchored to any of the four corners; with the controls hidden only the pinned ones show, packed against their corner. | |
| F43 | Resolution tags in device and mode lists: the standard's name (for example HD, Full HD, WQHD, 4K UHD; the nearest standard above, else Custom) and the aspect ratio (for example 16:9). | |
| F52 | Presentation setting with two modes. **Lowest latency** (default): one present per video frame; on a variable refresh monitor (G-SYNC, FreeSync) the monitor follows the video, so frames show at once and evenly, but while the app has focus the whole desktop, cursor included, runs at the video's rate (60 Hz). **Follow the monitor**: present at every monitor refresh, repeating the last frame, so the cursor stays smooth; each new frame waits for the next refresh (up to 6.9 ms at 144 Hz) and the cadence is uneven (2 or 3 refreshes per frame at 60 fps on 144 Hz). Only matters with variable refresh; the setting says so. Found in the app shell's latency run (its research R4). | |

### C. Listening (preview audio)

| ID | Feature | Spec |
|---|---|---|
| F12 | Play an audio input on a chosen output with the lowest possible latency. The main use is hearing the console through the capture card while playing. | |
| F13 | By default the input is the audio of the same device as the video (paired automatically); the user can choose another. | |
| F14 | Volume and mute. This audio is only for the user and never goes into recordings or streams. | |
| F47 | Settings list every audio input and output with an **Exclusive mode** checkbox each (off by default). Exclusive skips the system mixer for the lowest delay, but no other app can use that device meanwhile; the list says so. Windows only (WASAPI); measured on the spike against the monitor's own audio: 59 ms shared, 46 ms with the input exclusive, 18 ms with both. | |

### D. Output audio (recording and streaming)

| ID | Feature | Spec |
|---|---|---|
| F15 | Each output (recording, streaming) has its own audio sources list under its source: **Add audio** picks any audio input of the computer or **PC sound**; up to 8 sources; volume per source, adjustable live. The audio of the source device is added automatically and can be removed. | |
| F16 | **PC sound** captures apps individually and leaves out excluded apps (default: Discord). CapturaX itself is always excluded. Each output has its own app selection and exclusions. The app picker lists apps playing now, open, and seen before, and accepts an executable name typed by hand. Changes apply immediately, even while recording. | |
| F17 | Track 1 is always the mix of all sources. A **Separate track** checkbox per source adds that source as its own track in recordings. The checkboxes are hidden when only streaming. | |

### E. Screenshot

| ID | Feature | Spec |
|---|---|---|
| F19 | **Ctrl+F8** (F50) saves a PNG at the native resolution to the Pictures/CapturaX folder and copies it to the clipboard. | |

### F. Recording

| ID | Feature | Spec |
|---|---|---|
| F20 | **Ctrl+F10** (F50) starts and stops recording to MP4: H.264 or HEVC on the GPU (AV1 where supported) with AAC audio, in a configurable folder (default Videos/CapturaX). | |
| F21 | Quality presets — Original, High (up to 1080p), Medium (up to 720p, up to 60 fps), Light (720p30), Custom — with an estimate of size per hour. | |
| F22 | Crash-safe: if the computer crashes, the file is valid up to the last second. | |
| F23 | Correct colors from capture cards that send full-range video (no dark or crushed image). | |
| F24 | Recording time and file size shown while recording. | |

### G. Record and Stream panels

| ID | Feature | Spec |
|---|---|---|
| F25 | Recording and streaming are set up in two separate panels inside the main window, **Record** and **Stream** (like the settings panel), each with its own source and audio. Nothing ties them together: to record and stream the same thing, the user picks the same source in both. When the main window is not visible (closed to the tray, minimized, or the app started with `--live` or `--record`), the panel opens on its own as a compact window on the desktop instead: the same panel, at the same size and with the same look as inside the main window, without the preview and without opening the preview device. Both can be open at the same time, in either place. The compact windows come after the panels work inside the main window (roadmap #14). | |
| F26 | Each output's source is any device, screen or window (default: the preview device). Recording and streaming can run at the same time with different sources, different apps and different audio, or with the same ones; a device or screen used by both is captured once. | |
| F27 | While recording, the Record panel's options are locked until recording stops (volumes still change). The Stream panel's options can change during a live, and **Apply** restarts the stream; viewers reconnect by themselves. | |

The compact windows replace the "record / stream only mode" considered on 2026-11-04 (a
**Start without preview** setting): many people who lost Discord's screen sharing in Brazil have
no capture card and only stream a screen or a window. The main scenario and the roadmap's order
do not change (maintainer, 2026-11-06).

### H. Live by direct link (the PC serves the stream)

| ID | Feature | Spec |
|---|---|---|
| F28 | A fixed link. While not live the page shows "Offline" and starts playing by itself when the stream begins. | |
| F29 | Quality first; low delay is welcome but not a priority. Works on Safari/iPhone too. | |
| F30 | Automatic network setup: UPnP port mapping, CGNAT detection, and the links that work (internet IPv4, internet IPv6, local network). | |
| F31 | Stream quality set separately from recording: resolution, frame rate, automatic or manual bitrate, and the upload each viewer uses. | |
| F32 | Viewer list (address, watching for how long), block an address, viewer limit (unlimited by default). | |
| F33 | Hardened server: GET only on fixed routes, no file access from URLs, no route that controls the app, connection and request limits. | |
| F53 | Optional live password, off by default, chosen in the Stream panel. When set, the viewer page asks for it before playing; the media is not sent before it is accepted. Applies to the direct link and to the server live (F34). Watching without a password stays the default. | |

### I. Live through a server (peer-to-peer)

| ID | Feature | Spec |
|---|---|---|
| F34 | Peer-to-peer streaming (WebRTC): media goes from the user's PC straight to each viewer. A self-hosted server introduces both sides and serves the viewer page. When a direct connection fails (for example CGNAT on both sides), the server's TURN relay carries that viewer's media. | |
| F35 | The server is a container image with a compose file, keeping all data in one volume, documented so anyone can host their own. | |
| F36 | Server passwords: **admin** (the owner; can change the usage password from the app) and **usage** (for friends who may stream through the server; changing it disconnects whoever used the old one). Watching needs no password. | |
| F37 | Link names: a nickname (`/carlos`) or a generated code, one per app key at a time; a name unused for 7 days is released. The key is stored in the settings, so restoring a backup keeps the name. | |

### J. Viewer page

| ID | Feature | Spec |
|---|---|---|
| F38 | Plays in any browser, with volume and mute, fullscreen (double click), ambient light and "click for sound". Uses the viewer's browser language. The page has the app's look around the video: the video background (F56), the watermark (F57), the shadow (F58) and the theme; whether the viewer or the streamer chooses them is decided in its spec (maintainer, 2026-11-07). | |

### K. General

| ID | Feature | Spec |
|---|---|---|
| F39 | English and Portuguese (Brazil), automatic from the system language or chosen in the settings, switching without restart. | |
| F46 | Dark and light themes, chosen in the settings. | |
| F40 | Persistent settings. Categories are defined in their own spec. | |
| F41 | Portable: settings, keys and state next to the executable; if that folder is not writable, warn and use the per-user configuration folder. The installer copies files and configures the firewall; the app has a **Configure firewall** action for restored backups. GStreamer ships with the app. | |
| F44 | Updates: check GitHub Releases, notify about a new version, install it automatically when the user agrees. The check can be turned off. | |
| F45 | Windows and Linux (AppImage on Linux). | |
| F48 | Optional tray icon: with **Keep in tray when closed** on, closing the window keeps the app in the tray (Open, Quit). Hidden where the desktop has no tray. Later specs add **Live**, **Record** and **Settings** to its menu: each opens its panel (F25, settings) inside the main window when it is visible, else as a compact window of its own, identical to the panel. | |
| F49 | **Start with the system** (off by default), on Windows and Linux. With the tray option on, it starts straight to the tray, with no window. A moved portable folder keeps starting. | |
| F50 | Hotkeys that work both in the window and while another app (for example a game) is in focus: **Ctrl+F8** screenshot; **Ctrl+F9** opens the Stream panel (where F25 says) when not streaming, and stops the live when streaming; **Ctrl+F10** does the same for recording with the Record panel. Keys can be changed in the settings. There are no separate in-window keys for these actions. A key already taken by another program is not detected or handled; the user changes it if needed. On Linux under Wayland they go through the desktop's global shortcuts portal where it exists. | |
| F54 | Getting started guide: a short illustrated guide (screenshots of the app) that explains what is not obvious from the interface: the info bubbles and how to move them, the color range setting, the difference between the mode chosen and the resolution of the signal the device receives, and HDCP (a console with HDCP on shows a black image; how to turn it off). Reopened from the settings at any time. When it is shown first is decided in its spec (maintainer, 2026-11-06). Same spec: tooltips on the more complex settings (for example Accelerate GPU, color range, presentation), a few words of detail each (maintainer, 2026-11-07). | |
| F55 | **Accelerate GPU** setting: **Auto** (default) asks the GPU for full clocks whenever the app needs them except on battery, **Off** never asks, **On** asks whenever the app needs them, on battery too. Today the only such request is the preview's (device preview research R7: NVIDIA PowerMizer on Linux, about +17 W on an RTX 3050). Its spec also measures whether waking the GPU helps a live with the preview hidden (frames dropped, encoding time, GPU state, at 1080p60 and 1080p120, with and without the request) and adds the request to the live only if it helps; on Windows it decides what the setting does, since the preview needs no request there (maintainer, 2026-11-07: after a usable app with a working live). | |
| F51 | Recording and streaming status outside the window: the tray icon (F48) changes with what the app is doing (idle, recording, streaming, both), and the taskbar button shows a small status badge (on Windows a taskbar overlay icon; on Linux where the desktop supports it). | |

## Removed

| ID | Feature | Reason |
|---|---|---|
| part of F14 | Manual audio delay for lip-sync | Not needed in practice: recordings and streams keep audio and video on one clock, and the stack spike measured the preview's audio within the lip-sync limits of the video (12 ms ahead to 29 ms behind). Reconsider only if measurements show a need. |
| F18 | Manual audio/video sync offset | Same reason as above. |
| — | Peer-to-peer without a relay | Viewers behind restrictive NAT could not connect; replaced by F34 with a TURN fallback. |

## To reconsider

- **Hide preview and its audio** (maintainer, 2026-11-06): hiding the preview (F10) releases the
  device, but whether it also stops listening (C) is not decided. Decide in the spec that adds
  F10.

## Future ideas

Not planned; listed so they are not lost.

- Replay buffer: save the last N seconds, N set by the user. Maybe also for viewers in the
  viewer page, with their own N and their own resources. Both can be turned off. Check
  feasibility, browser side above all.
- Play a sound when a viewer joins the live (specs #10 and #12).
- End a live after a long inactivity, so it cannot be left on by mistake (the old app streamed
  for 2 days unnoticed). What counts as inactive is open: no viewers for a while, no input on the
  PC, or both (specs #10 and #12).
- For Voicemeeter users, suggest letting Voicemeeter read the capture card directly: the lowest
  possible audio delay (measured ≈ 0, against ≈ 100 ms through its virtual input).
- Server mode where the server distributes the stream (upload once, any number of viewers).
- macOS.
- Contributor License Agreement, when the first external contributor appears.
