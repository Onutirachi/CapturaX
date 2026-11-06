# CapturaX

Watch and hear your capture card with the lowest possible latency, good enough to play a
console through it. Capture screens and windows, take screenshots, record, and share a live
stream with friends by link. A simple alternative for these tasks, not a replacement for OBS.

> **Status:** early rewrite. The application shell works (window, settings, tray, test video);
> no capture source yet. The previous version is in
> [CapturaX-old](https://github.com/Onutirachi/CapturaX-old).

## Planned

- Capture cards opened in their native mode, with low-latency audio
- Screens and windows as a source
- Screenshots and crash-safe recordings with hardware encoding
- Live streaming by direct link, or peer-to-peer through a self-hosted server
- Windows and Linux, portable (settings stay in the program folder)

## Building

Prerequisites:

- Rust: `rust-toolchain.toml` pins the version; rustup installs it on the first build.
- GStreamer 1.28 with its development files (needed to build and to start):
  - Windows: the MSVC x86_64 installer with the development components.
    `.cargo/config.toml` points `PKG_CONFIG_PATH` at its `lib\pkgconfig` folder; add
    `C:\Program Files\gstreamer\1.0\msvc_x86_64\bin` to `PATH` to run.
  - Linux: Arch / CachyOS `gstreamer gst-plugins-base`; Ubuntu `libgstreamer1.0-dev
    libgstreamer-plugins-base1.0-dev libxkbcommon-dev libwayland-dev libdbus-1-dev
    libfontconfig1-dev`.

Build and run:

```sh
cargo run -p capturax                                      # debug build
cargo build --release                                      # target/release/capturax
cargo run -p capturax --release -- --test-video 1080p60 --stats
```

`--test-video` plays a moving test pattern (`1080p60`, `1440p60` or `WxH@FPS`) and `--stats`
prints presentation statistics once per second. `--help` lists every option. Settings are kept
in `capturax.toml` next to the executable.

Checks (as CI runs them): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `cargo deny check licenses bans`.

## Contributing

Development is spec-driven: every feature starts as a spec in `specs/`. Read the
[constitution](.specify/memory/constitution.md) and the [agent guide](AGENTS.md) first.

## License

[GPL-3.0](LICENSE)
