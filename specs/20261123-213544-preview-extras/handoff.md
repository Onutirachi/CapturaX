# Handoff: Preview Extras

**Updated**: 2026-11-24 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

US1's ambient light is built, tuned and checked on both systems (T001–T008e), with its
intensity popover. Left in US1: T009's 1080p120 runs. US2's button, "Preview hidden" state and the toolbar
while hidden are built and validated on Windows (T010–T013). The ezcap, the C920 and the PS5 are on the
Windows PC.

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

- 2026-11-24: T008 on Windows (measurements/acceptance.md): checks 1–6 pass. Tuned live with
  the maintainer through temporary sliders (removed): saturation 1.5, alpha 0.9. Then the glow
  must never disappear, so the luminance gate went (T008a: no average pass, no 1 × 1 texture,
  spec FR-003 changed). A gain × 2 blew out in GTA V's daylight (yellow and white bars), so a
  square-root lift replaced it (research R2). The maintainer approved the look in both games.
- 2026-11-24: T009 on Windows at 1080p60 (measurements/latency.md): no latency change. The glow
  updated only 39 times a second with the 16.6 ms minimum (frame jitter); it is now 15 ms and
  updates 60 times (research R1). The open question on the pacing is closed.
- 2026-11-24: T008b: `preview.ambient_intensity` (u8, default 100; missing or non-integer
  reads 100, outside 0–100 clamped), tested.
- 2026-11-24: T008c: app/ui/components/slider-button.slint (`SliderButton`, was
  volume-popover.slint). It keeps no state: `value` and `off` are in-properties and
  `value-changed(int, bool)` asks the parent, which sets them (main.slint does for the volume and
  the ambient light). `chip` adds the on chip; the chip also shows while open.
- 2026-11-24: T008d: the bulb on `SliderButton`; main.slint `ambient` and `ambient-intensity`
  are in-out, `ambient-changed(int, bool)`, `ambient-wheel(length)`. app.rs `set_ambient`
  saves both in one update; `UserEvent::AmbientWheel` steps ±5 % with its own `Wheel` (reuses
  `audio::volume::wheel`) and turns the light on. `Scene.ambient_intensity` (0–1);
  gpu.rs `glow_opacity` = ALPHA × video × toggle × intensity. The bulb shows `bulb_off.svg` (no
  rays) while off or at 0 % (maintainer, spec FR-001). pt-BR "Intensidade da luz ambiente".
- 2026-11-24: T008e on Windows: check 1a passes (measurements/acceptance.md).
- 2026-11-24: T009 glass to glass on Windows, windowed with bars, ambient on: 28.9 ms median,
  31.6 ms p95, as spec #3's L2. With MPO off (the maintainer's registry change that day, since
  removed) on and off both gave 42–43 ms: windows go through the compositor; fullscreen 29.0
  (measurements/latency.md).
- 2026-11-24: T010: `preview.hidden` (default `false`, rule 1), tested.
- 2026-11-24: T011 (app/src/preview.rs): `preview_hidden` beside the tray's `hidden`, and
  `closed()` = either, used where the tray flag decided (waiting, access watch, wake, Try again,
  mode, reopen, start). `with_access(access, preview_hidden)` starts in `Hidden`; app.rs passes
  `false` until T013. `hide()`: Close + `resume` from the open session (plus `ShowRange`), or
  `again` → `resume`; `SaveHidden(true)`. `show()`: `SaveHidden(false)`; `resume` opens, or
  waits (`Disconnected`) when not listed, or in the tray sets `Opening` and keeps `resume` for
  `Shown`; no `resume` → state `None` and `start()` once listed. While hidden `left()` keeps
  `resume` and the state (only the notice); `came()` gives the notice (Reconnected for the
  `resume` device) and opens nothing; `start()` with no device keeps `Hidden`. `ChooseDevice` /
  `NoticeOpen` while hidden set `resume` (chosen) and show. `Shown` from the tray while hidden
  opens nothing. `woke` is cleared on hide. app.rs saves `SaveHidden` through the store;
  ui.rs maps `Hidden` to "no-source" as a placeholder (T013 replaces it). 12 tests.
- 2026-11-24: T012: `preview_button(state, preview_hidden) -> PreviewButton { icon, chip,
  tooltip, enabled }`. Also disabled for `Blocked` with no device (none listed while camera
  access is denied, FR-006); data-model.md's table says so now. Tested.
- `HidePreview`, `ShowPreview`, `preview_hidden()` and `preview_button` carry
  `#[cfg_attr(not(test), expect(dead_code))]` until T013.
- 2026-11-24: T013: main.slint `preview-button` first on the right dock (IconButton,
  `device_tv` / `device_tv_off` on the `active` chip, "Hide preview" / "Show preview",
  `enabled` from `preview_button`); in properties `preview-hidden`, `preview-enabled`,
  `callback preview-toggled()`. State `hidden`: `device_tv_off`, title "Preview hidden" from
  main.slint (like `no-source`), no action. app.rs: `UserEvent::PreviewToggled` sends
  `ShowPreview` or `HidePreview`; `with_access(.., store preview.hidden)`; the hidden state shows
  at start before the first list (not the blank "opening"). Screenshot refuses it (not
  Playing). pt-BR: "Preview oculto", "Ocultar preview", "Mostrar preview". The `expect`s are gone.
  - The maintainer checked on Windows: button, state, volume while hidden, restart hidden, show,
    device pick. Empty selects while hidden looked broken, so FR-010a (spec, data-model, folded
    into T013): the toolbar shows and works for `resume`'s device (`hidden_device()`); mode and
    range picks save and open nothing; the Audio controller gets `Event::VideoChosen` (menu,
    value and picks for that device, no session), sent from `show_menus` (link read only on a
    change); `range(settings)` gives the stored range while hidden. Hidden is one app choice:
    it lasts through restarts and devices coming or leaving. Tested.
  - Then (maintainer): hiding only hides, every choice works as when shown. A device picked while
    hidden keeps it hidden (`SaveChoice` at once, becomes `resume`); only the button and **Open**
    on a notice show. A device that comes while hidden only joins the menu, as when shown.
  - FR-010b (maintainer): the Color range's Auto names a range only while video shows
    (`range()` gives the range in use only while `Playing`; toolbar.slint `range-known`, else
    "Auto" alone; `show_preview_state` shows the label again). So no Limited while hidden or
    while opening (the session reports the driver's declared range first).
- Commits: the pacing folded into "add the ambient light glow math", the tuning into "tune the
  ambient light glow on the PS5" (both unmerged).

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- None.

## Next step

- T014 (bubbles while hidden: the source bubble names `resume`'s device, the others empty),
  then T015 (no glow while hidden), then the manual T016 (Windows) and T017 (CachyOS).
- T009: 1080p120 when a PS5 game with a 120 Hz mode runs (glass to glass is done).
