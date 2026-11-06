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

- **Label**: `1920 × 1080 · 60` (frame rate rounded unless fractional: `59.94`); the format is not
  shown in the menu, it is in the stats header and the log.
- **Badge** (F43): by height, ≥ 2160 4K, ≥ 1440 QHD, ≥ 1080 Full HD, ≥ 720 HD, none below.

## Mode choice

What the user chose for a device; stored per device id.

| Value | Resolves to |
|---|---|
| `AutoSource` | The device's `signal` matched to a mode; hidden while `signal` is none |
| `AutoMaxResolution` | Largest resolution at 58–62 fps, highest such rate; none at 58+ → largest resolution at the device's highest rate |
| `AutoMaxFps` | Highest rate at 1080p; else at the largest resolution below 1080p; else at the smallest above |
| `Manual(w, h, fps)` | The device's mode with that size and rate; missing → the default automatic mode (spec edge case) |

- **Default**: `AutoSource` when shown, else `AutoMaxFps` (FR-008).
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
| `Opening(device)` | spinner state, no old frame | — |
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
  app falls back to another (spec edge case: the remembered choice comes back when it reappears,
  unless the user picked another).

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
estimated = F                       # the card receiving the whole frame from HDMI
          + (Windows ? F : 0)       # USB transfer, already inside the measured part on Linux
          + C_platform              # card processing and the monitor's own delay, calibrated
          + (vrr ? 0 : R / 2)       # average wait for the next refresh
```

`C_platform` starts at 5 ms on both platforms and is set from the first glass-to-glass runs
(quickstart "Latency budget"), so that measured + estimated matches the camera's median; SC-007
checks it within 5 ms. The display marks the estimate with "~".

**Bubble displays**: `measured` → "3 ms"; `measured+estimated` (default) → "3 ms + ~24 ms";
`total` → "~27 ms". Frame rate: `presented` → "60 fps"; `presented/nominal` (default) → "111 / 120
fps"; `nominal` → "120 fps".

## Settings additions

Crate `settings`, format version stays 1 (new tables only, read leniently; contracts/settings-file.md).

| Category | Field | Type | Default |
|---|---|---|---|
| `preview` | `device` | device id | none |
| `preview` | `recent` | list of device ids, most recent first, at most 8 | empty |
| `preview` | `presentation` | `lowest-latency` \| `follow-monitor` | `lowest-latency` |
| `overlay` | `latency` | `measured` \| `measured+estimated` \| `total` | `measured+estimated` |
| `overlay` | `fps` | `presented` \| `presented/nominal` \| `nominal` | `presented/nominal` |
| `devices."<id>"` | `name` | string (for people reading the file) | — |
| `devices."<id>"` | `mode` | mode choice text | default automatic mode |
| `devices."<id>"` | `color_range` | `auto` \| `full` \| `limited` | `auto` |

Unknown values fall back to their defaults; at most 32 device entries are kept (the least
recently used is dropped).
