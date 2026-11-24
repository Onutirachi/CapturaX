# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Implementation started with US1. T001 to T004 are done and committed. Nothing shows yet:
`Scene.ambient` is always false in app.rs until T007 wires the setting and the button.
Next is T005 (`glow=` in the `[loop]` line).

## Done in the last sessions

- 2026-11-23: T001: `preview.ambient` in crates/settings/src/model.rs (default `false`, rule 1:
  missing or non-boolean reads as `false`), with tests for the rule and a round trip.
- 2026-11-23: T002: app/src/render/glow.rs with `ease_factor`, `gate`, `bars`, `due` (the
  update pacing, `Due::{Wait, Ease(dt), Fill}`) and the R2 tune values as named constants, all
  tested (including the one white frame bound, FR-003).
- `bars(area, video)` takes the video area's scissor rectangle (`gpu::scissor`) and the
  letterboxed video (`gpu::letterbox`). It picks the axis with the wider gap, so a 1 px rounding
  gap on the other axis does not hide the real bars.
- 2026-11-23: T003: the glow's GPU passes.
  - gpu.rs: `Shaders` holds the composite module, the one bind layout, the pipeline layout and
    the sampler, with `pipeline(device, entry, format, blend)` and `bind(device, views, rect)`.
    The layers' pipeline uses it too (same result as before). `Layer` keeps its plane `views`.
  - `Gpu::update_glow(now) -> bool` makes the `Glow` at the first call and runs it on the video
    layer; it carries `#[expect(dead_code)]` until T004 calls it. `set_video` calls
    `glow.forget_source()` when the layer is made again; `clear_video` calls `glow.reset()`
    (the next update fills).
  - glow.rs: `Glow` owns the targets (512 × 288 … 32 × 18, history ×2, blurred, 1 × 1
    luminance, all Rgba16Float), five pipelines and the bind groups (made once; only the
    convert one is made again for a new layer). `update(.., source, now)` calls `due()` itself
    with its own `last`: Wait returns false, Fill uses factor 1, Ease uses `ease_factor(dt)`.
    The 32 × 18 box target is reused as the blur's middle step. The average reads the history,
    as R1 says.
  - composite.wgsl: `glow_convert`, `glow_box`, `glow_ease`, `glow_blur`, `glow_average`; the
    uniform's use for each is in the comment under `Rect`.
  - Tests on a real GPU without a surface (skipped with no adapter; only tests read back): fill,
    ease, wait, reset and pause fill; all 5 formats convert (grey and BGRx red); a half white
    frame keeps its sides and blurs symmetrically. They also build the layers' `fs` pipeline.
- 2026-11-23: T004: the glow's draw and pacing.
  - mod.rs: `Scene.ambient`; `Toggle` fades 0 <-> 1 over `glow::FADE` (250 ms) towards it,
    proportional to the distance (a turn from halfway takes half), and counts as an animation
    like the video fade. `Fade` gained a `length` (the video's stays `FADE`). After a present
    that showed a new frame, `gpu.update_glow(&scene, now)` runs when `scene.ambient` or the
    toggle level is above 0, and `stats.glow_updated()` counts it (printed by T005).
  - gpu.rs: `glow_bars(scene)` = `bars(scissor(area), letterbox(area, video))`, none without a
    video layer. `render(.., ambient)` draws the glow first (before watermark and shadow) with
    the layers' `fs` pipeline, one draw per bar with the bar as scissor, at opacity
    `ALPHA × video opacity × toggle`; skipped with no bar, zero opacity or no field.
    `update_glow` with no bar pauses the glow (`Glow::pause`: `last = None`), so the next
    update fills and nothing is drawn until then (R4a, covers a resize that makes bars appear).
  - glow.rs: `Glow::draw(queue, area, surface, opacity) -> Option<&BindGroup>` writes the
    kind 8 uniform (the area grown by `EDGE_GROW` per side; `to_r` = gate low, gate high,
    saturation) and returns `None` while there is no field (`last` is `None`). `gate()` is now
    `#[cfg(test)]`: the shader applies it, the function is its test reference.
  - composite.wgsl: kind 8 `glow(uv)`: cubic B-spline upscale in 4 bilinear taps, saturation,
    alpha from the 1 × 1 luminance through the gate, premultiplied.
  - GPU test: the draw saturates a colour, half-opens the gate on a mid grey, closes it on a
    dark frame, and draws nothing before a field or after a pause.

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- The pacing's 16.6 ms minimum is exact. With frame arrival jitter at 60 fps some updates may be
  skipped (the easing stays right because it uses the real `dt`). Check the `glow=` count in
  T005/T009: if it falls well below 60 at 60 fps, propose a lower threshold (about 15 ms) to the
  maintainer and update research R1 before the code.

## Next step

T005: `glow=<n>` at the end of the `[loop]` line from `Stats.glow` (already counted), line
tests updated plus one with `glow=60` (contracts/stats.md). Then T006 (shadow) and T007
(button and wiring), after which the glow can be seen in the app.
