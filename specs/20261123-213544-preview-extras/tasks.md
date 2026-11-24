---
description: "Tasks for the preview extras"
---

# Tasks: Preview Extras

**Input**: Design documents from `specs/20261123-213544-preview-extras/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: unit tests for every pure part (in the module's own `#[cfg(test)] mod tests`): the
settings rules, the glow's math (`ease_factor`, `gate`, `bars`, the update pacing), the preview
state machine with the hidden reason, the dock button's state, the bubble arrangement, the UI
dirty rectangle (`merge`, clamp) and the `[loop]` line. The glow's look, the drag, the layout at
every size, the cursors and the latency runs are manual acceptance (quickstart.md).

**Texts**: every user-facing text is written with `@tr("...")` in a `.slint` file (texts Rust
needs go in `app/ui/texts.slint`) exactly as the spec gives it, and its Portuguese (Brazil)
translation is added to `app/lang/pt-BR/LC_MESSAGES/capturax.po` in the same task, with the
catalog's existing terms; `app/tests/i18n.rs` must keep passing.

**Icons**: new icons are Tabler (MIT, 24 px stroke, as the existing ones), added to
`app/ui/assets/icons/` in the task that first uses them: `bulb.svg` (T007) and `pin.svg`
(T022). `device_tv.svg` and `device_tv_off.svg` exist already.

**Tasks marked (maintainer)** need the maintainer's hands, eyes or machines; the agent prepares
them, gives the exact steps, and records the results in `measurements/`. On the CachyOS PC the
agent builds, runs, closes and reads the stats over SSH itself; the maintainer only looks.
Hardware batching: before the ezcap or the C920 moves between PCs, every pending (maintainer)
task that needs it on the PC where it is now is done first.

**Commits**: at each checkpoint the agent shows what changed and commits only after the
maintainer validates it (Conventional Commits, no AI trailers). Pure or unwired tasks may be
committed after `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` pass, without waiting for a manual test. Nothing is pushed unless asked.
At the end of every working session the agent updates handoff.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US3 from spec.md

---

## Phase 1: Setup

No new crate, dependency or tool (plan "Technical Context"). Nothing to do.

## Phase 2: Foundational

Nothing blocks all three stories: each story's settings keys, state and UI are in its own
phase, so the stories stay independent. The partial UI upload (research R9) is in US3, which
needs it.

---

## Phase 3: User Story 1 - Ambient light around the video (Priority: P1) 🎯 MVP

**Goal**: the bulb on the left dock makes the bars around the video glow with the video's
colors, eased and gated by luminance, with no added latency.

**Independent Test**: quickstart checks 1–6 and the first two runs of the latency check
(ambient off, ambient on), on both systems.

- [x] T001 [P] [US1] Setting (contracts/settings-file.md): in crates/settings/src/model.rs add `ambient: bool` to `Preview`, default `false`, as `preview.ambient`; rule "A missing or non-boolean `ambient` or `hidden` reads as `false`"; tests for the rule and a round trip (unknown keys still kept)
- [x] T002 [P] [US1] Glow math (research R1–R3, data-model.md "Glow"): app/src/render/glow.rs (NEW, `mod glow` in app/src/render/mod.rs) with pure `ease_factor(dt) = 1 − exp(−dt / 150 ms)` "with dt clamped to 100 ms"; `gate(l) = clamp((l − 0.04) / 0.16, 0, 1)`; `bars(area, video) -> [Option<[u32; 4]>; 2]` (none when the video fills the area, two side bars, or top and bottom bars; a bar not wider than 1 physical pixel is none); the update pacing (`due(last, now)`: at most every 16.6 ms, returns the real `dt`, or "fill" when the last update is older than 100 ms or there is none, research R4a); the tune values (5 %, σ 2 texels, saturation 1.4, alpha 0.7, gate 0.04–0.20, fade 250 ms) as named constants with a comment pointing to research R2; tests for each, including one white frame moving the history by at most 1 − e^(−16.7/150) at 60 fps (FR-003)
- [x] T003 [US1] Glow passes (research R1; after T002): in app/src/render/glow.rs a `Glow` made once per GPU holding the work targets 512 × 288, 256 × 144, 128 × 72, 64 × 36 and the 32 × 18 history ×2 (used in turn), the blurred 32 × 18 and the 1 × 1 luminance, all Rgba16Float; app/src/render/composite.wgsl gains the entries: convert (the video layer's planes through the shader's own `convert`, any of the 5 formats, 4 bilinear taps per texel), box (one bilinear tap, 2×), ease (`history = mix(history, field, factor)`, factor from T002 in the uniform), separable Gaussian blur, and average (the 576 texels to Rec. 709 luminance); same file and bind layout, a pipeline per target format, built in app/src/render/gpu.rs beside the existing ones; `Glow::update(planes, dt)` encodes the passes in one command buffer; a new session's first frame and a "fill" from `due()` set the history directly (factor 1, research R4a); `clear_video` drops it; no read-back
- [x] T004 [US1] Glow draw and pacing (research R1, R3, data-model.md "Glow", "Scene"; after T003): `Scene.ambient: bool` in app/src/render/mod.rs; the video thread eases a toggle fade 0 ↔ 1 over 250 ms towards it, as it fades the video; layer kind 8 in app/src/render/gpu.rs and composite.wgsl drawn after the ground and before the watermark, shadow and video, inside the video area's scissor only through `bars()`' rectangles: the blurred field over the area enlarged by 5 % on each side, bicubic upscale (4 bilinear taps), saturation × 1.4, alpha `0.7 × gate(L) × video opacity × toggle fade`; skipped with no bar, no video layer or a zero fade; after a `present` that showed a new frame, `glow.update` runs when `due()`, there is a bar, and `Scene.ambient` is on or the fade is above 0 (its result is used by the next present; with the ambient light off nothing runs); the video thread counts the updates per second for T005; nothing new before `present` but the one draw (plan "Constraints")
- [x] T005 [US1] Stats (contracts/stats.md; after T004, which counts): app/src/stats.rs adds `glow=<n>` at the end of the `[loop]` line, the glow updates in that second; update the existing line tests and add one with `glow=60`
- [x] T006 [US1] Shadow (FR-005, research R4; after T004): `shadow_strength()` in app/src/app.rs treats **With ambient light** as on when `preview.ambient` is true; the video thread multiplies that shadow's strength by the glow's toggle fade, so the two come and go together; the luminance gate does not dim the shadow (FR-005); other shadow choices unchanged
- [x] T007 [US1] Button and wiring (FR-001; after T001, T004): in app/ui/main.slint (`left-dock`, where the dock buttons live) the ambient light button between `fullscreen-button` and the volume button, a toggle with the soft orange chip while on, icon `bulb.svg` (NEW, Tabler), tooltip "Ambient light" (pt-BR in the catalog), always enabled (also in a state and while the preview is hidden); app/src/app.rs reads `preview.ambient` at start, sets the button and `Scene.ambient`, and on a click flips it, saves through the store and sends the scene; both themes and down to 640 × 360
- [ ] T008 [US1] (maintainer) Windows with the ezcap and the PS5: quickstart checks 1–4 and 6, and check 5 without the "hide the preview" step (that step waits for US2, T016); tune the five R2 values with the maintainer; if the feel changes the variant, update FR-002 first; record in specs/20261123-213544-preview-extras/measurements/acceptance.md
- [ ] T009 [US1] (maintainer) Latency (SC-001): quickstart "Latency check" runs 1 and 2 (ambient off, ambient on) at 1080p60 and 1080p120 on Windows and on CachyOS (the agent over SSH), and spec #3's glass-to-glass check once with the ambient light on (maintainer's phone); record in measurements/latency.md with the median and p95 of `ready_ms`, `repl` and `glow`

**Checkpoint**: the MVP: the glow on both systems, within the latency budget.

---

## Phase 4: User Story 2 - Hide the preview (Priority: P2)

**Goal**: the first button of the right dock hides the preview, releasing the device and its
audio, and is remembered across restarts.

**Independent Test**: quickstart checks 7–11 with the C920 (and the ezcap for 9).

- [ ] T010 [P] [US2] Setting (contracts/settings-file.md): in crates/settings/src/model.rs add `hidden: bool` to `Preview`, default `false`, as `preview.hidden`; rule "A missing or non-boolean `ambient` or `hidden` reads as `false`"; tests for the rule and a round trip
- [ ] T011 [US2] State machine (research R5, data-model.md "Preview state"): in app/src/preview.rs add `preview_hidden: bool` (from `settings.preview.hidden` at construction), `PreviewState::Hidden`, `Event::HidePreview`, `Event::ShowPreview`, `Command::SaveHidden(bool)`, and `closed()` = `hidden || preview_hidden` used wherever the tray's `hidden` decides today; every row of data-model.md's table: hide while open or opening gives `Close` and sets `resume`; hide in an error or disconnected state moves `again` to `resume`; show reopens `resume` with the same mode and range, or `start()` picks without one, or waits for `Shown` in the tray; `ChooseDevice` while hidden shows and opens that device (still deferred in the tray); `Monitor`/`Relisted` update the list and `resume` and fire the notices, state stays `Hidden`; late `Session` events of the closing session are ignored; the first list while hidden sets `resume` and opens nothing; the state before the first list is `Hidden` when `preview.hidden` is true; tests for each row, plus hide → tray → `Shown` (stays hidden, opens nothing) and hide → device leaves → show (gives `Disconnected` waiting)
- [ ] T012 [US2] Dock button state (data-model.md "The dock button's state"; after T011): a pure `preview_button(state, preview_hidden) -> (icon, chip, tooltip, enabled)` in app/src/preview.rs: shown with a device open or in a device's state → `device_tv`, chip off, "Hide preview", enabled; shown with `NoDevices` or `NoSource` → disabled (FR-006: no device to show); hidden → `device_tv_off`, chip on, "Show preview", enabled; tests for each row
- [ ] T013 [US2] UI and wiring (FR-006, FR-008, FR-011, FR-012; after T010–T012): in app/ui/main.slint the preview button first on the right dock (before the screenshot button), its icon, chip, tooltip and enabled state from T012, tooltips "Hide preview" and "Show preview"; the "Preview hidden" state in app/ui/components/state-message.slint (icon `device_tv_off`, text "Preview hidden", no action) and a case in app/ui/previews/state-message-preview.slint; app/src/app.rs maps the click to `HidePreview`/`ShowPreview`, `PreviewState::Hidden` to that state, and `Command::SaveHidden` to a save through the store; the audio follows `VideoOpened`/`VideoClosed` as today (no audio change; check the volume button stays usable, FR-012); the screenshot button and hotkey refuse it as any state; new texts in the pt-BR catalog
- [ ] T014 [US2] Bubbles while hidden (FR-010, data-model.md "Info bubbles while hidden"; after T011): in app/src/app.rs, while `PreviewState::Hidden` the source bubble shows `resume`'s device name (nothing without one) and the mode, frame rate and latency bubbles get empty texts, so they are not laid out
- [ ] T015 [US2] Glow in the hidden state: confirm in app/src/render/mod.rs that the hidden state draws no glow and runs no glow update (no video layer), and that the ambient light button still flips the setting (quickstart check 5's hidden step)
- [ ] T016 [US2] (maintainer) Windows with the C920 and the ezcap: quickstart checks 7–11 and check 5's hidden step; SC-002 (release within 1 s, video back within the opening time + 0.5 s) and SC-003 (Task Manager, 30 s hidden vs 30 s "no source"); record in measurements/acceptance.md
- [ ] T017 [US2] (maintainer) CachyOS with the C920: checks 7–10 by the agent over SSH (the device released checked with `fuser /dev/video*`, Cheese opening it, the log), the webcam light by the maintainer; check 11 with `top` and the GPU's usage; record in measurements/acceptance.md

**Checkpoint**: hiding and showing on both systems; the state survives a restart.

---

## Phase 5: User Story 3 - Arrange the info bubbles (Priority: P2)

**Goal**: each bubble has a corner, a place and a pinned flag, changed by drag and the Pin
toggle, saved, and reset from the settings panel; pinned bubbles keep updating with the controls
hidden at no latency cost.

**Independent Test**: quickstart checks 12–16 and the third run of the latency check.

- [ ] T018 [P] [US3] Setting (contracts/settings-file.md, data-model.md "Settings"): in crates/settings/src/model.rs `BubbleKind` (`Source`, `Mode`, `Fps`, `Latency`; file names `source`, `mode`, `fps`, `latency`) and `Arrangement { corners: [Vec<BubbleKind>; 4], pinned: Vec<BubbleKind> }` (0 top left, 1 top right, 2 bottom left, 3 bottom right) as `Overlay.bubbles`, the `[overlay.bubbles]` table with `top-left`, `top-right`, `bottom-left`, `bottom-right`, `pinned`; invariant "every kind is in exactly one corner list"; read rules: "an unknown kind is dropped (logged)", "a kind listed twice keeps its first place", "a kind in no list is appended to `bottom-right`, in the order of rule 2", "`pinned` keeps only known kinds. Order does not matter, and duplicates collapse", "A missing `[overlay.bubbles]` table reads as the defaults"; the existing `[overlay]` keys unchanged; tests for each rule and a round trip
- [ ] T019 [P] [US3] Partial UI upload (research R9, data-model.md "UI image"): app/src/ui.rs `UiImage.dirty: [u32; 4]` (x, y, width, height in physical pixels) from the region the software renderer reports in `draw_if_needed` (its bounding rectangle), the whole image on a resize, a scale change or the first image; pure `merge(a, b)` and the clamp to the image, tested; app/src/render/mod.rs: when the `Inbox` replaces a UI image not yet uploaded, the result's `dirty` is the merge of both; app/src/render/gpu.rs `set_ui` uploads only `dirty` (`write_texture` with the origin and the buffer's row stride), and makes and uploads the texture whole when the size changes; check by hand that the controls, tooltips and menus still draw right (no stale strips) on Windows
- [ ] T020 [US3] Arrangement logic (data-model.md "Bubble arrangement"; after T018): app/src/bubbles.rs (NEW, pure, `mod bubbles` in app/src/main.rs) with `move_to(kind, corner, index)` ("Removes `kind` from its list and inserts it into `corner` at `index`, clamped to the list's length. Within the same corner, an index past the old place is reduced by one"), `toggle_pin(kind)`, `reset()` (the defaults; `overlay.latency`, `overlay.fps`, `overlay.mode` untouched) and `slint_arrays()` (`corner: [i32; N]`, `rank: [i32; N]`, `pinned: [bool; N]`, indexed Source 0, Mode 1, Fps 2, Latency 3); every operation keeps the invariant; tests: moves across and within corners, first and last places, pin, reset
- [ ] T021 [US3] Layout per corner (FR-013, FR-014, FR-017, research R6; after T020): app/ui/components/info-bubbles.slint takes `bubble-corner: [int]`, `bubble-rank: [int]`, `bubble-pinned: [bool]` (four entries from Rust; the layout is unrolled over six kinds and reads a missing index as a bubble with no value, data-model.md); a bubble is laid when it has a value and (the controls show or it is pinned); its offset in its corner is the sum over the other laid bubbles of that corner with a lower rank, unrolled over the six kinds as `before(k)` is today; per edge: rows packed at their corners between the docks, the wider row becomes a column when the two do not fit side by side and both do when that still does not fit, a column reads top to bottom in list order; a top row sits beside the toolbar when it fits clear of it by 8 px, else under it; while the controls are hidden pinned top bubbles pack to the top edge; `animate x, y` 220 ms; opacity 1 when pinned, `1 − hidden` otherwise; the bubbles stay under the settings panel and its backdrop (spec edge case); out properties with each corner's used extent; app/ui/main.slint passes the arrays (from `slint_arrays()` in app/src/app.rs at start and after each change); app/ui/previews/info-bubbles-preview.slint shows bubbles in all four corners
- [ ] T022 [US3] Pin (FR-014, FR-015; after T021): the hover controls in app/ui/components/info-bubbles.slint gain the **Pin** toggle (icon `pin.svg`, NEW, Tabler; chip on while pinned; tooltip "Pin" / "Unpin"), beside the display button for bubbles that have one; a pinned bubble shows a small pin before its text only while the controls show; the callback goes to app/src/app.rs → `toggle_pin` → save through the store → arrays pushed back; texts in the pt-BR catalog
- [ ] T023 [US3] Drag (FR-015, research R7; after T021): in app/ui/components/info-bubbles.slint each bubble's `TouchArea` starts a drag after 4 px of movement while pressed (a press without movement does nothing); while dragging the bubble follows the pointer out of the layout, the hover controls close, and the drag counts as `engaged` so the controls do not auto-hide; `BubbleDragLayer` (under the settings panel, over the video and the docks) draws the four dashed corner zones, lights the pointer's quadrant of the video area, and shows the orange 3 × 44 px bar at the insertion index (the count of that corner's other laid bubbles whose center lies before the pointer: x in a row, y in a column); release sends `bubble-moved(kind, corner, index)` → app/src/app.rs `move_to` → save → arrays; the window losing the pointer or being deactivated cancels the drag and nothing moves
- [ ] T024 [US3] Grab cursors (research R8; after T023): app/ui/main.slint `out property <int> pointer-cursor` (0 default, 1 grab, 2 grabbing) set by the bubble under the pointer or being dragged; after each pointer event app/src/app.rs reads it and calls `window.set_cursor` with `CursorIcon::Grab` or `CursorIcon::Grabbing`, unless a resize edge cursor applies (`frame::cursor`), which wins
- [ ] T025 [US3] Notices in every corner (FR-018; after T021): in app/ui/main.slint the notice placement (`notice-clear`, `notice-past`, `settings-notice-from`) uses the extent of the bubbles in the notice's corner (T021's out properties) instead of today's single `bubbles-box`; while the controls are hidden only the pinned bubbles count; spec #3's bottom right rules hold in every corner
- [ ] T026 [US3] Pinned texts keep updating (research R9; after T019, T021): in app/src/app.rs the per-second frame rate and latency texts update while the controls show or that bubble is pinned (today only while active), so a pinned bubble updates once a second with the controls hidden and nothing redraws when none is pinned
- [ ] T027 [US3] Reset row (FR-016; after T020): in app/ui/components/settings-panel.slint the Preview category gains the **Info bubbles** row with a **Reset** button and no text about the default; the callback goes to app/src/app.rs → `reset` → save → arrays; texts in the pt-BR catalog (with the catalog's existing term for Reset)
- [ ] T028 [US3] (maintainer) Windows: quickstart checks 12–16 (check 15's sizes set by the agent, the maintainer looks), and the partial upload's look from T019; record in measurements/acceptance.md
- [ ] T029 [US3] (maintainer) Latency (SC-001): quickstart "Latency check" run 3 (ambient on, frame rate and latency pinned, controls hidden) at 1080p60 and 1080p120 on Windows and on CachyOS (the agent over SSH), compared with run 1; `ui` about 1 per second, `ui_ms` well below today's full uploads; record in measurements/latency.md

**Checkpoint**: arrangement, pin, drag and reset on both systems; the pinned run within the
latency budget.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T030 [P] Docs: README (ambient light, hide preview, arranging the bubbles), AGENTS.md "Status", docs/product.md Spec column for F9, F10 and F42 and the "Hide preview and its audio" question moved out of "To reconsider" as decided (FR-012), docs/roadmap.md #6 done with its links
- [ ] T031 (maintainer) quickstart check 17 (both languages) and check 6 (both themes, Background Theme and Black, watermark on) on Windows; CachyOS screenshots of the same by the agent for the maintainer to look at
- [ ] T032 Handoff and acceptance summary in measurements/acceptance.md; the maintainer validates; the branch is ready for the pull request

---

## Dependencies & Execution Order

- No Setup or Foundational work; the three stories can start at once. They all edit
  crates/settings/src/model.rs (T001, T010, T018), app/src/app.rs and app/ui/main.slint, so
  in one working copy those tasks run one after another.
- US1: T001 and T002 in parallel; T003 after T002; T004 after T003; T005 and T006 after T004;
  T007 after T001 and T004; manual T008, T009 last.
- US2: T010 in parallel with T011; T012 after T011; T013 after T010–T012; T014 after T011;
  T015 after T013 (and US1's T004 for the glow part); manual T016, T017 last. Check 5's hidden
  step (T016) needs US1.
- US3: T018 and T019 in parallel; T020 after T018; T021 after T020; T022, T023, T025, T027
  after T021 (T027 only needs T020); T024 after T023; T026 after T019 and T021; manual T028,
  T029 last. T029's run 3 needs US1 (ambient on).
- Polish after the stories.

### Parallel example (US1)

```text
T001 setting    T002 glow math
```

### Parallel example (US3)

```text
T018 arrangement setting    T019 partial UI upload
```

## Implementation Strategy

1. US1 as the MVP: the glow's math, passes and draw, then the button; tune and measure SC-001
   on both systems before going on (batch the ezcap's checks where it is).
2. US2: the state machine with its tests first, then the button and state; check with the C920
   on both PCs.
3. US3: the setting, the partial upload and the arrangement logic, then the layout, pin, drag,
   cursors, notices and Reset; run SC-001's pinned run.
4. Polish: docs, languages and themes, handoff.
