# CapturaX

Watch and hear your capture card with the lowest possible latency, good enough to play a
console through it. Capture screens and windows, take screenshots, record, and share a live
stream with friends by link. A simple alternative for these tasks, not a replacement for OBS.

> **Status:** early rewrite. The app shows a capture card or webcam in its native mode and
> plays its sound with low latency, takes screenshots with a global key, with ambient light,
> a hideable preview and movable info bubbles, settings, tray and device notices. The previous
> version is in [CapturaX-old](https://github.com/Onutirachi/CapturaX-old).

## Planned

- Screens and windows as a source
- Crash-safe recordings with hardware encoding
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
- Linux audio: PipeWire (the app plays through it directly). Building needs its development
  files and `clang` (the `pipewire` crate generates its bindings): Arch / CachyOS `pipewire
  clang`; Ubuntu `libpipewire-0.3-dev clang`. Windows needs nothing more (WASAPI).

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

### Preview audio

The sound of the device being previewed plays on the chosen output with the lowest delay the
system allows. The **Audio** dropdown in the toolbar picks the input (**Auto** is the input of
the same physical device, remembered per video device; **No audio** turns it off) and, in its
**Output** group, the output (**System default** follows the system's choice). The volume button
in the left dock mutes with a click and shows the volume slider when the pointer rests on it;
it changes only the preview's sound, never the system's volume.

On Windows the settings' **Audio** category has an **Exclusive** switch per input and output:
an exclusive device skips the Windows mixer for the lowest delay, and no other app can use it
while the preview does. On Linux the app plays through PipeWire with a 128-frame quantum
(about 2.7 ms at 48 kHz) and has no exclusive mode.

### Ambient light

The bulb button in the left dock lights the area around the video (the bars of a capture that
does not fill the window) with the video's colors, as a strongly blurred copy of the frame that
follows the picture smoothly, without flashing on a single frame. Off by default. Resting the
pointer on the button shows an intensity slider (0 to 100 %), and the wheel over it changes the
intensity by 5 %. The choice applies to every device. The glow never holds a video frame back,
so it adds no latency. **Video shadow → With ambient light** in the settings draws the shadow
only while the light is on.

### Hiding the preview

The first button in the right dock hides the preview to save resources: the video and its sound
stop, the device is released (a webcam's light turns off, another app can open it) and the
video area says "Preview hidden". The toolbar keeps working: a device, mode, range or audio
choice made while hidden is saved and used when the preview shows again. Hidden lasts through
restarts, so an app started hidden opens no device until the button shows the preview again.

### Info bubbles

The bubbles over the video (source, mode, frame rate, latency) can each sit in any of the four
corners. Drag a bubble to move it: the corner it would land in glows and a slot shows its place
in that corner's order. Its hover controls (or Enter on a focused bubble) hold a **Pin** toggle:
pinned bubbles stay on screen while the controls are hidden, packed against their corner. The
arrangement is one for the app; the settings' **Preview** category has an **Info bubbles** row
whose **Reset** puts them all back at the bottom right, unpinned.

### Screenshots

**Ctrl+Shift+F8**, or the camera button in the right dock, saves the next frame of the preview as a
PNG at the device's native resolution, with the colors the preview shows and without the
interface, and copies it to the clipboard. A white outline flashes around the video and a short
capture sound plays on the preview's audio output (also when the preview is muted or has no
audio), so you know it was taken while playing. A notice then says whether the file was saved
and copied; **Show** opens the file manager on the file. When no frame can be taken (no image,
or too many screenshots still being written) nothing plays and nothing is saved.

Files are named `CapturaX <YYYY-MM-DD HH-MM-SS>.png` in local time and never overwritten. They
go to `Pictures/CapturaX` by default; the settings' **Folders** category changes, opens or
resets the folder. A chosen folder is kept as an absolute path, the default as "default", so a
copied installation uses the other system's Pictures folder.

The key works while another app (a game) has the focus, and while CapturaX is minimized or
hidden to the tray. The settings' **Hotkeys** category changes it: click the key and press the
new one (a function key alone, or a key with Ctrl, Alt or Win/Super), Esc to cancel; it can also
be cleared or reset. A key the system refuses (another program holds it) is logged and the row
says so. On Linux the key is global under X11 and, under Wayland, through the desktop's global
shortcuts portal: the desktop asks once to confirm the key and owns it from then on, so the
Hotkeys row shows the desktop's key and clicking it opens the desktop's shortcut settings. The
portal needs a `capturax.desktop` entry; when no package installed one, CapturaX writes a
hidden one in `~/.local/share/applications` (not shown in the menu). Without the portal the key
works only while CapturaX's window has the focus. The
copied image stays pasteable on Linux while CapturaX runs, and after it quits only with a
clipboard manager.

Developer options (`--help` lists every option):

| Option | Effect |
|---|---|
| `--list-devices` | Prints every device (name, id) with its modes and the automatic modes it shows, the audio inputs and outputs, and the audio input paired with each device, then exits |
| `--test-video <PRESET>` | Plays a moving test pattern (`1080p60`, `1440p60` or `WxH@FPS`) instead of a device |
| `--stats` | Prints presentation statistics once per second (also to `capturax.log`) |
| `--no-audio` | Starts without the preview audio, to compare the video's measurements with and without sound |
| `--screenshot-every <MS>` | Debug builds: takes a screenshot every `MS` milliseconds, as the key would, to measure their cost on the preview |
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
