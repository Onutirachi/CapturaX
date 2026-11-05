# Handoff: App Shell

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261104-112852-app-shell`

## State

Phases 1 and 2 committed; T060/T061 committed (`ci: add platform boundary test and workflow`;
the workflow has never run, it runs when the branch is pushed, T062, so T061 stays unchecked).
Phase 3 started: T021 (stats), T022 (test video) and T023 (wiring) done and checked on Windows,
committed (`feat: add test video and presentation stats`).

T021–T023 checks (Windows 11, RTX 5070 Ti, release, window 1280 × 720): `capturax --test-video
1080p60 --stats` prints the `[present]` header and one `[loop]` line per second: pres 60,
repl 0, ready_ms 1.1–1.2 median / 1.3–1.8 max, gap_ms 18–19 (first second higher: startup);
1440p60 video: ready_ms 1.6–1.7 median (not part of SC-001 any more). fmt, clippy (Windows;
`platform` and `settings` also for Linux), 54 tests pass.

T024 (placement) and T025 (shadows) done, checked and committed (`feat: add window placement
and shadow images`). fmt, clippy (Windows), 72 tests pass; the app starts with the shadows set in `Theme`
(not drawn until a component uses them, T029). The soft and tight images match the spike's PNGs
(mean alpha difference below 1 level; the hole edge is now antialiased).

T026 (focus ring, icon button, tooltip) and T027 (menu, select, toggle) done and checked on
Windows by the maintainer with a temporary demo in main.slint (removed): hover, also on a toggled
button; tooltip after 500 ms, padded, kept inside the window near the right edge; a select at the
bottom right opens above and aligned to its right; Tab order with the ring, the ring hides after a
click; menu keys. Menu scrolling was not exercised (14 items fit at 1280 × 720): check it at
640 × 360 once the settings panel exists (T033). fmt, clippy, 72 tests pass. Committed
(`feat: add focus ring, icon button, tooltip, menu, select and toggle`).

T028 (title bar) done and checked on Windows by the maintainer (logo, buttons, drag, double
click, Tab and Enter with the ring and tooltips, dimmed icons when inactive); fmt, clippy, 72
tests pass; committed (`feat: add title bar`). `components/title-bar.slint` calls `WindowState` itself (no
callbacks to forward); the 4 px drag threshold is in Slint, Rust only calls `drag_window`. Window
buttons are focusable (ring drawn inside, the bar's edge would clip it) with tooltips below
(Minimize, Maximize/Restore, Close) and icons dimmed while the window is inactive. Placed in
main.slint's chrome layer; T029 puts the toolbar before it so the hidden toolbar slides under the
bar. `AppWindow` exposes `max-button-x/y/width/height` (physical px, via `1phx`) for T040.

T029 (dock and chrome layer) done and checked on Windows by the maintainer; fmt, clippy, 72
tests pass; committed (`feat: add docks and chrome layer`; the window buttons' Tab change and
FR-020 were folded into `feat: add title bar`). Choices: the chrome is a `Rectangle` over the video
area with `clip: true`, so the hidden toolbar slides under the title bar without drawing the bar
last; Tab goes left dock → right dock. The toolbar slot is an empty `TouchArea` with no panel (an
empty bar over the video looked wrong; spec #3 gives it its panel). `components/dock.slint` is a
column only: each dock has one button, a row layout comes when a dock gets more. Compact layout
(`AppWindow.compact`, below 800 × 520): docks centered in the toolbar's 88 px row, the slot takes
the space between them, tooltips open below. The chrome also stays shown while a dock button has
keyboard focus. The settings button toggles `AppWindow.settings-open` (in-out, shown as active);
T033 draws the panel and T030 reads it for Esc.

First check of T029 by the maintainer: docks and sliding in ok, no shadow while hidden ok,
compact layout ok. Fixed after it, waiting for a second check:
- The chrome hid only on the next mouse move, without sliding: a Slint timer firing inside
  `update_ui` starts an animation after `about_to_wait` read `has_active_animations()`, so the
  loop went back to `ControlFlow::Wait`. It now reads the flag again after the update.
- Tab never reached the hidden docks: Slint's focus chain skips items outside their clip
  (`ItemRc::is_visible`, geometry based). `ui.rs` calls `AppWindow.reveal-chrome()` before
  dispatching Tab, which shows the chrome at once (no slide) so the docks are in place.
Open from that check:
- F11 does nothing: expected, it is T030.
- Snapped to a quarter of the screen with Windows snap, then fullscreen: a white line along the
  top. Look at it in T030 (fullscreen on the current monitor) and T031 (resizes).
- Second check: the chrome slides out after 2 s and Tab brings the docks back, ok. The white
  line is a 1 px light line along the whole top edge in fullscreen (maintainer's screenshot).
- Maintainer (2026-11-04): the title bar's window buttons leave the Tab order, as in native apps;
  the system keys cover them (Win+arrows, Alt+F4, F11). FR-020 and quickstart row 18 updated;
  `WindowButton` has no `FocusScope` or ring now, its tooltip shows on hover only. Checked.

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

- T026/T027: menus and tooltips go through the globals `Menus` and `Tooltips` and are drawn by
  `MenuLayer` and `TooltipLayer` in main.slint's top layer; a select gets its owner id from
  `Menus.new-id()` in `init` and learns its pick from `Menus.picks`. A menu takes every click
  while open (a click outside only closes it). The focus ring follows the global `Input.keyboard`,
  set by Rust (ui.rs). New `components/raised.slint` (shadows under menus and tooltips). The menu
  handles Esc itself, so T030 must read `Menus.open` before dispatching Esc to Slint. Recorded in
  plan.md (UI structure) and T030.

- T025 (maintainer, 2026-11-04): shadows are generated in Rust at 1×, once at start, not at the
  window's scale. Slint 1.18 draws a nine-slice corner at its edge size in image pixels times
  the window's scale (`fit9slice`), so an image made at 150% would be enlarged to 225%; the
  software renderer scales images by nearest pixel. T031 checks 125%, 150% and 200% for steps;
  if they show, the fallback is one shadow per surface at its exact size (research R8). spec.md
  (edge case on scale), research R8, plan, T025 and T031 updated.
- T024: when the saved monitor is not connected, the window is centered on the primary (the
  saved position is not reused); the size never goes below 640 × 360 logical at the monitor's
  scale; `Placement.size` is physical; `Monitor` has `bounds` instead of `position` + `size`;
  no monitor known → the OS places the window. winit does not report work areas, so T032 must
  give the bounds, or add a platform function for the work area (Windows `GetMonitorInfoW`
  `rcWork`). Recorded in data-model.md "Window placement".
- T022/T023: a frame is "ready" when the appsink hands it to the inbox, after its copy, as the
  spike measured (before the copy, 1080p read 2.3 ms). While the test video plays,
  `platform::precise_timers()` holds 1 ms timer resolution on Windows and opts out of Windows
  11's timer throttling: at the default 15.6 ms `videotestsrc` delivered frames in bursts and
  `gap_ms` read 32 ms. Recorded in research R9 and contracts/platform.md.
- Maintainer (2026-11-04): GStreamer is linked and needed to start; no start without it is
  supported (development builds use the install on `PATH`; the portable package ships it,
  constitution VI, spec #13). R9 and exit 69 in contracts/cli.md updated. SC-001 is measured
  with the 1080p60 test video on a 1080p screen and on a 1440p screen through NVIDIA DSR (no
  native 1440p monitor); spec.md, quickstart.md and T041 updated. The `1440p60` preset stays
  (FR-004).
- T060/T061: `cargo deny check bans` was already failing after Phase 2: Slint's default features
  bring `ksni` (via `i-slint-core`, feature `system-tray`) and `muda` (via
  `i-slint-backend-winit`). Those two Slint crates are now allowed wrappers in `deny.toml`.
  Turning Slint's default features off fails: without a Slint backend, `.slint` SVG images do
  not compile. GStreamer 1.28 on Windows ships one Inno Setup `.exe` (no MSI), installed in CI
  with `/VERYSILENT /NORESTART /ALLUSERS /TYPE=devel`; Ubuntu also needs `libfontconfig1-dev`.
  All recorded in research R13 and T061.
- Phase 2: the presentation order is derived from the wgpu backends the build has (DX12 → the
  Windows order, else the Linux order) instead of `cfg(windows)`, so `app/src` stays free of
  platform `cfg` (T060). `settings::LoadOutcome::Damaged { kept_as }` is an `Option<PathBuf>`
  (the rename can fail). A first start (no file) or a damaged file writes the current values at
  once (`Store::save_soon`, recorded in data-model.md "Saving"). `KNOWN_LANGUAGES` in
  `app/src/main.rs` is temporary until `i18n::LANGUAGES` (T042). Things not wired yet by design:
  window placement (T032), notices pushed (US3), window actions beyond the basics (T030), the
  translated renderer message (T058).
- Phase 1: Slint 1.18.1 is still the newest 1.x (research R1). `zbus = "5"` because ksni 0.3
  uses zbus 5 (one copy in the build; zbus 6 exists). `windows-sys` 0.61 with the features the
  platform tasks need, declared once in the workspace. `deny.toml` has
  `unused-allowed-license = "allow"` because later specs bring the remaining licenses; the
  `tray-icon` ban was checked by adding it to `app` (fails as expected).
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

- The record / stream only mode may change the product's main scenario and the roadmap's order;
  product.md asks to decide it before spec #3 starts.

## Next step

T030 (window actions) (tasks.md, US1): includes F11 and the white line along the top in fullscreen
after a quarter snap (see T029 above).
