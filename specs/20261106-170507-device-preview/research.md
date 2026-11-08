# Research: Device Preview

Decisions for [plan.md](plan.md). Each entry: decision, rationale, alternatives. Evidence comes
from the stack spike ([findings](../20261026-154808-stack-spike/findings.md)), the app shell
([research](../20261104-112852-app-shell/research.md), [latency](../20261104-112852-app-shell/measurements/latency.md)),
the GStreamer 1.28.8 sources (`sys/mediafoundation`, `sys/v4l2`), wgpu 30.0.1 sources, and runs
on the maintainer's machines on 2026-11-06: Windows with a Logitech C920 and a virtual camera (the
ezcap was not plugged in there), CachyOS with the ezcap (GStreamer 1.28.8, `uvcvideo`). "Inferred"
marks what was not run.

## R1. Device discovery and hot plug

- **Decision**: one `gst::DeviceMonitor` on `Video/Source` with `show_all_devices(true)`, running
  for the app's lifetime on a capture thread of its own; its bus messages `DEVICE_ADDED` and
  `DEVICE_REMOVED` update the list (FR-001). A device is kept when it comes from the platform's
  preferred provider, decided from the device itself, not by `cfg`: `device.api ==
  "mediafoundation"` or a device type named `GstV4l2Device` (the spike's rule; on Linux the
  PipeWire provider lists the same card again and is skipped). While the app waits for a device
  (no devices, or the open one disconnected) it also re-reads `monitor.devices()` every 3 s, so a
  missed add is caught.
- **Windows**: `mfdeviceprovider` registers for `KSCATEGORY_CAPTURE` interface notifications on a
  hidden window and re-probes every device on each arrival or removal (each opened for about
  40 ms), so events arrive tens to hundreds of ms late, and an audio interface of the same card
  can trigger extra re-probes (filtered as duplicates). A device that cannot be opened during the
  probe (camera access denied) is not listed at all: see R6. Verified in source; plug and unplug
  not run.
- **Linux**: `v4l2deviceprovider` watches udev (`video4linux`), posts add and remove; the ezcap's
  metadata node `/dev/video1` is not listed (no capture capability). Verified in source and by
  listing; a physical unplug was not run (quickstart checks it). No `DEVICE_CHANGED` is posted.
- **Rationale**: GStreamer already does hot plug on both platforms; one code path, no OS code.
- **Alternatives**: own `RegisterDeviceNotification`/udev watchers in `platform`: duplicate what
  the providers do. Polling only: slower and, on Windows, opens every device every time.

## R2. Device identity

- **Decision**: the stable key is a string built from the device's properties (data-model.md
  "Device"):
  - Windows: `mf:` + `device.path` lowercased (the interface's symbolic link). With a USB serial
    the path does not depend on the port (C920: `\\?\usb#vid_046d&pid_082d&mi_00#d&41d17f6&0&0000#{…}\global`);
    without one Windows derives it from the port, so the card keeps its identity only on the same
    port.
  - Linux: `v4l2:` + `device.vendor.id:device.product.id:device.serial@device.bus_path` (ezcap:
    `32ed:3201:ezcap_ezcap_Live_GAMER_ULTRA_00000001@pci-0000:03:00.0-usb-0:3:1.0`). The serial
    is generic (`00000001`), so the bus path tells identical cards apart. `/dev/videoN` is never
    stored.
- **A card moved to another port**: without a USB serial its key changes (Windows path, Linux bus
  path), so it is a new device and its choices start from the defaults. Re-matching it by name and
  USB ids was considered and dropped (maintainer, 2026-11-06: not important; spec FR-003).
- **Numbering**: devices with the same name get " (2)", " (3)" in list order (FR-003).
- **Rationale**: these are the only properties that survive reconnects and tell identical cards
  apart on each platform (verified by `gst-device-monitor-1.0` on both).
- **Alternatives**: the name only: two identical cards collide. Windows container id: not exposed
  by the provider.

## R3. Modes, formats and the automatic modes

- **Decision**: modes come from the device's caps as in the spike (`capture/src/mode.rs`, tag
  `spike/stack-20261026`): each structure × frame rate (ranges become their ends plus common
  steps). One entry per resolution × frame rate, with the best format by this order: NV12, YUY2,
  UYVY, I420, BGRx, MJPG (decoded), then any other raw format (converted, R4). One Windows
  exception: when a device offers MJPG and NV12 at exactly the same sizes and NV12 is declared
  full range, NV12 is the Camera Frame Server decoding MJPG (C920, verified), so YUY2 goes before
  it at that size when offered.
- **Automatic modes** (F3, FR-007), computed by pure functions (`crates/capture`, unit tested):
  - **Auto max resolution**: the largest resolution with a frame rate within 58–62, at its
    highest such rate; if none reaches 58 (most webcams), the largest resolution at the device's
    highest frame rate (C920: 1080p30, not 2304 × 1536 at 2 fps).
  - **Auto max FPS**: the highest frame rate at 1080p; without 1080p, at the largest resolution
    below it; without those, at the smallest resolution above.
  - Two automatic modes resolving to the same native mode: only the first is shown (order: Auto
    source, Auto max resolution, Auto max FPS).
- **Auto source is not available on any device in this spec.** Neither platform reports a UVC
  card's HDMI input: on Linux `VIDIOC_QUERY_DV_TIMINGS` returns `ENOTTY` on the ezcap, its only
  input "Camera 1" has no status flags, and `uvcvideo` posts no source-change events (verified);
  on Windows Media Foundation exposes no such property for UVC and the plugin reads no extension
  units (inferred from the UVC spec and the source). The ezcap has an undocumented vendor
  extension unit (GUID `7ca138cf-71f2-4ec5-8d4c-f087741cabae`, 24 controls) that might hold it;
  reverse engineering it is out of scope. The capture crate keeps a `signal` slot in the device
  description (always none for now) so a device that reports its input can add Auto source
  without changing the menu logic. The spec allows it (FR-007: shown only when reported).
- **Default** (FR-008; maintainer, 2026-11-07, after T029): Auto source when shown; else
  1920 × 1080 at the highest rate within 58–62 fps when the device has it; else Auto max
  resolution. T029 measured Auto max FPS (1080p120 on the ezcap) with the PS5 at 60 Hz at a
  34.0 ms median against 28.7 ms in the 1080p60 mode (measurements/latency.md, T029), so a
  60 Hz source, the common case, opens in the 60 mode; a user with a 120 Hz source picks Auto
  max FPS and sees the effect in the bubbles. Auto max resolution was not taken as the
  fallback for every device because on the ezcap it is 2560 × 1440 at 60, which upscales a
  1080p source and was not measured. Earlier default (2026-11-06): Auto max FPS, for low
  latency and smooth motion.
- **Auto max FPS on the ezcap**: 1920 × 1080 at 120. With a 120 Hz source this is
  the fastest mode (half the card's wait for a frame, spike "Remaining latency levers"); with the
  PS5 at 60 Hz the card delivers about 111 fps and the spike measured every path 3–5 ms slower
  than in the 60 mode; T029 measured +5.3 ms with a 60 Hz source (quickstart L3) and 31.5 ms
  against a 120 Hz passthrough (L3b), which led to the default above; the frame rate bubble's default
  ("111 / 120 FPS") shows the mismatch to the user.
- **Tags** (F43; maintainer, 2026-11-07, replacing the four badges by height): a name tag by the
  old CapturaX rule over its list of standards (nearest standard at or above, else Custom) and an
  aspect ratio tag (the real ratio, snapped to a listed ratio within 3%), fills by ratio family
  with white text; rules, list and fills in data-model.md "Mode". Chosen on a mockup page with
  the real C920 and ezcap modes. The old rule gives a loose name between standards (960 × 720
  reads "HD"), kept for continuity with the old app; the aspect tag tells the real shape. The
  ratio list keeps 11:9 (CIF) and 25:16 (WQSXGA) so they are not snapped to a neighbour.
- **Alternatives**: infer the source rate from delivered frames (a 120 mode delivering about 111
  fps means a 60 Hz source): cannot infer the source resolution, and switching modes by guess
  would restart the device under the user; rejected.

## R4. Capture pipeline and frame upload

- **Decision**: one pipeline per open device:
  `source ! capsfilter(mode) [! jpeg decoder] [! videoconvert] ! appsink`.
  The appsink asks for the device's own raw format and hands its planes to the video thread
  without conversion; the composite shader turns YUV into RGB with the frame's matrix (BT.601 or
  BT.709, from the colorimetry, BT.709 when unknown at 720p and above) and the color range in use
  (R5). Uploaded formats: NV12 (R8 + RG8 textures), YUY2 and UYVY (RGBA8 at half width, unpacked
  in the shader), I420 (three R8), BGRx and RGBA (as is). Anything else, P010 included, gets a
  `videoconvert` to NV12 (logged). MJPEG decoders, in order: `nvjpegdec`, `d3d12mjpegdec`,
  `jpegdec` (the spike's order). The appsink keeps `sync=false max-buffers=1 drop=true`; plane
  buffers are reused through a small pool (no allocation per frame).
- **Rationale**: the spike converted to RGBA before the app (`d3d12upload ! d3d12convert !
  d3d12download` on Windows, `videoconvert` on CPU on Linux) and still measured 30.4 ms; skipping
  the conversion removes a GPU round trip on Windows and a CPU conversion on Linux, and halves to
  thirds the bytes uploaded (1080p: NV12 3.1 MB, RGBA 8.3 MB). At idle the NVIDIA link may drop
  to PCIe Gen1 (inferred): 8.3 MB is ≈ 4 ms there, NV12 ≈ 1.6 ms, which matters for R7. The
  range becomes a shader value, so Full, Limited and Auto switch without touching the pipeline
  (FR-010).
- **Zero copy**: not in this spec. `mfvideosrc` copies each frame once to system memory (no GPU
  path); `v4l2src io-mode=dmabuf` works on the ezcap but its buffers are system memory exported
  as DMA-BUF, wgpu has no public import, and an NV12 upload is ≈ 1 ms at full clocks (verified
  negotiation; cost inferred).
- **Alternatives**: keep the spike's RGBA conversion (simpler shader; slower and the range would
  need a pipeline change). GStreamer's GL or D3D12 sinks: rejected by the spike (R13 there).

## R5. Color range detection

- **Decision**: the appsink callback samples the luma plane on a 1/8 grid (every 8th pixel of
  every 8th row: about 32 000 samples at 1080p, tens of microseconds) once every 500 ms, never on
  the video thread. It feeds a pure detector (`crates/capture/src/analysis.rs`):
  - **Range (Auto)**: before evidence, the range the device declares (Limited when undeclared).
    If at least 0.1% of samples in one sampling are below 16 (8-bit), the range becomes Full and
    stays Full until the device or mode changes. Full and Limited set it directly.
- **No black-image detection** (maintainer, 2026-11-06): an HDCP hint was planned from five
  seconds of black samplings; dropped with spec FR-020, since black video with a signal also
  comes from consoles that are off, RCA adapters and VHS tapes.
- **Rationale**: the spike found 8% of luma values below 16 on a PS5 home screen with the ezcap
  declaring limited range; a 0.1% threshold ignores a few noisy pixels. Sampling off the video
  thread keeps FR-015.
- **Alternatives**: a GPU reduction pass: no CPU cost, but adds GPU work and a read back to the
  video thread. Histogram of the whole frame: needless.

## R6. Device errors and their states

- **Decision**: the capture crate maps each pipeline failure to one of `InUse`, `Blocked`,
  `Gone`, `NoSignal`, `Failed(message)` (data-model.md "Preview state"):
  - **Linux** (verified on the ezcap, unplug inferred): `RESOURCE/BUSY` → InUse (posted when
    the format is set); `RESOURCE/OPEN_READ_WRITE` with "Permission denied" in the debug text →
    Blocked, naming the node; `RESOURCE/NOT_FOUND`, or `RESOURCE/READ` together with a
    `DEVICE_REMOVED` of the same device → Gone.
  - **Windows** (in-use verified with two pipelines on the C920; privacy and unplug inferred): the
    plugin posts only generic errors (`CORE/STATE_CHANGE` when opening fails, `STREAM/FAILED`
    flow −5 when the first sample fails; in use is `0xC00D3704
    MF_E_HW_MFT_FAILED_START_STREAMING` in the debug log only). So: if the camera consent for
    desktop apps is denied (`platform::camera_access()`, reading
    `CapabilityAccessManager\ConsentStore\webcam` and `webcam\NonPackaged` `Value` under HKCU and
    HKLM) → Blocked; else if the device is no longer listed → Gone; else a `STREAM/FAILED` right
    after PLAYING → InUse; else Failed. Because a denied device disappears from the list (R1),
    "No devices" on Windows first checks the consent and shows Blocked when it is denied.
  - **No signal**: the pipeline plays but no frame arrives within 5 s.
  - The GStreamer error and debug text always go to the log.
- **Privacy page**: `platform::open_camera_privacy()` opens `ms-settings:privacy-webcam` on
  Windows; Linux has none (the state names the device node instead, FR-021).
- **Rationale**: the only reliable signals on each platform, with no Media Foundation code of our
  own.
- **Alternatives**: a GStreamer log hook parsing HRESULTs from `mfsourceobject` warnings: fragile
  across versions. Our own Media Foundation probe through `windows-sys`: precise, but a second
  capture path to keep; rejected (maintainer, 2026-11-06: no over-engineering for what neither
  the card nor Windows reports). If acceptance shows the heuristic misreads "in use", the Windows
  side drops it and shows the generic Failed state ("Cannot open <name>", **Try again**).

## R7. Keeping the GPU awake on Linux

- **Decision**: while the preview presents video, `platform::gpu_performance()` returns a guard
  that, on Linux with the NVIDIA driver, loads `libnvidia-ml.so.1` at run time and calls
  `nvmlDeviceSetPowerMizerMode_v1(device, NVML_POWER_MIZER_MODE_PREFER_MAXIMUM_PERFORMANCE)` for
  the GPU the video thread uses (matched by PCI bus id from wgpu's adapter info), restoring the
  previous mode when the guard drops (preview hidden, device closed, quit). A missing library,
  symbol or a refusal is logged once and ignored. Windows and other GPUs: no-op (Windows measured
  0.8–1.2 ms at default clocks).
- **Evidence**: since driver 590.48.01 `nvidia-settings` sets PowerMizer on Wayland through this
  call as a normal user; `nvml.h` lists no permission error for it (it does for locked clocks);
  a user report says the mode holds only while the requesting process lives, which suits the app
  (a crash undoes it). Not yet run on the CachyOS PC: the first task validates it with a short
  script and `nvidia-smi --query-gpu=pstate,clocks.gr,clocks.mem,pcie.link.gen.current,power.draw`.
- **Per process, checked on the CachyOS PC** (T025, 2026-11-07): the driver keeps one request
  per process and applies the highest. `nvidia-settings -a GpuPowerMizerMode=1` alone wakes the
  GPU only until nvidia-settings exits; setting 0 from nvidia-settings while the example holds 1
  leaves the GPU at P0, and nvidia-settings reads 0 meanwhile (its own request). Restoring on drop
  therefore only withdraws CapturaX's request: it can neither override a choice made in another
  program nor slow down another program's GPU work, which the driver's adaptive mode keeps at
  full clocks while it has load.
- **Together with R4**: fewer bytes per frame narrows the gap at low clocks; the PowerMizer
  request closes it. The power cost (P0 while previewing, estimated +10–20 W on the RTX 3050) is
  measured and recorded (FR-012).
- **If NVML does not hold the clocks**: fall back to a small keep-alive GPU workload on the video
  thread sized by measurement (it must keep both graphics and memory clocks up), recorded with
  its power cost; decided with the maintainer.
- **Later** (maintainer, 2026-11-07): this spec requests only while the preview presents video,
  always (no setting). A live with the preview hidden and an **Accelerate GPU** setting (Auto:
  whenever needed except on battery; Off: never; On: whenever needed) are product.md F55, roadmap #15, after a working
  live; that spec measures whether the request helps a live before adding it.
- **Ruled out**: `~/.nv/nvidia-application-profiles-rc` (GLX only, no power key), `__GL_*`
  variables (none for Vulkan power), `PerfLevelSrc` and locked clocks (root), GameMode's NVIDIA
  options (X11 `nvidia-settings -a`, one-shot), AMD `power_dpm_force_performance_level` and Intel
  `gt_min_freq_mhz` (root; no such hardware to measure here), `VK_NV_low_latency2` boost (needs a
  wgpu fork).
- **New dependency**: `libloading` (ISC, maintained, widely used) in `crates/platform`, Linux
  only, to load NVML without linking it (the app must start on machines without NVIDIA).

- **AMD and Intel** (maintainer, 2026-11-06): not covered. No such GPU is available to measure
  whether they step down under the preview's load, and the known controls need root (above).
  Recorded as a known limit; checked when the maintainer or a contributor has one (the
  quickstart's L4/L6 comparison with `radeontop` or `intel_gpu_top` instead of `nvidia-smi`).

## R8. Windows stalls at presentation mode changes (investigation)

- **Decision**: the plan does not presume a fix; it runs a measured investigation (FR-023, FR-024)
  and records the cause in measurements/stalls.md:
  1. A developer option `--present-log <file>` writes one line per present with the raw
     `QueryPerformanceCounter` value at acquire and at present return (through `platform::qpc()`),
     plus window events (focus, fullscreen) with their QPC.
  2. PresentMon 2.3.1, elevated: `PresentMon-2.3.1-x64.exe --process_name capturax.exe --qpc_time
     --v1_metrics --output_file pm.csv --timed 90 --terminate_after_timed`; read `PresentMode`,
     `AllowsTearing`, `MsInPresentAPI`, `MsUntilDisplayed`, `MsBetweenDisplayChange`, matched to
     the app's log by QPC (`CPUStartQPC`).
  3. Matrix on the native 1080p monitor: G-SYNC on and off (NVIDIA Control Panel → Program
     settings → capturax.exe → Monitor Technology), MPO on and off (`HKLM\SOFTWARE\Microsoft\Windows\Dwm`
     `OverlayTestMode=5`, reboot; may be ignored on 24H2+, PresentMon shows it), plus
     `OverlayMinFPS=0` and `WGPU_DX12_PRESENTATION_SYSTEM=visual` (DirectComposition swap chain,
     wgpu 30) as cheap trials. Ten focus changes and ten fullscreen entries each.
  4. Language switch stalls: per-stage timing (catalog switch, Slint property update, UI draw,
     upload, present) logged under `--stats`, and the same QPC log, to see whether the main thread
     or the video thread holds.
- **What is known**: wgpu 30 on DX12 already uses a flip-discard swap chain with the waitable
  object, `SetMaximumFrameLatency`, and `ALLOW_TEARING` always set when supported, so the flags
  never change between modes; Mailbox is `Present(0,0)`. A focused DX12 flip-model window behaves
  as exclusive fullscreen for G-SYNC "fullscreen only"; similar 300 ms alt-tab stalls with G-SYNC
  in DX12 borderless are reported for Unreal and disappear with G-SYNC off. No public source
  explains the stall 18–20 s after entering fullscreen.
- **Outcomes**: if the cause is a G-SYNC switch, the remedy available to the app is an NVIDIA
  driver profile (NVAPI DRS, `VRR_APP_OVERRIDE`/`VRR_MODE`), which turns variable refresh off for
  the app and may need elevation: that trades F52's default benefit, so it goes to the maintainer
  as a decision, not into this plan. If the cause is in the app (for example a surface
  reconfigure or a blocking acquire at focus change), it is fixed (SC-006).
- **Alternatives**: forcing composition (a transparent overlay window): avoids the switch but
  loses independent flip's latency; rejected.

## R9. Presentation setting (F52)

- **Decision**: **Lowest latency** keeps the current loop (Mailbox; a present per new frame or UI
  change). **Follow the monitor** switches the surface to `PresentMode::Fifo` with
  `desired_maximum_frame_latency = 1` and drives the loop by the swap chain: acquire (blocks until
  about the next refresh), then take the newest frame or reuse the last uploaded one, composite,
  present. Taking the frame before the acquire would add up to one more refresh, so the order is
  fixed. Switching reconfigures the surface only (no device reopen) and only when the user
  changes the setting. The stats header shows the mode (`mode=fifo-follow`).
- **Cost**: 0–6.9 ms at 144 Hz (≈ 3.5 ms average) per frame; about 144 composites a second,
  uploads only on new frames. On Vulkan the image count is latency + 1, clamped to the driver's
  minimum; it is logged. KWin uses variable refresh for fullscreen windows only by default, so on
  Linux the setting matters only there.
- **Alternatives**: Mailbox with a timer at the refresh rate: needs the refresh timing from the
  OS and drifts; Fifo gives it for free.

## R10. Fullscreen from a snapped window (FR-025)

- **Decision**: `platform::arranged_rect(window) -> Option<rect>`: on Windows, when
  `IsWindowArranged` says the window is snapped, its `GetWindowRect`; the app keeps it on entering
  fullscreen and, after leaving, puts the window back there with `platform::set_outer_rect`
  (`SetWindowPos`), since winit restores the place before the snap. Linux returns none (KWin
  restores tiles itself, checked in the app shell).
- **Alternatives**: winit's own restore: the bug itself.

## R11. Measured latency and the estimate (FR-016a, SC-007)

- **Decision**: each frame carries `captured: Instant`, computed in the appsink callback as
  `Instant::now() − (clock.time() − (pts + base_time))`; the video thread records
  `presented − captured` per frame (median per second for the bubble and `lat_ms` in the stats).
  What the measured part covers differs by platform, and the estimate covers the rest:
  - **Linux**: `v4l2src` turns the driver's monotonic timestamp into the PTS (`do-timestamp`
    stays false); `uvcvideo` stamps a frame when its first packet arrives, so the measured part is
    "frame starts arriving at the host → presented" and includes the USB transfer, which the card
    spreads over the whole frame period (measured at the source pad: 8.2 ms at 1080p120, 32.9 ms at
    4K30). If `v4l2src` judges the driver timestamps bad it assumes one frame silently; a warning
    goes to the log.
  - **Windows**: `mfvideosrc` (IMFSourceReader, the default path) stamps the running time when the
    sample reaches GStreamer and ignores Media Foundation's sample time, so the measured part is
    "frame reaches the app → presented" (≈ 1–3 ms).
  - **Estimate** (pure function, data-model.md "Latency estimate"): what precedes the measured
    part plus what follows the present: the card receiving the whole frame from HDMI (one source
    frame period), on Windows also its USB transfer (one more frame period), a fixed card and
    display constant per platform, and the wait for the monitor's refresh (half a refresh period
    without variable refresh, zero with it while focused). The constants start from the spike's
    numbers and are calibrated once from this spec's glass-to-glass runs (SC-001, SC-002); SC-007
    then checks the Total against those medians within 5 ms.
- **Rationale**: real numbers where the platform gives them, an explicit, testable estimate for
  the rest, as the maintainer chose (FR-016a).
- **Alternatives**: one cross-platform definition (arrival in the app on both): throws away the
  Linux driver timestamp. Media Foundation's device timestamp (`MFSampleExtension_DeviceTimestamp`):
  not reachable through `mfvideosrc`.

## R12. Where the code goes

- **Decision**: a new crate `crates/capture` holds everything GStreamer for devices: the monitor
  and its events, device descriptions and identity, modes and automatic modes, the pipeline and
  its errors, frame planes with colorimetry and capture time, and the luma analysis. It has no OS
  calls (GStreamer is the abstraction; the provider is chosen from device properties, R1), so it
  sits outside `platform` without breaking principle IV; the boundary test is extended to it. The
  test video moves into it as a source like any other, producing NV12 so the YUV path is the one
  measured. `app` gains `preview.rs` (the state machine of data-model.md, settings, reconnect),
  the UI bridge for the toolbar, bubbles and the Preview category, and the YUV composite in
  `render/`. `platform` gains `camera_access`, `open_camera_privacy`, `gpu_performance`,
  `qpc`, `arranged_rect`, `set_outer_rect` (contracts/platform.md).
- **Rationale**: the mode logic and the analysis are pure and unit tested without a device;
  pipeline tests use `videotestsrc` (constitution VII).
- **Alternatives**: capture inside `app`: the app crate grows and its tests need GStreamer for
  every run.

## R13. Interface pieces

- **Decision**: the toolbar slot gets its panel with three selects: **device**, **mode**, **color
  range** (Float order, left to right). They look like the spike's `dropdown.slint`
  (maintainer, 2026-11-07): a raised control of 240 × 48 (narrower in the compact layout) with
  its small title inside, above the chosen value in bold and that item's badge (the title in the
  muted tone, not the spike's faint one, which falls below 4.5:1 on the hovered control; no
  pressed tint, as the spike); its menu is at
  least 320 px wide and slides open, group headers in faint text, an item's detail on a second
  line under its name. The settings panel's selects keep the shell's 36 px look (they sit in
  44 px rows). `MenuItem` grows optional fields: `kind` (item, group header), `detail` (second
  line, for the resolved mode of an automatic mode and the device's detail line), `badge`
  (resolution badge) and `disabled`; the existing selects keep working with text only. The device select shows "No devices" as a disabled item when the list
  is empty (never an empty menu). Info bubbles (source, mode, frame rate, latency) sit in the
  video area's top left corner with the chrome, in the spike's shape (black at 55%, opaque text);
  the frame rate and latency bubbles show a settings button on hover, drawn in the top layer (app
  shell plan), which opens a menu with their three displays. The settings panel gains **Preview**
  (Tabler `device-tv` icon) with the Presentation select and its tooltip.
- **States** (FR-019): the app shell's `StateMessage` with icons from Tabler (`device_tv_off` no
  devices / disconnected, `lock` blocked, `player_pause` in use, `antenna_bars_off` no signal,
  `alert_triangle` failed); opening shows the Fluent progress ring (an arc that grows, shrinks
  and turns, as in Windows 11), drawn by the app, no text, only after 1 s (R17).

## R14. Dependencies

| Crate | Where | Purpose | License | Status |
|---|---|---|---|---|
| `libloading` 0.8 | `crates/platform`, Linux | load `libnvidia-ml.so.1` at run time (R7) | ISC | maintained, ~200 M downloads |
| `windows` 0.62 | `crates/platform`, Windows | WinRT toast notifications (R15); already in the tree through Slint and wgpu, only features added | MIT or Apache-2.0 | Microsoft, maintained |

GStreamer bindings are already in the workspace; `crates/capture` uses `gst`, `gst-app`,
`gst-video`. Windows registry and `ShellExecuteW` come from the `windows-sys` features already
declared. No GStreamer plugin beyond those the spike used (`mfvideosrc`, `v4l2src`, `jpegdec`,
`videoconvert`, `appsink`; `nvjpegdec`/`d3d12mjpegdec` when present).

## R15. Device notices (FR-027)

- **Decision**: the routing is pure app logic: monitor `Added`/`Removed` after `Listed` → a
  notice; window focused → in-app notice (one balloon drawn by the app in Slint, no toast
  library: top layer, bottom right of the video area above the dock, about 5 s, × and a click
  anywhere close it, only the latest message); else → `platform::notify(notice)`. A system notification's action comes
  back as a user event through the `EventLoopProxy`, as tray events do.
  - **Linux**: `org.freedesktop.Notifications.Notify` over D-Bus with `zbus` (already a
    dependency of `crates/platform` for the theme), app name `CapturaX`, the app icon, actions
    `["open", "<Open>"]` when there is one, `replaces_id` of the previous notice (one at a time);
    the `ActionInvoked` signal maps back to the device id. No service on the bus → logged once,
    no-op.
  - **Windows**: WinRT `ToastNotificationManager` (`windows` crate, features
    `UI_Notifications`, `Data_Xml_Dom`; the crate is already in the tree through Slint and wgpu,
    version 0.62). A portable app has no package identity, so it sets
    `SetCurrentProcessExplicitAppUserModelID("CapturaX")` and registers that id under
    `HKCU\Software\Classes\AppUserModelId\CapturaX` (`DisplayName`, `IconUri` to the app's icon
    next to the executable), written when a notification is first shown and rewritten if the
    folder moved; the toast's button is handled through the toast's `Activated` event while the
    app runs (no COM activator, no start from a toast); `ToastNotifier::Hide` of the previous
    toast keeps one at a time. Notifications off for the app → `Setting` is not `Enabled`, logged
    once, no-op. Windows hides toasts while another app is fullscreen (focus assist); with
    CapturaX fullscreen and focused the in-app notice is used anyway.
- **To validate first** (task T061, maintainer): on Windows 11 the toast shows with the name
  CapturaX and its icon from an unpacked build and its **Open** button reaches the app; on KDE
  Plasma the notification shows with **Open** and the action reaches the app. If the Windows
  toast does not work without a shortcut or a package, the fallback (a balloon on the app's own
  tray icon) is brought to the maintainer before any change.
- **Rationale**: both are the platforms' own services, no new crate on Linux and only features of
  a crate already compiled on Windows; the in-app notice needs no platform code.
- **Alternatives**: a tray balloon (`Shell_NotifyIcon` `NIF_INFO`): `tray-icon` does not expose
  it, and Windows 11 shows it as a toast from "Windows Explorer"; `notify-rust`: on Windows it
  borrows PowerShell's app id (wrong name and icon), on Linux it duplicates what `zbus` does.
- **Constitution VI**: the `AppUserModelId` key is a display registration, not settings, like the
  autostart entry; it is written only for the current user and holds no state.

## R16. UI presents between slow video frames

- **Problem** (maintainer, 2026-11-07): the app shell's rule "UI presents ride on video frames"
  (app shell research R3, T041) makes the interface animate at the video's frame rate. With the
  C920 at 30 fps the chrome animates at 30 fps, and with G-SYNC the monitor then refreshes at
  30 Hz, so the mouse cursor moves at 30 Hz too. The stack spike, which presented every UI change
  at once, stays smooth with the same camera. Below 20 fps the shell's 50 ms "flowing" window
  already lets UI images present on their own between frames (except the 50 ms after each
  frame), so the worst range is about 20–60 fps; a webcam's 2 fps modes still stutter for the
  first 50 ms after each frame.
- **Decision**: a UI image or scene change is presented on its own, also while the video flows,
  when the next video frame is not expected within one monitor refresh plus a margin: expected
  next frame = last frame + the source's frame period (median of the recent arrival intervals,
  the open mode's rate before there are any). Otherwise it rides on the next video frame as
  today. A UI present delays the next acquire by at most one refresh, so a frame arriving as
  expected never waits for it; a frame arriving early can wait up to one refresh, only while the
  UI changes. With 120 fps sources nothing changes (no gap); at 30 fps on a 60–144 Hz monitor
  nearly every UI image fits between frames.
- **Validation**: `ready_ms` median and max with the chrome animating, before and after, with the
  C920 (30 fps), the test video (60 fps) and the ezcap (120 fps); idle unchanged; SC-001 and
  SC-003 still met; the chrome and the cursor smooth with G-SYNC on the C920.
- **Alternatives**: presenting every UI change at once (the spike; fails SC-001 while
  animating, T041); predicting the vsync (rejected in the app shell as fragile); a deeper swap
  chain (a vsync more latency).

## R17. Source switch fade (FR-013)

- **Decision** (maintainer, 2026-11-07, chosen on the mockup page): a switch of device or mode
  fades the current video to the backdrop over about 150 ms and the first frame of the next one
  in over about 150 ms; the start fades in too. The opening state shows nothing for its first
  second, then the Fluent progress ring alone (no text; replaces `loader_2` and "Opening <name>").
  Errors and a disconnect show their state at once (no fade wait).
- **How**: the video thread draws the video with an opacity uniform it eases itself at each
  present (no main thread round trip, no extra frame of delay: a new frame is presented at once,
  only dimmed during the fade-in). On a switch the old session closes at once and the video
  thread keeps its last frame only to fade it out (the inbox's `clear_video` becomes a fade-out,
  then clear); the new session opens meanwhile, so the fade adds no time to the switch (opening
  takes 300–400 ms, longer than the fade). The 1 s spinner delay is a Slint timer on the opening
  state.
- **Rationale**: the spinner and its text flashed for a few hundred ms on every switch; a fade
  reads as one smooth change, and the spinner still tells the user when a device is slow.
- **Alternatives**: crossfade old and new: needs both sessions open at once, which a USB card
  cannot do for two of its modes, and doubles the bandwidth for two devices; spinner with text:
  the previous design, found heavy by the maintainer.

## R18. Video backdrop: background, watermark, shadow (FR-028 to FR-031)

- **Decision** (maintainer, 2026-11-07, chosen on a mockup page:
  https://claude.ai/artifact/5vWgGWi4i6VvkMk4Vr8YiD, private): the ground around the video is the
  theme's deepest tone or black; the CapturaX X lies on it as a watermark (on by default, soft,
  cut a little by the right edge), and a soft shadow around the video is **Always**, **With
  ambient light** or **Off** (Always by default since 2026-11-07). Every combination is possible: plain black bars (black,
  watermark off), the theme's ground with the X, and later the ambient light over either.
- **How**: the UI layer is drawn over the video, so the backdrop is the video thread's: the
  composite pass clears to the ground color, draws the watermark as one more textured rectangle
  (an alpha mask of `brand/X.svg` rasterized by `build.rs` with `resvg`, like the icons, 1024 px
  tall, tinted and faded by its uniform), then the shadow (a rectangle larger than the video whose
  fragment shader computes the blurred rounded-rectangle falloff from the same signed distance as
  the video's corners), then the video. Two more draws per present, no texture upload after the
  first: no measurable cost expected; `ready_ms` is compared before and after, as for R17. The
  state messages lose their own fill so the ground and the X show behind them; their text tones
  stay checked against the theme's deep tone, and against black for the black background
  (app/tests/contrast.rs). The settings reach the video thread through the `Scene`.
- **Segmented control** (FR-031): one Slint component, `segmented.slint`, used for the choices of
  two or three short options (Video background, Video shadow, Presentation; Watermark is an on/off
  switch): a deep-toned track with 8 px corners, the chosen segment filled with the accent (6 px
  corners, accent ink text; maintainer, 2026-11-07, instead of a raised segment with a dot);
  arrows move the choice, radio group semantics per segment. The shadow is drawn half as strong
  on the light theme's ground, where a black shadow looked dirty.
- **Live viewers** (F38, spec #10): the viewer page gets the same backdrop (background, watermark,
  shadow, theme, ambient light); who chooses them is that spec's question.
- **Alternatives**: the watermark in the UI layer: it would cover the video. A fixed watermark
  image per window size: a new texture on every resize; the mask scaled by the GPU is enough at
  this opacity.
