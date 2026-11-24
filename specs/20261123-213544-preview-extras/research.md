# Research: Preview Extras

Decisions behind [plan.md](plan.md). Each item gives the decision, the rationale and the
alternatives. Tuning values marked **tune** are set with the maintainer in the app.

## R1. Where the glow is computed

**Decision**: on the GPU, in the video thread, **after** `present`. The glow reads the video
planes already uploaded for the frame just shown and updates a small color field. The next
present draws that field. The update runs at most once per 1/60 s: at 120 fps every other frame
updates, and the easing uses the real time step. The passes are:

1. **Convert**: the video layer's planes go through the shader's own `convert` (any of the 5
   formats) into 512 × 288 RGB. Each output texel takes 4 bilinear taps, which is a box of about
   4 × 4 source pixels at 1080p.
2. **Reduce**: four 2× box passes (one bilinear tap each) take it to 256 × 144, 128 × 72,
   64 × 36 and 32 × 18.
3. **Ease**: the 32 × 18 field is blended into the history (two Rgba16Float textures used in
   turn): `history = mix(history, field, 1 − exp(−dt / 150 ms))`.
4. **Blur**: a separable Gaussian (σ ≈ 2 texels, **tune**) on the history at 32 × 18.
5. **Average**: one fragment reads the 576 history texels into a 1 × 1 luminance (Rec. 709
   weights), for the gate (R2).

**Rationale**:

- The frame is never held (FR-004): the present path is unchanged except for the draw (R3).
- The passes touch a few hundred thousand texels in all, less than 0.05 ms of GPU time on the
  RTX 3050.
- Their CPU cost on the video thread (encoding about 9 passes, roughly 0.1 ms) comes after the
  present, while the thread would wait for the next frame anyway.
- Reading from the uploaded planes needs no copy. Working with Rgba16Float avoids the 8-bit
  stalls of an easing step this small (at 120 fps the factor is about 0.05).
- The mockup's look comes from these same parameters: 32 × 18, τ 150 ms, and the luminance
  gate.

**Alternatives**:

- **CPU, on the frame's planes**: a 32 × 18 average of a 1080p NV12 frame is cheap. But it runs
  on the appsink or video thread before the upload, which adds time to the frame path, against
  principle I.
- **Mipmaps of a full-size RGB copy**: a 1080p or 4K render target per frame, about 10× the
  memory traffic.
- **Reading back the average for the gate**: a map and a wait on the GPU.
- **A compute shader**: the same work, but a second pipeline type for no gain.

## R2. How the glow looks ("Fill", D1)

**Decision**: the blurred 32 × 18 field is stretched over the video area enlarged by 5 % on
each side (the mockup's −5 %, ×1.1), so each bar takes the color of the video's nearest side.
The draw shader:

- samples the field with a bicubic upscale (4 bilinear taps), so no bilinear diamonds show at
  4K;
- applies saturation × 1.4;
- draws it at alpha `0.7 × gate × video opacity × toggle fade` over the background (F56).

The gate is `clamp((L − 0.04) / 0.16, 0, 1)`, with `L` the eased average luminance (mockup), so
a dark frame fades to the plain background (FR-003, spec edge case) instead of a grey haze. The
toggle fades over 250 ms. The watermark (F57) stays drawn over the glow, at its current opacity.

All five numbers (5 %, σ, 1.4, 0.7, gate 0.04 to 0.20) are **tune** values: they start from the
mockup and are set with the maintainer on the ezcap with the PS5. A change in feel goes into the
spec's FR-002 wording only if the variant changes.

**Rationale**: these are the mockup's values, which the maintainer chose. The single-frame flash
check comes from the easing: one white frame moves the history by at most 1 − e^(−16.7/150)
≈ 10 %, so it never flashes (FR-003).

**Alternatives**: per-texel gating (dark corners would cut holes in a bright scene); the CSS
`blur(110px)` as a wide screen-space blur at draw time (25 taps per screen pixel in the present
path).

## R3. Drawing only in the bars

**Decision**: the glow layer is drawn inside the video area's scissor, as the watermark and the
shadow are, but only through the bar rectangles. These are the one or two strips between the
letterboxed video and the area, from a pure `bars(area, video) -> [Option<[u32; 4]>; 2]`. With
no bar wider than 1 physical pixel the layer is skipped, and so is its update (spec edge case:
no cost). The video is drawn over it, so the rounded corners show the glow behind them.

**Rationale**: it bounds the extra fill to what is visible, and makes "video fills the area"
cost nothing.

**Alternative**: one quad over the whole area under the video, which fills every pixel the video
then covers.

## R4. Ambient light and the shadow (FR-005)

**Decision**: `shadow_strength()` treats **With ambient light** as "on" when `preview.ambient`
is true. The video thread already draws the shadow only while the video layer exists. So the
glow and the shadow appear together and fade together (the toggle fade multiplies the shadow
strength too). The luminance gate does not dim the shadow: a dark scene keeps its shadow.

## R4a. A stale history

**Decision**: the update stops while there is no bar, while the ambient light is off and while
no new frame comes. The first update after a pause longer than 100 ms fills the history
directly (factor 1), as a new session's first frame does. Easing from the old field would show
the colors of a scene that may be minutes old for about half a second, after a resize, a toggle
or a return from fullscreen.

## R5. Hiding the preview in the state machine

**Decision**: a second reason to keep the device closed, beside the tray's `hidden`, in
`preview.rs`:

- **`preview_hidden`** comes from `settings.preview.hidden` at start, and changes with
  `HidePreview` / `ShowPreview`. The device is closed while `hidden || preview_hidden`, and both
  use the existing `resume`.
- **Hide**: an open or opening session is closed (`Command::Close`), with `resume` set from it.
  An error or disconnected state keeps its `again` device as `resume`. The state becomes
  `PreviewState::Hidden`.
- **Show**: `preview_hidden` is cleared. If the window is shown, `resume` opens exactly as after
  the tray, with the same mode and range (FR-009). Without `resume`, `start()` picks as at
  launch.
- **Device menu while hidden**: the preview is shown and the picked device opened. In the tray
  this cannot happen, since there is no menu.
- **While hidden**: monitor events update the list and `resume`, and the connected, reconnected
  and disconnected notices fire as today. The state stays `Hidden`.
- **Start while hidden**: `start()` records `resume` and shows `Hidden` from the first frame
  drawn (FR-011).

`app.rs` maps `Hidden` to the state component (`device_tv_off`, "Preview hidden", no action).
The screenshot button and the hotkey already refuse every state that is not `Playing`.

**Rationale**: the tray path already does closing, remembering and reopening, with tests. A
second flag reuses it and inherits its edge cases: hiding while opening cancels the open, and a
returning device is not reopened until shown.

**Alternatives**: closing from `app.rs` around the state machine (two places would decide
whether a device is open); treating the hide as the tray (the tray reopens on `Shown`, and the
hidden state must survive the tray and a restart).

## R6. Bubble layout in Slint

**Decision**: the layout stays in Slint, where the bubbles' widths are known. Rust hands over,
per bubble kind (index 0–3 now, up to 5 with specs #9 and #10):

- `bubble-corner: [int]` (0 top left, 1 top right, 2 bottom left, 3 bottom right);
- `bubble-rank: [int]` (the place in its corner's list);
- `bubble-pinned: [bool]`.

A bubble is "laid" when it has a value and (the controls show or it is pinned). Its offset in
its corner is the sum, over the other laid bubbles of the same corner with a lower rank, of
their width or height plus the gap. Slint functions have no loops, so these sums are unrolled
over the six kinds, as `before(k)` is today.

Per edge (top or bottom):

- **Rows**: each corner's row is packed against its corner, between the docks as today.
- **Columns**: when the two rows of an edge do not fit side by side, the wider one becomes a
  column, and both do when even that does not fit (the mockup's rule).
- **Column order**: a column reads top to bottom in list order, so a bottom column keeps its
  last item at the corner.
- **Top corners**: a top row sits beside the toolbar when it fits clear of it (8 px). Otherwise
  it goes under the toolbar (FR-017). While the controls are hidden, pinned top bubbles pack to
  the top edge.

Positions animate (`animate x, y` 220 ms) as the mockup. Opacity is `1` for pinned bubbles and
`1 − hidden` for the others.

**Rationale**: moving widths to Rust would need a measure-and-report round trip (a frame late,
plus a feedback loop). Four, and later six, kinds unroll fine.

**Alternatives**: one component per corner fed by a model (a bubble cannot move between
instances while keeping its focus and hover state, and each bubble's content differs); layout in
Rust.

## R7. Dragging a bubble

**Decision**: each bubble's `TouchArea` starts a drag after 4 px of movement while pressed (a
press without movement does nothing, FR-015).

- **While dragging**: the bubble follows the pointer, out of the layout. The hover controls are
  dismissed and the controls do not auto-hide, since the drag counts as `engaged`.
  `BubbleDragLayer` draws the four corner zones (dashed, as in the mockup) and lights the zone of
  the pointer's quadrant of the video area. The orange 3 × 44 px bar marks the insertion point,
  which is the count of that corner's other laid bubbles whose center lies before the pointer
  (x in a row, y in a column).
- **Release**: `bubble-moved(kind, corner, index)` goes to Rust, which applies `move_to`, saves,
  and pushes the arrays back.
- **Cancelled**: losing the pointer (window deactivated) cancels the drag and nothing moves.

**Rationale**: the mockup's behavior, which the maintainer validated (D5). Moving only on
release keeps a single save per drag.

## R8. Grab cursors

**Decision**: Slint's `MinimalSoftwareWindow` does not forward `mouse-cursor` to the window, so
the app does it. `main.slint` exposes `out property <int> pointer-cursor` (0 default, 1 grab,
2 grabbing), set by the bubble under the pointer or being dragged. After each pointer event,
`app.rs` reads it and calls `window.set_cursor` with `CursorIcon::Grab` or
`CursorIcon::Grabbing`, unless a resize edge cursor applies (`frame::cursor`), which wins.

**Rationale**: it uses the same path as the resize cursors, with no platform code.

## R9. Pinned bubbles update while the controls are hidden

**Decision**: today the frame rate and latency texts freeze while the controls are hidden, so
the UI does not redraw every second (spec #3). A pinned bubble must keep updating, so its text
updates while `active || pinned`. This brings back a once-a-second UI redraw while playing. Two
costs come with it:

- the main thread copies the whole UI buffer into the `UiImage`;
- the video thread uploads the whole UI texture: 8 MB at 1080p, 33 MB at 4K.

**Decision (cont.)**: the UI hand-off becomes partial, built in this spec:

- `draw_if_needed` keeps the region the software renderer reports as repainted (its bounding
  rectangle), and `UiImage` gains `dirty: [u32; 4]`.
- A full repaint (a resize, a scale change, the first image) marks the whole image.
- The video thread uploads only that rectangle (`write_texture` with an origin and the buffer's
  row stride).
- The `Inbox` keeps only the newest UI image. When a newer image replaces one not yet uploaded,
  the two rectangles are merged, so no region is lost. The newest buffer holds every pixel,
  because the renderer reuses one buffer.
- The main thread still copies the whole buffer into the `Arc` (about 1 ms at 1080p, off the
  present path). Making that copy partial too is left for a measured need.

**Rationale**: the cost is known without measuring. A pinned frame rate bubble changes about
200 × 44 px each second, but today that redraw uploads the whole texture (8 MB at 1080p, 33 MB
at 4K) on the video thread. The change is contained in `ui.rs`, `render/mod.rs` and `gpu.rs`.
It also helps today's case of the controls showing, where the texts already change every second.
Building it now saves a round of measuring on both PCs only to then build it anyway
(maintainer, 2026-11-23).

**Measured**: SC-001's runs include one with the frame rate and latency bubbles pinned and the
controls hidden. The `ui_ms` and `ui` fields of the stats show the uploads.

**Alternative**: measure first and build only on failure. This was rejected for the reason
above.

**Spec**: SC-001 is extended with this run (spec updated 2026-11-23, plan finding).

## R10. Saving the arrangement

**Decision**: the arrangement lives in `[overlay.bubbles]` as four corner lists of kind names,
in reading order (left to right in a row, top to bottom in a column), plus a `pinned` list
(contracts/settings-file.md). Reading is lenient:

- an unknown kind is dropped;
- a repeated kind keeps its first place;
- a kind missing from every list joins the end of the bottom right list. This is also how the
  recording and live bubbles of specs #9 and #10 first appear.

A drop, a pin or a Reset saves at once through the store's atomic write.

**Rationale**: lists carry the order without explicit numbers that could collide, and a hand
edit stays readable.

**Alternative**: a `corner`, `order` and `pinned` table per kind (two kinds can claim the same
order, which needs a tie rule).
