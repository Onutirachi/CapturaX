# Implementation Plan: Preview Extras

**Branch**: `feat/20261123-213544-preview-extras` | **Date**: 2026-11-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261123-213544-preview-extras/spec.md`

## Summary

The ambient light is drawn by the video thread's own renderer. After each present that showed
a new frame (at most 60 times a second), a few tiny GPU passes turn the uploaded video planes
into a 32 × 18 color field. The field is eased over time (τ = 150 ms) and blurred. On the next
present one extra layer draws that field, stretched over the video area, only in the bars. It is
saturated and brightened, as the mockup's "Fill" variant (without its luminance gate, research
R2). The video frame never waits for the glow, and nothing is read back from the GPU.

Hiding the preview is a second "closed" reason in the pure preview state machine, beside the
existing "window in the tray" one. Both share the same resume logic. The video session closes,
the audio follows the video session as today, and the state "Preview hidden" shows. The flag
is saved, so a start while hidden opens no device.

The info bubbles gain an arrangement (a corner, a place in that corner's order, and a pinned
flag for each bubble), saved in `[overlay.bubbles]`. Slint lays the bubbles out per corner and
handles the drag with grab cursors, which the app forwards to the window. Pinned bubbles keep
showing and updating while the controls are hidden.

## Technical Context

**Language/Version**: Rust 1.99, edition 2024

**Primary Dependencies**: as the screenshot spec: wgpu 30, Slint 1.18.1 (software renderer
over the app's swap chain), winit 0.30. No new crate.

**Storage**: `capturax.toml`. New keys `preview.ambient` and `preview.hidden`, and a new table
`[overlay.bubbles]` (contracts/settings-file.md).

**Testing**: `cargo test` covers:

- the preview state machine with the new hidden reason (hide or show while opening, playing,
  disconnected or in the tray, a start while hidden, and the device-menu pick);
- the arrangement model (move, pin, reset, read rules);
- the glow's pure math (easing factor, bar rectangles, update pacing);
- the settings read and write rules.

The glow's look, the drag, the layout at every size and the latency runs are checked by hand
(quickstart.md).

**Target Platform**: Windows 11 and Linux (CachyOS, KDE Plasma Wayland)

**Project Type**: desktop app (Cargo workspace)

**Performance Goals**:

- SC-001: with the ambient light on, the median frame ready → presented time grows by at most
  0.5 ms and the p95 by at most 1 ms, with no extra dropped frames, at 1080p60 and 1080p120.
- The same budget holds with pinned bubbles updating while the controls are hidden (research
  R9).
- SC-002: the device is released within 1 s of hiding.
- SC-003: while hidden, CPU and GPU use equal the "no source" state.

**Constraints**:

- Nothing is added to the video thread before `present` except one quad drawn in the bars.
- No GPU read-back.
- No OS call outside `crates/platform` (the cursor goes through winit, already in the app).
- Every new text goes in both catalogs.
- Down to 640 × 360.

**Scale/Scope**: one GPU effect, one dock button on each side, one new state, the four
bubbles' arrangement (six later), one settings row.

No NEEDS CLARIFICATION remains. The glow's tuning values come from the mockup and are adjusted
with the maintainer in the app (research R2).

## Constitution Check

*Constitution 1.2.0. Checked before research and again after design: passes, no deviation.*

| Principle | How the plan complies |
|---|---|
| I. Preview latency | The glow's update runs after `present`, from planes already on the GPU, at least 15 ms apart. The draw is one quad limited to the bars and skipped when there are none (R1, R3). The video is never held. Pinned bubbles redraw the UI once a second. Only the changed rectangle is uploaded, and that is measured (R9). Budget and method in SC-001, on both systems. |
| II. Simple by default | One on/off for the glow with a fixed look. One dock button to hide. Drag is the only way to move a bubble, plus one Reset row. Defaults equal today's look. The "Preview hidden" state has no action, by the maintainer's decision (it is a chosen state, not an empty or error one). |
| III. One native pipeline | The glow is part of the preview's own renderer, which the constitution allows. Hiding closes the same session that every consumer will share (specs #9 and #10 keep it open). |
| IV. Platform layer | No new platform code. Cursors go through winit, like the resize cursors. |
| V. Private and safe | No network and no file access. |
| VI. Portable | Three new keys in the settings file. Unknown and missing values read leniently. |
| VII. Spec-driven, tested | Spec approved and clarified on 2026-11-23. Pure logic is unit tested. Manual checks are in the quickstart. No new dependency. |
| Technology constraints | Rust. Slint over the app's swap chain. Texts in both catalogs. Two new Tabler icons (`bulb`, `pin`), plus `bulb_off`: `bulb` without its rays. |
| Workflow | English. Conventional Commits without AI trailers. Commits only after validation. |

## Design

### Ambient light (video thread)

```text
new frame → set_video (upload) → render(scene): ground · glow (bars only) · watermark · shadow · video · UI → present
                                                       ▲
             after present, ≥ 15 ms: glow.update(video planes, dt) ─┘ (used by the next present)
             convert 512×288 → 2× box ×4 → 32×18 → ease into history (τ 150 ms) → blur
```

- `render/glow.rs` (NEW) holds the `Glow`: its textures (Rgba16Float), the passes, the update
  pacing, and the pure math (`ease_factor(dt)`, `bars(area, video)`), which
  is unit tested.
- `composite.wgsl` gains:
  - layer kind 8 (glow): a smooth upscale of the blurred field over the area enlarged by 5 %,
    saturation × 1.5, lift (power 0.5), alpha 0.9 × the video's opacity × the toggle's fade;
  - a small conversion entry that writes linear RGB into the 512 × 288 target with the
    shader's own `convert`;
  - box, blend and blur entries.

  Same file and bind layout, with separate pipelines per target format.
- `Scene` gains `ambient: bool` and `ambient_intensity: f32` (FR-001a, research R2b; the
  intensity multiplies the glow's opacity at draw time). The video thread fades the glow in and out over 250 ms
  towards it. The update runs only while the ambient light is on (or fading) and there is a
  bar. The glow's history is filled directly by a new session's first frame and by the first
  update after a pause longer than 100 ms, and is dropped in `clear_video`.
- The shadow: `shadow_strength()` returns the strength for **With ambient light** when the
  ambient light is on (FR-005). The glow and the shadow are drawn on the same condition: video
  on screen.

### Hide preview (main thread, pure state)

- `preview.rs`:
  - New `Event::HidePreview` and `Event::ShowPreview`, and a `preview_hidden` flag beside the
    tray's `hidden`. The device is closed while either is set, and both share `resume`.
  - New `PreviewState::Hidden`, shown while `preview_hidden` is set, whatever happens underneath:
    a disconnect or a return only updates `resume` and the notices.
  - `ChooseDevice` while hidden clears `preview_hidden` and opens the device (FR-009). It still
    defers while in the tray.
  - `start()` while hidden records `resume` and opens nothing (FR-011).
  - New `Command::SaveHidden(bool)`.
  - Data-model.md "Preview state" has the full table.
- Audio: unchanged. It follows `VideoOpened` and `VideoClosed` (FR-012). The volume button
  stays usable, as today.
- Specs #9 and #10: `Command::Close` becomes "stop presenting" while a recording or a stream
  holds the session. Nothing is built for it here (spec Assumptions).

### Info overlay (Slint + main thread)

- `crates/settings`: `Overlay` gains `bubbles: Arrangement`, which holds four ordered corner
  lists and a pinned set (data-model.md).
- `app/src/bubbles.rs` (NEW, pure) has `move_to(kind, corner, index)`, `toggle_pin`, `reset`,
  and the arrays handed to Slint: each kind's corner, its rank in that corner, and whether it is
  pinned.
- `info-bubbles.slint`:
  - Per-corner layout from those arrays (research R6), and pinned bubbles stay while the
    controls hide.
  - The Pin toggle joins the hover controls, and the pin mark shows inside a bubble.
  - The drag (R7), with drop zones and the insertion bar in a layer (`BubbleDragLayer`).
  - Out properties give each corner's used extent, so the notices keep clear in any corner
    (FR-018).
- `main.slint`: the ambient light button (left dock), the preview button (right dock, first),
  the "Preview hidden" state, `pointer-cursor`, and the bubble arrays and callbacks.
- `settings-panel.slint`: the Preview category gains the **Info bubbles** row with **Reset**.
- `app.rs`: wiring, the cursor forwarded to winit (R8), and the per-second texts kept updating
  for pinned bubbles (R9).

### Partial UI upload (R9)

- `ui.rs`: `UiImage` gains `dirty: [u32; 4]`, the repainted region's bounding rectangle (the
  whole image on a full repaint).
- `render/mod.rs`: a newer UI image that replaces a waiting one merges both rectangles.
- `gpu.rs`: `set_ui` uploads only the rectangle. The texture is made again, and uploaded whole,
  when the size changes.

## Project Structure

### Documentation (this feature)

```text
specs/20261123-213544-preview-extras/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R10
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── settings-file.md # additions
│   └── stats.md         # the `glow` field of the [loop] line
├── checklists/requirements.md
├── handoff.md
├── measurements/        # latency.md, acceptance.md (during implementation)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/settings/src/model.rs     # preview.ambient, preview.hidden, overlay.bubbles
app/src/
├── render/glow.rs               # NEW: Glow (R1–R3)
├── render/{mod.rs, gpu.rs, composite.wgsl}
├── bubbles.rs                   # NEW: arrangement logic (pure)
├── preview.rs                   # hidden reason, PreviewState::Hidden
├── app.rs, ui.rs, stats.rs      # wiring, cursor, pinned texts, partial UI image, glow count
app/ui/
├── main.slint
├── components/{info-bubbles, settings-panel, state-message, dock}.slint
├── previews/info-bubbles-preview.slint
├── texts.slint
└── assets/icons/{bulb, bulb_off, pin}.svg # NEW (Tabler, MIT)
app/lang/pt-BR/LC_MESSAGES/capturax.po
```

**Structure Decision**: no new crate. The glow is part of the renderer. The arrangement's
storage lives in `crates/settings` and its rules in the app, beside `preview.rs`.

## Complexity Tracking

No constitution violation to justify.
