# Data Model: Device Preview

Entities from the spec's Key Entities, with fields, rules and state changes. The settings file
additions are in [contracts/settings-file.md](contracts/settings-file.md); the capture crate's
interface in [contracts/capture.md](contracts/capture.md).

## Device

Crate `capture`, built from a `gst::Device` of the preferred provider (research R1).

| Field | Type | Rule |
|---|---|---|
| `id` | string | Stable key (research R2): `mf:<device.path lowercased>` or `v4l2:<vid>:<pid>:<serial>@<bus_path>` |
| `name` | string | `display_name`; numbered " (2)", " (3)" when repeated (FR-003) |
| `modes` | list of Mode | From caps, one per resolution × frame rate, best format (research R3); sorted largest first |
| `signal` | `Option<Signal>` | The input the device reports (resolution, frame rate); always none in this spec (research R3) |
| `gst` | `gst::Device` | Kept to create the source element; not stored |

- **Matching a remembered id**: exact `id` only; a card without a serial moved to another port
  is a new device (spec FR-003).
- A device not from the preferred provider is never listed.

## Mode

| Field | Type | Rule |
|---|---|---|
| `width`, `height` | u32 | Reported by the driver |
| `fps` | fraction (num, den) | 59.94 kept as 60000/1001; compared within 0.5 |
| `format` | enum (NV12, YUY2, UYVY, I420, BGRx, RGBA, MJPG, Other(name)) | Best of the device's formats for this size and rate (research R3) |

- **Label** (maintainer, 2026-11-07, mockup variant F1): the size as text, `1920 × 1080`, followed
  by the frame rate in an outlined tag, `[60 FPS]` (rate rounded unless fractional: `59.94 FPS`);
  no separator character. The format is not shown in the menu, it is in the stats header and the
  log. "FPS" is upper case in every text of the app (labels, bubbles).
- **Tags** (F43; maintainer, 2026-11-07, chosen on a mockup page): every mode row reads
  `1920 × 1080 [60 FPS]` on the left and shows two filled tags on the right, `[Full HD] [16:9]`.
  An automatic row has its name as text and `1920 × 1080 [60 FPS]` as its detail line; a device
  row's detail line is the automatic choice's name, if any, then the same size and rate tag
  ("Auto max resolution · 1920 × 1080 [30 FPS]"). The select button shows the size, the rate tag
  and the name tag (the aspect tag only in the menus); it elides the text when narrow.
- **Text output** (`--list-devices`, the log): the settings text of the mode (`1920x1080@60`,
  `1920x1080@59.94`, ASCII, the same as `mode` in the settings file and `[source] mode=`), then
  the format and the tags in brackets, in aligned columns: `1920x1080@60   NV12  [Full HD] [16:9]`
  (maintainer, 2026-11-07: a line can be copied into the settings file as is).
  - **Name tag** (the old CapturaX rule): the standard of the same size; else the nearest standard
    at or above: the smallest standard height ≥ the mode's height, among those the narrowest width
    ≥ the mode's width, else the widest of that height; a height above the tallest standard or
    below the shortest → `Custom` (same text in both languages). Its fill is its standard's ratio
    family (below); `Custom` is neutral.
  - **Aspect tag**: the real ratio, width:height reduced by their greatest common divisor; when it
    is not one of the listed ratios, the nearest listed ratio within 3% (relative difference of
    width / height) is shown instead (`2560 × 1080` → `21:9`, `1366 × 768` → `16:9`); else the
    reduced fraction. Listed ratios: 1:1, 5:4, 11:9, 4:3, 3:2, 25:16, 16:10, 5:3, 16:9, 17:9,
    21:9, 32:9. Neutral fill.
  - **Fills**: our own, one per ratio family, white text at 4.5:1 or more, the same in both
    themes (palette tokens, checked by app/tests/contrast.rs):

    | Family | Fill | Family | Fill | Family | Fill |
    |---|---|---|---|---|---|
    | 1:1 | `#9c3d2e` | 3:2 | `#0f7a6e` | 16:9 | `#6d6bbb` |
    | 5:4 | `#8f6a00` | 25:16 | `#4b5f8a` | 17:9 | `#2f7d32` |
    | 11:9 | `#6b4f3a` | 16:10 | `#756f92` | 21:9 | `#be185d` |
    | 4:3 | `#5d6b25` | 5:3 | `#c2410c` | 32:9 | `#7e3fbf` |
    | Custom | `#62666f` | aspect tag | `#4a4d55` | | |

  - **Standards** (name, width × height), from the old CapturaX list, corrected (1600 × 900 is
    HD+, not WSXGA; 2560 × 1440 is WQHD) and completed:

    | | | | |
    |---|---|---|---|
    | 8K FF 8192 × 4320 | 8K UHD 7680 × 4320 | 5K UHD 5120 × 2880 | WQUXGA 3840 × 2400 |
    | QUXGA 3200 × 2400 | DQHD 5120 × 1440 | DFHD 3840 × 1080 | 5K UW 5120 × 2160 |
    | 4K DCI 4096 × 2160 | 4K UHD 3840 × 2160 | WQSXGA 3200 × 2048 | QSXGA 2560 × 2048 |
    | SQFHD 1920 × 1920 | QHD+ 3200 × 1800 | WQXGA 2560 × 1600 | QXGA 2048 × 1536 |
    | UWQHD 3440 × 1440 | WQHD 2560 × 1440 | WUXGA 1920 × 1200 | UXGA 1600 × 1200 |
    | UWFHD 2560 × 1080 | 2K DCI 2048 × 1080 | Full HD 1920 × 1080 | HDV 1440 × 1080 |
    | WSXGA+ 1680 × 1050 | SXGA+ 1400 × 1050 | SXGA 1280 × 1024 | QuadVGA 1280 × 960 |
    | HD+ 1600 × 900 | WXGA+ 1440 × 900 | XGA+ 1152 × 864 | WXGA 800 1280 × 800 |
    | FWXGA 1366 × 768 | WXGA 1280 × 768 | XGA 1024 × 768 | HD 1280 × 720 |
    | WSVGA 1024 × 600 | SVGA 800 × 600 | WSVGA PAL 1024 × 576 | PAL 768 × 576 |
    | SD 576p 720 × 576 | qHD 960 × 540 | FWVGA 854 × 480 | WVGA 800 × 480 |
    | SD 480p 720 × 480 | VGA 640 × 480 | nHD 640 × 360 | HVGA 480 × 320 |
    | SIF 384 × 288 | CIF 352 × 288 | QVGA 320 × 240 | CGA 320 × 200 |
    | QCIF 176 × 144 | QQVGA 160 × 120 | SQCIF 128 × 96 | |

## Mode choice

What the user chose for a device; stored per device id.

| Value | Resolves to |
|---|---|
| `AutoSource` | The device's `signal` matched to a mode; hidden while `signal` is none |
| `AutoMaxResolution` | Largest resolution at 58–62 fps, highest such rate; none at 58+ → largest resolution at the device's highest rate |
| `AutoMaxFps` | Highest rate at 1080p; else at the largest resolution below 1080p; else at the smallest above |
| `Manual(w, h, fps)` | The device's mode with that size and rate; missing → the default mode (spec edge case) |

- **Default** (FR-008): `AutoSource` when shown; else `Manual(1920, 1080, r)` with `r` the
  device's highest rate within 58–62 fps at 1920 × 1080 (60 before 59.94), when it has one;
  else `AutoMaxResolution`. Nothing is stored until the user chooses (a device with no stored
  choice follows this rule each time it opens).
- **Shown automatic modes**: in the order above, skipping one that resolves to the same mode as
  an earlier one (FR-007).
- Stored as text: `auto-source`, `auto-max-resolution`, `auto-max-fps`, `1920x1080@60`,
  `1920x1080@59.94`.

## Color range choice

Per device id: `Auto` (default), `Full`, `Limited`. The range in use (research R5):

```text
Full / Limited chosen ─► that range
Auto ─► declared range (Limited when undeclared)
           │ a sampling with ≥ 0.1% luma < 16
           ▼
         Full (until the device or mode changes)
```

The color range select shows `Auto (Full)` / `Auto (Limited)` with the range in use.

## Frame

Crate `capture`, handed to the video thread (replaces the shell's RGBA-only `VideoFrame`).

| Field | Type | Rule |
|---|---|---|
| `width`, `height` | u32 | |
| `format` | NV12, YUY2, UYVY, I420, BGRx, RGBA | Anything else was converted to NV12 in the pipeline (research R4) |
| `planes` | up to 3 byte buffers, rows packed | Buffers come from a reuse pool |
| `matrix` | BT.601 or BT.709 | From the colorimetry; unknown → BT.709 at 720p and above, BT.601 below |
| `range` | Full or Limited | The range in use (above), set by the session |
| `captured` | `Instant` | `now − (clock.time − (pts + base_time))` (research R11) |
| `ready` | `Instant` | When the appsink handed it over (frame ready → presented, app shell SC-001) |

## Preview state

App `preview.rs`. What the video area shows (FR-019); one at a time.

| State | Shown | Actions |
|---|---|---|
| `NoDevices` | state "No devices" + hint | opens the device menu |
| `Opening(device)` | the old video fading out, then the backdrop; after 1 s the spinner alone (no text, research R17) | — |
| `Playing` | video | — |
| `Blocked(device)` | "Camera access blocked" (Windows) / "Cannot open /dev/videoN" (Linux) | Windows: **Open privacy settings**; **Try again** |
| `InUse(device)` | "In use by another app" | **Try again** |
| `Disconnected(device)` | "<name> disconnected" | opens the device menu (waits for it meanwhile) |
| `NoSignal(device)` | "No signal" | **Try again**; mode menu stays reachable |
| `Failed(device)` | "Cannot open <name>" | **Try again** |
| `NoSource` | the shell's "No source": devices exist but none is open (not reached at start, which always opens one; kept for later specs that close the preview, F10) | opens the device menu |

```text
start ─► devices? ── none ─► NoDevices ── added ─► Opening(chosen, FR-005 order)
             │ some
             ▼
          Opening(device) ── first frame ─► Playing
             │ error                           │ pipeline error / removed
             ▼                                 ▼
  Blocked │ InUse │ NoSignal │ Failed      Disconnected ── same id added ─► Opening
             │ Try again                       │ user picks another device
             ▼                                 ▼
          Opening                           Opening(other)

Playing ── user picks mode / device ─► Opening
Playing ── window to tray ─► (device closed) ── window shown ─► Opening
any ── system sleep → wake errors ─► Disconnected ─► Opening when listed again
```

- **Start order (FR-005)**: the last used device if listed; else the most
  recently used listed device; else the first capture card; else the first webcam. Capture card
  vs webcam: unknown on both platforms in general, so "first" is the list order with names
  containing "cam" last (heuristic, logged).
- The last used device is saved when a device reaches `Playing` after a user choice, not when the
  app falls back to another (spec edge case: the remembered choice is kept for the next start;
  when it is plugged in later it only appears in the menu, no automatic switch).

## Device notice (FR-027)

- **Kind**: `Connected` (with **Open**), `Reconnected` (the open device came back and reopens by
  itself), `Disconnected`. Only after the monitor's first `Listed`.
- **Where**: `InApp` when the window has focus, else `System` (not focused, minimized, hidden to
  the tray).
- **One at a time**: a new notice replaces the one showing (in-app: the single balloon shows the
  latest message; system: `replaces_id` on Linux, the previous toast hidden on Windows).
- **Closing (in-app)**: after about 5 s (not while hovered), on the × button, or on a click
  anywhere on the balloon; **Open** opens the device and closes it.
- **Open**: same as picking the device in the menu; from a system notification it also shows the
  window.

## Preview measurement

| Value | Source | Shown |
|---|---|---|
| presented fps | video thread, presents with a new frame per second | frame rate bubble, `pres` |
| nominal fps | the open mode | frame rate bubble |
| measured latency | median of `presented − captured` per second | latency bubble, `lat_ms` |
| estimated latency | estimate below | latency bubble |

### Latency estimate

Pure function in `app` (unit tested), inputs: platform, source frame period `F` (from the mode),
monitor refresh period `R`, variable refresh engaged (`vrr`, true while focused with the
**Lowest latency** setting on a VRR monitor when known, else false).

```text
estimated = (Windows ? F : 0)       # the frame arriving from the card, before the measured part
          + C_platform              # card processing and the monitor's own delay, calibrated
          + (vrr ? 0 : R / 2)       # average wait for the next refresh
```

The card sends a frame over USB while it receives it from HDMI, so the frame arrives at the
host about one source frame period after it starts: on Windows that period comes before the
measured part (which starts when the whole frame reaches the app), on Linux it is inside it
(the driver stamps the first packet). The first version counted one period for HDMI and, on
Windows, one more for USB; the calibration (T040, measurements/latency.md) found one period too
many on Windows, and the same reasoning applies to Linux, where the L4 run predates the measured
part and the check waits for the Linux batch (maintainer, 2026-11-08).

`C_platform` started at 5 ms on both platforms and is set from the first glass-to-glass runs
(quickstart "Latency budget", L1 and L4), so that measured + estimated matches the camera's
median; SC-007 checks it within 5 ms. The display marks the estimate with "~".

**Bubble displays**: `measured` → "3 ms"; `measured+estimated` (default) → "3 ms + ~24 ms";
`total` → "~27 ms". Frame rate: `presented` → "60 FPS"; `presented/nominal` (default) → "111 / 120
FPS"; `nominal` → "120 FPS". Mode: `size` → "1920 × 1080"; `size+name` → "1920 × 1080 [Full HD]";
`size+aspect` → "1920 × 1080 [16:9]"; `size+name+aspect` (default) → "1920 × 1080 [Full HD] [16:9]".

## Settings additions

Crate `settings`, format version stays 1 (new tables only, read leniently; contracts/settings-file.md).

| Category | Field | Type | Default |
|---|---|---|---|
| `preview` | `device` | device id | none |
| `preview` | `recent` | list of device ids, most recent first, at most 8 | empty |
| `preview` | `presentation` | `lowest-latency` \| `follow-monitor` | `lowest-latency` |
| `preview` | `background` | `theme` \| `black` | `theme` |
| `preview` | `watermark` | bool | `true` |
| `preview` | `shadow` | `always` \| `ambient` \| `off` | `always` |
| `overlay` | `latency` | `measured` \| `measured+estimated` \| `total` | `measured+estimated` |
| `overlay` | `fps` | `presented` \| `presented/nominal` \| `nominal` | `presented/nominal` |
| `overlay` | `mode` | `size` \| `size+name` \| `size+aspect` \| `size+name+aspect` | `size+name+aspect` |
| `devices."<id>"` | `name` | string (for people reading the file) | — |
| `devices."<id>"` | `mode` | mode choice text | default mode |
| `devices."<id>"` | `color_range` | `auto` \| `full` \| `limited` | `auto` |

Unknown values fall back to their defaults; at most 32 device entries are kept (the least
recently used is dropped).

## Video backdrop

What the video thread draws under the video (FR-028 to FR-030, research R18), bottom to top:

1. **Ground**: the theme's deepest tone, or black (`preview.background`).
2. **Watermark** (`preview.watermark`): the X of `brand/X.svg`, in the text tone of the ground
   (white on the dark theme and on black, black on the light theme), at 6% opacity (12% where the
   ambient light shines, spec #6); its height is the video area's height, its bottom on the area's
   bottom, its right side cut by the area's right edge by 12% of its width.
3. **Ambient light** (spec #6).
4. **Shadow** (`preview.shadow`): around the letterboxed video, black, soft (a 24 px blur at 35%
   and a 64 px blur spread 4 px at 20%; half as strong on the light theme's ground; maintainer,
   2026-11-07, softer than the mockup), with **Always**, or with **With ambient light** while the
   ambient light is on.
5. **Video**, with its fade (research R17); then the UI layer, whose state messages leave the
   ground visible (no fill of their own).
