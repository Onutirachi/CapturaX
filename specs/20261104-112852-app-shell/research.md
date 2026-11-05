# Research: App Shell

Decisions for the plan. Each entry: decision, rationale, alternatives considered. The render
architecture comes from the stack spike ([findings](../20261026-154808-stack-spike/findings.md)
"Decisions", [research](../20261026-154808-stack-spike/research.md) R13, R14, R15) and is not
reopened here; entries below say how the shell builds it.

## R1. Toolchain and versions

- **Decision**: Rust 1.99 (pinned in `rust-toolchain.toml`, edition 2024), Slint 1.18 (at least
  1.18.1, the version the spike measured; the first task moves to the newest 1.x patch if one
  exists and re-runs the render test screen; checked 2026-11-04: 1.18.1 is still the newest
  1.x), winit 0.30, wgpu 30, gstreamer-rs 0.25 against
  GStreamer 1.28 on developer machines (the spike's versions, research R1 of the spike).
- **Rationale**: the spike validated exactly this set on both platforms; the latency budget
  (SC-001) is stated against its numbers.
- **Alternatives**: winit 0.31 or a newer wgpu — not validated by the spike; upgrading is a
  separate change with its own measurement.

## R2. Code layout

- **Decision**: a Cargo workspace with three crates:
  - `app` (binary `capturax`): startup, window and event loop, Slint bridge, video thread,
    test video, stats, i18n and theme resolution, window placement.
  - `crates/settings`: the settings file (format, lenient load, unknown values kept, atomic save,
    location choice from probe results). Pure Rust, no OS calls beyond `std::fs`.
  - `crates/platform`: every OS-specific call (application folder and AppImage, per-user folder,
    single instance, autostart, tray, system theme on Linux, system message box, Windows snap
    layouts flyout). The only crate that depends on `windows-sys`, `tray-icon`, `zbus` or `ksni`.
- **Rationale**: principle IV is enforced by the dependency graph: `app` cannot call an OS API
  that `platform` does not expose, and CI checks that no other crate depends directly on the OS
  crates (R13).
  `settings` is the logic with the most edge cases (US3, SC-007) and gets its own test suite.
- **Alternatives**: one crate with modules — principle IV would rest on review only; a crate per
  concern (render, i18n, theme) — more boundaries than the shell's size justifies.

## R3. Threads and the Slint bridge

- **Decision**: as the spike after T052 (research R15):
  - **Main thread**: winit event loop, the Slint platform (one `MinimalSoftwareWindow`), window
    actions, settings changes. It draws the UI with Slint's software renderer
    (`RepaintBufferType::ReusedBuffer`) only when Slint reports a change, at most once per monitor
    refresh, and hands the finished image to the video thread.
  - **Video thread** (`video`): owns the wgpu device, surface and layers; keeps only the newest
    video frame, UI image and scene in an inbox (mutex + condvar); composites and presents when a
    video frame arrives; counts the stats (R9). A UI image or scene change is presented on its
    own only while the video is not flowing (no frame yet, the window was hidden, or no frame for
    50 ms); while it flows, the change rides on the next video frame (see below).
  - **Test video**: a GStreamer pipeline whose appsink callback (GStreamer's streaming thread)
    puts each frame in the inbox and wakes the video thread.
  - **Settings saver**: a thread that writes the file 300 ms after the last change (R6).
- The spike's later decision "the UI renders on its own thread" is met by this split: the UI
  thread is the main thread, and the video thread never waits for it; it composites the last
  finished UI image (FR-001, FR-002). A video frame arriving while the UI draws is presented at
  once with the previous UI image.
- **UI presents ride on video frames** (T041, 2026-11-05): presenting every UI image on its own
  failed SC-001 while the chrome animated. The surface keeps one present in flight
  (`desired_maximum_frame_latency = 1`), so after a UI-only present the next acquire waits for
  the compositor's vsync, and a video frame arriving then waited up to 13–14 ms (median up to
  8 ms in seconds with 80–90 UI images; idle stayed 1.2 / 1.6 ms). Measured in
  measurements/raw/win-1080-win-chrome.txt. Cost of the change: the UI shows at most one video
  frame later (≤ 16.7 ms at 60 fps) and animates at the video's frame rate. Rejected: a deeper
  swap chain queue (the measured time improves but the frame reaches the screen a vsync later);
  predicting the vsync to fit a UI-only present before the next frame (fragile, little gain).
  With that alone, opening the settings panel still gave one second at 3.5 ms max: the UI image
  was uploaded between the frame's arrival and its present. The video thread now uploads a UI
  image as soon as it arrives, while it waits for the next frame.
- Slint timers and animations run from the main loop (`update_timers_and_animations`, winit
  `ControlFlow::WaitUntil` the next Slint timer). They pause while Windows holds the main thread
  during a window move (expected, spike T052); the video does not.
- **Alternatives**: Slint's own window — 1.5 frames slower (spike SC-002); UI on a third thread —
  Slint components are not `Send`, and winit events would have to cross threads for nothing.

## R4. Presentation choice and renderer failure

- **Decision**: the video thread tries, in order, and keeps the first that creates a surface:
  Windows — Direct3D 12 Mailbox, Direct3D 12 Immediate, Direct3D 12 Fifo, Vulkan Mailbox, Vulkan
  Fifo; Linux — Vulkan Mailbox, Vulkan Immediate, Vulkan Fifo, OpenGL Fifo. The choice is written
  to the log and to the stats header (FR-003); `--backend` and `--present` override it for
  measurements (contracts/cli.md). If none works, the app shows the system message
  `renderer-unavailable` (FR-036) and exits with code 2.
- The message is translated: the Slint UI is created before the GPU (the software renderer needs
  no GPU), so the text is read from the `Texts` global in the resolved language and handed to
  `platform::show_system_message`.
- **Rationale**: the spike measured DX12 + Mailbox and Vulkan + Mailbox fastest; Mailbox does not
  tear and matched Immediate on DX12.
- **Alternatives**: Immediate first — tears with no measured gain on DX12; letting wgpu pick the
  backend — on Windows it may pick Vulkan, ≈ 15 ms slower through DWM.
- **Variable refresh (found in T041, 2026-11-05)**: on the maintainer's 144 Hz G-SYNC monitor,
  with G-SYNC set to full screen only, the monitor follows the windowed app's presents while
  CapturaX has focus, so the whole desktop, cursor included, runs at about 60 Hz (also while
  other windows animate); without focus, or with G-SYNC off, the compositor runs at 144 Hz.
  Likely cause, not confirmed: Windows promotes the window's DX12 flip-model swap chain to its
  own hardware plane (independent flip), which the driver treats as full screen; PresentMon's
  present mode would confirm it. Kept on purpose: with variable refresh each video frame shows
  at once and evenly. There is no public Windows API for a window to refuse it (only NVIDIA's
  NVAPI driver profiles); users can turn it off for `capturax.exe` in the NVIDIA Control Panel
  (Manage 3D settings → Program settings → Monitor Technology: fixed refresh). Measured `ready_ms` differs between the two (median about
  1.0–1.2 ms with focus, 1.4–1.6 ms without), and a focus change gives one second with a 13–19 ms
  max while the driver switches; T041 measures both. A setting to present at the monitor's
  refresh instead, keeping the cursor smooth at some latency cost, is F52 in docs/product.md,
  for spec #3.

## R5. Frameless window on Windows and Linux

- **Decision**: the spike's T037 behavior, ported: winit without decorations (on Windows with
  `with_undecorated_shadow` for the system shadow and rounded corners); the title bar starts the
  system move (`drag_window`) after 4 px so a double click reaches the bar; a 6 px band along the
  edges and corners sets the resize cursor and starts `drag_resize_window` (off when maximized or
  fullscreen); fullscreen is borderless on the current monitor (DXGI flip model gets independent
  flip there, spike T047). Minimum inner size 640 × 360 logical (FR-009).
- **Shadow off in fullscreen (Windows)**: for the undecorated shadow, winit moves the client area
  down 1 px in `WM_NCCALCSIZE` (`top += 1`), so a 1 px strip painted by the system stays along
  the top, also in fullscreen; after a snap it keeps the snapped window's old pixels (a white line
  as wide as the snapped window, found in T029's check). `platform::set_fullscreen_frame` turns
  the shadow off before entering fullscreen and back on after leaving it. Windowed, the 1 px strip
  stays (winit's own trade-off for the shadow).
- **One row less while windowed (Windows)**: winit also moves the client's bottom down 1 px
  (`bottom += 1`) when the window is not maximized, so only 719 of a 720-row client are inside
  the window, and the system squeezes a 720-row swapchain into those 719 rows. Everything is
  resampled vertically: about 0 px of shift at the top, half a pixel mid-window, so text in the
  docks, panels and menus reads blurred while the title bar stays sharp (found while
  investigating text sharpness, measured by screen captures against offscreen renders: no
  whole-row shift matched). Drawing the UI and the surface 1 row shorter makes the screen equal
  the render exactly. `platform::drawn_size(window)` gives that size: the inner size minus one
  row while the window is neither maximized (winit sets the client to the work area then) nor
  fullscreen (shadow off); the inner size otherwise and on Linux. The app uses it for the UI,
  the surface and the resize band, and reads it again once the events of a change are handled,
  because `is_maximized` turns true after the first events of a maximize (T032).
- **Dragging across monitor scales (Windows)**: on `WM_DPICHANGED` during a drag, winit keeps
  the size the system suggests but moves the window so that the pointer keeps its place along the
  title bar. That can put the window's center back on the old monitor (from 100 % to 150 % it
  landed exactly on the edge), and the system sends the opposite `WM_DPICHANGED` at once.
  After that nested change the system's move loop keeps a wrong size: on every mouse move it
  set the window to 853 × 480 instead of 1280 × 720, so the window shrank (or grew the other way)
  and left the pointer. This was found in T031's check, reproduced with a script and the message
  log. `platform::follow_dpi_rect` subclasses the window: during a move or resize loop
  (`WM_ENTERSIZEMOVE` to `WM_EXITSIZEMOVE`), the `WM_WINDOWPOSCHANGING` that winit's handler
  sends while answering `WM_DPICHANGED` gets the suggested rectangle. The suggested rectangle
  keeps the window on the new monitor and in step with the move loop. Outside a loop (keyboard
  moves, a scale changed in the system settings, maximized, fullscreen) winit's own handling
  stays.
- **Snap layouts flyout (FR-011, best effort)**: in `platform::windows::snap`, subclass the winit
  window (`SetWindowSubclass`) and answer `WM_NCHITTEST` with `HTMAXBUTTON` inside the maximize
  button's rectangle, which the app reports in physical pixels whenever the laid-out rectangle
  changes (none in fullscreen or while a menu is open; the top resize band is left out of it).
  Because the system then sends non-client messages over the button, the subclass turns
  `WM_NCMOUSEMOVE`, `WM_NCMOUSELEAVE`, `WM_NCLBUTTONDOWN` and `WM_NCLBUTTONUP` into the button's
  hover, press and click; presses are not passed on (no classic caption button), moves are (they
  start the flyout and the system's own leave tracking, so no `TrackMouseEvent` of ours).
  Found in T040: the hit test alone is not enough. The system took the caption to be the 1 px
  strip winit leaves above the client for the shadow (`GetTitleBarInfoEx`: a maximize button
  1 px tall) and opened no flyout over our button, even with the system frame turned on and
  `HTMAXBUTTON` in the client area. The system's own maximize button of the same window did open
  it. Extending the DWM frame into the client down to the button's bottom edge
  (`DwmExtendFrameIntoClientArea`, top margin only; 1 px is not enough) makes the flyout open.
  The video covers the extended area with opaque pixels: the title bar's pixels are unchanged.
  The 1 px strip above it, though, became see-through (the desktop behind showed; black over a
  dark background), which the maintainer found ugly; `DWMWA_CAPTION_COLOR` does not change it.
  So once the frame is extended, the subclass undoes winit's strip in `WM_NCCALCSIZE` (client =
  the whole window while not maximized; maximized keeps winit's work area): the extension now
  gives the shadow, the usual frameless-with-shadow technique. `drawn_size` then finds no hidden
  row. The frame stays extended when the button goes away (fullscreen, a menu open); only the hit
  test stops. Timeboxed to one day; dropped and recorded as a known limit if moving, resizing or
  the button's own hover and click break (spec FR-011).
- **Fast moves at 40–50 fps on Windows (spec edge case)**: accepted as a known limit and not
  investigated (maintainer, 2026-11-04). The acceptance checks only that the window moves
  smoothly and the video never stalls (SC-003); the stats' `gap_ms` shows a stall if one returns.
- **Alternatives**: a system frame — not the Float look; drawing our own non-client area through
  `WM_NCCALCSIZE` — more Windows code for what winit already does.

## R6. Settings file

- **Decision**: TOML (`toml` crate, order kept), file `capturax.toml`, log `capturax.log`, both in
  the settings location (data-model.md). Format and rules in
  [contracts/settings-file.md](contracts/settings-file.md).
  - **Lenient load**: each known value is read on its own; a missing or invalid value takes its
    default (unknown language or theme → Automatic), the rest of the file still counts. Only a
    file that is not valid TOML (or not UTF-8) counts as damaged (FR-030).
  - **Unknown values kept**: the loaded document is kept as a table; saving writes the known
    values into it, so unknown keys and categories survive (FR-030, newer versions).
  - **Atomic save**: write a temporary file in the same folder, flush and `sync_all`, then replace
    the target (`tempfile::NamedTempFile::persist`: `rename` on Linux, `MoveFileExW` with
    replace on Windows), then sync the folder on Linux (FR-029, SC-007).
  - **Debounce**: saved 300 ms after the last change by the saver thread, and flushed on quit.
  - **Damaged file**: renamed to `capturax.toml.damaged-<unix seconds>`, defaults used, notice
    shown.
- **Location** (FR-026–FR-028): application folder = folder of the executable, or of `$APPIMAGE`
  when set (Linux AppImage). It is writable when a probe file can be created and removed there.
  Order: application folder if writable; else per-user folder (`%APPDATA%\CapturaX` on Windows,
  `$XDG_CONFIG_HOME/capturax` or `~/.config/capturax` on Linux, through `dirs`); else memory
  only. A readable `capturax.toml` in a read-only application folder is loaded first and later
  saves go to the per-user folder (spec edge case). If the per-user folder already has a newer
  file than the read-only one, the per-user file wins (it holds the user's later changes).
- **SC-007 test**: an integration test spawns a child process that saves in a loop and kills it at
  a random moment, 100 times, then checks the file parses and holds either the old or the new
  values.
- **Alternatives**: JSON — no comments, less pleasant to edit by hand; RON — less known; a
  `serde` struct with `#[serde(flatten)]` for unknown keys — rejects the whole file on one bad
  value, which the edge cases forbid.

## R7. Languages

- **Decision**: Slint bundled translations, as the spike (T036): English source strings in the
  `.slint` files with `@tr`, gettext catalogs in `app/lang/<code>/LC_MESSAGES/capturax.po`
  compiled into the binary, `slint::select_bundled_translation` to switch at runtime. Text that
  Rust needs to show (notices, tray menu, system messages) is never built in Rust: Rust passes an
  identifier and the text comes from the `Texts` global in Slint, so every text has one catalog.
- The language list (code and name in itself) is a constant in `app/src/i18n.rs`; adding a
  language is a new `.po` and one line there (FR-024).
- **System language**: `sys-locale`; any `pt*` → `pt-BR`, anything else → `en` (FR-022, edge case
  pt-PT).
- **CI check (FR-039)**: an integration test in `app/tests/i18n.rs` extracts every `@tr("...")`
  from `app/ui/**/*.slint`, compares the set with the msgids of every `.po` (missing, empty or
  stale entries fail, naming them), and fails on a string literal bound to a `text` property
  without `@tr`. Rust code passes identifiers, so it has no user-facing literals to check; the
  tray and system message texts are read from `Texts` and covered by the same test.
- **Alternatives**: `fluent` or `gettext` in Rust — a second catalog next to Slint's; Slint's
  `slint-tr-extractor` in CI — an extra tool install for what a 60-line test does.

## R8. Theme

- **Decision**: one Slint global `Theme` in `app/ui/theme.slint` holding every token (colors,
  type, spacing, radius, shadow) for Dark and Light, selected by a `dark` property (FR-016). The
  tokens are the spike's T027 values. Rust resolves Automatic to Dark or Light and sets `dark`.
- **System theme**: Windows — winit `Window::theme()` and `WindowEvent::ThemeChanged`. Linux —
  winit 0.30 does not document system theme detection on Wayland or X11, so
  `platform::linux::theme` reads the XDG Settings portal (`org.freedesktop.appearance`
  `color-scheme`) with `zbus` (blocking API) and listens to `SettingChanged` on a thread. If no
  portal answers, Automatic resolves to Dark. The first Linux task checks whether winit already
  reports it; if it does, the portal code is dropped.
- **Contrast (FR-018)**: `app/tests/contrast.rs` reads the color tokens from `theme.slint` and
  checks a declared list of text-on-surface pairs at 4.5:1 in both themes; for semi-transparent
  panels over the video it checks against a white and a black video behind them.
- **Shadows above 100% (spike open question)**: the nine-slice shadow images are generated in
  Rust at 1× (a port of the spike's `tools/make_shadow.py`: rounded rectangle, two Gaussian
  layers, hollow centre), once at start, and handed to Slint as images with their nine-slice
  edges (`Image::set_nine_slice_edges`). Slint 1.18 draws a nine-slice corner at its edge size
  in image pixels times the window's scale (`fit9slice`), so an image made at the window's scale
  would be enlarged twice; and the software renderer scales images by nearest pixel, not
  smoothly. Maintainer decision, 2026-11-04. **Replaced after T031's check (2026-11-05)**: at
  125% and 150%, the nearest-pixel enlargement made the shadows look low resolution. It also put
  the cut-out 1 px off the surface: a light pixel between the surface and its shadow on one side,
  and a dark one on the other. Screenshots over a white test video measured it.
- **Shadows, current decision: one image per surface at its exact physical size.**
  `Theme.surface` is a pure callback that Rust implements. `Raised` calls it with its size, the
  window's scale (`1px / 1phx`), its fill and the theme's shadow strength, so the binding runs
  again on a resize, a scale change or a theme change.
  - For each size, Rust draws the two layers (soft and tight) into one image, in physical
    pixels. The Gaussian is approximated by three box blurs, which cost O(n) whatever the sigma,
    so even a large menu at 150 % takes a few milliseconds. Images are cached by size and
    scale.
  - Slint draws the image at its own size (1:1), so nothing is enlarged. The element sits 36 px
    out from the surface, the soft layer's margin; 36 × each Windows scale step (25%) is a whole
    number of physical pixels.
  - The image also holds the surface's fill, as `Theme.surface(width, height, scale, fill,
    shadow)`. Fill and cut-out share one coverage per pixel, so the edge and the shadow always
    meet. Two earlier tries failed the maintainer's check. With Slint drawing the fill and the
    cut-out on the surface's shape, the two anti-aliasings disagreed on the curves and left small
    dark or light "teeth" at the bottom corners. With the cut-out 1 px inside, the dense shadow
    under the edge made a dark outline. The shadow is still cut out under the fill, so a
    semi-transparent fill (`panel`) does not darken. The theme's shadow strength goes into the
    image, not the element's opacity, which would fade the fill too.
- **Alternatives**: images generated at the window's scale with nine-slice — Slint enlarges them
  again (above); an exact Gaussian per surface size — the same look, but tens of milliseconds
  for a large menu at 150 %; one blurred nine-slice source per scale, copied out to each size
  — its middle is uniform only past 3 sigma from the corners, too large for a 56 px dock; @1x and @2x image sets — wrong at 125% and 150% for the same
  reason; theme values in Rust pushed into Slint — the `.slint` preview loses them.

## R9. Test video and statistics

- **Decision**: `--test-video <WxH@fps>` (presets `1080p60`, `1440p60`) builds a GStreamer
  pipeline `videotestsrc is-live=true pattern=ball` (moving) → RGBA caps → appsink
  (`sync=false`, `max-buffers=1`, `drop=true`), the same appsink hand-off spec #3 uses for real
  devices. `--stats` prints one line per second (contracts/stats.md): frames received, presented,
  dropped in the inbox, frame ready → presented median and max, longest gap between presents,
  UI images, UI draw time median and max, window moves, presentation in use.
- The app links the GStreamer libraries, so they must be present to start at all: in this spec's
  development builds from the GStreamer install (`bin` on `PATH` on Windows); in the portable
  package they ship next to the executable or inside the AppImage (constitution VI, spec #13).
  Every real use needs GStreamer anyway; there is no start without it to support.
- A frame is ready when the appsink callback hands it to the inbox, after copying it out of the
  GStreamer buffer, as the spike measured it (SC-001 compares with the spike's numbers).
- While the test video plays, `platform::precise_timers()` keeps the Windows timer resolution at
  1 ms (`timeBeginPeriod`; Linux: nothing). At the default 15.6 ms, `videotestsrc` paces its
  frames to the timer's ticks: 60 per second, but in bursts with gaps of 25–32 ms, so `gap_ms`
  could not show the video loop's own gaps (measured 2026-11-04: `gap_ms` 32 → 18 ms at 60 fps).
  Windows 11 ignores the request while the window is covered unless the process opts out with
  `SetProcessInformation(ProcessPowerThrottling, IGNORE_TIMER_RESOLUTION)`, which the guard also
  does (without it, some runs fell back to 31 ms gaps).
  Spec #3 decides whether capture devices need it too.
- **Rationale**: constitution III (media on GStreamer); spec #3 reuses the path and the stats.
- **Alternatives**: a Rust frame generator — custom media code the constitution asks to avoid,
  and it would not exercise the appsink path.

## R10. Single instance

- **Decision**: a local socket named from a hash of the settings location (or of the executable
  path when settings are in memory): a Windows named pipe created with
  `FILE_FLAG_FIRST_PIPE_INSTANCE`, a Linux abstract Unix socket (`std`, released by the kernel on
  exit, no stale file). The first instance listens on a thread; a second start connects, sends
  `show` and reads the answer (contracts/single-instance.md). The first instance shows its window
  (also from the tray) and tries to focus it; on Wayland it answers that it could not raise the
  window, and the second start shows the system message `already-open` before exiting.
- **Alternatives**: a lock file — stale after a crash and needs a second channel to raise the
  window; the `interprocess` crate — a dependency for about 100 lines.

## R11. Tray, autostart, system message

- **Tray (FR-014a)**: Windows — `tray-icon` (Shell_NotifyIcon, menu through `muda`), driven by the
  winit loop's message pump. Linux — `ksni` (StatusNotifierItem over D-Bus, blocking feature, no
  GTK); the tray is available when `org.kde.StatusNotifierWatcher` has an owner on the session
  bus; otherwise the setting is hidden and closing quits (GNOME without the extension, F48). Menu:
  Open, Quit, texts from `Texts`. Icon: `brand/X.svg` rasterized at build time (R12).
- **Autostart (FR-014b)**: Windows — value `CapturaX` in
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` with `"<exe>" --autostart`; it counts as
  on when present and not disabled in `...\Explorer\StartupApproved\Run` (Task Manager's switch).
  Linux — `$XDG_CONFIG_HOME/autostart/capturax.desktop` with `Exec=<exe or $APPIMAGE>
  --autostart`; on when present and neither `Hidden=true` nor `X-GNOME-Autostart-enabled=false`.
  At start, if on and pointing elsewhere, the entry is rewritten to the running executable. The
  settings panel always reads the real state; the settings file does not store it.
- **System message (FR-036, R10)**: Windows — `MessageBoxW`. Linux — `kdialog --error`, else
  `zenity --error`, else only stderr and the log.
- **Alternatives**: `tray-icon` on Linux — needs GTK and libappindicator; the `auto-launch` crate —
  does not read Task Manager's disabled state.

## R12. Assets

- **Decision**: from the spike tag `spike/stack-20261026`, adopt Inter (OFL) and the Tabler icons
  used by the shell (MIT; title bar, dock, close, chevrons), into `app/ui/assets/`. The logos are
  referenced from `brand/` (FR-006). `app/build.rs` rasterizes `brand/X.svg` with `resvg` into
  the window and tray icon sizes (16–256 px) and embeds them.
- **Linux window icon**: on Wayland the compositor takes the icon from the `.desktop` file that
  matches the app id (`capturax`); development builds have none, so a generic icon shows there
  until packaging (spec #13). Known limit, not an acceptance gate.

## R13. CI

- **Decision**: GitHub Actions, one workflow `.github/workflows/ci.yml` on pull requests and
  pushes to `main`:
  - `checks` (Ubuntu): `cargo fmt --check`, `cargo deny check licenses bans` (GPL-3.0
    compatibility; `tray-icon`, `muda` and `ksni` reachable only through `platform`).
    `windows-sys` and `zbus` also arrive transitively through winit, wgpu and Slint, so
    `cargo deny` cannot ban them by name; the test `app/tests/platform_boundary.rs` fails when
    `app` or `settings` lists an OS crate as a direct dependency. Slint's default features also
    bring `ksni` (its `system-tray` feature, through `i-slint-core`) and `muda` (its winit
    backend), so those two Slint crates are allowed wrappers in `deny.toml`. Turning Slint's
    default features off was tried and dropped: without a Slint backend, `.slint` SVG images do
    not compile (checked 2026-11-04).
  - `build-test` matrix `windows-latest`, `ubuntu-24.04`: install GStreamer (Ubuntu: apt
    `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev` plus the window system headers and
    `libfontconfig1-dev` for Slint's font lookup; Windows: the 1.28 MSVC x86_64 installer, which
    since 1.28 is one Inno Setup `.exe` holding runtime and development files, run with
    `/VERYSILENT /NORESTART /ALLUSERS /TYPE=devel` and cached by version),
    `cargo clippy --all-targets -- -D
    warnings`, `cargo test`, `cargo build --release`.
  - Rust cache (`Swatinem/rust-cache`) to stay under 20 min (SC-008).
- Merge blocking (FR-038) is a repository rule on `main` requiring these checks; the maintainer
  sets it (quickstart.md), since it is a repository setting, not a file.
- GPU presentation cannot run on hosted runners; render timing is manual acceptance (spec
  Assumptions).
- **Alternatives**: a self-hosted runner with a GPU — not needed until a spec automates latency.

## R14. Dependencies

| Crate | Purpose | License | Status |
|---|---|---|---|
| slint, slint-build 1.18 | UI, software renderer, translations | GPL-3.0 (one of its three licenses) | active |
| winit 0.30 | window and events | Apache-2.0 | active |
| wgpu 30 | GPU surface and composition | MIT / Apache-2.0 | active |
| gstreamer, -app, -video 0.25 | test video (and spec #3) | MIT / Apache-2.0 (GStreamer LGPL) | active |
| bytemuck, pollster | GPU buffers, blocking on wgpu setup | MIT / Apache-2.0 / Zlib | active |
| clap 4 | command-line options | MIT / Apache-2.0 | active |
| log | logging facade (own size-limited file writer) | MIT / Apache-2.0 | active |
| toml | settings format | MIT / Apache-2.0 | active |
| tempfile | atomic replace | MIT / Apache-2.0 | active |
| sys-locale | system language | MIT / Apache-2.0 | active |
| dirs | per-user configuration folder | MIT / Apache-2.0 | active |
| anyhow | error context in the app | MIT / Apache-2.0 | active |
| windows-sys (Windows) | message box, named pipe, registry, subclass | MIT / Apache-2.0 | active |
| tray-icon (Windows) | tray icon and menu | MIT / Apache-2.0 | active |
| zbus (Linux) | XDG portal theme, tray detection | MIT | active |
| ksni (Linux) | StatusNotifierItem tray | Unlicense | active |
| resvg (build) | rasterize the app icon | MPL-2.0 | active |

All are compatible with GPL-3.0-only; `cargo deny` keeps it so.
