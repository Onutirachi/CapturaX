# CapturaX

Watch and hear your capture card with the lowest possible latency, good enough to play a
console through it. Capture screens and windows, take screenshots, record, and share a live
stream with friends by link. A simple alternative for these tasks, not a replacement for OBS.

> **Status:** early rewrite. The app shows a capture card or webcam in its native mode (video
> only, no audio yet), with settings, tray and device notices. The previous version is in
> [CapturaX-old](https://github.com/Onutirachi/CapturaX-old).

## Planned

- Low-latency audio from the capture card
- Screens and windows as a source
- Screenshots and crash-safe recordings with hardware encoding
- Live streaming by direct link, or peer-to-peer through a self-hosted server
- Windows and Linux, portable (settings stay in the program folder)

## Building

Prerequisites:

- Rust: `rust-toolchain.toml` pins the version; rustup installs it on the first build.
- GStreamer 1.28 with its development files (needed to build and to start):
  - Windows: the MSVC x86_64 installer, complete (devices come through its Media Foundation
    plugin), with the development components. `.cargo/config.toml` points `PKG_CONFIG_PATH`
    at its `lib\pkgconfig` folder; add `C:\Program Files\gstreamer\1.0\msvc_x86_64\bin` to
    `PATH` to run.
  - Linux: Arch / CachyOS `gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad`;
    Ubuntu `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-good
    gstreamer1.0-plugins-bad libxkbcommon-dev libwayland-dev libdbus-1-dev
    libfontconfig1-dev`. Devices open through V4L2 (plugins-good); MJPG modes also need
    `jpegparse` (plugins-bad), and NVIDIA's `nvjpegdec` is used when present.

Build and run:

```sh
cargo run -p capturax                                      # debug build
cargo build --release                                      # target/release/capturax
cargo run -p capturax --release -- --test-video 1080p60 --stats
```

At start the app opens the device you last used, or else the first capture card, or else the
first webcam, in its native mode. Device, mode and color range are picked in the toolbar at
the top of the video, and each device remembers its mode. Settings are kept in
`capturax.toml` and the log in `capturax.log`, both next to the executable.

Developer options (`--help` lists every option):

| Option | Effect |
|---|---|
| `--list-devices` | Prints every device (name, id) with its modes and the automatic modes it shows, then exits |
| `--test-video <PRESET>` | Plays a moving test pattern (`1080p60`, `1440p60` or `WxH@FPS`) instead of a device |
| `--stats` | Prints presentation statistics once per second (also to `capturax.log`) |
| `--present-log <FILE>` | Writes one line per acquire, present and window event, to match against PresentMon |
| `--backend`, `--present` | Force the graphics backend (`dx12`, `vulkan`, `gl`) or the present mode |

### GPU performance request (Linux, NVIDIA)

At its default clocks an NVIDIA GPU on Linux adds several milliseconds to every frame. While
the preview shows video, CapturaX asks the driver to prefer maximum performance (PowerMizer,
through `libnvidia-ml.so.1`, NVIDIA driver 590 or newer) and withdraws the request when the
video stops or the app quits. The driver keeps one request per program, so this does not
change a mode set elsewhere. Without the NVIDIA driver nothing is requested; AMD and Intel
GPUs are not covered.

To see it, look for the `gpu performance:` lines in `capturax.log` ("PowerMizer prefers
maximum performance on ..." when video starts, "PowerMizer back to ..." when it stops) and
watch the GPU stay in `P0` while the preview plays:

```sh
nvidia-smi --query-gpu=pstate,clocks.gr,clocks.mem,power.draw --format=csv -lms 500
```

`nvidia-settings` shows only its own request, not CapturaX's.

Checks (as CI runs them): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `cargo deny check licenses bans`.

## Contributing

Development is spec-driven: every feature starts as a spec in `specs/`. Read the
[constitution](.specify/memory/constitution.md) and the [agent guide](AGENTS.md) first.

## License

[GPL-3.0](LICENSE)
