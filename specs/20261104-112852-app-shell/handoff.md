# Handoff: App Shell

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261104-112852-app-shell`

## State

Spec approved by the maintainer (2026-11-04). `/speckit-plan` done: plan.md, research.md
(R1–R14), data-model.md, contracts/ (cli, settings-file, stats, single-instance, platform),
quickstart.md. Committed as `docs: add app shell plan` (unmerged). The maintainer accepted fast-move
frame drops on Windows as a known limit (spec Edge Cases, SC-003). `/speckit-tasks` done:
tasks.md (T001–T066, 8 phases), committed as `docs: add app shell tasks`. `/speckit-analyze`
found no critical issue; its fixes are applied (see Decisions), not yet committed.

## Done in the last sessions

- `/speckit-specify`: spec.md and checklists/requirements.md (all items pass).
- Clarifications: minimum window 640 × 360 with a compact layout below Float's 800 × 520
  (FR-009); Windows 11 snap layouts flyout best effort, dropped if fragile (FR-011); settings as a
  panel inside the main window, no separate windows (FR-025).
- FR-006 now names the brand assets: `brand/CapturaX-light.svg` (white text) in the dark theme,
  `brand/CapturaX-dark.svg` (black text) in the light theme, `brand/X.svg` for the window,
  taskbar and tray icon. `brand/` is committed with the spec.
- Out of Scope now points global hotkeys to F50 and the tray / taskbar status to F51.
- Product decisions outside this spec went to the stacked branch `docs/outputs-and-hotkeys`
  (based on this branch): constitution 1.1.1 "one source per output"; recording and streaming
  as two independent outputs in separate Record and Stream panels inside the main window, as
  the settings panel (F15, F16, F25-F27); hotkeys Ctrl+F8 screenshot, Ctrl+F9 stream, Ctrl+F10
  record, in the window and global, configurable, conflicts not handled (F19, F20, F50); tray
  icon and taskbar badge showing recording and streaming status (F51). Not pushed.

## Decisions

- While writing the tasks: `cargo deny` cannot ban `windows-sys` and `zbus` by name (winit, wgpu
  and Slint pull them in), so a test `app/tests/platform_boundary.rs` checks the direct
  dependencies of `app` and `settings` instead (research R2, R13 and plan updated).
- From `/speckit-analyze`: winit's Windows and Linux window extensions (undecorated shadow, app
  id) go through `platform::window_attributes`, so `platform` depends on winit (contracts/platform.md);
  Esc closes a menu, else the settings panel, else fullscreen (FR-007); language and theme switch
  times are logged for SC-005; the interrupted-save test kills only after the first save; wgpu is
  built without GL on Windows so a forced missing backend always fails (renderer failure check).
  Left open, low: no check catches a user-facing literal in Rust (C4); refresh-rate change between
  monitors is not in the quickstart (C5).

- All recorded in spec.md, plan.md, research.md, docs/product.md and the constitution; none live
  only here. Main plan choices: three-crate workspace (`app`, `crates/settings`,
  `crates/platform`); UI drawn on the main thread, video thread owns wgpu (spike R15); TOML
  settings `capturax.toml` with lenient load and atomic replace; shadows generated at the display
  scale; test video through GStreamer `videotestsrc`; tray `tray-icon` (Windows) / `ksni` (Linux).

## Open questions

- None in the spec.
- The record / stream only mode may change the product's main scenario and the roadmap's order;
  product.md asks to decide it before spec #3 starts.

## Next step

Maintainer validates the analyze fixes; fold them into `docs: add app shell tasks`; then
implementation from T001 after explicit approval (`/speckit-implement`).
