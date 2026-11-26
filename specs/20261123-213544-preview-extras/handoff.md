# Handoff: Preview Extras

**Updated**: 2026-11-26 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

US1's ambient light is built, tuned and checked on both systems (T001–T008e), with its
intensity popover. Left in US1: T009's 1080p120 runs. US2's button, "Preview hidden" state and the toolbar
while hidden are built and validated on Windows (T010–T013); no bubble while hidden (T014), the glow
check (T015) and the acceptance on both systems (T016, T017) are done: US2 is complete. US3 has
begun: the arrangement setting (T018), the partial UI upload (T019), the arrangement logic
(T020), the layout per corner (T021), the Pin (T022), the drag (T023), the grab cursors (T024), the notices per corner (T025), the pinned texts updating (T026) and the Reset row (T027) are built. The ezcap and
the PS5 are on the Windows PC; the C920 is on the CachyOS PC.

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
- 2026-11-24: T014: no bubble shows while hidden (FR-010 changed by the maintainer after seeing a
  lone source bubble that stayed on hide and hid and showed again on show). `show_info` already
  empties every bubble outside `Playing`, so no code changed; spec, data-model and quickstart say so.
  - The maintainer finds the bubbles appearing and leaving abrupt. Proposed (not yet approved):
    in FR-017 and T021, a bubble that gains or loses its value fades in or out (quick duration,
    keeping its last text while fading) while the others slide 220 ms to their places.
- 2026-11-24: T015, confirmed in code (no change): hiding closes the session, `clear_video` drops
  the video layer and resets the glow; with no layer `glow_bars` gives no bar (nothing drawn) and
  `update_glow` runs only after a present with a new frame (none while hidden). The ambient
  button's `set_ambient` does not depend on the state.
- 2026-11-24: T016 on Windows (measurements/acceptance.md): checks 5 (hidden step), 7–11 pass.
  SC-002 from the log (released in 23–457 ms; first frame on show 423–471 ms); SC-003 with
  Windows counters (CPU from the process's processor time, GPU from the GPU Engine
  counters): 0.00 % hidden and 0.00 % shown with no device, 0.21 % / 0.43 % playing the C920.
  Quickstart check 8 rewritten for FR-010a (picks while hidden open nothing). The agent could
  not capture the screen (permission refused), so the maintainer clicked.
- 2026-11-25: T017 on CachyOS (measurements/acceptance.md): checks 7–11 pass, run by the agent
  over SSH; the maintainer watched the webcam light. Released in 88–144 ms, first frame on show
  428–561 ms; SC-003 hidden 0.17 % CPU / 0.00 % GPU against 0.20 % / 0.93 % waiting with no
  device. The **Open** check used a `v4l2loopback` Virtual Cam as the second device (made before a
  restart: one made while the app runs is not listed, known since device-preview).
  - The CachyOS clone is on a detached HEAD at d624e61 (its local branch still holds the commits
    from before they were amended); its `capturax.toml` was restored from
    `~/capx/capturax.toml.before-t017`.
- 2026-11-25: T018: crates/settings/src/model.rs `BubbleKind` (`ALL`, `as_str`, `parse`),
  `Arrangement { corners, pinned }` as `Overlay.bubbles` (`Overlay` is no longer `Copy`;
  `store.get()` returns by value, so app.rs did not change), `CORNER_KEYS`, `BOTTOM_RIGHT`, all
  exported. `read_arrangement`: a missing or non-table `[overlay.bubbles]` is the default; a key
  that is not an array reads as empty (logged); `pinned` is kept in kind order. Tests for each
  rule and a round trip. contracts/settings-file.md: "rule 2" was a slip for rule 3 (kind order)
  and the second "6." is now 7.
- 2026-11-25: T019: app/src/ui.rs `Rect`, pure `merge` and `clamp` (tested); `UiImage.dirty` from
  the bounding box of Slint's `PhysicalRegion` (`render` returns it), the whole image on a full
  repaint. render/mod.rs `send_ui` merges the region of an image not yet uploaded into the next
  (tested). gpu.rs `set_ui` uploads only `dirty` (`upload_rect`, offset into the packed rows) and
  a new texture (first, new size, new video thread) whole. The maintainer looked on Windows with
  the test video and the ezcap: normal; the logs had no region warning.
  - Bubbles never show with `--test-video`: `show_info` fills them only while a device plays
    (since device-preview). Use a device to look at bubbles.
  - `up_ms` (maintainer, 2026-11-25): a new last field of the `[loop]` line, the `set_ui` time on
    the video thread; `ui_ms` stays the main thread's draw + copy. contracts/stats.md, research
    R9, quickstart and T029 use `up_ms` for the partial upload's cost.
- 2026-11-25: T020: app/src/bubbles.rs (pure, module-level `expect(dead_code)` until T021–T023
  and T027 call it): free functions over `settings::Arrangement` (the type lives in the settings
  crate): `move_to(&mut, kind, corner, index)` (a corner past 3 is ignored, logged),
  `toggle_pin(&mut, kind)` (kept in kind order), `reset(&mut Overlay)` (only `bubbles`) and
  `slint_arrays(&) -> SlintArrays { corner, rank: [i32; 4], pinned: [bool; 4] }`. 6 tests, with
  an invariant check after each move.
- 2026-11-25: the maintainer chose sliding over the proposed fade (T014's note): FR-017, R6 and
  T021 now say how, after the stack spike.
- 2026-11-25: T021: app/ui/components/info-bubbles.slint `InfoBubbles` now covers the video area
  and lays each kind in its corner from `bubble-corner`, `bubble-rank`, `bubble-pinned` (main.slint
  in properties, pushed by app.rs `show_arrangement` at start; T022, T023 and T027 call it after
  their changes). It takes `toolbar-box`, `left-dock-box`, `right-dock-box` (main.slint's
  notice boxes) and computes rows, columns, beside or under the toolbar (research R6) with
  functions unrolled over six kinds. Out: `boxes` / `pinned-boxes` (each corner's extent with the
  controls shown / hidden, for T025) and `notice-fits` (main.slint's `notice-joins`);
  `bubbles-box` is `info.boxes[3]` until T025. Bubbles slide (no opacity): two steps on hide and
  show as the spike (`spread`, `arrived`); a bubble glides on `sx, ox, sy, oy` (side + offset, so a
  resize moves it at once); past the edge its x snaps (`outside`), so it enters straight up or
  down; one that loses its value keeps its text (`*-held`) and its x (`held-x`). The shown texts
  are bindings (value, else held) so the width is right from the first frame. Bubbles are always
  `visible` (offscreen when not laid). `BubbleControlsLayer` opens below a bubble in the window's
  top half. bubbles.rs: per-item `expect(dead_code)` on `move_to`, `toggle_pin`, `reset`.
  - FR-017 (maintainer): a new source's bubbles slide in together after the video's fade-in
    (`render::FADE`, now `pub`; app.rs `info_at`) and once its first second has frames
    (`info_ready`; the video thread's seconds come 0–1 s after the first frame).
  - Slint starts an animation at its clock's time when the property changed (properties_animations
    "Anchor timing to when the change was first signalled"); the clock moves only in
    `update_timers`, so after idle a change from Rust started an animation partly or fully over
    (random pops). app.rs `window_event`, `user_event`, and `about_to_wait` before `tick` and the
    delayed `show_info` now call `ui.update_timers()` first. Confirmed with a temporary trace
    (removed): every entry then animated 212–585 ms.
  - The maintainer checked on Windows (ezcap, release): the preview's buttons, default corner,
    hide/show preview and chrome slides, and four corners with a pinned fps (a test
    `[overlay.bubbles]` in target/release/capturax.toml, restored after).
- 2026-11-25: T022: every bubble has hover controls (the source's hold only Pin): the Pin toggle
  (`pin.svg`, chip while pinned, "Pin" / "Unpin", pt-BR "Fixar" / "Desafixar") and the display
  button beside it for mode, frame rate and latency. A pinned bubble shows a 10 px orange
  `pin_filled.svg` in its top right corner while the controls show, as the spike (maintainer),
  gone as the pinned bubbles close up. app.rs `bubble-pin-toggled(kind)` → `bubbles::kind_at` →
  `toggle_pin` → store → `show_arrangement`.
  - Keyboard (maintainer, FR-015): the same controls as the pointer. Every bubble takes the
    focus; Enter, Space, Up or Down opens its controls with the focus on Pin
    (`BubbleControls.show-for-keys`, `wants-focus`); Tab / Right and Shift+Tab / Left move; Esc
    or Tab past an end calls `give-back` and the bubble takes the focus again (`backs`). A display
    menu opened from the keyboard gives the focus back to its button when it closes.
  - Slint pitfall: a `changed` handler does not run when its property is first read already at
    the new value (here `focused`, read after `pin.focus()` in the layer's `init`), so
    `take-focus` sets `BubbleControls.focused` itself; without it the 250 ms timer hid them.
  - Slint's `debug()` goes to stderr, lost in the Windows GUI app; for a trace, add a temporary
    `debug_log` to `ui.rs`'s `Platform` that logs it.
  - The maintainer checked on Windows: pointer, keyboard, the source's controls and the mark.
  - Tab order (maintainer): the bubbles come last, after the docks (main.slint's `info` moved after
    the right dock, still under the notices and the settings panel).
- 2026-11-25: T023: the drag (info-bubbles.slint). A press that moves 4 px starts it (`Bubble`'s
  `moved`; `pointer-event` up drops, cancel cancels; `WindowState.active` false or the chrome
  hiding cancel too). The bubble follows the pointer by `drag-x`, `drag-y` (offset from its
  layout place, animated back to 0 on release while the layout moves it, so it slides from the
  drop point). `BubbleDragLayer` (inside `InfoBubbles`, under the bubbles): an orange radial
  glow from the corner under the pointer (circle centered on the corner, gone ~300 px out) and a
  solid orange slot (accent 35 %, no border). app.rs `bubble-moved(kind, corner, index)` →
  `SettingChange::BubbleMoved` → `move_to` → store → `show_arrangement`.
  - Look (maintainer): the first build's four dashed zones and 3 × 44 bar were replaced by
    variant F of the drop zones mockup (https://claude.ai/artifact/D4WycMtijeV9PbFtqTgEkC),
    slot without a dashed border; spec FR-015, US3 #2, research R7, quickstart 13 updated.
  - The layout functions take a mode `m` (was `all: bool`): 0 pinned only, 1 all, 2 all with the
    moving bubble at the target (`corner-in`, `rank-in`; the others make room), 3 all but the
    dragged one. Ranks are doubled in `rank-in` (the slot is `2 × index − 1`): a float rank in a
    conditional with an int was truncated and the slot tied with its neighbour.
  - The target (`drop-corner`, `drop-index`) is set in `aim()` on each move, not bound: a binding
    loops through the layout (Slint's check is static). Entering a corner it is measured on mode
    3; within the corner on mode 2 (the bubbles around the slot), so it never flickers. The
    pointer (`pointer-x`, `pointer-y`) is likewise assigned, not bound to the bubble's place.
  - After the drop, `pending` keeps the drop's arrangement until Rust's arrays come back
    (`changed bubble-corner` / `bubble-rank`, 300 ms fallback), so nothing slides back and forth.
  - The maintainer checked on Windows (ezcap, release): moves across and within corners, press
    without movement, Alt+Tab cancel, kept after a restart. Settings backup from before the test:
    target/release/capturax.toml.before-t023.
- 2026-11-26: T024: the grab cursors. info-bubbles.slint `Bubble.cursor` (1 over it, 2 from the
  press on, before it moves: maintainer) and `InfoBubbles.cursor` (the highest); main.slint
  `out property pointer-cursor`. frame.rs `cursor(direction, ui)`: an edge's resize shape wins
  over the open hand, the closed hand wins over the edge (a drag into a corner crosses the band).
  app.rs `update_cursor` after each pointer event and each `update_ui` (a timer can cancel a
  drag), setting the window's cursor only on a change.
  - Windows has no grab cursor (winit shows the four-way arrow), so the app uses the maintainer's
    own drawings, app/ui/assets/cursors/grab.svg and grabbing.svg (a 22 px square; mockup
    https://claude.ai/artifact/9SVtogaGoESmXddRQA4NWm). build.rs rasterizes them into 32–96 px
    cursors (22 of every 32 pixels, centred); app/src/cursor.rs `GrabCursors` makes the size
    nearest to 32 × scale with winit's `CustomCursor` (again on `ScaleFactorChanged`), click
    point at the palm, (17, 19) at 32 px. `platform::has_grab_cursors()` is true on Linux, where
    the cursor theme's hands are used. Spec FR-015 and research R8 updated.
  - The maintainer checked on Windows (ezcap, release): hover, press, drag across the edge,
    release, edges, chrome hidden. Not yet looked at on CachyOS (theme hands; T028's run).
  - The formatting `cargo fmt` wanted in T023's `on_bubble_moved` is folded into its commit.
- 2026-11-26: T025: the notices in every corner (FR-018). info-bubbles.slint: in properties
  `notice-corner` (0–3, main.slint's `notice-index`), `notice-width` (always the event notice's
  now), `notice-appear`; out `notice-joins` / `notice-joins-pinned` (`joins(m)` for modes 1 / 0)
  replace `notice-fits`. The notice joins its corner's row when the row reaches the corner
  (`band-s0` / `band-e0` at the margin, row at `top-y` / `bottom-y`: no dock and no toolbar in
  the way) and the notice fits beside both rows of its edge (only its own row beside the
  toolbar). `reserve(c, m)` leaves it room on its side, in its corner and in the partner when they
  share the band (`band-s` / `band-e` = the `0` versions with the reserve). main.slint:
  `notice-hidden-at` (controls hidden: joined at the corner, else past `info.pinned-boxes`, else
  over them) replaces `notice-corner` as the slide's hidden end; `notice-clear` moves past the
  notice's corner box and its partner's (`notice-past-bubbles`); the settings notice likewise.
  `bubbles-box` is gone. Spec FR-018's wording was ambiguous (join only while hidden) and now says
  spec #3's bottom right rules hold in every corner.
  - The maintainer checked on Windows (ezcap, release): one bubble per corner, two pinned, all four
    notice positions, controls shown and hidden, 640 × 360. A notice comes from Ctrl+Shift+F8.
  - Found: closing the settings panel while a control's tooltip shows leaves it on screen (old,
    not this spec); recorded in docs/roadmap.md #17 UI review with the tested fix.
- 2026-11-26: T026: the freeze was in info-bubbles.slint, not app.rs (app.rs already sends the
  texts each second; tasks.md corrected). `fps-live` / `latency-live` = `active || pinned(2 / 3)`
  pick the live value or the held one, per bubble; `hold-fps` / `hold-latency` keep the held text
  current while live, so a bubble unpinned (or reset) with the controls hidden shows its last
  value. The maintainer checked on Windows (ezcap, release, `--stats`): pinned bubbles update
  each second with the controls hidden (`ui=2`), nothing redraws with none pinned (`ui=0`).
  - The maintainer turned `keep_in_tray` off in target/release/capturax.toml on purpose: closing
    the window then quits, so a rebuild does not fail on an app left in the tray.
- 2026-11-26: T027: settings-panel.slint `ButtonRow` (name and an `ActionButton`; only the button
  acts, so a stray click on the row resets nothing) and the Preview category's last row **Info
  bubbles** / **Reset**, no text about the default. `Prefs.bubbles-reset()` →
  `SettingChange::BubblesReset` → `bubbles::reset` → store → `show_arrangement`. pt-BR "Bolhas de
  informação" (Reset was already "Restaurar"). The maintainer checked on Windows (ezcap, release):
  moved and pinned bubbles, Reset brought the default, kept after a restart (quickstart check 16).
- Commits: the pacing folded into "add the ambient light glow math", the tuning into "tune the
  ambient light glow on the PS5" (both unmerged).

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- None.

## Next step

- US3 (bubbles): T028 (maintainer: quickstart checks 12–16 on Windows), then T029. An arrangement can be tried by editing `[overlay.bubbles]` in the settings file.
- T009: 1080p120 when a PS5 game with a 120 Hz mode runs (glass to glass is done).
