---
description: "Tasks for the app shell"
---

# Tasks: App Shell

**Input**: Design documents from `specs/20261104-112852-app-shell/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: unit tests for every pure module (in the module's own `#[cfg(test)] mod tests`) and the
integration tests named by the plan: `crates/settings/tests/interrupted_save.rs` (SC-007),
`app/tests/i18n.rs` (FR-039), `app/tests/contrast.rs` (FR-018),
`app/tests/platform_boundary.rs` (constitution IV). Render timing and window behavior are manual
acceptance (quickstart.md), since hosted CI has no GPU.

**Spike code**: "port from the spike" means read the file from the tag with
`git show spike/stack-20261026:spike/layered-preview/<path>` and rewrite it into the named file;
the ambient light, info bubbles, dropdown contents and audio are not ported (plan, Structure
Decision).

**Texts**: every task that adds a user-facing text writes it with `@tr("...")` in a `.slint` file
(texts Rust needs go in `app/ui/texts.slint`, research R7) and adds its Portuguese (Brazil)
translation to `app/lang/pt-BR/LC_MESSAGES/capturax.po` in the same task.

**Tasks marked (maintainer)** need the maintainer's hands or machines; the agent prepares them and
records the results.

**Commits**: at each checkpoint the agent shows what changed and commits only after the
maintainer validates it (Conventional Commits, no AI trailers). Nothing is pushed unless asked.
At the end of every working session the agent updates handoff.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: the workspace builds, formats and lints on both platforms.

- [X] T001 Create the workspace Cargo.toml (resolver 3; members `app`, `crates/settings`, `crates/platform`; `[workspace.package]` edition 2024, license `GPL-3.0-only`, version 0.1.0, publish false; `[workspace.dependencies]` with every crate and version of research R14; `[profile.release] debug = "line-tables-only"` as the spike) and rust-toolchain.toml (channel 1.99, components rustfmt and clippy)
- [X] T002 [P] Create rustfmt.toml (`max_width = 120`) and .cargo/config.toml (copy the `PKG_CONFIG_PATH` setup from `git show spike/stack-20261026:spike/.cargo/config.toml`)
- [X] T003 [P] Create deny.toml: `[licenses]` allow list compatible with GPL-3.0-only (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, Unlicense, GPL-3.0-only, CC0-1.0, BSL-1.0); `[bans]` with `deny = [{ crate = "tray-icon", wrappers = ["capturax-platform"] }, { crate = "muda", wrappers = ["tray-icon"] }, { crate = "ksni", wrappers = ["capturax-platform"] }]` (research R13); `cargo deny check licenses bans` passes
- [X] T004 [P] Create the crate skeletons: app/Cargo.toml (package `capturax`, binary `capturax`, `settings = { package = "capturax-settings", path = "../crates/settings" }`, `platform = { package = "capturax-platform", path = "../crates/platform" }`) with app/src/main.rs; crates/settings/Cargo.toml (package `capturax-settings`) with crates/settings/src/lib.rs; crates/platform/Cargo.toml (package `capturax-platform`, `windows-sys` and `tray-icon` under `[target.'cfg(windows)'.dependencies]`, `zbus` and `ksni` under `[target.'cfg(target_os = "linux")'.dependencies]`) with crates/platform/src/lib.rs, crates/platform/src/windows/mod.rs, crates/platform/src/linux/mod.rs
- [X] T005 [P] Copy from the spike tag into app/ui/assets/: `fonts/Inter.ttf`, `fonts/OFL.txt`, and the icons `chevron`, `chevron_left`, `chevron_right`, `close`, `fullscreen`, `fullscreen_exit`, `maximize`, `minimize`, `unmaximize`, `settings`, `preview_off` (`.svg`); add the Tabler `info-circle` and `alert-triangle` icons (MIT, same 24 px stroke style) for notices; add app/ui/assets/icons/LICENSE with the Tabler MIT notice (research R12)
- [X] T006 Check the newest Slint 1.x patch (`cargo search slint`); if newer than 1.18.1, set it in the workspace Cargo.toml and record the version in research R1 (the latency run T041 checks it; if it fails, go back to 1.18.1)

**Checkpoint**: `cargo build`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass on Windows; maintainer validates; commit `build: add cargo workspace`.

---

## Phase 2: Foundational

**Purpose**: an undecorated window that shows the Slint UI through the video thread, settings
loaded and saved, the log, and the notice component. Every story builds on it.

**⚠️ No user story starts before this phase is done.**

- [X] T007 [P] Implement app/src/cli.rs with clap per contracts/cli.md: `--fullscreen`, `--windowed` (together → exit 64), `--autostart`, `--test-video <PRESET|WxH@FPS>` (presets `1080p60` = 1920×1080@60, `1440p60` = 2560×1440@60; anything else parsed as `WxH@FPS` or exit 64), `--stats`, `--backend <dx12|vulkan|gl>`, `--present <mailbox|immediate|fifo>`, `--lang <CODE>`; produce the `StartupOptions` of data-model.md; unit tests for every option, the presets, bad `WxH@FPS` and the conflict
- [X] T008 [P] Implement app/src/log.rs: a `log::Log` writer to `capturax.log` in a given folder, one line per event (`<RFC 3339 UTC> <LEVEL> <target> <message>`), "at most 1 MiB; when full it is renamed `capturax.log.1` (replacing the previous one) and a new file starts" (contracts/settings-file.md, Log); with no folder it writes to stderr only; unit tests for rotation with a temp folder
- [X] T009 [P] Implement the settings model in crates/settings/src/model.rs: `Settings { general: General, window: WindowSettings }` with the fields, types and defaults of data-model.md (`version` 1; `language` `auto` | language code, default `auto`; `theme` `auto` | `dark` | `light`, default `auto`; `start_fullscreen` false; `keep_in_tray` false; `width`, `height` logical px default 1280 × 720, "at least 640 × 360"; `x`, `y` physical px, default none; `maximized` false; `monitor` string, default none); `Settings::read(&toml::Table, known_languages: &[&str])` lenient per contracts/settings-file.md rule 3 (each value on its own, missing or invalid → default, unknown language or theme → `auto`, width or height below the minimum → the minimum) and `Settings::write_into(&self, &mut toml::Table)` that writes only known keys, keeping unknown keys and tables (rule 4); unit tests: empty table, every invalid value, unknown language and theme, higher `version` read leniently, unknown keys and a `[future]` table survive a read–write round trip
- [X] T010 Implement crates/settings/src/file.rs (depends on T009): `load(path) -> LoadOutcome` with `NoFile`, `Loaded(toml::Table)`, `Damaged { kept_as: PathBuf }` ("not valid TOML, not UTF-8, or unreadable: renamed to `capturax.toml.damaged-<unix seconds>`", rule 5); `save(path, &toml::Table)` atomic per research R6 (`tempfile::NamedTempFile::new_in(folder)`, write, `sync_all`, `persist` over the target, then `File::open(folder)?.sync_all()` on Linux); unit tests with a temp folder for each outcome and for save replacing an existing file
- [X] T011 Implement crates/settings/src/store.rs (depends on T010): `Store` holding the loaded table, the `Settings` and an optional file path (`None` = memory only); `get()`, `update(|&mut Settings|)` that marks it dirty; a saver thread that saves 300 ms after the last change; `flush()` that saves now if dirty and is called on quit (research R6); export the public API from crates/settings/src/lib.rs; unit tests for debounce (several updates → one save) and flush
- [X] T012 [P] Implement in crates/platform/src/lib.rs, windows/mod.rs and linux/mod.rs: `app_folder() -> PathBuf` (executable's folder; on Linux the folder of `$APPIMAGE` when set, FR-026) and `show_system_message(title: &str, text: &str)` (Windows `MessageBoxW` with `MB_ICONERROR`; Linux `kdialog --title <title> --error <text>`, else `zenity --error --title <title> --text <text>`, else stderr only; research R11); `window_attributes(attrs: winit::window::WindowAttributes) -> WindowAttributes` (Windows `with_undecorated_shadow(true)`; Linux app id `capturax` through `with_name("capturax", "capturax")`; contracts/platform.md), so `capturax-platform` depends on winit; declare the empty modules `instance`, `tray` and `autostart` in lib.rs, windows/mod.rs and linux/mod.rs so T035, T037 and T038 do not edit the same files
- [X] T013 [P] Port app/ui/theme.slint from the spike's `ui/theme.slint`: global `Theme` with `in property <bool> dark` and every token picking its Dark or Light value (FR-016: "one cool neutral in four tone steps, orange `#FF7304` accent with dark ink on orange, `accent-text` for orange text in light, Inter bundled with the app, depth by tone steps and shadow images, semi-transparent panels only (dock, title bar, overlay), opaque menus, buttons and icons, a 2 px focus ring as the only outline"); type, spacing, radius tokens; shadow image properties set from Rust (`in property <image>`); import `assets/fonts/Inter.ttf`; drop the ambient light tokens
- [X] T014 [P] Create app/ui/texts.slint (global `Texts` with an `out property <string>` per text Rust needs, each `@tr(...)`; start with `app-name`) and app/ui/window-state.slint (global `WindowState`: `in property <bool> maximized`, `fullscreen`, `active`; callbacks `minimize()`, `toggle-maximize()`, `close()`, `toggle-fullscreen()`, `start-move()`; ported from the spike's `ui/window-state.slint`)
- [X] T015 Create app/build.rs (depends on T013, T014): `slint_build::compile_with_config("ui/main.slint", CompilerConfiguration::new().with_bundled_translations("lang"))`; rasterize `../brand/X.svg` with `resvg` into RGBA icons of 16, 24, 32, 48, 64, 128 and 256 px written to `OUT_DIR` and embedded by app/src/icon.rs (`include_bytes!`); `rerun-if-changed` on `ui/`, `lang/` and `../brand/X.svg`; create app/lang/pt-BR/LC_MESSAGES/capturax.po with its gettext header (`Language: pt_BR`, UTF-8)
- [X] T016 Create app/ui/main.slint (depends on T013–T015): `export component AppWindow inherits Window` with the Theme background, a video area element whose position, size and corner radius are exposed as `out property` for Rust, and named empty slots for the layers of the plan (state message, chrome, notices, settings panel, top layer); re-export `Theme`, `Texts`, `WindowState`
- [X] T017 Implement app/src/ui.rs (depends on T016), ported from the spike's `src/main.rs`: a Slint `Platform` with one `MinimalSoftwareWindow` (`RepaintBufferType::ReusedBuffer`); forwarding of winit pointer, wheel, key, text, focus and scale factor events to Slint; `draw_if_needed()` that renders into an RGBA buffer only when Slint reports a change, at most once per monitor refresh, and returns a `UiImage { pixels: Arc<[u8]>, width, height, draw_ms }`; `next_timer()` for `ControlFlow::WaitUntil` (research R3)
- [X] T018 Implement app/src/render/mod.rs, app/src/render/gpu.rs and app/src/render/composite.wgsl (depends on T017), ported from the spike's `src/gpu.rs` and `src/shader.wgsl`: the `video` thread owning the wgpu instance, device and surface; an `Inbox` (mutex + condvar) keeping only the newest video frame (with its ready `Instant`), the newest `UiImage` and the scene (video rect, radius, surface size); the thread composites and presents when a frame or a UI image arrives, the UI always over the last video frame, letterboxed with rounded video corners; presentation choice in the order of research R4 (Windows DX12 Mailbox → DX12 Immediate → DX12 Fifo → Vulkan Mailbox → Vulkan Fifo; Linux Vulkan Mailbox → Vulkan Immediate → Vulkan Fifo → GL Fifo), overridden by `--backend` and `--present` without fallback, choice written to the log; wgpu built with the `dx12` and `vulkan` features on Windows and `vulkan` and `gles` on Linux, so a forced backend the platform lacks always fails; `start(...) -> Result<VideoThread, RendererError>`, `resize`, `set_visible(false)` that stops presenting, `stop()` that joins the thread
- [X] T019 Implement app/src/app.rs and app/src/main.rs (depends on T007, T008, T011, T012, T018): the startup sequence of the plan steps 1, 2 (settings from `platform::app_folder()` only, location fallback comes in US3), 4 and 5 (winit window created hidden and undecorated, attributes passed through `platform::window_attributes`, window icon from app/src/icon.rs, surface, video thread; on `RendererError` log it and exit 2, the message comes in US4), then show the window; a winit `ApplicationHandler` with a user event enum for platform events (`EventLoopProxy`); the quit sequence of the plan (stop the video thread, flush settings, destroy the window) on close; log the time from process start to the first present (SC-004)
- [X] T020 [P] Create app/ui/components/notice.slint and app/src/notices.rs (depends on T016): one non-blocking, dismissible notice at the top of the video area with an icon (`info-circle` or `alert-triangle`) and a text that may name a folder (FR-034); Rust keeps a queue keyed by notice id and shows "each id at most once per run" (data-model.md); place it in the notices layer of app/ui/main.slint

**Checkpoint**: `cargo run -p capturax` shows an undecorated window filled with the theme background, drawn by the video thread; closing quits cleanly; `capturax.toml` is written next to the executable; `cargo test` passes; maintainer validates; commit `feat: add window, render loop and settings store`.

---

## Phase 3: User Story 1 - A window that never holds the video back (Priority: P1) 🎯 MVP

**Goal**: the Float frameless window and chrome with every window action, tray, autostart and
single instance, and a test video with presentation statistics proving the video never waits.

**Independent Test**: `capturax --test-video 1080p60 --stats`; exercise every window action and
chrome state (quickstart 2–6, 12–18) and read the `[loop]` lines (quickstart, Latency budget).

- [x] T021 [P] [US1] Implement app/src/stats.rs per contracts/stats.md: a collector fed by the video thread (`received`, `presented(ready: Instant)`, `replaced`, `ui_image(draw_ms)`, `window_moved`) producing per second the `[loop]` line `t= recv= pres= repl= ready_ms=<median>/<max> gap_ms= ui= ui_ms=<median>/<max> moves=` (`-` when there is no sample) and the `[present] backend= mode= adapter= surface=` header; unit tests for median, max, gap and the exact line format
- [x] T022 [P] [US1] Implement app/src/test_video.rs (research R9): initialize GStreamer when `--test-video` is given (its libraries are linked, research R9); pipeline `videotestsrc is-live=true pattern=ball ! video/x-raw,format=RGBA,width=W,height=H,framerate=F/1 ! appsink sync=false max-buffers=1 drop=true`; the appsink callback puts each frame with its ready `Instant` in the inbox and wakes the video thread; GStreamer or `videotestsrc` missing → message on stderr and exit 69; `stop()` sets the pipeline to Null
- [x] T023 [US1] Wire stats and the test video into app/src/render/mod.rs, app/src/ui.rs and app/src/main.rs (depends on T021, T022): count in the video thread, print with `--stats` to stdout and the log, header on start and on presentation change; stop the test video first in the quit sequence
- [X] T024 [P] [US1] Implement app/src/placement.rs: pure `fit(saved: &WindowSettings, monitors: &[Monitor { name, position, size, work_area, scale }], primary: usize) -> Placement` with the rules of data-model.md ("prefer the saved monitor if connected, else the primary; reduce the size to fit the monitor's work area; move the window so it is fully visible; without a saved position, center on the chosen monitor"); unit tests for a missing monitor, an oversized window, a window partly off screen, no saved position, mixed scales
- [X] T025 [P] [US1] Implement app/src/shadows.rs, a port of the spike's `tools/make_shadow.py` (`git show spike/stack-20261026:spike/tools/make_shadow.py`): a nine-slice shadow image for a rounded rectangle with two Gaussian layers and a hollow centre, generated at 1× as `slint::SharedPixelBuffer<Rgba8Pixel>` with its nine-slice edges and margin, one per shadow token of the theme (soft, tight), set in `Theme` once at start (research R8: Slint scales nine-slice corners by the window's scale itself); unit tests for the output size, nine-slice edges, left-right symmetry, the offset below and a transparent centre
- [X] T026 [P] [US1] Create app/ui/components/focus-ring.slint, icon-button.slint and tooltip.slint, ported from the spike's `ui/components/basics.slint`: 2 px focus ring shown only on keyboard focus (FR-020), icon button with hover, pressed, disabled and tooltip text, tooltip as an element of the top layer
- [X] T027 [P] [US1] Create app/ui/components/menu.slint, select.slint and toggle.slint, ported from the spike's `ui/components/dropdown.slint` and `basics.slint`: menus drawn in the top layer; placement per plan "Menu placement" (default below and aligned to the left of its control; if it does not fit inside the window on that side it opens on the other side, left instead of right, above instead of below); a `select` built on the menu; a `toggle`; all keyboard usable (arrows, Enter, Esc) with the focus ring
- [X] T028 [US1] Create app/ui/components/title-bar.slint (depends on T026), ported from the spike's `ui/components/title-bar.slint`: logo `@image-url("../../../brand/CapturaX-light.svg")` in the dark theme and `CapturaX-dark.svg` in the light theme (FR-006), minimize, maximize or restore (icon by `WindowState.maximized`) and close; the drag area calls `WindowState.start-move()` and double click `toggle-maximize()`; exposes the maximize button's rectangle in physical pixels for T040; hidden in fullscreen
- [X] T029 [US1] Create app/ui/components/dock.slint and fill the chrome layer of app/ui/main.slint (depends on T026–T028), ported from the spike's `ui/components/dock.slint`, `ui/components/toolbar.slint` and `ui/directions/float.slint`: toolbar slot over the top of the video (empty, for spec #3), left and right docks holding only the fullscreen and settings buttons (FR-015); chrome hides 2 s after the pointer leaves the video area and shows again when it reaches the top of the video (FR-008); Float layout at 800 × 520 and above, compact layout below it with the docks in the toolbar's row and no overlaps (FR-009); keyboard focus order through every control (FR-020)
- [X] T030 [US1] Implement the window actions in app/src/app.rs (depends on T029), ported from the spike's `src/window.rs` (T037 behavior, research R5): move with `drag_window` after 4 px so a double click reaches the bar; a maximized window restores under the pointer; double click maximizes or restores; a 6 px band along edges and corners sets the resize cursor and starts `drag_resize_window`, off when maximized or fullscreen; minimum inner size 640 × 360 logical; F11 and the dock control toggle borderless fullscreen on the current monitor; Esc closes an open menu, else the settings panel, else leaves fullscreen, and never quits (FR-007; the menu closes itself on Esc inside Slint, so read `Menus.open` before dispatching the key to Slint, or one Esc also closes the panel); `WindowState` kept in sync with the window
- [X] T031 [US1] Send the scene to the video thread and handle scale changes in app/src/app.rs and app/src/ui.rs (depends on T025, T030): the video area's position, size and radius from `AppWindow` on every layout change; on `ScaleFactorChanged` resize the UI buffer and the surface, so text and icons stay sharp from 100% to 200% and when moving between monitors (FR-019); draw at `platform::drawn_size` so the system does not squeeze the image into the rows the undecorated shadow hides (research R5); check the 1× shadows (T025) at 125%, 150% and 200% for steps or seams, and if they show, switch to the fallback in research R8 (spec first)
- [X] T032 [US1] Save and restore the window placement in app/src/app.rs (depends on T024, T030): at start, `placement::fit` with winit's `available_monitors()` and `primary_monitor()`; on move, resize and maximize, `store.update` the `window` category (not while fullscreen; the state to return to is kept) (FR-012)
- [X] T033 [US1] Create app/ui/components/settings-panel.slint and wire it in app/ui/main.slint and app/src/ui.rs (depends on T027, T029): a panel over the video opened from the dock's settings button and closed with Esc or its close control, video keeps playing behind it, also in fullscreen (FR-025); rows Start in fullscreen, Keep in tray when closed (hidden when `platform::tray::available()` is false), Start with the system, and the app's version (`env!("CARGO_PKG_VERSION")`); each change goes through `store.update` (saved without a button, FR-029); language and theme rows come in US2
- [X] T034 [US1] Apply the start mode in app/src/main.rs (depends on T030, T033): `--fullscreen` or `--windowed` override `start_fullscreen` for this start without saving it (FR-012a); Esc or F11 returns to the saved window state
- [X] T035 [P] [US1] Implement the single instance in crates/platform/src/lib.rs (shared part in single_instance.rs), windows/instance.rs and linux/instance.rs per contracts/single-instance.md: `instance_name(path) -> String` (`capturax-` + FNV-1a 64-bit hash as 16 hex digits), `claim_instance(name) -> Claim::{First(Listener), Running(Client)}` (Windows named pipe `\\.\pipe\<name>` with `FILE_FLAG_FIRST_PIPE_INSTANCE`; Linux abstract Unix socket `\0<name>`, `AddrInUse` → running), the listener thread accepting only the line `show` and answering `raised` or `shown-not-raised` from a callback, anything else closes the connection; `Client::show()` with a 1 s timeout; `can_raise_window()` (Windows true, Linux false on Wayland, true on X11); unit tests for the hash and line parsing, crates/platform/tests/single_instance.rs for a first and a second claim in one process
- [X] T036 [US1] Wire the single instance into app/src/main.rs (depends on T035, T019): claim after the settings load (plan step 3, name from the settings file path or, in memory, the executable path); a second start sends `show`, and on `shown-not-raised` shows the system message `already-open` (text from `Texts`), on no answer logs it, then exits 0; the first instance shows its window (also when hidden in the tray) and focuses it through the `EventLoopProxy` (FR-013)
- [X] T037 [P] [US1] Implement the tray in crates/platform/src/tray.rs (shared part), windows/tray.rs and linux/tray.rs per contracts/platform.md and research R11: `tray::available()` (Windows true; Linux `org.kde.StatusNotifierWatcher` has an owner on the session bus, via `zbus` blocking), `tray::create(icons: &[IconImage], labels { title, open, quit }, on_event) -> Tray` (Windows `tray-icon` with a `muda` menu; Linux `ksni` blocking), click and Open → `TrayEvent::Open`, Quit → `TrayEvent::Quit`, `set_labels` for a language change
- [X] T038 [P] [US1] Implement autostart in crates/platform/src/windows/autostart.rs and linux/autostart.rs (research R11): `autostart::state() -> bool` (Windows value `CapturaX` in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, on unless disabled in `...\Explorer\StartupApproved\Run`; Linux `$XDG_CONFIG_HOME/autostart/capturax.desktop` present and neither `Hidden=true` nor `X-GNOME-Autostart-enabled=false`), `autostart::set(on, exe)` with `"<exe>" --autostart` (Linux `Exec=` uses `$APPIMAGE` when set), `autostart::target() -> Option<PathBuf>`, `autostart::executable()` (the program an entry should start; `$APPIMAGE` on Linux when set); unit tests for the `.desktop` parsing as a pure function
- [X] T039 [US1] Wire tray and autostart into app/src/app.rs and app/src/main.rs (depends on T033, T036–T038): with Keep in tray on and a tray available, close hides the window, calls `set_visible(false)` on the video thread and keeps the tray icon; Open or a click shows it again, Quit runs the quit sequence (FR-014a); started with `--autostart`, Keep in tray on and a tray available → hidden, whatever Start in fullscreen says (FR-014b); the panel's Start with the system row reads `autostart::state()` each time the panel opens; at start, if on and `target()` is not `autostart::executable()`, rewrite it; tray labels from `Texts` (`tray-open`, `tray-quit`)
- [X] T040 [US1] Windows 11 snap layouts flyout, best effort, timeboxed to one day (FR-011, research R5): crates/platform/src/windows/snap.rs `snap::attach(window, on_event) -> Snap` and `Snap::set_button(rect)` subclassing the window with `SetWindowSubclass`, answering `WM_NCHITTEST` with `HTMAXBUTTON` inside the rectangle and turning `WM_NCMOUSEMOVE`, `WM_NCMOUSELEAVE`, `WM_NCLBUTTONDOWN`, `WM_NCLBUTTONUP` into the button's hover, press and click events, and extending the DWM frame into the client down to the button (`DwmExtendFrameIntoClientArea`, found needed in T040); Linux no-op; wired in app/src/app.rs with the rectangle from T028. If moving, resizing or the button's own hover and click break, remove the code and record the flyout as a known limit in spec.md (FR-011) and handoff.md
- [ ] T041 [US1] (maintainer) Run the latency budget of quickstart.md (1080p60 test video on a 1080p screen and on a 1440p DSR screen, windowed and fullscreen, windowed also with the app unfocused on a variable refresh monitor, every interface state: idle, chrome showing and hiding, the settings panel opening and closing; a menu is measured in T050, none exists before T046) on Windows 11 and CachyOS KDE Plasma Wayland, with quickstart checks 2–6, 12–18; the agent records the `[present]` header, the `[loop]` lines and SC-001–SC-003 verdicts in specs/20261104-112852-app-shell/measurements/latency.md (with the Linux GPU clock state); language and theme switch states are measured after US2 (T050)

**Checkpoint**: SC-001–SC-003 met on both platforms in the states above; maintainer validates; commit `feat: add frameless window, chrome, tray and single instance`.

---

## Phase 4: User Story 2 - The app speaks my language and wears my theme (Priority: P2)

**Goal**: Automatic, English and Português (Brasil); Automatic, Dark and Light; both switch at
once without a restart; every text pair at 4.5:1, every control icon at 3:1.

**Independent Test**: switch the system language and theme, start the app, then change both in
the settings panel and check every visible text and surface (quickstart 1 and 7).

- [x] T042 [P] [US2] Implement app/src/i18n.rs: `LANGUAGES: &[Language { code, native_name }]` = `en` "English", `pt-BR` "Português (Brasil)" (FR-023); `resolve(setting, system_locale) -> &'static str` ("`auto` → `pt-BR` if the system locale starts with `pt`, else `en`; a code not in `LANGUAGES` → as `auto`", data-model.md); `system_locale()` with `sys-locale`; unit tests for `pt-BR`, `pt-PT`, `pt`, `en-US`, `fr-FR`, an unknown code, none
- [x] T043 [P] [US2] Implement app/src/theme.rs: `resolve(setting, system: Option<SystemTheme>) -> bool /* dark */` ("`auto` follows the system, unknown system → dark"); unit tests
- [x] T044 [US2] Apply the language in app/src/main.rs and app/src/ui.rs (depends on T042): at start `slint::select_bundled_translation` for `resolve(settings.language or --lang, system_locale())` (`--lang` is not saved); on change from the panel, select it again, re-read every `Texts` value Rust holds (tray labels via `set_labels`, the visible notice), all within the same main loop turn (FR-022); log the time from the selection to the next UI image handed to the video thread as `language switch <code> <ms> ms` (SC-005)
- [x] T045 [US2] Follow the system theme (depends on T043): Windows in app/src/app.rs with winit `Window::theme()` and `WindowEvent::ThemeChanged`; Linux first check whether winit 0.30 reports `ThemeChanged` on KDE Plasma Wayland, and only if not implement crates/platform/src/linux/theme.rs `system_theme_watch(sender)` reading the XDG Settings portal `org.freedesktop.appearance` `color-scheme` with `zbus` blocking and listening to `SettingChanged` on a thread (research R8); record the outcome in research R8; set `Theme.dark` on start and on every change, logo swap included; log `theme switch <dark|light> <ms> ms` measured as in T044 (SC-005)
- [X] T067 [US2] Rework app/ui/components/settings-panel.slint and its layer in app/ui/main.slint into the central panel of FR-025 and plan.md (depends on T033): backdrop that dims the video and closes on click, panel centered on the video area, categories General / Appearance / About with icons in a left column in a sunken well, the chosen one tinted with the accent as a toggled tool button (icons only, names in tooltips, in the compact layout), the existing rows under General, the version under About, keyboard focus kept inside while open, the chrome no longer held shown by the panel; Esc order unchanged (FR-007)
- [X] T046 [US2] Add the Language and Theme rows under Appearance in app/ui/components/settings-panel.slint (depends on T067, T044, T045): Language = Automatic + each `LANGUAGES` native name (from Rust as a model, so a third language needs no UI change, FR-024); Theme = Automatic, Dark, Light; each change applies at once and goes through `store.update`
- [x] T047 [P] [US2] Create app/tests/i18n.rs (research R7, FR-039): extract every `@tr("...")` from `app/ui/**/*.slint` (handling escaped quotes and `{}` placeholders); parse every `app/lang/*/LC_MESSAGES/capturax.po`; fail naming the text for a msgid missing, with an empty msgstr, or stale (in the catalog but no longer in the UI); fail naming file and line for a string literal assigned to a `text`, `title`, `tooltip` or `label` property without `@tr`
- [x] T048 [P] [US2] Create app/tests/contrast.rs (FR-018): parse the color tokens of `app/ui/theme.slint` for Dark and Light; check a declared list of text-on-surface pairs (every text level on every neutral tone it is used on, `accent-ink` on accent, `accent-text` on the light surfaces, notice and state message texts) at 4.5:1, and the icons of controls (dock, title bar, categories, notice, select) at 3:1, with the WCAG relative luminance formula; for the semi-transparent panels, check against a white and a black video behind them; fail naming the pair and its ratio
- [x] T049 [US2] Complete app/lang/pt-BR/LC_MESSAGES/capturax.po for every text so far and fix any contrast pair that fails (depends on T047, T048); `cargo test` passes
- [ ] T050 [US2] (maintainer) Quickstart checks 1 and 7 on both platforms, and the language and theme switch and menu states of the latency budget; the agent adds them to specs/20261104-112852-app-shell/measurements/latency.md (SC-001, SC-002); SC-005 is read from the `language switch` and `theme switch` log lines, each under 100 ms

**Checkpoint**: both languages and themes switch at once on both platforms; maintainer validates; commit `feat: add languages and themes`.

---

## Phase 5: User Story 3 - My settings are kept next to the app (Priority: P2)

**Goal**: settings in the app's folder, the per-user folder as fallback with a notice, memory as
last resort, damaged files kept aside, saves that survive power loss.

**Independent Test**: run from a writable folder, change settings, restart; copy the folder and
start there; run from a read-only folder; replace the file with garbage (quickstart 8–11).

- [X] T051 [P] [US3] Implement crates/settings/src/location.rs: `resolve(app_folder, app_writable, user_folder: Option<PathBuf>, user_writable, file_times) -> Location { kind: AppFolder(path) | UserFolder(path) | Memory, read_from: Option<PathBuf> }` with the order of data-model.md (app folder if writable; else per-user folder; else memory) and the read-only rule of research R6 (a readable `capturax.toml` in a read-only app folder is read first and later saves go to the per-user folder; if the per-user folder has a newer file, the per-user file wins); table-driven unit tests for every branch
- [X] T052 [P] [US3] Implement in crates/platform/src/lib.rs, windows/mod.rs and linux/mod.rs: `user_config_folder() -> Option<PathBuf>` (`%APPDATA%\CapturaX`; `$XDG_CONFIG_HOME/capturax`, else `~/.config/capturax`, through `dirs`) and `is_writable(folder) -> bool` (create the folder if missing, create and remove a probe file `.capturax-write-probe`)
- [ ] T053 [US3] Wire the location into app/src/main.rs (depends on T020, T051, T052): plan step 2 resolves the location, starts the logger there (stderr in memory), loads from `read_from` or the location's file; notices `settings-fallback` naming the folder (FR-027), `settings-not-kept` (FR-028), `settings-damaged` (FR-030) with texts in `Texts`; the single-instance name (T036) uses the resolved location
- [X] T054 [P] [US3] Create crates/settings/tests/interrupted_save.rs (SC-007): the test re-runs its own binary as a child (selected by an environment variable) that saves alternating settings A and B in a loop through `file::save`; the child prints `saving` on stdout after its first completed save; the parent waits for that line, then kills it after a random 0–20 ms, 100 times, and after each kill checks that `capturax.toml` parses and equals A or B, and no temporary file breaks the next `load`
- [ ] T055 [US3] (maintainer) Quickstart checks 8–11 on both platforms (read-only folder on Windows with `icacls`, on Linux with `chmod a-w`; on Linux also from an AppImage-like `$APPIMAGE` set to a file in another folder)

**Checkpoint**: US3 scenarios 1–5 pass; maintainer validates; commit `feat: add settings location and fallbacks`.

---

## Phase 6: User Story 4 - Problems are shown with the next step (Priority: P3)

**Goal**: one state message component, the "no source" state in the video area, and the
renderer failure message.

**Independent Test**: start with no source; force the shell-level problems (read-only folder,
damaged file, no renderer) and check what is shown.

- [ ] T056 [P] [US4] Create app/ui/components/state-message.slint (FR-033): "an icon, a short title, an optional one-line detail, and zero, one or two actions (primary and secondary)", in both themes; `callback action(string /* action id */)`; keyboard usable with the focus ring; and app/ui/previews/state-message-preview.slint showing the four variants (no action, one, two, with and without detail) for `slint-viewer`
- [ ] T057 [US4] Show the `no-source` state in the video area of app/ui/main.slint when no source is open (FR-035): icon `preview_off`, title from `@tr`, no action in this spec; hidden while the test video plays; dispatch `action(id)` to Rust in app/src/ui.rs with a log line for unknown ids
- [ ] T058 [US4] Show `renderer-unavailable` in app/src/main.rs (FR-036, research R4): when the video thread returns `RendererError`, read the title and text from `Texts` in the resolved language and call `platform::show_system_message`, then exit 2; check it with `--backend gl` on Windows and `--backend dx12` on Linux (not built for that platform; forced choices do not fall back)
- [ ] T059 [US4] (maintainer) US4 scenarios 1–4 on both platforms (no source, notice dismissal and once per run, renderer failure message in both languages)

**Checkpoint**: maintainer validates; commit `feat: add state message and renderer failure message`.

---

## Phase 7: User Story 5 - Every change is checked on both platforms (Priority: P3)

**Goal**: format, lint, tests, licenses and builds on Windows and Linux on every pull request,
blocking the merge.

**Independent Test**: open a pull request with a formatting error, then a failing test, then a
Linux-only build break, then an untranslated text; each is reported and blocks the merge.

- [x] T060 [P] [US5] Create app/tests/platform_boundary.rs (constitution IV, research R13): parse `app/Cargo.toml` and `crates/settings/Cargo.toml` (every `dependencies` table, target-specific ones included) and fail naming the crate if any of `windows-sys`, `windows`, `tray-icon`, `muda`, `ksni`, `zbus`, `libc`, `nix` is a direct dependency; and fail naming the file if `cfg(windows)`, `cfg(unix)` or `cfg(target_os` appears under `app/src/` or `crates/settings/src/` (except `#[cfg(test)]`)
- [ ] T061 [US5] Create .github/workflows/ci.yml per research R13 (on `pull_request` and `push` to `main`): job `checks` on `ubuntu-24.04` with `cargo fmt --check` and `EmbarkStudios/cargo-deny-action` (`check licenses bans`); job `build-test` with matrix `windows-latest`, `ubuntu-24.04`: install GStreamer (Ubuntu apt `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libxkbcommon-dev libwayland-dev libdbus-1-dev libfontconfig1-dev`; Windows the 1.28 MSVC x86_64 `.exe` installer with `/TYPE=devel` (one installer since 1.28, research R13), cached by version, `PKG_CONFIG_PATH` and `PATH` set), `Swatinem/rust-cache`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`; target under 20 minutes (SC-008)
- [ ] T062 [US5] (maintainer) Push the branch and open the pull request when the maintainer asks; add the rule on `main` (Settings → Rules) requiring `checks` and `build-test` (quickstart, Repository rule); check the four failures of the Independent Test on a throwaway branch and record the CI times

**Checkpoint**: CI green on both platforms in under 20 minutes; maintainer validates; commit `ci: add checks on windows and linux`.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T063 [P] Update README.md: build and run instructions (prerequisites of quickstart.md, `cargo run -p capturax`, `--test-video` and `--stats`), and the status line of AGENTS.md ("Early rewrite. There is no application code yet.") to the shell's state
- [ ] T064 [P] Check the dependency list against research R14 (`cargo tree --depth 1 -e normal` per crate): every direct dependency is listed with its purpose and license; remove any not used
- [ ] T065 (maintainer) Full manual acceptance, quickstart 1–18 on Windows 11 and CachyOS KDE Plasma Wayland at 100% and at 150% or 200% (SC-009); the agent records the results and every known limit (fast moves on Windows, Linux window icon, snap flyout if dropped) in specs/20261104-112852-app-shell/measurements/acceptance.md
- [ ] T066 Update handoff.md, mark the spec Done in docs/roadmap.md if it tracks status, and prepare the pull request description (what the shell delivers, measurements, known limits)

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: after Setup; blocks every story.
- **US1 (Phase 3)**: after Foundational. The MVP.
- **US2 (Phase 4)**: after Foundational; T046 needs the central settings panel (T067) and T050 the
  latency setup of US1 (T041).
- **US3 (Phase 5)**: after Foundational (it extends T011, T019 and uses the notice T020); T053
  also touches the single-instance name from T036 when US1 is done first.
- **US4 (Phase 6)**: after Foundational; independent of the other stories.
- **US5 (Phase 7)**: after Setup only; T060 and T061 can be done right after Phase 1 so CI checks
  everything that follows. Its tests run T047 and T048 once US2 exists.
- **Polish (Phase 8)**: after every story.

### Within the phases

- T009 → T010 → T011; T013, T014 → T015 → T016 → T017 → T018 → T019.
- US1: T021, T022 → T023; T026 → T028 → T029 → T030 → T031, T032, T033 → T034; T035 → T036;
  T037, T038 → T039; T028 → T040; all → T041.
- US2: T042 → T044; T043 → T045; T033 → T067; T044, T045, T067 → T046; T047, T048 → T049.
- US3: T051, T052 → T053.

## Parallel Example: User Story 1

```text
T021 stats.rs          T022 test_video.rs      T024 placement.rs     T025 shadows.rs
T026 basic components  T027 menu components    T035 single instance  T037 tray   T038 autostart
```

## Implementation Strategy

### MVP first

1. Phase 1 and Phase 2 → a window drawn by the video thread.
2. Optionally Phase 7 (T060, T061) right away, so CI guards the rest.
3. Phase 3 (US1) → measure the latency budget (T041). **Stop and validate**: if SC-001–SC-003
   fail, fix the render loop before adding anything on top.

### Incremental delivery

US1 → US2 → US3 → US4 → US5 (if not done early) → Polish, one commit per checkpoint after the
maintainer validates it. The pull request is opened only when the maintainer asks.

## Notes

- Every task names its files; the agent does not touch files outside them without saying why.
- If a decision changes during implementation, update spec.md, plan.md or research.md first.
- Spike code is a starting point, not a dependency: nothing imports from `spike/`.
