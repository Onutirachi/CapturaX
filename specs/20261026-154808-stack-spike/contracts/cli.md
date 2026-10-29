# Contract: Prototype Command Lines

All binaries run from `spike/` with `cargo run --release -p <name> -- <options>`.

## Shared options (native-preview, slint-preview, audio-preview)

| Option | Meaning |
|---|---|
| `--list` | print devices and their modes, then exit |
| `--device <name or index>` | video device (default: first capture device) |
| `--mode <WxH@FPS[:FORMAT]>` | e.g. `1920x1080@60:NV12`; default: Auto (largest resolution with ≥ 59 fps) |
| `--range <full or limited>` | override the color range the device declares (the ezcap declares limited, 16–235, but sends values below 16) |
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
| `--theme <dark or light>` | starting theme (switchable at runtime) |
| `--test-video <file>` | use a video file instead of a device (visual review only) |

## layered-preview

Own render loop (research R13). Shared options, plus:

| Option | Meaning |
|---|---|
| `--present <mode>` | `immediate`, `mailbox` or `fifo` (default: the first one the GPU supports, in that order) |
| `--backend <api>` | graphics API: `vulkan`, `dx12` or `gl` (default: wgpu's choice; on the maintainer's NVIDIA it picks Vulkan) |
| `--encoder` | add the hardware H.264 encoder branch (load test, T013); prints the encoder used, and with `--stats` the encoded frames per second |
| `--audio` | also play the audio input paired with the video device on the system default output, as `audio-preview --free-run --buffer 20` does |
| `--ui <name>` | UI over the video: `overlay` (default, measured path), `render-test` (T048), `theme-test` (T027), `float` (the Float direction, T032) |
| `--theme <dark or light>` | starting theme (default `dark`; the Float direction switches it at runtime) |
| `--test-video` | a live test pattern (`videotestsrc`) in `--mode` (default `1920x1080@60:NV12`) instead of a device (visual review only) |

With `--ui float` and no device found, the window opens with a message saying what to do (connect
a device or use `--test-video`) instead of failing.

## audio-preview

| Option | Meaning |
|---|---|
| `--input <name>` | audio input (default: paired with the video device when detectable) |
| `--output <name>` | audio output (default: system default) |
| `--list-audio` | print audio inputs and outputs, then exit |
| `--free-run` | play each block as soon as it arrives, without waiting for the pipeline clock (output `sync=false`, as the previous app did), through a queue that drops the oldest audio beyond `--max-queue` |
| `--max-queue <ms>` | with `--free-run`, the most audio waiting between input and output (default 20 ms) |
| `--buffer <ms>` | output buffer size (GStreamer's default is 200 ms); the output writes in blocks of half of it, at most 10 ms |
| `--exclusive <input, output or both>` | open that side in WASAPI exclusive mode (Windows; shorter device period, other apps cannot use the device meanwhile) |

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
