# Acceptance (quickstart manual checks)

## T008: ambient light, Windows (RTX 5070 Ti, Windows 11), 2026-11-24

The ezcap at 1080p60 with the PS5 in Astro's Playroom, window 1720 × 720 (bars at the sides),
release build. The maintainer looked; the agent ran the app.

| # | Check | Result |
|---|---|---|
| 1 | The bulb on the left dock lights the bars with the video's side colors; orange chip; tooltip | Pass |
| 2 | Scene changes; Video shadow → With ambient light, toggling the bulb | Pass: the glow follows smoothly, the shadow comes and goes with it |
| 3 | White flash, dark scene, PS5 boot | Pass: no flash. Dark scenes faded to the plain background then; with the gate dropped they show their dark colors (checked again after T008a) |
| 4 | Resize, F11 16:9 (no bars), 4:3 | Pass |
| 5 | Unplug the card; restart with the bulb on (without the hide step, which waits for T016) | Pass |
| 6 | Both themes, Background Theme and Black, watermark on | Pass. The watermark stays drawn over the glow in the theme's colour (dark on the light theme, light on the dark one), so it looks different between themes; the maintainer keeps it so |

Tuning (research R2): the maintainer asked for a stronger glow than CachyOS's alpha 0.9 / gate
0.12. A gain was added, and the maintainer set the five draw values live with temporary sliders
(not kept): alpha 0.9, saturation 1.5, gain 2. Then the maintainer asked that the glow never
disappear: the luminance gate was dropped (FR-003, research R2, T008a), so dark scenes show their
dark colors. In GTA V's daylight the gain blew out (yellow and near white bars, maintainer's
screenshot), so it became a lift (power 0.5, research R2).

The glow updated only 39 times a second at 60 fps here (latency.md); the pacing's minimum went
from 16.6 ms to 15 ms (research R1) and it updates 60 times a second.

## T008e: ambient light intensity, Windows, 2026-11-24

Same setup (ezcap 1080p60, PS5), release build; the maintainer looked, the agent ran the app.

| # | Check | Result |
|---|---|---|
| 1a | The bulb's popover, slider, wheel, 0 %, a move while off, restart, keyboard | Pass. The volume button was checked again on the shared slider button (T008c): unchanged |

The maintainer asked for the bulb without its rays while it does not glow (off, or on at 0 %, as
the volume's icon shows mute at 0 %): `bulb_off.svg`, spec FR-001 updated. Checked: pass.

## T016: hide the preview, Windows, 2026-11-24

The C920 and the ezcap (PS5) on the Windows PC, release build. The maintainer clicked and
looked; the agent ran the app, restarted it and read `capturax.log`.

| # | Check | Result |
|---|---|---|
| 5 | Hidden step: ambient on, hide; toggle the bulb while hidden | Pass: no glow while hidden; the bulb changes the setting |
| 7 | Hide the C920 | Pass: "Preview hidden", webcam light off, the Windows Camera app opens it, sound stops, no bubble, screenshot button disabled, the hotkey refused with its notice |
| 8 | Show again; device, mode and range picks while hidden; **Open** on a "connected" notice | Pass: same device and mode; picks open nothing and the shown preview opens the picked device; **Open** shows the preview with its device |
| 9 | Hidden: unplug and replug the card; tray and back; quit and start | Pass: notices only, nothing reopens, still hidden; after the restart the devices are listed and no session opens (no webcam light) |
| 10 | Hide while opening; no device | Pass: hidden 131 ms into the opening, the session closed in 457 ms. With both devices unplugged and the preview shown, the C920 is waited for ("disconnected") |
| 11 | CPU and GPU, 30 s each | Pass (SC-003), below |

SC-002 (from the log): hiding released the device in 23 ms (ezcap) and 301–457 ms (C920, the
longest while it was opening); showing again gave the first frame in 423–471 ms, as a pick from
the menu (447–493 ms). Once the state became Playing 486 ms after its first frame (a busy main
thread while the window came back from the tray); the video itself shows from the first frame.

SC-003: capturax.exe's CPU (its processor time over the whole machine) and the busiest GPU engine
type (Windows' GPU Engine counters, summed over its engines), 30 one-second samples each:

| State | CPU | GPU |
|---|---|---|
| Preview hidden | 0.00 % | 0.00 % |
| Shown, no device (waiting for the C920) | 0.00 % | 0.00 % |
| Reference: the C920 playing (10 s) | 0.21 % | 0.43 % |

With both cameras unplugged the app waits for the C920 rather than showing "no source" (a third
camera, NVIDIA Broadcast, stays listed), so the waiting state stands for it: no session in both.

## T017: hide the preview, CachyOS (RTX 3050, KDE Plasma Wayland), 2026-11-25

The C920 (and a `v4l2loopback` "Virtual Cam" as the second device) on the CachyOS PC, release
build, window 1720 × 720. The agent ran everything over SSH (ydotool, screenshots, `fuser`,
`pactl`, `capturax.log`); the maintainer watched the webcam light.

| # | Check | Result |
|---|---|---|
| 7 | Hide the C920 | Pass: "Preview oculto" with `device_tv_off`, `/dev/video0` free (session closed in 144 ms), another app opened it (ffmpeg; Cheese is not installed), the app's PipeWire streams gone, no bubble, screenshot button disabled, the hotkey refused (logged `NoImage`); the light went off within 1 s (maintainer) |
| 8 | Show again; mode and range picks while hidden; device pick while hidden; **Open** on a "connected" notice | Pass: same device; picks (1600 × 896 at 30, Limited) saved and opened nothing, the shown preview opened in them (`range=limited`); picking the Virtual Cam kept it hidden; **Open** on "HD Pro Webcam C920 conectado" showed the preview with the C920 |
| 9 | Hidden: unplug and replug (USB unbind/bind); close to the tray and reopen; quit and start | Pass: notices only ("desconectado", "reconectado"), nothing reopened; from the tray and back nothing opened; after the restart both devices listed, no session, `/dev/video0` free |
| 10 | Hide while opening; no device | Pass: hidden 74 ms into the opening, the session closed in 78 ms. With no device (C920 unbound, loopback removed) and the preview shown, the C920 is waited for ("desconectado", as on Windows) |
| 11 | CPU and GPU, 30 s each | Pass (SC-003), below |

SC-002 (from the log): hiding released the device in 88–144 ms; showing gave the first frame in
428–561 ms after the click.

SC-003: `top` (capturax's CPU, one-second samples) and `nvidia-smi pmon` (the process's SM use):

| State | CPU | GPU |
|---|---|---|
| Shown, no device (waiting for the C920) | 0.20 % | 0.93 % |
| Preview hidden | 0.17 % | 0.00 % |

A `v4l2loopback` camera made while the app runs is not listed (known since device-preview), so
the Virtual Cam was made before a restart; the restart doubled as check 9's.
