# Data Model: Preview Extras

## Settings (`crates/settings`)

| Field | Type | Default | Spec |
|---|---|---|---|
| `preview.ambient` | bool | false | FR-001 |
| `preview.ambient_intensity` | u8, 0–100 | 100 | FR-001a |
| `preview.hidden` | bool | false | FR-011 |
| `overlay.bubbles` | `Arrangement` | all four at the bottom right, in order, none pinned | FR-013, FR-016 |

`Arrangement { corners: [Vec<BubbleKind>; 4], pinned: Vec<BubbleKind> }`. Corners are indexed
0 top left, 1 top right, 2 bottom left, 3 bottom right. `BubbleKind` is `Source`, `Mode`,
`Fps` or `Latency` (`Recording` and `Live` come later). Invariant: every kind is in exactly one
corner list. Read and write rules are in contracts/settings-file.md.

## Bubble arrangement (app, `bubbles.rs`, pure)

| Operation | Effect |
|---|---|
| `move_to(kind, corner, index)` | Removes `kind` from its list and inserts it into `corner` at `index`, clamped to the list's length. Within the same corner, an index past the old place is reduced by one (the drop bar's index counts the dragged bubble's old slot). |
| `toggle_pin(kind)` | Adds `kind` to `pinned` or removes it. |
| `reset()` | Returns the defaults. Display choices (`overlay.latency`, `overlay.fps`, `overlay.mode`) are left as they are (spec Key Entities). |
| `slint_arrays()` | Gives `corner: [i32; N]`, `rank: [i32; N]` and `pinned: [bool; N]`, indexed by kind (Source 0, Mode 1, Fps 2, Latency 3), with N the number of kinds that exist (4 now). Slint reads a missing index as a bubble with no value, so specs #9 and #10 only add kinds. |

Every operation keeps the invariant. Unit tests cover moves across and within corners, the
first and last places, pin, reset, and the read rules.

## Preview state (`preview.rs`, additions)

New fields: `preview_hidden: bool`. New state: `PreviewState::Hidden`. New events:
`HidePreview` and `ShowPreview`. New command: `SaveHidden(bool)`. `closed()` is
`hidden || preview_hidden`.

| Event | Not hidden | Preview hidden |
|---|---|---|
| `HidePreview` | Set `preview_hidden`. An open session gives `Close` and sets `resume` from it. An error or disconnected state moves `again` to `resume`. State → `Hidden`. `SaveHidden(true)`. | Nothing. |
| `ShowPreview` | Nothing. | Clear `preview_hidden` and `SaveHidden(false)`. In the tray, the state returns to what `resume` implies and opens on `Shown`. Otherwise `resume` opens (same mode and range), and without `resume`, `start()` picks. |
| `ChooseDevice(id)` | As today. | As `ShowPreview`, with `resume` set to `id`, chosen. |
| `Monitor` / `Relisted` | As today. | The list and `resume` are updated and notices fire as today. The state stays `Hidden`. A `resume` device that left stays in `resume`, and showing then gives `Disconnected` (waiting) as after the tray. |
| `Session` events | As today. | Only from a session still closing: ignored, as for a closed session. |
| Monitor's first list | `start()` as today. | `start()` sets `resume` and opens nothing. |

The state before the first list while `preview.hidden` is true is `Hidden`, not `NoSource`.

The dock button's state:

| Preview | Icon | Chip | Tooltip | Enabled |
|---|---|---|---|---|
| Shown, a device open or in a state of a device | `device_tv` | off | Hide preview | yes |
| Shown, `NoDevices`, `NoSource` or `Blocked` with no device (none listed) | `device_tv` | off | Hide preview | no |
| Hidden | `device_tv_off` | on | Show preview | yes |

## Info bubbles while hidden (FR-010)

The source bubble shows `resume`'s device name, or nothing without one. The mode, frame rate and
latency bubbles get empty texts, so they are not laid out (research R6).

## Glow (`render/glow.rs`)

| Item | Value |
|---|---|
| Field | 32 × 18 Rgba16Float history (2 textures used in turn) plus a blurred copy |
| Work targets | 512 × 288, 256 × 144, 128 × 72, 64 × 36 (Rgba16Float, made once per GPU) |
| Update | After a present that showed a new frame, at least 15 ms apart (research R1), only while the ambient light is on or its fade is above 0, and only when there is a bar. `ease_factor(dt) = 1 − exp(−dt / 150 ms)`, with dt clamped to 100 ms. |
| Reset | The history is filled directly (factor 1, no easing) by a new session's first frame and by the first update after a pause longer than 100 ms (no bars, ambient light off, no new frame), so a stale field never shows. `clear_video` drops it. |
| Draw | Layer kind 8, through the bar rectangles only. Colour `clamp(saturate(c, 1.5), 0, 1)^0.5`, alpha `0.9 × intensity × video opacity × toggle fade` (tuned, research R2). |
| Toggle fade | The video thread eases 0 ↔ 1 over 250 ms towards `Scene.ambient`, as it fades the video. |

Pure functions with unit tests: `ease_factor`, `bars(area, video)` (none, two side bars
or top and bottom bars, the 1 px threshold) and the update pacing.

## Scene (`render/mod.rs`, addition)

`ambient: bool` is the ambient light setting. It is sent with the scene, and the video thread
fades the glow (and a **With ambient light** shadow) towards it. `ambient_intensity: f32`
(0–1, the setting / 100) multiplies the glow's opacity at draw time, with no fade: the slider
already moves smoothly. It does not dim the shadow.

## UI image (`ui.rs`, addition)

`UiImage.dirty: [u32; 4]` is x, y, width and height of the repainted region, in physical
pixels. It covers the whole image after a full repaint. When the `Inbox` replaces a waiting
image, the result's `dirty` is the bounding rectangle of both. Pure helpers with unit tests:
`merge(a, b)` and the clamp to the image.
