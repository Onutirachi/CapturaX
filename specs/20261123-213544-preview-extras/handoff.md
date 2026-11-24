# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Implementation started with US1. T001 and T002 are done and committed (pure, not wired yet).
Next is T003, the glow's GPU passes.

## Done in the last session

- 2026-11-23: T001: `preview.ambient` in crates/settings/src/model.rs (default `false`, rule 1:
  missing or non-boolean reads as `false`), with tests for the rule and a round trip.
- 2026-11-23: T002: app/src/render/glow.rs with `ease_factor`, `gate`, `bars`, `due` (the
  update pacing, `Due::{Wait, Ease(dt), Fill}`) and the R2 tune values as named constants, all
  tested (including the one white frame bound, FR-003). The module carries
  `#[expect(dead_code)]` in app/src/render/mod.rs until T003/T004 use it: remove it then.
- `bars(area, video)` takes the video area's scissor rectangle (`gpu::scissor`) and the
  letterboxed video (`gpu::letterbox`). It picks the axis with the wider gap, so a 1 px rounding
  gap on the other axis does not hide the real bars.

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- The pacing's 16.6 ms minimum is exact. With frame arrival jitter at 60 fps some updates may be
  skipped (the easing stays right because it uses the real `dt`). Check the `glow=` count in
  T005/T009: if it falls well below 60 at 60 fps, propose a lower threshold (about 15 ms) to the
  maintainer and update research R1 before the code.

## Next step

T003 (glow passes in app/src/render/glow.rs, composite.wgsl and gpu.rs), then T004 (draw and
pacing). Read research R1–R4a and data-model.md "Glow" and "Scene" first.
