# Handoff: App Shell

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261104-112852-app-shell`

## State

Spec drafted, its open questions answered, committed (`docs: add app shell spec`, unmerged);
waiting for the maintainer's approval before `/speckit-plan`. docs/roadmap.md marks spec #2 in
progress.

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

- All recorded in spec.md, docs/product.md and the constitution; none live only here.

## Open questions

- None in the spec.
- The record / stream only mode may change the product's main scenario and the roadmap's order;
  product.md asks to decide it before spec #3 starts.

## Next step

Maintainer approves spec.md; then `/speckit-plan`.
