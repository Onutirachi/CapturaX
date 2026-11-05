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

T030 (window actions) done and checked on Windows by the maintainer; fmt, clippy, 77 tests
pass; committed (`feat: add window actions`). New `app/src/frame.rs` (pure, tested): 6 px resize band scaled by the
window's scale, its cursor, and the Esc order (`frame::escape`). `app.rs`: F11 and Esc are handled
before Slint (key repeats ignored); Esc with a menu open goes to Slint, else closes
`AppWindow.settings-open`, else leaves fullscreen, never quits; a left press inside the band calls
`drag_resize_window` and does not reach Slint; no band while maximized or fullscreen; the cursor
resets when fullscreen toggles. A maximized window restoring under the pointer is the system's
own `drag_window` behavior (nothing in our code).
First check by the maintainer: F11, resize band, restore under the pointer, minimum size and the
Esc order ok. The white top line came back after a mouse snap to a top corner and was reproduced
by script (drag the bar to the corner, F11, read the screen with `CopyFromScreen`): a 1 px white
line as wide as the snapped window. Cause: winit moves the client area 1 px down for the
undecorated shadow (`WM_NCCALCSIZE`, `top += 1`); the system-painted strip keeps old pixels.
Hiding the DWM border did not help. Fix: new `platform::set_fullscreen_frame(window, on)`
(Windows: `set_undecorated_shadow(!on)`, Linux no-op), called before entering and after leaving
fullscreen; contracts/platform.md and research R5 updated. Script after the fix: black top edge
after snapping to either top corner. Windowed, the 1 px strip stays (winit's trade-off). Second
check by the maintainer: ok.

T031 (scene and scale changes): most of it came with Phase 2 (the scene, in physical pixels, is
sent on every update where it changed; the surface follows `scene.size`; `ScaleFactorChanged`
reaches Slint and the `Resized` after it resizes the buffer). Added: `Ui` forces one whole repaint
after a size or scale change (`full_repaint`: `NewBuffer` for that draw, then back to
`ReusedBuffer`). Slint's partial renderer tracks changes in logical pixels, so after moving to a
monitor with another scale (new physical size, same logical size) nothing was dirty and the new,
empty buffer could stay empty or keep the old scale.

T031 first check (maintainer): dragging to a monitor with another scale shrank the window while
dragging and it left the pointer, and coming back was hard. Reproduced with
`scratchpad/dpidrag.ps1`-style scripted drags across this machine's 100 % and 150 % monitors and
a message log. Cause: winit's `WM_DPICHANGED` repositioning (research R5, "Dragging across
monitor scales"). Fixed by `platform::follow_dpi_rect` (`crates/platform/src/windows/dpi_rect.rs`,
contract updated). Scripted drags now keep 1280 × 720 at 100 % and 1920 × 1080 at 150 %, both ways,
under the pointer. The maintainer could not judge the shadows over the dark test video (the
`ball` pattern is black). Screenshots over a temporary local `pattern=white` build (not kept)
showed soft shadows without steps at 100 %, 125 % and 150 %. They also showed a dark horizontal
line out of the tooltip's middle at every scale: the tooltip was about 30 px tall, under twice the
shadow radius, so the nine-slice corners overlapped. The tooltip is now at least
2 × `radius-m` tall (`tooltip.slint`; the rule is noted in `raised.slint`). This is a fix to the
tooltip commit (a192d43), to be folded into it.

T031 second check (maintainer, using a local `target/release/capturax-white.exe` built with
`pattern=white`, never committed): the docks looked low resolution and at other scales the
shadow started 1 px outside the surface. Pixel measurements agreed: at 125 % and 150 % one side
had a light pixel between the surface and its shadow, and the other side a dark one. Switched to
research R8's fallback (research R8 and the plan updated first). `Theme.shadow(width, height,
scale)` is a pure callback that Rust implements: `shadows::make` draws one image per surface size
in physical pixels (two layers, three box blurs, cut-out 1 px inside the surface), cached in
`Ui` by size and scale. `Raised` uses one image, drawn 1:1.

Third check (maintainer): better, but a black outline around the buttons. Tooltips read better
at 125 % than at 100 %, and the tooltip sat too close to the button. Changes:
- The cut-out is now the surface's exact shape (research R8 updated). At 150 % the edge goes
  171 → 81 with no outline and no gap.
- Tooltip text is `text-m` (14 px) instead of `text-s`.
- `TooltipLayer` keeps 16 px from the control (`offset`; 8 px clear past the dock's edge) and
  8 px from the window's edges (`gap`).

Fourth check (maintainer): no outline and a good tooltip distance. There were small "teeth" at
the bottom corners: Slint's anti-aliased fill and the image's cut-out disagreed on the curve.
Text at 100 % read darker and blurrier than at 125 %.
- The image now holds the fill too: `Theme.surface(width, height, scale, fill, shadow)`, with
  one coverage per pixel for the fill and the cut-out. The theme's shadow strength is in the
  image, not in the element's opacity. `Raised.fill` is a `color`; research R8 updated. At 100 %
  the dock's corner now goes 81 → shadow monotonically.
- Text: Slint's software renderer has no gamma or hinting, so light text on dark looks thin at
  12–14 px. Weights 500, 550 and 600 and 15 px were compared with local test copies. Snapping
  the tooltip and dock positions to whole physical pixels was also tried, for the maintainer's
  observation that the title bar's tooltips read better; it did not help and was removed.
  Then a local test copy with a grid of tooltips was built (12–16 px × weights 400–700). The
  maintainer chose 15 px at weight 400: acceptable at 100 %, very good at 125 %. Rendering the
  UI at double resolution would make 100 % as smooth, at about 4× the drawing cost, and was not
  pursued.
- The tooltip's 32 px minimum height was removed: per-size images have no nine-slice limit.

The dock corners are confirmed fine. fmt, clippy (Windows, Linux), 80 tests pass. Not
committed. `target/release/capturax-white.exe` is the current code with a white test video
(local only).

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

All T031 checks passed (dragging between monitors confirmed by the maintainer). After the
maintainer approves the commits:
- fold the tooltip changes into a192d43 (`tooltip.slint`: 15 px text, 16 px from the control);
- commit T031 with `follow_dpi_rect` and the R8 fallback;
- go to T032
(save and restore the window placement) (tasks.md, US1).
