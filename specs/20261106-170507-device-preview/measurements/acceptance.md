# Acceptance: Device Preview

Quickstart "First checks" and "Manual acceptance" ([quickstart.md](../quickstart.md)) per user
story. The latency budget is in [latency.md](latency.md).

## US1 (T027)

"First checks" 2–3 and "Manual acceptance" 1. Not checkable yet, recorded as not checked: the
device select with its badge and detail line (T031, US2) and `lat_ms` in the `[loop]` line
(T036, US3).

### Windows 11 — 2026-11-07

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, preview monitor C24FG7x 144 Hz
G-SYNC, DX12, release build of `feat: hold the gpu performance request while previewing`.
Devices: Logitech HD Pro Webcam C920 and the phone's Windows virtual camera ("S23 FE de Carlos
(Câmera Virtual do Windows)"). Checked by the maintainer by hand; the agent read the log. The
ezcap was not at the Windows PC.

| Check | Result | Notes |
|---|---|---|
| First check 2: start without `[preview]` → the card in Auto max FPS | Pass (rule as designed) | No capture card connected. The virtual camera has no "cam" in its name, so the start order (data-model.md "Start order") took it as a capture card and opened it over the C920, four starts in a row: `1280x720@30 NV12 bt709 range=limited(auto)`, first frame 26–69 ms after the choice. The heuristic was kept (maintainer, 2026-11-07): a first-start guess, the user's choice is remembered. Not saved as the last used device (fallback, not a user choice), as designed. |
| First check 3: `[source]` line with `--stats` | Pass | `[source] device="HD Pro Webcam C920" … mode=1920x1080@30 format=NV12 matrix=bt601 range=full(auto)` |
| US1 1: restart → same device and mode | Pass | No device menu yet (T031) and the virtual camera is hard to turn off, so the maintainer pasted the C920's id from `--list-devices` into `[preview] device`: `start: last used device`, 1080p30 NV12, first frame 365 ms after the choice |
| US1 1: color range Auto / Full / Limited | Pass | Maintainer; switches are not logged |
| US1 1: fullscreen and windowed | Pass | Maintainer |
| US1 1: image matches the passthrough | Not checked | Needs the ezcap and the PS5 |
| 150 %, English | Not checked | |

GPU request: "gpu performance requested (adapter 0000:01:00.0)" after the first frame and
"released" on quit in every run (no-op on Windows, contracts/platform.md). In the C920 run the
camera delivered 15 fps (privacy shutter closed, auto exposure): `recv=15`, `gap_ms` ≈ 80, `ready_ms` 0.8 median,
≤ 1.4 max after the first second.

### CachyOS KDE Plasma Wayland — 2026-11-07

**Environment**: CachyOS, KDE Plasma 6 Wayland, NVIDIA GeForce RTX 3050, one 1080p 120 Hz
monitor, Vulkan, release build of `feat: hold the gpu performance request while previewing`
copied to `~/capx/t027` with no settings file. Device: ezcap Live Gamer Ultra. Driven by the agent
over SSH (`ydotool`, `spectacle` screenshots, `nvidia-smi` once per second); raw logs in
[raw/t027/](raw/t027/).

| Check | Result | Notes |
|---|---|---|
| First check 2: start without `[preview]` → the card in Auto max FPS | Pass | `start: first capture card`, 1920x1080@120 NV12, first frame 392–393 ms after the choice |
| First check 3: `[source]` line with `--stats` | Pass | `range=limited(auto)` from the declared range at `Playing`, then `full(auto)` once the detector ran |
| US1 1: restart → same device and mode | Pass | Same card and mode (start order, the only device); a stored range (`full`) applied on restart |
| US1 1: color range Auto / Full / Limited | Pass | Button `Auto (Completa)`; Limitada and Completa apply at once (`[source] range=limited`, `range=full`, no new `Playing`), saved in `devices."<id>".color_range`; back to Auto → `full(auto)` |
| US1 1: fullscreen and windowed | Pass | F11 and Esc, toolbar in both |
| GPU request (T026) | Pass | P8 210 MHz 5.8 W → P0 1777/7001 MHz 24 W within a second of the first frame, held while previewing; hide to tray → "gpu performance released", "PowerMizer back to Adaptive", P5 then P8 within 2–4 s |

The first runs had no HDMI input (the card's own "Please Connect a Video Source" image, pure
black and white, where the range barely shows). With the PS5 on a PlayStation Store page (bright
colors, dark gray background), screenshots of the windowed preview:

| Choice | `[source]` | Dark background (left margin, luma min / mean) | White button | Yellow frame |
|---|---|---|---|---|
| Auto | `full(auto)` | as Full | | |
| Full | `full` | 5 / 9.7 | (253, 255, 255) | (246, 221, 35) |
| Limited | `limited` | 0 / 0.0 | (255, 255, 255) | (255, 239, 27) |

Limited crushes the dark gray to black and clips the yellow, as expected for a full-range source
read as limited; Auto picks Full. The maintainer checked by eye that the preview matches the
passthrough in Auto (and the screenshots above): Pass.

Not checked on CachyOS: 150 %, English.

The ezcap on Windows (T027 asks for it "when it is at the Windows PC") is left for the next time
the card is there; the range behaviour on Windows was confirmed by the maintainer with the C920.

## US2 (T034)

"Manual acceptance" 2 and 8.

### Windows 11 — 2026-11-13

**Environment**: RTX 5070 Ti, DX12, the preview on the central monitor (100 %, Portuguese),
release build of `a36e8db` in a new folder `target/t058w`. Devices: ezcap Live GAMER ULTRA
(internal card, an HDMI source at 1080p60) and HD Pro Webcam C920 (USB). Checked by the
maintainer by hand; the agent read the times from the log
([raw/t058-windows/us2.log](raw/t058-windows/us2.log)).

| Check | Result | Notes |
|---|---|---|
| `--list-devices` (First check 1) | Pass | The ezcap: `auto-max-resolution` → 2560x1440@60, `auto-max-fps` → 1920x1080@120, modes 3840x2160@30 down to 1280x720@60, no Auto source; the C920: `auto-max-resolution` → 1920x1080@30 NV12, no MJPG mode chosen (the Frame Server's NV12 at the same sizes) |
| First start without settings (First check 2) | Pass | `start: first capture card`, the ezcap in 1920x1080@60 NV12, first frame 450 and 483 ms after the choice |
| `--stats` (First check 3) | Pass | `[source] … format=NV12 matrix=bt709 range=limited(auto)`, then `range=full(auto)` once the content was read; `lat_ms` median 2.0–2.3 ([raw/t058-windows/first-start-stats.txt](raw/t058-windows/first-start-stats.txt)); one second with `gap_ms` 88.7, not repeated |
| Device menu: name and aspect tags, no screens | Pass | |
| Mode menu: Automatic group without duplicates, Modes largest first | Pass | |
| The fade on a switch, spinner only on a slow open | Pass (no slow open) | No open took over 1 s |
| Video within 2 s of a switch (SC-004) | Pass | Device switches 467–584 ms, mode picks 513–580 ms |
| Each device keeps its own mode and range | Pass | The C920 on 1920 × 1080 [24 FPS] and Limitada, the ezcap on 1080p60 Auto; switching back and forth kept each; separate `devices."<id>"` entries |
| With the device menu open, unplug and plug the webcam: it leaves within 3 s (SC-004) | Pass | `device removed` 76 ms after Windows marked the C920 absent (Kernel-PnP event 1010), 19 ms after its camera interface was disabled in a second run |
| … and comes back within 3 s (SC-004) | **Miss, known limit** (maintainer, 2026-11-13) | `device added` 5.4 s after the USB arrival (`DEVPKEY_Device_LastArrivalDate`), 4.1 s after the camera interface was enabled (polled with `pnputil /enum-interfaces`, second run). The wait is the system's or the webcam's: a fresh Media Foundation enumeration started at the arrival blocked ~5 s and returned with the C920, against ~1.2 s otherwise ([raw/t058-windows/plug-fresh-enumeration.txt](raw/t058-windows/plug-fresh-enumeration.txt), [plug-interface.txt](raw/t058-windows/plug-interface.txt)); `gst-device-monitor-1.0 -f` alone shows the same delay. Back in the menu it opened in its mode, first frame 377 ms after the add (research R1) |
| Two identical devices (quickstart 8) | Not checked | No two identical devices at the Windows PC (checked on CachyOS with two loopback cameras) |

**T076 on Windows**: the C920's MJPG is `YCbCr-4:2:2` here too, and `mfvideosrc ! image/jpeg ! jpegparse ! jpegdec` decodes it ([raw/t058-windows/t076-c920-mjpg-jpegdec.txt](raw/t058-windows/t076-c920-mjpg-jpegdec.txt)). `d3d12mjpegdec` and `nvjpegdec` are not registered on this PC, so whether `d3d12mjpegdec` takes 4:2:2 could not be checked; the rule stays (research R4). The app skips a decoder that is not there and would use `jpegdec`; it never picks MJPG for the C920 on Windows anyway.

### CachyOS KDE Plasma Wayland — 2026-11-10

**Environment**: as US1 on CachyOS, the monitor now at 60 Hz (KDE's display settings), release
build of `7709c4e` with T076, T077 and the tray re-read fix (working tree), in a new folder
`~/capx/batch` with no settings file. Devices: HD Pro Webcam C920 (USB), and instead of the ezcap,
which is at the Windows PC, a `v4l2loopback` camera ("Virtual Cam", 1920 × 1080 at 60 NV12 from
`videotestsrc`; the module ships with the CachyOS kernel). Driven by the agent over SSH; the
maintainer plugged and unplugged the C920.

| Check | Result | Notes |
|---|---|---|
| Device menu: name, resolution and aspect tags, the mode line, no screens | Pass | |
| Mode menu: Automatic, then Modes largest first | Pass | C920: Auto max resolution, then 1920 × 1080 at 30, 24, 20, 15, 10, 7.5, 5, then 1600 × 896 … |
| Video within 2 s of a switch (SC-004) | Pass | C920 → Virtual Cam ~150 ms, Virtual Cam → C920 508 ms, a mode pick 455 ms (open → first frame in the log) |
| Each device keeps its own mode and range | Pass | Virtual Cam on Limited, the C920 on 1920 × 1080 at 24 with Auto; switching back and forth kept each; separate `devices."<id>"` entries |
| With the device menu open, unplug and plug the webcam: it leaves and comes within 3 s (SC-004) | Pass | Removed 13 ms after the kernel's disconnect, added 243 ms after the kernel found it |
| Two identical devices (quickstart 8) | Pass | Two loopback cameras with the same name: "Virtual Cam" and "Virtual Cam (2)", each with its own id (bus path), the second opens; only one mode each |
| The fade on a switch, spinner only on a slow open | Pass (2026-11-12, no slow open) | ezcap switches 423–640 ms: a screenshot 120 ms after a pick shows the backdrop (X watermark) and no spinner; no open took over 1 s, so the spinner was not seen |
| A capture card (Automatic group from a card, Auto max FPS) | Pass (2026-11-12) | See below |

**Capture card, 2026-11-12**: release build of `1c96d41` (T055–T057) in `~/capx/final`, no
settings file; the ezcap with the PS5 at 1080p60, then also the C920 (it had dropped off the
bus 12 s after boot, "device firmware changed", until the maintainer plugged it in again).
Driven by the agent over SSH; the maintainer unplugged and plugged the C920; log in
[raw/linux-batch/capturax.log](raw/linux-batch/capturax.log).

| Check | Result | Notes |
|---|---|---|
| `--list-devices` (First check 1) | Pass | One entry for the card (no metadata node), automatic `auto-max-resolution` → 2560x1440@60, `auto-max-fps` → 1920x1080@120, no Auto source; modes 3840x2160@30 down to 1280x720@60 |
| First start without settings (First check 2) | Pass | `start: first capture card`, `1920x1080@60` NV12 (FR-008: 1080p at about 60 when the device has it; the quickstart's "Auto max FPS (1080p120)" predates T065 and is corrected), first frame 403–436 ms after the choice in six starts, `range=full(auto)` |
| Mode menu: Automatic group without duplicates, Modes largest first | Pass | "Auto resolução máxima 2560 × 1440 [60 FPS]", "Auto FPS máximo 1920 × 1080 [120 FPS]", then 3840 × 2160 [30] … 1280 × 720 [60], each with name and aspect tags |
| Device menu: name, tags, detail line | Pass | "ezcap Live GAMER ULTRA: ezcap L", `4K UHD` `16:9`, detail "3840 × 2160 [30 FPS]" while in that mode |
| Video within 2 s of a switch (SC-004) | Pass | Auto max FPS 423 ms, 2560 × 1440 @ 60 474 ms, 3840 × 2160 @ 30 640 ms, back to 1080p60 427 ms |
| Each device keeps its own mode and range | Pass | The C920 opened in Auto max resolution (1920 × 1080 [30 FPS], it has no 60 mode); set to 1920 × 1080 [24 FPS] and Limitada; the ezcap came back in 1080p60 Auto, the C920 again in 24 and Limitada; separate `devices."<id>"` entries. Device switches 514 and 385 ms |
| With the device menu open, unplug and plug the webcam: it leaves and comes within 3 s (SC-004) | Pass | Menu open on the ezcap: `device removed` 0.4 s before the kernel's "USB disconnect" line (the video node goes first), `device added` 5 ms after uvcvideo found it; the menu stayed open and showed the C920 again with its mode (24 FPS), now listed after the ezcap |

**Found**: the C920's MJPEG modes failed at once on Linux ("No valid frames decoded"): it sends
4:2:2 JPEG, which `nvjpegdec` rejects. Fixed in T076 (decoder chosen by the sampling, `jpegdec`
for 4:2:2; research R4). A `v4l2loopback` device added while the app runs is not seen by the V4L2
provider (only the PipeWire one, which the app skips): the loopback camera has to exist before
the app starts. A real USB webcam is seen at once.

## Video backdrop (T071)

### Windows 11

Pass (maintainer, 2026-11-10; checked after T070, not recorded at the time).

### CachyOS KDE Plasma Wayland — 2026-11-10

Pass (maintainer). Release build of `5d6bad0` in `~/capx/batch`, the monitor at 119.88 Hz.
Devices: the C920 and a `v4l2loopback` camera ("Virtual Cam", 2560 × 1080 at 60 NV12, SMPTE bars
from `videotestsrc`, 21:9). Checked: every combination of background, watermark and shadow in
both themes; the Virtual Cam fullscreen (the X's tips in the bars); a switch between the C920 and
the Virtual Cam (the X between the fades); a state (the X behind it); the settings after a
restart.

## US3 (T039)

"Manual acceptance" 3, with the agent comparing the bubbles with `--stats` (SC-007, first two
parts). Logs in [raw/t039/](raw/t039/): `run2.txt` (the checks), `run3.txt` (after the restart).

### Windows 11 — 2026-11-08

**Environment**: as US1 on Windows (RTX 5070 Ti, preview on the C24FG7x 144 Hz G-SYNC, windowed,
DX12), release build of `feat: feed the info bubbles` (d28c777). Device: ezcap Live GAMER ULTRA.
Source: the CachyOS PC's HDMI output into the ezcap, 1920 × 1080 at 60 Hz, then 119.88 Hz (KDE
display settings), instead of the PS5: any 60 Hz source serves the "111 / 120" check, and the
PC can also hold 120 Hz. Checked by the maintainer by hand, with screenshots; the agent read the
log at the screenshot times (clock of the log file's start).

| Source → card mode | Frame rate bubble | Latency bubble | `[loop]` at the same seconds | Result |
|---|---|---|---|---|
| 60 Hz → 1080p60 | `60 / 60 FPS` | `2 ms + ~42 ms` | `pres=60`, `lat_ms=2.2–2.3` median | Pass |
| 60 Hz → Auto max FPS (1080p120) | `112 / 120 FPS` | `2 ms + ~25 ms` | `pres=111–112`, `lat_ms=2.1` | Pass |
| 119.88 Hz → 1080p120 | `111 / 120 FPS` | `2 ms + ~25 ms` | `pres=111–112`, `lat_ms=2.1–2.2` | Pass |

The estimate matches the formula with `C_WINDOWS` = 5 ms (before T040) and the 144 Hz monitor:
2 × 16.7 + 5 + 3.5 = 41.8 ms at 60 FPS, 2 × 8.3 + 5 + 3.5 = 25.2 ms at 120 FPS.

| Check | Result | Notes |
|---|---|---|
| Four bubbles: source, mode, frame rate, latency | Pass | Bottom right of the video area, with the chrome |
| Latency displays Measured / Measured + estimated / Total, tooltip | Pass | Maintainer: every choice marked, applied at once |
| Frame rate displays Presented / Presented / nominal / Nominal | Pass | Maintainer |
| Mode displays (resolution, + name, + aspect ratio, + both) | Pass | Maintainer |
| Choices after a restart | Pass | Saved as `[overlay] latency = "total"`, `fps = "nominal"`, `mode = "size+aspect"`; after quit and restart: `~27 ms` (`lat_ms` 2.2 + 25.2), `120 FPS`, `1920 × 1080 [16:9]` |
| Presented within ±1 of `pres`, measured within 1 ms of `lat_ms` | Pass | See the table above |
| 150 %, English | Not checked | |

**Seen during the run, outside US3** (maintainer: stutters, 120 Hz does not feel like 120 Hz,
the chrome not smooth):

- In the 1080p120 mode the ezcap delivers 111–113 frames a second on Windows (`recv`) with a
  60 Hz and with a 119.88 Hz source, as in T029 (latency.md, L3 and L3b); the app presents what
  it receives (`pres` ≈ `recv`). Eight missing frames a second at 120 Hz are visible as judder.
  On CachyOS the same card gives 111–120 (latency.md, T064). So the quickstart's "about 111 /
  120" with a 60 Hz source also holds with a 120 Hz source on Windows.
- Present stalls: in 31 of 396 seconds `ready_ms` max was 17–40 ms (normally ≤ 1.6), some with
  replaced frames (`repl` 1–3), for example t=115, 136–139, 226, 229–230 and eight seconds in a
  row at t=294–301; some with UI activity, some without. These are SC-006's Windows stalls,
  investigated at T052.

### CachyOS KDE Plasma Wayland — 2026-11-12

**Environment**: release build of `ef15524` in `~/capx/t058`, no settings file, the monitor at
119.88 Hz without variable refresh, 100 %, Portuguese. Device: the ezcap with the PS5 at 1080p60
(home screen). Driven by the agent over SSH with screenshots; logs in
[raw/t058-cachyos/](raw/t058-cachyos/).

| Source → card mode | Frame rate bubble | Latency bubble | `[loop]` at the same seconds | Result |
|---|---|---|---|---|
| 60 Hz → 1080p60 | `60 / 60 FPS` | `10 ms + ~20 ms` | `pres=60`, `lat_ms=9.6–9.7` median | Pass |
| 60 Hz → Auto max FPS (1080p120) | `120 / 120 FPS` (once `116 / 120`) | `10 ms + ~20 ms`, Total `~30 ms` | `recv=120 pres=120`, `lat_ms=9.6` | Pass |

On Linux the ezcap delivers 120 frames a second in the 1080p120 mode with the PS5 at 60 Hz
(duplicated frames), so the quickstart's "about 111 / 120" is the Windows driver's figure (T039
above). The estimate is `C_LINUX` 16.3 + half the 119.88 Hz refresh 4.2 = 20.5 ms at either rate
(no frame period term on Linux); Total `~30 ms` against L4's 30.1 ms.

| Check | Result | Notes |
|---|---|---|
| Four bubbles: source, mode, frame rate, latency | Pass | Bottom right of the window, over the side bar of the letterbox |
| Latency tooltip | Pass | "10 ms: dentro do CapturaX (medido) / ~20 ms: placa de captura e monitor (estimado)" |
| Hover settings button of each bubble with its displays | Pass | Latency: Medida / Medida + estimada / Total; frame rate: Exibidos / Exibidos / nominal / Nominal; mode: four displays |
| Choices after a restart | Pass | `[overlay] latency = "total"`, `fps = "nominal"`, `mode = "size+aspect"`; after the restart `~30 ms`, `120 FPS`, `1920 × 1080 [16:9]`, the ezcap again in Auto max FPS |
| 150 %, English | Pass | See "Full acceptance (T058)" |

## System notifications (T061, research R15)

`cargo run -p capturax-platform --example notify -- <seconds> [<replace after>]`; the maintainer
watched and clicked. Both passed on 2026-11-09.

### Windows 11 — 2026-11-09

Unpacked build.

| Check | Result | Notes |
|---|---|---|
| Toast shows with the name CapturaX and the X icon | Pass | Key `HKCU\Software\Classes\AppUserModelId\CapturaX` and `%LOCALAPPDATA%\CapturaX\notification.ico` written on the first notice |
| **Open** reaches the app with the notice's tag | Pass | `example:first` from the first toast, `example:second` from the second |
| A new notice replaces the previous one | Pass, with a delay | Windows shows one popup of an app at a time: the first goes away at once, the second comes in after the exit animation |

- First run: no toast. `Setting()` failed with 0x80070490 (no Start menu shortcut) while `Show`
  works; an unreadable setting no longer stops the toast.
- Replacing a toast still on screen (`-- 15 3`): hiding the previous and then showing the new
  one, or a shared `Tag` and `Group`, brought the second popup in late; with no hide at all the
  second waited for the first's whole life (not stacked). Showing the new one and then hiding
  the previous was the quickest, still a short delay the system imposes (research R15).
- The first example exited on the first action and replaced after 2 s, which looked erratic; it
  now prints every action and takes the replace delay as an argument (8 s by default).

### CachyOS KDE Plasma Wayland — 2026-11-09

Run over SSH in the desktop session (`DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`).

| Check | Result | Notes |
|---|---|---|
| Notification shows with the name CapturaX and the X icon | Pass | Icon from the `image-data` hint, no icon file |
| **Open** reaches the app with the notice's tag | Pass | `example:first` at 2.3 s, `example:second` at 9.2 s |
| A new notice replaces the previous one | Pass | With 3 s the second took the first's place at once |

- With `replaces_id`, a second notice sent after the first had left the screen (8 s) only
  updated the history entry, with no popup. The previous notice is now closed
  (`CloseNotification`) and the new one sent as new; both popups then showed (research R15).

## US4 (T046)

"Manual acceptance" 4 and 6; the maintainer tried each case by hand, the agent read the app's
log (`capturax.log` next to the build) at the times given.

### Windows 11 — 2026-11-10

**Environment**: as US3 on Windows, release build of `a8f44a7` plus the fixes below (working
tree). Devices: HD Pro Webcam C920 (USB) and ezcap Live GAMER ULTRA (inside the PC, cannot be
moved to another port). Window 640 × 360 and larger, Portuguese and English.

| Check | Result | Notes |
|---|---|---|
| Start with no device → "No devices", device menu the same; plug one in → it opens | Pass | `NoSource -> NoDevices (monitor list, 0 devices)`, the webcam plugged in opened by itself, balloon "connected" with no **Open** |
| Unplugging every device while one plays | Pass | "Disconnected" naming the open device, which is waited for (FR-014); the quickstart said "No devices" here and was corrected |
| Camera privacy off while playing → "Access blocked"; **Open privacy settings**; on again → plays | Pass | `Playing -> Blocked` 1 s after the stream error (`access Denied`); after fix 3 the device reopens by itself within 3 s (`camera access is back, opening the device again`), **Try again** still works |
| Privacy off while "disconnected" | Pass after fixes 1 and 2 | Before: nothing changed, and access back gave "No devices". After: "Access blocked" (device not named), back on → "disconnected" again, or the device reopens in the same mode when plugged in meanwhile (`Blocked -> Opening (monitor list, 1 devices)`, "reconnected" notice) |
| Card open in OBS → "In use"; **Try again** after closing OBS plays | Pass | The Windows in-use mapping (research R6) read it right |
| Unplug while playing → "disconnected"; plug back → video, same mode | Pass | C920; first frame 377 ms after the device came back, 1920 × 1080 at 30 |
| Moved to another USB port | Pass | C920: reopened with its stored choices (its identity follows it); the ezcap cannot be moved |
| No HDMI signal → "No signal" after 5 s | Not checkable | The ezcap sends its own "no signal" picture as video, the C920 always has a picture (maintainer: no device here can give it) |
| Notices with the window focused (FR-027): connected with **Open**, disconnected, × and click close, quick plug and unplug → one balloon, open card → disconnected then reconnected with no button | Pass | |
| Notices with another app focused, minimized, in the tray → system notifications, **Open** shows the window and opens the device | Pass | `notice: … -> system` |
| Notice placement (FR-027, FR-032): default bottom left, each corner at once, bottom right in the bubble row, 640 × 360 | Pass | |
| Settings notices (FR-033): damaged file or read-only folder → warning balloon, whole text, stays until closed | Pass | |
| Close to the tray → webcam light off; show → reopens | Pass | |
| Minimize and restore → video at once | Pass | The device stays open while minimized (spec edge case) |
| Sleep and wake → the preview comes back | Pass | Maintainer; no `woke from sleep` line in today's log, so the session lived through it (the wake path was seen on 2026-11-10 03:23, `woke from sleep, opening … again`) |
| English | Pass | States and notices |
| 150 % | Pass | Maintainer, a quick look |

**Found and fixed during the run** (spec and research updated first where the behavior changed):

1. Camera access back restarts the device monitor (T043), and its first list went through the
   start order: "disconnected" became "No devices". A restarted monitor's list is now read as a
   re-read, so the device is still waited for.
2. Privacy turned off while "disconnected" changed nothing. Maintainer (2026-11-10): the blocked
   state replaces it without naming the device; back on → "disconnected", or the device reopens
   if it was plugged in meanwhile (spec edge case).
3. Maintainer (2026-11-10): while a listed device is blocked, camera access is read every 3 s and
   access turned back on reopens it once; **Try again** stays (Linux nodes, unknown access, the
   R6 mapping) (spec US4 scenario 2, FR-021, research R1).
4. The toolbar's icon-look select (window below about 892 px) kept its tooltip after a pick with
   the pointer, even once the controls hid: the menu gives the focus back to the select. The
   focus now shows the tooltip only while the keyboard is in use; a pointer pick shows the new
   value at once, until the pointer leaves the select or 1.5 s with it elsewhere (maintainer).

### CachyOS KDE Plasma Wayland — 2026-11-10

**Environment**: as US2 on CachyOS (C920 and the loopback camera, `~/capx/batch`). The
maintainer plugged and unplugged the C920 and pressed **Open** on system notifications and the
tray icon; the agent drove the rest over SSH and read the log.

| Check | Result | Notes |
|---|---|---|
| Start with no device → "No devices", device menu the same; plug one in → it opens | Pass after T076 | The C920 was seen at once and opened by itself, "connected" balloon with no **Open**; the open failed on its 4:2:2 MJPEG until T076 |
| `chmod 000` on the node of a listed device → blocked state naming the node; **Try again** after restoring | Pass | "Não foi possível abrir /dev/video0 · Sem permissão para usar o dispositivo"; **Try again** played 505 ms later. With the node blocked before the app starts the device is not listed at all ("No devices"): the V4L2 provider cannot probe it |
| Card open in `gst-launch-1.0` → "In use"; **Try again** after closing it | Pass | `mapped to InUse` 1.4 ms after `Playing` |
| Unplug while playing → "disconnected"; plug back → video within 3 s, same mode | Pass | 1920 × 1080 at 24 again, first frame 366 ms after the device came back (0.6 s after the kernel saw it) |
| Moved to another USB port | Pass (as designed) | The C920 came back as a new device (Linux keeps the bus path in the key even with a serial): "connected" with **Open**, the old one still "disconnected" (maintainer: accepted, research R2) |
| No HDMI signal → "No signal" after 5 s | Not checkable | No card here |
| Notices with the window focused: disconnected, connected with **Open** (opens it), ×, a click on the balloon, quick plug and unplug → one balloon, open device → disconnected then reconnected | Pass | The balloon stays while the pointer moves over it (9 s), a click on its text closes it without opening |
| Notices with another app focused, minimized, in the tray → system notifications; **Open** shows the window and opens the device | Pass after T077 | The device opened but the window stayed behind (Wayland: winit's `focus_window` does nothing); T077 activates it with the notification's XDG activation token: to the front, out of minimized, out of the tray |
| Tray icon → window to the front | Pass after T077 | Token from the tray host (`ksni`) |
| Notice placement: four corners, 640 × 360 with the controls shown | Pass | With the settings notice (read-only app folder): clear of the toolbar and the dock, above the bubbles |
| Settings notice (read-only folder) | Pass | Warning icon, title, whole text with the fallback folder, stays |
| Close to the tray → webcam light off; show → reopens | Pass | Session closed in 96–131 ms; the C920 opened in 24 again from the tray icon |
| Minimize and restore | Pass | |
| Sleep and wake → the preview comes back | Pass | `woke from sleep: 21 s`, first frame 401 ms after reopening |
| English | Pass | |
| 150 % | Not checked | The CachyOS monitor runs at 100 % |

**Found and fixed during the run**:

1. T076: the JPEG decoder is chosen by the JPEG's chroma subsampling (above, US2).
2. T077: **Open** on a system notification and the tray icon did not bring the window to the
   front on Wayland. The app now activates its surface with the XDG activation token the
   notification service and the tray host send with the click (research R15).
3. In the tray, a disconnected device that came back kept the 3 s list re-read running (and
   logging) until the window was shown. A device listed again in the tray now only waits for the
   window (research R1).

Seen, not the app's: the C920 sometimes resets itself on its first stream after being plugged in
(`device firmware changed`, a USB disconnect and reconnect within 2.3 s); the app showed
"disconnected", then "reconnected", and played 3.4 s after the click.

## US5 (T049)

"Manual acceptance" 5: the Presentation setting (Settings → Preview). Logs in
[raw/t049/](raw/t049/).

### Windows 11 — 2026-11-11

Pass (maintainer). Release build of `e466c04`, Direct3D 12, the C24FG7x with G-SYNC. With
**Follow the monitor** and the app focused the maintainer saw the difference on the monitor
(the cursor smooth). `--stats` (`windows.txt`, 640 × 360 window, a 60 FPS source): the header
`mode=fifo-follow` on the switch, then `pres=60` and `rep=84` a second (the monitor running above
the source's rate), `lat_ms` median 6.2–6.3 against 2.4–2.7 in Mailbox; back to Lowest latency
the header `mode=mailbox` and `rep=0`. No Opening state, no session change on either switch.
Seen: each switch costs one late present (switch to Follow `gap_ms` 63.7; back to Mailbox
`ready_ms` max 59.4, `gap_ms` 48.9), the surface being reconfigured; only on the switch.

### CachyOS KDE Plasma Wayland — 2026-11-11

Pass (maintainer). Release build of `e466c04` in `~/capx/batch`, Vulkan, RTX 3050, the monitor at
119.88 Hz without variable refresh; the loopback camera ("Virtual Cam", 2560 × 1080 at 60).
`--stats` in a Konsole (`cachyos.txt`): the header `mode=fifo-follow` on the switch, then
`pres=60 rep=54–61` a second (one repeat per frame at 119.88 Hz), `lat_ms` median 19.7–24.6
against 18.9–19.0 in Mailbox (the loopback camera's `lat_ms` includes its feeder); back to Lowest
latency `mode=mailbox`, `rep=0` from the next second. No Opening state, no session change, no late
present on either switch (`gap_ms` max 25.7). Without variable refresh the cursor shows no
difference here, as expected.

## Full acceptance (T058)

The whole quickstart (SC-008), building on the per-story records above; this section has what
they did not cover (150 %, English, the card in places it was not used) and the known limits.

### CachyOS KDE Plasma Wayland — 2026-11-12

**Environment**: as US3 on CachyOS above (`ef15524`, the ezcap with the PS5). KDE's scale set to
150 % for the run (`kscreen-doctor`, back to 100 % afterwards), the app started with `--lang en`.

| Check | Result | Notes |
|---|---|---|
| US1: restart → same device and mode | Pass | The ezcap again in Auto max FPS, first frame 430–437 ms after the choice |
| 150 %, English: toolbar, device and mode menus, bubbles | Pass | "Device", "Mode", "Auto max FPS", "Color range" "Auto (Full)", "Automatic" / "Modes" groups; tags and text crisp, nothing cut; the mode menu scrolls in the 960 × 540 window |
| 150 %, English: settings panel | Pass | General, Appearance, Preview (Video background, Watermark, Video shadow, Presentation "Lowest latency" / "Follow the monitor"), About |
| 150 %: fullscreen (F11) and back | Pass | |
| Item 6: close to the tray with the card | Pass | `gpu performance released`, session closed in 3 ms, `window hidden in the tray`, P5; shown again → first frame 437 ms, `gpu performance requested`, P0 |
| Item 6: sleep and wake with the card | Pass | `systemctl suspend` from the agent, the maintainer woke the PC: `woke from sleep: 22 s since the last tick`, the ezcap opened again, first frame 390 ms after the choice, back to Playing (behind KDE's lock screen) |

Seen: a second start from a shell (no activation token) shows the hidden window but cannot raise
it on Wayland ("its window was shown but could not come to the front"); the tray icon and
notifications carry a token and raise it (T077).

### Windows 11 — 2026-11-13

**Environment**: as US2 on Windows above (`a36e8db`, `target/t058w`, the ezcap and the C920).
The window on the Arzopa monitor below the central one, set to 150 % by the maintainer (opened
there at 1800 × 993 physical, 1200 × 662 logical), the app started with `--lang en`. Checked by
the maintainer by hand; log in [raw/t058-windows/scale150-en.log](raw/t058-windows/scale150-en.log).

| Check | Result | Notes |
|---|---|---|
| 150 %, English: toolbar, device and mode menus, bubbles | Pass | |
| US1: Color range Auto shows `Auto (Full)`, Full and Limited change the image at once | Pass | On the C920 and the ezcap |
| US2 at 150 %: device and mode switches | Pass | First frame 470–518 ms after each choice (the C920, the ezcap in Auto max FPS, 2560 × 1440 @ 60, 1080p60) |
| US3: the 1080p120 mode with a 60 Hz source reads about "111 / 120 FPS"; each bubble's hover settings button offers its three displays; the choice survives a restart | Pass | |
| 150 %, English: settings panel | Pass | General, Appearance, Preview, About |
| 150 %: fullscreen (F11) and back | Pass | |
| Item 7: snap to the left half, F11, F11 → back in the left half (FR-025) | Pass | |
| The ezcap's identity | Read | A USB composite device with the serial `00000001` (`USB\VID_32ED&PID_3201\00000001`) on port 7 of the card's own USB controller; the id the app keeps is the camera interface path (`mf:\\?\usb#vid_32ed&pid_3201&mi_00#9&37ee7830&0&0000#…`), whose `9&37ee7830` prefix Windows derives from that parent. Caps recorded in `crates/capture/tests/data/ezcap-windows.txt` |

### Known limits

- **The ezcap Live GAMER ULTRA is an internal PCI Express card** (the system sees a UVC device on
  the card's own USB controller): it cannot be unplugged while running, so "disconnected",
  "reconnected" and "another port" are checked with the C920 (US4).
- **The ezcap never reports "No signal"**: without an HDMI input it sends its own video reading
  "EZCAP CONNECT SOURCE", which the app shows as video. The "No signal" state (5 s without
  frames) is not checkable with the devices at hand.
- No Auto source mode on UVC cards (the ezcap exposes fixed modes only).
- Windows "in use" versus "failed" is a heuristic (research R6); it read OBS right in US4.
- The ezcap's identity: on Linux it carries the serial `00000001` and its bus path; on Windows
  the camera interface path under its serial-numbered USB parent (`00000001`). Being internal,
  its port never changes, so its choices always follow it.
- **A webcam plugged in on Windows reaches the menu in about 4–5 s** (C920: 4.1 s after its
  camera interface is enabled, 5.4 s after the USB arrival), missing SC-004's 3 s; the device is
  not readable sooner (a fresh Media Foundation enumeration blocks until then). Removal is
  immediate, and on Linux the same webcam arrives in 0.2–0.5 s (research R1; maintainer
  accepted, 2026-11-13).
- On Windows no hardware MJPEG decoder is registered on the maintainer's PC; MJPG goes to
  `jpegdec` (research R4), and webcams with the Camera Frame Server get NV12 anyway.
- System notifications are hidden by the system while another app is fullscreen (R15).
- On Linux the ezcap gives 120 FPS in the 1080p120 mode with a 60 Hz source; on Windows about
  111 (driver), visible as judder (T039).
