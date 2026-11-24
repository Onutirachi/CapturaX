# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Implementation started with US1. T001, T002 and T003 are done and committed (not wired yet).
Next is T004, the glow's draw and pacing in the video thread.

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
- For T004's draw: the blurred field is `Glow.blurred` and the luminance `Glow.luminance` (both
  private views; add accessors or a bind group there). The `#[expect(dead_code)]` on `mod glow`
  in app/src/render/mod.rs and on `update_glow` go away when T004 uses them.

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- The pacing's 16.6 ms minimum is exact. With frame arrival jitter at 60 fps some updates may be
  skipped (the easing stays right because it uses the real `dt`). Check the `glow=` count in
  T005/T009: if it falls well below 60 at 60 fps, propose a lower threshold (about 15 ms) to the
  maintainer and update research R1 before the code.

## Next step

T004 (layer kind 8 draw through `bars()`, `Scene.ambient` with the 250 ms toggle fade, and the
call to `update_glow` after a present). Read research R1–R3 and data-model.md "Glow" and
"Scene" first.
