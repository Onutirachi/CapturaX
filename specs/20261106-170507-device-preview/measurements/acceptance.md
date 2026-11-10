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

### Windows 11

Not recorded yet.

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
| The fade on a switch, spinner only on a slow open | Not checked | Switches took at most 0.5 s |
| A capture card (Automatic group from a card, Auto max FPS) | Not checked | Needs the ezcap |

**Found**: the C920's MJPEG modes failed at once on Linux ("No valid frames decoded"): it sends
4:2:2 JPEG, which `nvjpegdec` rejects. Fixed in T076 (decoder chosen by the sampling, `jpegdec`
for 4:2:2; research R4). A `v4l2loopback` device added while the app runs is not seen by the V4L2
provider (only the PipeWire one, which the app skips): the loopback camera has to exist before
the app starts. A real USB webcam is seen at once.

## Video backdrop (T071)

### Windows 11

Pass (maintainer, 2026-11-10; checked after T070, not recorded at the time).

### CachyOS KDE Plasma Wayland

Not run yet.

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
