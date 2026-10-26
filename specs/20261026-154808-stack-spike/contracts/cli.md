# Contract: Prototype Command Lines

All binaries run from `spike/` with `cargo run --release -p <name> -- <options>`.

## Shared options (native-preview, slint-preview, audio-preview)

| Option | Meaning |
|---|---|
| `--list` | print devices and their modes, then exit |
| `--device <name or index>` | video device (default: first capture device) |
| `--mode <WxH@FPS[:FORMAT]>` | e.g. `1920x1080@60:NV12`; default: Auto (largest resolution with ≥ 59 fps) |
| `--stats` | print frames per second and dropped frames every second (nothing is written) |
| `--record` | like `--stats`, and also append the rows to `measurements/pipeline-stats.csv`; use only for official measurement runs |

## native-preview

| Option | Meaning |
|---|---|
| `--sink <name>` | `d3d12` (default on Windows), `d3d11`, `gl` (default on Linux), `wayland`, `vulkan` |
| `--fullscreen` | open fullscreen on the current monitor |

## slint-preview

| Option | Meaning |
|---|---|
| `--bare` | video only, no chrome (measured path B) |
| `--transfer <cpu or gpu>` | how frames reach Slint (default `cpu`) |
| `--renderer <name>` | Slint renderer: `femtovg` (OpenGL, default), `femtovg-wgpu`, `skia` (Direct3D on Windows), `skia-opengl`, `skia-wgpu`, `software` (T014) |
| `--encoder` | add the hardware H.264 encoder branch (load test) |
| `--direction <float, frame or rail>` | starting visual direction (switchable at runtime) |
| `--theme <dark or light>` | starting theme (switchable at runtime) |
| `--test-video <file>` | use a video file instead of a device (visual review only) |

## audio-preview

| Option | Meaning |
|---|---|
| `--input <name>` | audio input (default: paired with the video device when detectable) |
| `--output <name>` | audio output (default: system default) |
| `--list-audio` | print audio inputs and outputs, then exit |

## browser/index.html

Opened from disk in Edge. Device and mode chosen in the page; shows the measured frame rate.

## tools/analyze.py

| Command | Meaning |
|---|---|
| `uv run tools/analyze.py video <clip> --path <p> --variant <v> --mode <m> --platform <os>` | detect flashes and append video samples |
| `uv run tools/analyze.py audio <clip> --platform <os> --output <device> --minute <n>` | detect beep pairs and append audio samples |
| `uv run tools/analyze.py summary` | write summary tables (markdown) from all samples |

On the first run of a clip the script asks the user to mark the TV and preview regions once (two
rectangles) and remembers them per clip.
