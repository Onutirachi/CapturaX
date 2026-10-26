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

- More than one source at a time (scenes, compositing, picture-in-picture, webcam overlay).
- Streaming to third-party platforms (RTMP, Twitch, YouTube, Kick).
- Video editing or effects.

## Features

The **Spec** column is filled in when a spec covers the feature.

### A. Video sources

| ID | Feature | Spec |
|---|---|---|
| F1 | List capture cards and webcams; the list updates when a device is plugged in or removed. | |
| F2 | Open a device in a native mode (resolution × frame rate × format reported by the driver), with no software scaling. One **Mode** list shows only the modes the device really supports. | |
| F3 | **Auto** mode (default): the largest resolution that keeps about 60 fps, at its highest frame rate. **Smoothest** mode: the highest frame rate from 720p up, shown only when it differs from Auto. | |
| F4 | When a device disconnects, show it; when it comes back, reopen it automatically. | |
| F5 | Screens and windows as a source for recording and streaming, with the cursor and without a capture border. On Windows the app lists them (screens named like "Screen 2 - 27G2G4"); on Linux Wayland the system picker chooses them. | |

The preview shows devices only. Screens and windows are sources of the output (recording and
streaming), and a **View output** action shows the output in the preview.

### B. Preview

| ID | Feature | Spec |
|---|---|---|
| F6 | Preview the device with the lowest possible latency. | |
| F7 | Measured latency and frame rate, updated live. | |
| F8 | Frameless window with its own title bar; fullscreen with F11, leave with Esc. | |
| F9 | Optional ambient light: the area around the video glows with the video's colors. | |
| F10 | Hide the preview to save resources; recording and streaming continue. | |
| F11 | Explicit empty and error states with the next action: no devices, access blocked by the OS, device in use, device disconnected. Never an empty list. | |
| F42 | Capture info overlay (source, mode, measured FPS and latency, recording/live status) over the video, pinnable; can be detached as a floating window that stays on top. | |
| F43 | Resolution badges (for example HD, Full HD, QHD, 4K) in device and mode lists. | |

### C. Listening (preview audio)

| ID | Feature | Spec |
|---|---|---|
| F12 | Play an audio input on a chosen output with the lowest possible latency. The main use is hearing the console through the capture card while playing. | |
| F13 | By default the input is the audio of the same device as the video (paired automatically); the user can choose another. | |
| F14 | Volume and mute. This audio is only for the user and never goes into recordings or streams. | |

### D. Output audio (recording and streaming)

| ID | Feature | Spec |
|---|---|---|
| F15 | Audio sources list under the output source: **Add audio** picks any audio input of the computer or **PC sound**; up to 8 sources; volume per source, adjustable live. The audio of the source device is added automatically and can be removed. | |
| F16 | **PC sound** captures apps individually and leaves out excluded apps (default: Discord). CapturaX itself is always excluded. The app picker lists apps playing now, open, and seen before, and accepts an executable name typed by hand. Changes apply immediately, even while recording. | |
| F17 | Track 1 is always the mix of all sources. A **Separate track** checkbox per source adds that source as its own track in recordings. The checkboxes are hidden when only streaming. | |

### E. Screenshot

| ID | Feature | Spec |
|---|---|---|
| F19 | F9 saves a PNG at the native resolution to the Pictures/CapturaX folder and copies it to the clipboard. | |

### F. Recording

| ID | Feature | Spec |
|---|---|---|
| F20 | F10 starts and stops recording to MP4: H.264 or HEVC on the GPU (AV1 where supported) with AAC audio, in a configurable folder (default Videos/CapturaX). | |
| F21 | Quality presets — Original, High (up to 1080p), Medium (up to 720p, up to 60 fps), Light (720p30), Custom — with an estimate of size per hour. | |
| F22 | Crash-safe: if the computer crashes, the file is valid up to the last second. | |
| F23 | Correct colors from capture cards that send full-range video (no dark or crushed image). | |
| F24 | Recording time and file size shown while recording. | |

### G. Record / Stream window

| ID | Feature | Spec |
|---|---|---|
| F25 | One window to set up the output: source and audio (shared by recording and streaming), then a Recording column and a Streaming column, each shown when its switch is on. | |
| F26 | The source is any device, screen or window (default: the preview device). Recording and streaming always use the same source. | |
| F27 | While recording, the source and the audio devices are locked (volumes still change). While only streaming they can change, and **Apply** restarts the stream; viewers reconnect by themselves. | |

### H. Live by direct link (the PC serves the stream)

| ID | Feature | Spec |
|---|---|---|
| F28 | A fixed link. While not live the page shows "Offline" and starts playing by itself when the stream begins. | |
| F29 | Quality first; low delay is welcome but not a priority. Works on Safari/iPhone too. | |
| F30 | Automatic network setup: UPnP port mapping, CGNAT detection, and the links that work (internet IPv4, internet IPv6, local network). | |
| F31 | Stream quality set separately from recording: resolution, frame rate, automatic or manual bitrate, and the upload each viewer uses. | |
| F32 | Viewer list (address, watching for how long), block an address, viewer limit (unlimited by default). | |
| F33 | Hardened server: GET only on fixed routes, no file access from URLs, no route that controls the app, connection and request limits. | |

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
| F38 | Plays in any browser, with volume and mute, fullscreen (double click), ambient light and "click for sound". Uses the viewer's browser language. | |

### K. General

| ID | Feature | Spec |
|---|---|---|
| F39 | English and Portuguese (Brazil), automatic from the system language or chosen in the settings, switching without restart. | |
| F40 | Persistent settings. Categories are defined in their own spec. | |
| F41 | Portable: settings, keys and state next to the executable; if that folder is not writable, warn and use the per-user configuration folder. The installer copies files and configures the firewall; the app has a **Configure firewall** action for restored backups. GStreamer ships with the app. | |
| F44 | Updates: check GitHub Releases, notify about a new version, install it automatically when the user agrees. The check can be turned off. | |
| F45 | Windows and Linux (AppImage on Linux). | |

## Removed

| ID | Feature | Reason |
|---|---|---|
| part of F14 | Manual audio delay for lip-sync | Not needed in practice; the new pipeline keeps audio and video on one clock. Reconsider only if measurements show a need. |
| F18 | Manual audio/video sync offset | Same reason as above. |
| — | Peer-to-peer without a relay | Viewers behind restrictive NAT could not connect; replaced by F34 with a TURN fallback. |

## Future ideas

Not planned; listed so they are not lost.

- Replay buffer: save the last N seconds.
- Server mode where the server distributes the stream (upload once, any number of viewers).
- macOS.
- Contributor License Agreement, when the first external contributor appears.
