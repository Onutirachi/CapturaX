# Quickstart: Running the Stack Spike

How to set up, run and measure. Command details are in [contracts/cli.md](contracts/cli.md).

## Prerequisites

- Windows: Rust stable (MSVC), Visual Studio Build Tools, GStreamer 1.28.x
  `gstreamer-1.0-msvc-x86_64-<version>.exe` with the development components selected (installer
  type "development" or `/TYPE=devel`), `uv`, Edge, OBS.
- CachyOS: Rust stable, GStreamer with base, good and bad plugins plus the PipeWire plugin and
  NVIDIA codec support, `uv`.
- Hardware: ezcap Live Gamer Ultra, PS5 connected through the card's HDMI passthrough to the TV
  (game mode on), Galaxy S23 FE.
- On the PS5: HDCP off (Settings > System > HDMI) and an A/V sync test video (flash and beep once
  per second) open in the YouTube app, or played from a USB drive in the media gallery if the app
  refuses to play with HDCP off.

## 1. Check the setup

```sh
gst-inspect-1.0 --version          # 1.28.x
gst-device-monitor-1.0 Video/Source # the card and its modes
cd spike && cargo run --release -p native-preview -- --list
```

Expected: the card is listed with 1920x1080@60 and its highest frame-rate mode.

## 2. Video latency, per path and mode

For each path (native, slint-cpu, slint-gpu, browser, obs) and each mode (1080p60, highest fps):

1. Start the path fullscreen on the monitor next to the TV, in the mode under test.
2. Film the TV and the monitor together for 25 seconds at 240 fps, both screens fully in frame.
3. `uv run tools/analyze.py video <clip> --path <path> --variant <variant> --mode <mode> --platform windows`

Expected: at least 20 samples per run; no delay negative or above 500 ms.

Encoder load: repeat the slint-layered run with `--encoder` and compare (variant `encoder-on`).

## 3. Audio

1. `cargo run --release -p audio-preview -- --output <speakers>` with the PS5 sync video playing.
2. Record 25 seconds of normal video with sound, TV and PC speakers both audible.
3. `uv run tools/analyze.py audio <clip> --platform windows --output <speakers> --minute 0`
4. Leave it running; record again at minute 30 (`--minute 30`).

Expected: delay median reported; offset to the video median of the same session within
45 ms ahead / 125 ms behind; drift under 10 ms. Then listen with the headset while playing and note
whether any offset is noticeable.

## 4. Visual directions and capabilities

`cargo run --release -p layered-preview -- --device ezcap --backend dx12 --present mailbox --ui components --theme dark --stats`

Without the card: add `--test-video`.

Switch themes in the app; check every capability in spec User Story 4 and record
the outcomes and screenshots in `findings.md`.

## 5. Linux

Move the card to the CachyOS computer and repeat steps 1–2 for the fastest path and step 4 for the
chosen direction, recording the session type.

## 6. Summary

`uv run tools/analyze.py summary` then complete `findings.md` with the decisions.
