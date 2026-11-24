# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Implementation started with US1. T001 to T007 are done and committed: the ambient light works
in the app. The glow was tuned with the maintainer on CachyOS (part of T008, see below). Next
are the rest of T008 (on Windows) and T009 (latency); the ezcap is on the CachyOS PC now.

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

- 2026-11-23: T005: app/src/stats.rs prints `glow=<n>` at the end of the `[loop]` line (reset
  each second); line tests updated, plus `glow_updates_count_for_one_second` (`glow=60`).

- 2026-11-23: T006: the With ambient light shadow.
  - app.rs: `Backdrop.shadow` is true for Always and With ambient light, and
    `Backdrop.shadow_ambient` marks the second. `shadow_strength()` gives the full strength for
    both (half on the light theme's ground, as before).
  - render/mod.rs: `Scene.shadow_ambient`; `Scene::faded(ambient)` multiplies a With ambient
    light shadow's strength by the toggle level, in the video thread right after the toggle
    step. So the shadow fades out too when the ambient light is turned off (the main thread
    cannot fade it). The luminance gate is not applied. Tested.
  - settings model: the `Shadow::Ambient` doc no longer says "none until then".

- 2026-11-23: T007: the ambient light button.
  - main.slint: `ambient-button` (IconButton, `bulb.svg` from Tabler, tooltip "Ambient light",
    `active` = the on chip) between fullscreen and volume, always enabled; its focus keeps the
    chrome shown like the other dock buttons. `in property ambient` and
    `callback ambient-changed(bool)` (asks for the other value).
  - app.rs: `Backdrop.ambient` from `preview.ambient` feeds `Scene.ambient`.
    `SettingChange::Ambient(on)` saves it, sets the button and calls `apply_backdrop` (the next
    scene carries it). The start value is set next to the volume's.
  - pt-BR: "Luz ambiente".
  - Checked on CachyOS over SSH (screenshots): place, tooltip (pt-BR), on chip in both themes,
    640 × 360, saved and kept after a restart, off again. The maintainer validated it.
- 2026-11-24: T008 (CachyOS part): tuned with the maintainer on Astro's Playroom: alpha 0.7 →
  0.9 and the gate fully open at 0.12 instead of 0.20 (research R2, data-model, plan updated;
  the GPU test's mid grey is now 20/255). The maintainer liked it and checks it once more on
  Windows. The loop stays clean with the glow on at 1080p60 (`ready_ms` 0.8, `repl=0`,
  `glow=56–60`).
  - Over SSH, ydotool's pointer reaches the window only after a click on its title bar
    (`moves=0` until then); the dock shows when the pointer comes from the window's left edge.

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- The pacing's 16.6 ms minimum is exact. With frame arrival jitter at 60 fps some updates may be
  skipped (the easing stays right because it uses the real `dt`). Check the `glow=` count in
  T005/T009: if it falls well below 60 at 60 fps, propose a lower threshold (about 15 ms) to the
  maintainer and update research R1 before the code.

## Next step

- While the ezcap is on CachyOS: T009's Linux half (the agent over SSH: 60 s with ambient off and
  on at 1080p60; 1080p120 needs the PS5's output set to 120 Hz by the maintainer), recorded in
  measurements/latency.md.
- Then on Windows: T008 (quickstart checks 1–4 and 6, check 5 without the hide step, and the
  maintainer's last look at the glow's strength) and T009's Windows half.
- Open question above (the 16.6 ms pacing): explained to the maintainer on 2026-11-24, no
  decision yet; the agent recommends about 15 ms.
