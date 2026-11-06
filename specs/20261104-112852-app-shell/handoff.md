# Handoff: App Shell

**Updated**: 2026-11-06 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261104-112852-app-shell`

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

The dock corners are confirmed fine. fmt, clippy (Windows, Linux), 80 tests pass. Committed
(`feat: handle scale changes`; the tooltip changes folded into the tooltip commit).

T032 (window placement) done and checked on Windows by the maintainer; fmt, clippy (Windows;
`platform` also for Linux), 80 tests pass; committed (`feat: save and restore window placement`). New `platform::work_area(monitor)`
(Windows `GetMonitorInfoW` `rcWork`, Linux none → the monitor's bounds; contracts/platform.md
updated, `windows-sys` feature `Win32_Graphics_Gdi`). At start `placement::fit` with winit's
monitors; the window is created with the position (outer, physical), the size as logical at the
chosen monitor's scale and `with_maximized`. Saved once per main loop turn after `Moved` or
`Resized` (`about_to_wait`): on Windows, maximizing sends `Moved` before winit's `is_maximized`
turns true, and saving inside the event stored the maximized size as the normal one. Not saved
while fullscreen or minimized; while maximized only the flag (on another monitor: that monitor and
no position). Rules added to data-model.md "Window placement".
Scripted checks (`scratchpad/place.ps1`-style SetWindowPos / ShowWindow, close, reopen, on this
machine's five 1920 × 1080 monitors at 100 %, 125 % and 150 %): first start centered on the
primary's work area; reopened at the same place and size on 100 % and 125 % monitors and on the
monitor at negative x; 1920 × 1080 on the 150 % monitor reduced to its 993 px work area; maximized
reopens maximized and restores to the size before; minimized then closed keeps the last normal
placement. A script's `WM_CLOSE` to a minimized window misses it (.NET's `MainWindowHandle` points
elsewhere then): close from the taskbar while minimized is in the manual check.
Also fixed: in `platform/src/lib.rs` the `set_fullscreen_frame` doc comment sat on
`follow_dpi_rect`; folded into `feat: handle scale changes`.

T033 (settings panel) done and checked on Windows by the maintainer; fmt, clippy, 80 tests
pass; committed (`feat: add settings panel`). New `components/settings-panel.slint`: `SettingsPanel` (a `Raised` surface, title,
close button with tooltip "Close (Esc)", rows in a `Flickable`, version at the bottom) and the
global `Prefs` (values set by Rust, one `*-changed` callback per row). Placed in main.slint's
settings layer: beside the right dock and centered on it, 360 px wide (less in a narrow window);
in the compact layout right-aligned below the toolbar's 88 px row; taller than the room, the rows
scroll. The chrome stays shown while the panel is open (so its anchor, the dock, does not slide
away). Each row is a `ToggleRow` (a click on the row flips it). Changes reach `App` as
`UserEvent::Setting` (the `Store` is not `Clone`) and go through `store.update`. Not wired yet,
by design: Start with the system only logs (the system's entry holds it, T039), and
`Prefs.tray-available` stays true until T039 sets it from `platform::tray::available()`. Menu
scrolling at 640 × 360 still needs a select in the panel: check it in T046.
Scripted checks (screenshots at 1280 × 720, 900 × 600 and 640 × 360; clicks on two rows saved
`start_fullscreen` and `keep_in_tray` as true).

T034 (start mode) done and checked on Windows by the maintainer; fmt, clippy, 81 tests pass;
committed (`feat: apply start mode`). `StartupOptions::starts_fullscreen(saved)` (cli.rs, tested): `--fullscreen` or
`--windowed` win over Start in fullscreen for this start, never saved. Applied in `resumed` after
the placement and before the window is shown, through the same `set_fullscreen` as F11, so it is
borderless on the monitor the window was placed on and Esc or F11 returns to that placement.
Scripted checks: the six combinations of saved on/off × no option, `--fullscreen`, `--windowed`
open as expected and leave `start_fullscreen` unchanged; started with `--fullscreen`, Esc returns
to the saved window. (Scripted keys need a click on the window first: Windows does not let a
script bring it to the front.)

T035 (single instance) done, reviewed by the maintainer and committed (`feat: add single instance channel`; nothing
to see in the app until T036 wires it); fmt, clippy (Windows; `platform` also for Linux), 93
tests pass (Linux tests run in CI only). Shared part in `crates/platform/src/single_instance.rs`
(name, protocol, listener thread), OS parts in `windows/instance.rs` and `linux/instance.rs`;
`instance` is no longer a public module, the items are at the crate root. API for T036:
`instance_name(path)`, `claim_instance(name) -> Claim::{First(Listener), Running(Client)}`,
`Listener::serve(on_show: Fn() -> bool) -> Serving` (dropping `Serving` stops the thread and
releases the name: quit sequence), `Client::show() -> Option<Answer>`, `can_raise_window()`.
`on_show` runs on the listener thread: T036 sends a user event through the `EventLoopProxy` and
returns `can_raise_window()`. Choices recorded in contracts/single-instance.md: the path's folder
is canonicalized and lowercased on Windows before hashing; one pipe instance, remote clients
rejected, client opens with identification-only security; Windows pipe reads have no timeout,
so a stalled client blocks the listener (later starts get no answer and exit 0) and the stop
gives up after 1 s. `Win32_System_IO` added to the `windows-sys` features (`ConnectNamedPipe`).

T036 (single instance wired) done, checked by the maintainer and committed
(`feat: wire single instance`); fmt, clippy (Windows), 93 tests pass. `main.rs` claims after the
settings load and before `save_soon`, so a second start writes nothing; a claim error is logged
and the app starts without it (contract updated). The listener serves from `start()` once the
event loop exists: `on_show` sends `UserEvent::ShowWindow` and returns `can_raise_window()`.
`App::show_window`: video thread `set_visible(true)`, window `set_visible(true)`,
`set_minimized(false)`, winit's `focus_window` (it needs a visible, restored window; on Windows it
uses the Alt key trick). The `Serving` is dropped in `exiting` after the settings flush, before
the window. On `shown-not-raised` the second start creates the `Ui` only to read `Texts`
(`app-name` as title, new `already-open`, pt-BR in the `.po`). Scripted checks (Windows, release):
second start with the first minimized or behind Notepad → first restored and in the foreground,
second exits 0 in about 105 ms in all (the exchange 0.4 ms; `Start-Process -Wait` adds about
1 s of its own); a temporary build answering `shown-not-raised` showed the message box in
Portuguese (Slint picked the system language) and exited 0. A window hidden from outside with
`ShowWindow(SW_HIDE)` stays hidden (winit still thinks it is visible): not the real case, T039
hides through winit; check showing from the tray there.

T037 (tray) done, checked by the maintainer with a temporary example (checker icon: click,
menu, relabel, Quit; deleted) and committed (`feat: add tray icon`); fmt, clippy (Windows and Linux), `cargo deny check bans` ok, platform
tests pass. New shared `crates/platform/src/tray.rs` (`TrayEvent`, `Labels { title, open, quit }`,
`IconImage { size, rgba }`, `available`, `create`, `Tray::set_labels`, pure `pick` and `argb`,
tested); `tray` is no longer an OS module re-export. Windows: `tray-icon` 0.26 (its own `muda`
0.21; Slint's is 0.19); the library keeps its event handlers in set-once cells, so they are
installed once per process and forward to a static slot the current `Tray` fills and its drop
clears; left click up → Open, menu Open / separator / Quit, the menu only on right click; icon
size `GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem())` (new `windows-sys` feature
`Win32_UI_HiDpi`); `&` escaped in menu texts. The tray must be created on the main thread once
the event loop exists (T039). Linux: `ksni` blocking, id `capturax`, all icon sizes as ARGB
pixmaps, `activate` → Open, `_` escaped; drop waits for its shutdown. Contract row and T037
updated to the real signature.

T038 (autostart) done, reviewed by the maintainer and committed
(`feat: add start with the system`); fmt, clippy (Windows and Linux), platform tests pass.
Windows (`windows/autostart.rs`): `RegGetValueW` / `RegSetKeyValueW` / `RegDeleteKeyValueW` on
HKCU, no key handles. `set(true)` writes `"<exe>" --autostart` and removes Task Manager's
StartupApproved mark (so turning it on in the app wins); `set(false)` removes both. Linux
(`linux/autostart.rs`): `$XDG_CONFIG_HOME` (absolute) or `~/.config`, entry rewritten whole on
`set(true)`, removed on `set(false)`; `Exec` quoted per the Desktop Entry spec (`"`, `` ` ``, `$`,
`\` escaped, then backslashes doubled as a string value, `%` doubled), `program` reverses it.
New `autostart::executable()` holds the `$APPIMAGE` rule so T039 compares `target()` with it
(contract and T038/T039 updated). Checks: the Linux tests ran on Windows through a scratch crate
(`#[path]` to the file; WSL has no Rust); a scratch program against the real registry: on →
state true and the target read back; a Task Manager disabled mark (03…) → state false; on again
clears the mark; a 02… mark → true; off → both values gone (registry left clean).

T039 (tray and autostart wired) done, checked by the maintainer (all manual rows, also a
restart with and without the tray) and committed (`feat: wire tray and start with the system`); fmt, clippy (Windows), 99 tests pass.
- Choice (kept after the maintainer's check): the tray icon is shown whenever Keep in tray is on and the desktop has a
  tray (created at start and when the row is turned on, removed when it is turned off), not only
  while the window is hidden; F51 later shows recording and streaming status on it.
- `App::sync_tray` (main thread, after the event loop runs), icons from `icon::ICONS`, labels from
  `Texts` (`app-name`, new `tray-open` "Open" / `tray-quit` "Quit", pt-BR "Abrir" / "Sair").
  A tray that fails to create is logged and closing then quits. `Prefs.tray-available` from
  `tray::available()` once at start.
- `App::close` (title bar close and `CloseRequested`): with a tray, hides the window and calls the
  video thread's `set_visible(false)`; else quits. Tray Open and a click → `show_window` (the
  single instance's path); Quit → the quit sequence, where the tray drops before the
  single-instance listener.
- `--autostart` with a tray: the window is created and placed but never shown, the video thread
  hidden, Start in fullscreen ignored.
- Start with the system: `Prefs.start-with-system` read from `autostart::state()` at start and in
  `Prefs.opened()` (the `SettingsPanel`'s `init`, run each time it opens); a change calls
  `autostart::set(on, executable())` and reads the state back (a failure is logged and the row
  shows the real state). `main.rs::follow_autostart` (first instance only, after the claim):
  when the state is on and `target()` is not `executable()`, rewrites the entry; a disabled entry
  is left alone.
- Scripted checks (Windows, release, settings file backed up and restored, registry cleaned):
  Keep in tray on, `WM_CLOSE` → process alive, window hidden; a second start → shown and in the
  foreground; a Run value pointing to `D:\Old Place\capturax.exe` and `--autostart` → started
  hidden in the tray, the value rewritten to this executable; a second start showed the window.
  Not scripted (manual): tray click, Open and Quit from the menu, the panel's rows.

T040 (snap layouts flyout) done, checked by the maintainer and committed
(`feat: add snap layouts flyout`). fmt, clippy
(Windows; `platform` also for Linux), 100 tests pass. New shared `crates/platform/src/snap.rs`
(`SnapEvent::{Hover, Press, Click}`, `ButtonRect`, `attach(window, on_event) -> Snap`,
`Snap::set_button(Option<rect>)`), Windows `windows/snap.rs`, Linux no-op; `windows-sys` features
`Win32_Graphics_Dwm` and `Win32_UI_Controls`. App: `UserEvent::Snap`, `App::sync_snap_button`
after each `update_ui` (button from `AppWindow.max-button-*`, the top resize band cut off by the
pure `frame::snap_button`, tested; None in fullscreen or with a menu open), hover and press into
the new `WindowState.max-hover` / `max-pressed`, click → toggle maximize. `WindowButton` has
`system-hover` / `system-pressed`; no tooltip under system hover (the flyout takes its place).
- Found while doing it (research R5 updated): `HTMAXBUTTON` alone gave no flyout. The system
  took the caption to be winit's 1 px shadow strip (`GetTitleBarInfoEx`: maximize button 1 px
  tall), even with the system frame on; the system's own maximize button of our window did open
  it. `DwmExtendFrameIntoClientArea` with a top margin down to the button's bottom makes it open
  (1 px is not enough). Also: our own `TrackMouseEvent` is not needed, the system tracks the
  leave.
- The maintainer found the top strip ugly (checked with temporary light-theme builds, deleted). It was
  see-through, not black. Now the subclass undoes winit's 1 px strip in `WM_NCCALCSIZE` once the
  frame is extended (research R5); the frame stays extended without a button. Scripted: client
  1280 × 720 (no hidden row), the background meets the bar directly, flyout still opens; click
  maximizes and restores, the shadow is there, F11 and back returns to 1280 × 720.
- Scripted checks (Windows, release, 100 %, `scratchpad` snap.ps1 / click.ps1-style, injected
  mouse input; `SetCursorPos` alone is not enough for the flyout): hit test 9 only inside the
  button and below the 6 px band; the flyout opens over our button; click maximizes, second click
  restores; press then release over the video does nothing; highlight clears on leave.
- Checked by the maintainer: the strip is gone and the flyout opens. Not checked by hand: 125 % /
  150 %, and the system setting for the flyout turned off; look at them in T041's runs.

T041 (latency budget) Windows part done: 18 runs on the 1080p 144 Hz monitor and at 1440p through
DSR, results and verdicts in [measurements/latency.md](measurements/latency.md) (SC-001, SC-002
and SC-003 met with the relaxed texts, maintainer 2026-11-05), raw logs in `measurements/raw/`
(earlier logs from before the fixes in `raw/before/`). Committed (`docs: relax latency criteria
after the Windows measurements`, `docs: add Windows latency measurements`). CachyOS runs pending, and quickstart checks 2–6 and 12–18 of T041 not run formally (most were
checked task by task). Found and fixed during the run, all checked by the maintainer and folded
into their commits (rebased):
- `perf: keep UI updates off the video frame's path` (new commit, after T040): every UI image was
  presented on its own and the next video frame waited for the compositor's vsync (up to 14 ms).
  UI changes now ride on the next video frame (presented alone only when no frame came for
  50 ms), a UI image is uploaded as it arrives, and UI drawing is paced to the monitor's refresh
  also for input and resizes (pointer moves gave up to 413 images a second). Research R3.
- Folded into `feat: add docks and chrome layer`: the docks glided into place during a resize
  (their x/y were animated); now only a `hidden` factor is animated. A size change closes an open
  tooltip and menu (they stayed where their control had been).
- Folded into `feat: add focus ring, icon button, tooltip, menu, select and toggle`: the focus
  ring was a dot in the control's middle (a plain component has no size; it now inherits
  `Rectangle`). `Tooltips.dismiss()` added.
- `docs: add presentation setting for variable refresh monitors (F52)`.
Measuring notes: the agent launches each run in the background
(`capturax.exe --test-video 1080p60 --stats --windowed|--fullscreen | Out-File ...`) and the
maintainer acts and closes with Alt+F4; the window reopens where it was last closed (check the
monitor first); a click to focus changes the readings (focus = variable refresh on).

T042 (`app/src/i18n.rs`) done, checked and committed (`feat: add language list and resolution`): `LANGUAGES`,
`codes()` (now passed to `settings::Store`, replacing main.rs's temporary `KNOWN_LANGUAGES`),
`resolve` and `system_locale` (`sys-locale`; marked dead code until T044 uses them). Portuguese is
matched by the locale's language subtag (`pt`, `pt-PT`, `pt_BR.UTF-8`, any case; not `ptx`);
codes are exact (`pt-br` is unknown, so automatic). fmt, clippy, cargo-deny, 107 tests pass.
The CachyOS part of T041 is postponed to one Linux session with T045 (Linux) and T050.

T043 (`app/src/theme.rs`) done and committed (`feat: add theme resolution`; nothing to see until T045):
`SystemTheme { Dark, Light }` (new here; T045 converts winit's `Theme` or the portal's
`color-scheme` into it) and `resolve(ThemeSetting, Option<SystemTheme>) -> bool` (dark), both
marked dead code until T045. fmt, clippy, 110 tests pass.

T044 (apply the language) done and committed (`feat: apply the language`); fmt,
clippy, 111 tests pass. `main.rs` reads the saved language from the table before the
single-instance claim (`Settings::read`), so the second start's `already-open` message is in the
same language; `--lang` wins for this start and is never saved; logged as `language <code>`.
`Ui::new(language)` calls `ui::select_language` right after creating the component (Slint
requires a component, and otherwise keeps its own pick from the system locale). New
`i18n::setting` / `to_setting` (saved value ↔ `auto` or a code). The switch from the panel:
`SettingChange::Language(&'static str)` (dead code until T046 sends it) → `store.update` and
`App::apply_language`: select, tray `set_labels`, window title, the notice on screen read again,
then `language switch <code> <ms> ms` logged when the next UI image goes to the video thread
(`App.switch`, which T045 reuses for `theme switch`). `Notice.text` is now a
`fn(&Texts) -> SharedString`, so a notice is re-read after a switch. Not checkable before T046:
the switch and its log line.

T045 (follow the system theme) done, checked by the maintainer and committed
(`feat: follow the system theme`); fmt,
clippy (Windows; `platform` also for Linux), 111 tests pass on Windows (the 2 new Linux tests of
`linux/theme.rs` run in CI). winit 0.30.13 does not report the theme on Wayland or X11 (its docs),
so the portal code is in: new `platform::theme::{current, watch, Theme}` (research R8 outcome,
contract rows replace `system_theme_watch`). App: `App.system_theme` read in `resumed` right
after the window is created and before the first draw, `App::apply_theme` sets `Theme.dark`
(logo, shadows and backdrop follow it) from `theme::resolve(setting, system)`; changes come from
`WindowEvent::ThemeChanged` (Windows) and `UserEvent::SystemTheme` (Linux watch), each logged as
`system theme ...`; a change after the start logs `theme switch <dark|light> <ms> ms` with the
next UI image (through `App.switch`, shared with the language). `SettingChange::Theme` is dead
code until T046. `theme::SystemTheme` converts from winit's `Theme`. Start on this machine: log
`system theme Some(Dark)`, matching `AppsUseLightTheme = 0`. To check: switching Windows between
dark and light while the app runs: checked (Windows 11, release, 1080p60 test video), log
`theme switch light 60.9 ms`, `theme switch dark 38.2 ms` (SC-005 under 100 ms).

T067 (central settings panel) done, checked by the maintainer and committed (`feat: move settings
to a central panel with categories`); 111 tests pass. `SettingsPanel` (in a backdrop `Rectangle`
over the video area, `Theme.c.scrim`, a click on it closes) is centered, at most 760 × 520, 24 px
from the edges (compact: fills, 8 px). Categories General / Appearance / About (enum `Category`,
not saved, opens on General) as `CategoryItem` with Tabler icons (new `adjustments_horizontal`,
`palette`; `info_circle`) in a `base`-toned well; the chosen one tinted as the dock's toggled settings button (`accent-soft`, icon `accent-icon`,
name `accent-text`; maintainer's choice after trying a solid accent fill, which read as the
record / live buttons; a selection bar and a divider line were refused, FR-016).
Compact: icons only, names in tooltips (500 ms hover or keyboard focus). About: the wordmark
(32 px, by theme) and the version. Focus: the chosen category takes the focus on creation;
`IconButton.focusable` keeps the docks out of the Tab order while the panel is open;
`reveal-chrome` does nothing then; closed from the keyboard, the focus goes back to the settings
button. The chrome is no longer held shown by the panel. New texts (General, Appearance, About)
are not in the pt-BR catalog yet (T049).

T046 (Language and Theme rows) done, checked by the maintainer and committed (`feat: add language
and theme settings`); fmt, clippy (Windows), 113 tests pass. `SelectRow` (name and a `Select`,
at most 200 px) under Appearance. Language: `Prefs.languages` is a `[MenuItem]` model built by
Rust (`app.rs::set_language_items`: new `Texts.automatic`, then each `LANGUAGES` native name), set
again in `apply_language` so Automatic follows the switch; `MenuItem` is exported from main.slint.
Theme: items inline in Slint (`@tr` Automatic, Dark, Light). Indexes ↔ settings in the pure
`i18n::choice_index` / `choice` and `theme::choice_index` / `choice` (tested; an unknown language
code shows Automatic). Menu scrolling at 640 × 360 cannot be exercised: the menus have 2–3 items.
New texts (Language, Theme, Automatic, Dark, Light) wait for T049.

T048 (`app/tests/contrast.rs`) done and committed (`test: add text contrast check`): reads the
`Palettes` tokens of theme.slint and checks a declared pair list at 4.5:1 for text and 3:1 for
control icons (FR-018 extended by the maintainer, 2026-11-06), with hover and press tints blended
and the dock's panel over a white and a black video. The test found the toggled dock tool's icon
(`accent-icon` on `panel + accent-soft`) at 1.94-3.24:1 depending on the video. Fixed (folded into
the icon button and central panel commits): a toggled tool sits on an opaque chip (`raised` under
`accent-soft`); the `accent-icon` token is gone, toggled tool and chosen category icons use
`accent-text` like the category name (light theme #9e4500: 5.14 / 4.60 / 4.21:1 at rest, hover,
press). Tried and refused by the maintainer, compared side by side: a deeper orange `#c25000`,
a white chip without tint, a dark chip, a solid orange chip (reads as record / live).

T047 (`app/tests/i18n.rs`) and T049 (pt-BR catalog, 20 texts added; no contrast fix needed) done,
fmt, clippy (Windows), 118 tests pass; Portuguese texts checked in the app by the maintainer;
committed (`feat: complete the Portuguese catalog`, `test: add translation check`). The test extracts `@tr` literals (escapes, `{}`,
struct literal fields; fails on `@tr` contexts or plurals, which the catalogs do not use),
reads the `.po` files (fuzzy counts as untranslated) and fails on a non-empty literal in a
property named `text`, `title`, `tooltip`, `label` or ending in `-label` etc. without `@tr`.

T050 Windows part done (2026-11-06), recorded in [measurements/latency.md](measurements/latency.md)
"Windows 11, settings states (T050)", raw logs `measurements/raw/win-{1080,1440}-{win,fs}-{menu,language,theme}.txt`
(+ `win-1080-win-language-2.txt`, a repeat, and two isolated tests `win-1080-win-title-test.txt`,
`win-1080-win-flush-test.txt`). Quickstart 1 (shown in 396 ms, pt-BR, dark) and 7 checked by the
maintainer. Menu and theme states meet SC-001/SC-002; SC-005 met (language 1.1–4.2 ms, theme
2.9–37 ms). Language switches stall the video (and the main thread) 110–960 ms about one time in
ten; not the window title, not the settings save on this HDD (isolated tests). Cause unknown; the
maintainer accepts it (settings, not real use). Unchecked in tasks.md until the Linux part.
The analysis script used: `measurements/raw` lines `[loop]`, first 3 s and the last line left
out, per run: seconds, median range, seconds with max > 3 ms, `pres` 59–61.

T051 (`crates/settings/src/location.rs`) done and committed (`feat: add settings location
resolution`; nothing to see until T053): `resolve(app_folder, app_writable, user_folder:
Option<&Path>, user_writable, FileTimes { app, user }) -> Location { kind: LocationKind, read_from }`
with `folder()`, `file()` and `load_path()`. Choices: a per-user file wins only when strictly
newer (same time → the app folder's file); in memory `read_from` is the newer of the two files
found, so a read-only setup still starts with the user's values. Table test for every branch.
T052 done and committed (`feat: add per-user settings folder and write probe`):
`platform::user_config_folder()` (`dirs::config_dir()` + `CapturaX` / `capturax`; `dirs` added to
`platform`) and `platform::is_writable(folder)` in lib.rs (shared: creates the folder, then
creates with truncate, not `create_new`, so a probe left by a killed start does not count, and
removes `.capturax-write-probe`). fmt, clippy (Windows; `platform` also for Linux), cargo-deny ok,
tests pass.
T054 (`crates/settings/tests/interrupted_save.rs`) done and committed (`test: add interrupted save
check`): the child is the test `child_saves` of the same binary, selected by
`CAPTURAX_INTERRUPTED_SAVE_FOLDER`; it writes `
saving
` straight to stdout (libtest leaves
"test child_saves ... " open on the line). Kill after 0–20 ms (xorshift, no `rand`). Windows run:
100 kills in 4.4 s, A 54 / B 46, file always valid. 124 tests pass in the workspace.
Found: a kill leaves the save's `.capturax-*.tmp` beside the file (78 of 100 kills). Harmless to
loading, but they pile up in the folder; real saves are rare and short, so it needs a crash or
power loss during one. Open: remove stale `.capturax-*.tmp` at start in T053 (spec/data-model
first), maintainer to decide. Decided (maintainer, 2026-11-06): remove them; contracts/settings-file.md
rule 6 and data-model.md "Settings location" updated.

T053 (location wired) done, checked by the maintainer (folders `target/check-readonly` and
`target/check-damaged`) and committed (`feat: keep settings in the per-user folder or in memory`). fmt, clippy, 125 tests pass.
- `main.rs::settings_location`: probes the app folder; the per-user folder only when the app
  folder is not writable (so `%APPDATA%\CapturaX` is not created needlessly); file times from
  opening `capturax.toml` in each. Logger in `location.folder()` (stderr in memory); first log
  line `settings: <kind>`, then `settings read from <path>` when `read_from` is used. Loads
  `load_path()`; a first start or damaged file saves at once, as before; in memory nothing.
- Single-instance name from `location.file()`, in memory from the app folder's file. After the
  claim, `settings::file::remove_temps(folder)` (logged when it removed any).
- Notices: `notices::settings_fallback(folder)` (info), `settings_not_kept()` (warning),
  `settings_damaged(kept_as)` (warning; shows the kept file's name, or a text without it when the
  rename failed). `Notice` gained `arg` and `text: fn(&Texts, &str)`; the texts are public pure
  functions in the `Texts` global (`invoke_settings_*` from Rust), 4 new texts with pt-BR. Queued
  in `start` right after `App::new`.
- Scripted checks (Windows, release, copies of the exe in the scratchpad, `icacls /deny`
  W,D,DC): read-only folder with a pt-BR / light `capturax.toml` → UserFolder, file read from
  the app folder, notice in Portuguese naming `C:\Users\...\Roaming\CapturaX`, nothing written
  there until a change; also `%APPDATA%\CapturaX` denied → memory, warning notice, light theme
  from the read-only file, no log in AppData; garbage `capturax.toml` and two `.capturax-*.tmp`
  in a writable folder → damaged notice naming `capturax.toml.damaged-<s>`, both temps removed,
  defaults saved. Test folders and `%APPDATA%\CapturaX` removed afterwards.
- The fallback notice wrapped the path after `C:` (Slint's line breaking); the path now has its
  own line (maintainer).
- Maintainer (2026-11-06): the small windows share one header, icon + title + close control
  at the other end (new `components/panel-header.slint`). The settings panel's header got the
  settings icon (folded into `feat: move settings to a central panel with categories`, FR-025 and
  plan into its docs commit); notices got a title of their own (`Texts.settings-*-title`, pt-BR)
  above the text, aligned with the title (folded here, FR-034 and data-model updated). Checked by
  the maintainer with the `target/check-*` folders.

T055 Windows part done (2026-11-06): quickstart 8–11 checked by the maintainer, all pass, with
copies of the release exe in `target\check-t055\row{8,9,10,11}` (removed afterwards, with
`%APPDATA%\CapturaX`). Row 10 also checked that a change survives a restart in the per-user folder
(not covered by T053's scripted checks). Note for read-only folders: `icacls /deny user:(W)` on a
folder also denies SYNCHRONIZE, so the folder cannot even be listed; use
`(WD,AD,WEA,WA,DC)` instead. Unchecked in tasks.md until the Linux part.

T056–T058 done (2026-11-06), checked on Windows by the maintainer (T059 Windows part: no source
without a source, hidden with `--test-video 1080p60`, `--backend gl` message in English and
pt-BR, exit 2; notice dismissal as in T053/T055) and committed (`feat: add state message and
renderer failure message`). T059 unchecked until the Linux part (`--backend dx12`).
- `components/state-message.slint`: `StateMessage` (icon, title, one-line `detail` cut with an
  ellipsis, `primary` / `secondary: StateAction { id, label }`, empty id = none,
  `action(id)`), on `deep`; a private `ActionButton` (primary on the accent, secondary on
  `top`, hover / press, Enter or Space, focus ring). Preview `ui/previews/state-message-preview.slint`
  (`slint-viewer` 1.18.1 installed with `cargo install`; a click on the background switches
  the theme). `ui/previews` is left out of the translation test (sample texts).
- `no-source`: icon `device_tv_off` (Tabler 3.31.0, maintainer's pick against
  `plug_connected_x`, which is for a disconnected device later; `preview_off` is kept for the
  hidden preview), title `@tr("No source")`, no action; `AppWindow.source-open` set from
  `app.rs` (test video running). Unknown actions are logged in `ui.rs`.
- `renderer-unavailable`: `App::renderer_unavailable()` gives `Texts.app-name` and
  `Texts.renderer-unavailable`; main.rs shows it after the loop when the exit code is 2 (also
  when the window cannot be created).
- Contrast pairs added for the state message on `deep`.
- Product decisions recorded in docs/product.md (own `docs:` commit): the no-source action opens
  the app's source picker (the toolbar's), not one of its own; F10 hiding the preview releases the
  device and shows a "preview hidden" state, whether it also stops listening is open. The term
  "preview" stays (maintainer: it tells at once what it is); do not propose renaming it.

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

- Resolved, kept for the record: text sharpness (maintainer, 2026-11-05): text in menus and dock tooltips still reads worse than
  in the title bar's tooltips, although all use the same `Tooltip` component and font (15 px).
  Snapping positions to whole physical pixels did not help (T031). After this spec, a dedicated
  investigation to fix it for good: find what differs (glyph position, background under the
  text), then compare Slint's software renderer against a renderer with gamma and hinting
  (Skia), or drawing the UI at 2× (about 4× the drawing cost). Decide with the maintainer.
  The maintainer wants this right after T034, and suspects the popovers' transparency, though at
  125 % it reads well. Check also the path of the UI image: menus and tooltips are opaque
  (`Raised`, fill drawn in the Rust image), but the whole UI is composited over the video by the
  video thread, so any resampling or alpha handling there would blur only some text.
  Findings (2026-11-05): the composite is 1:1 (full-surface quad, texture the surface's size), not
  the cause. Slint 1.18's text path (`draw_glyph_run` in i-slint-renderer-software, parley) puts
  each glyph at a quarter-pixel x and rasterizes it there with swash, unhinted, no gamma; the
  baseline is snapped to a whole pixel. Inter's 15 px advances are fractional, so inside one word
  some letters land on the pixel grid (sharp stems) and others half a pixel off (stems smeared
  over two columns): measured in offscreen renders, the two `l` of "Fullscreen" have different
  pixels. Which letters blur depends on the word and its x, so "Close" happened to be sharp and
  snapping the tooltip did not help. The bundled Inter (variable) has no hinting program (`fpgm`).
  The maintainer then saw that text is very good in fullscreen and poor only in a window. Real
  cause, measured (screen captures against offscreen renders, Windows, 100 %): winit's undecorated
  shadow (`WM_NCCALCSIZE`: `top += 1; bottom += 1` when not maximized) leaves 719 visible rows of
  the 720-row client, and Windows squeezes the 720-row swapchain into them. Near the top the shift
  is about 0 (title bar sharp), mid-window about half a pixel (docks, panel, menus blurred).
  Without the shadow, or with the surface and UI drawn 1 row shorter (719), the screen equals
  the render exactly (difference 0). Maximized (client = work area) and fullscreen (shadow off,
  `set_fullscreen_frame`) are not affected.
  The maintainer checked a 719-row test build by eye: fixed. Implemented (research R5,
  contracts/platform.md and T031 updated first): `platform::drawn_size(window)`, on Windows
  measured from the rectangles (client rows and columns past `GetWindowRect`), not from winit's
  flags; Linux returns the inner size. `app.rs` uses it at start, on `Resized` and again in
  `about_to_wait` after a move or resize; the resize band uses `ui.size()`. Screen capture of the
  windowed app with the panel open equals the exact reference (difference 0 at the top, the panel
  and lower). fmt, clippy (Windows; `platform` also for Linux), 81 tests pass. Checked by the
  maintainer (windowed, maximized, restored, snapped, fullscreen) and committed on its own
  (`fix: draw at the window's visible size`): it uses T032's code, so it could not be folded
  into `feat: handle scale changes` without rewriting the later commits (maintainer's choice).
  The quarter-pixel glyph placement remains, but the maintainer finds fullscreen text very good,
  so patching Slint is not pursued. The test binaries and the offscreen render example were
  deleted. Closed.
- Settings panel shape: decided (maintainer, 2026-11-05): a central panel with categories
  (General, Appearance, About) in a left column, a tab row in the compact layout; the video is
  dimmed behind it and a click outside closes it. FR-025, plan.md (UI structure) and the new
  T067 (before T046) updated.

- The record / stream only mode may change the product's main scenario and the roadmap's order;
  product.md asks to decide it before spec #3 starts.
- Windows stalls at presentation mode changes (T041): a focus change stalls the video 229–401 ms
  on the native 1080p monitor, fullscreen gives one 51–73 ms stall about 18–20 s after entering it,
  and one unexplained 625 ms stall happened once. At 1440p through DSR the focus change and drags
  do not stall. Hypothesis: the swap chain moving in and out of its own hardware plane (MPO /
  independent flip). Planned for spec #3 (roadmap #3, research R4); the temporary per-stage
  timing used to look for the 625 ms stall was not kept.
- Language switch stalls (T050): about one language switch in ten stalls video and UI for
  110–960 ms on Windows (theme 2 in 321, menu 1 in 269 s). Not the title, not the save. Accepted
  by the maintainer for this spec; look at it with the Windows stalls in spec #3 (per-stage
  timing), or before if it shows in real use.
- G-SYNC: with G-SYNC set to full screen only, the focused windowed app still makes the monitor
  follow it (cursor at about 60 Hz). Kept; F52 (spec #3) adds a "follow the monitor" mode.

## Next step

One CachyOS KDE Plasma Wayland session (maintainer's PC), release build, test video
`1080p60`, results into measurements/latency.md "CachyOS KDE Plasma Wayland", raw logs
`measurements/raw/linux-*.txt`, GPU clock state (`nvidia-smi -q -d PERFORMANCE`) per run:
1. T041 Linux: the runs of the Windows table (idle, chrome, panel; window and fullscreen; 1080p
   and 1440p; unfocused if the monitor has variable refresh) and SC-003 moving and resizing.
2. T045 Linux: Theme Automatic follows a Plasma color scheme change (portal), `theme switch`
   logged.
3. T050 Linux: quickstart 1 and 7, then menu, language and theme runs as on Windows.
4. T055 Linux: quickstart 8–11 (read-only folder with `chmod a-w`; also with `$APPIMAGE` set to
   a file in another folder).
5. Quickstart 2–6 and 12–18 if wanted formally.
6. T059 Linux: US4 scenarios, renderer message with `--backend dx12`, in both languages.
Then T041, T050, T055 and T059 are checked. Before that session, on Windows: T063/T064.
