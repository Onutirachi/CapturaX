---
description: "Tasks for the screenshot"
---

# Tasks: Screenshot

**Input**: Design documents from `specs/20261121-134830-screenshot/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: unit tests for every pure part (in the module's own `#[cfg(test)] mod tests`): still
conversion and PNG, file naming and writing, the `Screenshots` state, `Hotkey`, the key recorder,
the WAV reader and voice, the sound player's routing with a fake session, the settings rules; the
extended `app/tests/platform_boundary.rs`. Hotkeys, clipboard, dialogs, sound, flash and latency
are manual acceptance (quickstart.md).

**Texts**: every user-facing text is written with `@tr("...")` in a `.slint` file (texts Rust
needs go in `app/ui/texts.slint`) exactly as the spec gives it, and its Portuguese (Brazil)
translation is added to `app/lang/pt-BR/LC_MESSAGES/capturax.po` in the same task, with the
catalog's existing terms (for example "Captura de tela" for Screenshot, "Atalhos" for Hotkeys,
"Mostrar", "Configurações"); `app/tests/i18n.rs` must keep passing.

**Icons**: new icons are Tabler (MIT, 24 px stroke, as the existing ones), added to
`app/ui/assets/icons/` in the task that first uses them.

**Tasks marked (maintainer)** need the maintainer's hands or machines; the agent prepares them,
gives the exact commands, and records the results in `measurements/`. Hardware batching: before
the ezcap or the C920 moves between PCs, every pending (maintainer) task that needs it on the PC
where it is now is done first.

**Commits**: at each checkpoint the agent shows what changed and commits only after the
maintainer validates it (Conventional Commits, no AI trailers). Pure or unwired tasks may be
committed after `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`
pass, without waiting for a manual test. **The placeholder sound `app/sounds/screenshot.wav` is
never committed**: it stays untracked until the maintainer sends the final sound (T040). Code
that embeds it (`include_bytes!` in app/src/sounds.rs, T023) would break CI without the file, so
T023 and anything that needs it to build are committed together with the final file; until then
they stay as uncommitted work on the branch. Nothing is pushed unless asked. At the end of
every working session the agent updates handoff.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: dependencies and the placeholder sound are in place.

- [X] T001 Dependencies (research R15): in the workspace Cargo.toml add `arboard = { version = "3.7", default-features = false, features = ["image-data", "wayland-data-control"] }`, `png = "0.18"` and `x11rb = "0.13"` with a comment naming this spec and the research item; add to `windows` the features `Win32_UI_Shell_Common`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_Com` (if missing) and to `windows-sys` `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_WindowsAndMessaging` (if missing); `arboard.workspace = true` in crates/platform/Cargo.toml, `x11rb.workspace = true` under its Linux dependencies, `png.workspace = true` in crates/capture/Cargo.toml; add `"arboard"` and `"x11rb"` to `OS_CRATES` in app/tests/platform_boundary.rs; check with `cargo tree -d` that no second version of `arboard`, `png`, `image` or `x11rb` appears; `cargo build`, `cargo test` and `cargo deny check licenses bans` pass
- [X] T002 [P] Placeholder sound (research R10): tools/make_screenshot_sound.py (standard library only: `wave`, `math`, `struct`) writes app/sounds/screenshot.wav, 120 ms, 48 kHz mono 16-bit: 1.8 kHz for 50 ms then 1.2 kHz for 70 ms, each with a 2 ms attack and an exponential decay, peak −3 dBFS; run it; add `app/sounds/screenshot.wav` to `.git/info/exclude` (not .gitignore: the final file is committed) and say so in handoff.md

---

## Phase 2: Foundational

**Purpose**: probes that can change the spec, and the pure parts every story uses.

**⚠️ CRITICAL**: T003 and T004 can change the spec (research R4, R7); a failure stops that part
for the maintainer's decision, and the spec is updated before the code.

- [ ] T003 [P] Clipboard probe (research R4): crates/platform/examples/clipboard_probe.rs takes `<width> <height>`, waits 5 s (so another app can take the focus), puts a test pattern (eight color bars plus a gradient) on the clipboard through `arboard::Clipboard::set_image`, prints the call's time, and keeps running until Enter (Linux serves the selection while it runs); (maintainer) run it on Windows and on CachyOS (Wayland; an X11 session if installed) as quickstart "Probes" 1 says, paste in GIMP or Krita, Paint, Dolphin and Discord in the browser; record in specs/20261121-134830-screenshot/measurements/probes.md
- [ ] T004 [P] GlobalShortcuts probe (research R7): crates/platform/examples/shortcuts_probe.rs (Linux; on Windows prints that it is Linux only) over `zbus` blocking: `org.freedesktop.host.portal.Registry.Register("capturax", {})` when the interface exists (print whether), `CreateSession`, `BindShortcuts` with one shortcut `screenshot` (description "Screenshot", `preferred_trigger` from the argument, default `CTRL+F8`), prints the response's shortcuts with `trigger_description`, every `Activated`/`Deactivated` and `ShortcutsChanged`, and with `--configure` calls `ConfigureShortcuts` when the portal's version is ≥ 2; (maintainer) run it on CachyOS: key with another app focused, with a fullscreen game, after a restart of the example, and `--configure`; record what KDE asks the first time and whether the binding is kept in measurements/probes.md
- [X] T005 [P] Still conversion (research R2, data-model.md "Still image"): move `conversion(matrix, range)` and its tests from app/src/render/gpu.rs to crates/capture/src/still.rs (pub, re-exported; gpu.rs uses it); add `Still { width, height, rgb: Vec<u8> }`, `still(&Frame) -> Still` for NV12, I420 (bilinear chroma, the shader's sample positions), YUY2, UYVY (the shader's hand filter), BGRx and RGBA (copied), `Still::rgba() -> Vec<u8>` (alpha 255) and `Still::png() -> Result<Vec<u8>, String>` with `png` 0.18, `Compression::Fast`, adaptive filter, sRGB chunk, RGB 8-bit; tests: flat colors in every format and both matrices and ranges equal the shader's rows within ±1, a 2×2 chroma pattern, odd sizes, PNG round trip with `png::Decoder` (size, pixels)
- [X] T006 [P] Settings (contracts/settings-file.md): in crates/settings/src/model.rs add `Screenshot { folder: String }` (`"default"`) as `[screenshot]` and `Hotkeys { screenshot: String }` (`"Ctrl+F8"`) as `[hotkeys]`, kept as text as `mode` is (the app parses them); rules: "A missing `folder`, an empty or relative path, or a non-text value reads as `"default"`"; "A missing hotkey takes its default; `""` is kept as no key"; an invalid text takes the default (the app checks validity, the settings crate keeps text); tests for each rule and a round trip
- [X] T007 [P] Hotkey (data-model.md "Hotkey", research R14): the pure part of crates/platform/src/hotkeys.rs: `Modifier`, `Modifiers`, `Key` (the closed list), `Hotkey { modifiers, key }`, `parse` (ignores case and spaces), `Display` (file form, `Ctrl+Alt+Shift+Super+<Key>` in that order), `allowed()` ("F1–F24 alone; any other key with a modifier; A–Z, 0–9 and Space need Ctrl, Alt or Super"), `to_portal()` (`CTRL+SHIFT+F8`, `LOGO` for Super), `windows_vk()`, `x11_keysym()`; `shown(platform)` with `Win` on Windows; tests for each
- [X] T008 Frame hand-off (research R1, data-model.md "Frame hand-off"): in app/src/render/mod.rs add to `State` a queue `stills: VecDeque<(u64 serial, Instant asked, Sender<Option<Frame>>)>` and `flash_at: Option<Instant>`; `Inbox::request_still(serial, reply)`; after the video thread uploads a presented frame, if a request waits, the frame is moved into its reply instead of dropped and `flash_at = now` (only while shown); while not shown, the next arriving frame answers the oldest request (no flash); a request older than 500 ms is answered `None`; `clear_video` and `set_session` answer every waiting request `None`; unit tests on `State` for order, one frame per request, timeout, clear
- [X] T009 Event notice (research R13): generalize the device notice (app/src/app.rs `DeviceNotice`, `notice_text`, main.slint `device-notice-*`, app/ui/components/notice.slint) into an event notice with a kind per message (existing device kinds unchanged in look and behavior) and screenshot kinds `saved`, `saved-not-copied`, `copied-not-saved`, `failed`, `nothing`, `busy` (icons `camera.svg`, `camera_off.svg`; actions **Show**, **Settings** or none, data-model.md "Outcome"); system notifications use the tags `show:<path>` and `settings:screenshot`, dispatched in the notification action callback; device notices keep working (check by unplugging the C920)

**Checkpoint**: probes recorded; pure parts tested; device notices unchanged.

---

## Phase 3: User Story 1 - Save the moment while playing (Priority: P1) 🎯 MVP

**Goal**: Ctrl+F8 saves the frame on screen as a PNG, copies it, plays the sound, flashes the
edges and shows the notice, inside or outside the window, without a dropped frame.

**Independent Test**: quickstart checks 1–7 and 10, and the latency check, on both systems.

- [X] T010 [P] [US1] File writing (research R3, data-model.md "File name"): app/src/screenshot.rs `file_name(time, n)` (`CapturaX YYYY-MM-DD HH-MM-SS.png`, ` (n)` for n ≥ 2), `write_png(folder, time, bytes) -> Result<PathBuf, WriteError>`: create the folder when its parent exists, remove `.part` files older than 1 minute, write and sync `.<name>.part`, `hard_link` to the first free name, remove the `.part`; on an unsupported hard link, check-then-rename; `WriteError` kinds `FolderNotFound`, `NoPermission`, `DiskFull`, `Other` from `io::ErrorKind`/raw OS codes; tests with `tempfile`: names, numbering, an existing file never replaced, no `.part` left, missing parent
- [X] T011 [P] [US1] `Screenshots` state (data-model.md "Screenshot request"): pure, in app/src/screenshot.rs: `request(shows_image) -> Command | Outcome` ("not an image → outcome `Nothing`; `waiting == 4` → outcome `Busy`; else `waiting += 1` and the command `Take(serial)`"), `taken`, `not_taken`, `written`; tests for every rule and a burst of 6
- [X] T012 [P] [US1] Files, part 1 (contracts/platform.md "Files", research R12): crates/platform/src/files.rs with `default_screenshots_folder()` (`dirs::picture_dir()` + `CapturaX`, else home + `Pictures/CapturaX`), `show_file`, `open_folder`; windows/files.rs (`SHOpenFolderAndSelectItems` with `ILCreateFromPathW`, `ShellExecuteW("open")`) and linux/files.rs (`org.freedesktop.FileManager1.ShowItems`/`ShowFolders`, else `org.freedesktop.portal.OpenURI.OpenDirectory`/`OpenURI` over `zbus`), each on a short-lived thread so the caller never waits; check by hand on both systems
- [X] T013 [US1] Clipboard (contracts/platform.md "Clipboard", research R4; after T003 passes): crates/platform/src/clipboard.rs, a `clipboard` thread owning one `arboard::Clipboard` for the app's lifetime, `set_image(width, height, rgba, timeout)` sending the job and waiting for its answer at most `timeout`
- [X] T014 [US1] Worker (research R5): in app/src/screenshot.rs a `screenshot` thread started at the first screenshot: `still` → `rgba` (shared `Arc<[u8]>`) → `png` → `write_png` into the folder resolved from the settings at the frame's time ("default" → `default_screenshots_folder()`) → `Clipboard::set_image` (2 s) → posts `written(serial, result)` to the event loop; logs one line per screenshot as contracts/cli.md shows; at quit waits at most 3 s
- [X] T015 [P] [US1] Windows hotkeys (research R6, contracts/platform.md "Hotkeys"): crates/platform/src/windows/hotkeys.rs, a `hotkeys` thread with a message loop, `RegisterHotKey(NULL, id, mods | MOD_NOREPEAT, vk)` per binding, `WM_HOTKEY` → `Pressed(action)`, `set`/`suspend` posted with `PostThreadMessageW`, a refused key logged once and reported `Bound { shown: None }`, unregister on drop; `configure()` false, `changed_by_desktop()` false
- [X] T016 [P] [US1] Linux X11 hotkeys (research R9): crates/platform/src/linux/hotkeys/x11.rs with `x11rb`: own connection, `GrabKey` on the root window ×4 (Num Lock, Caps Lock), keycodes from `GetKeyboardMapping`, `XkbPerClientFlags` detectable auto-repeat, one `Pressed` per press; chosen when the session is X11 (`XDG_SESSION_TYPE`/`DISPLAY` without `WAYLAND_DISPLAY`)
- [ ] T017 [US1] Linux Wayland hotkeys (research R7; after T004 passes): crates/platform/src/linux/hotkeys/portal.rs from the probe: register the app id, session, `BindShortcuts` with every binding (`to_portal()`), `Activated` → `Pressed`, response and `ShortcutsChanged` → `Bound { shown: trigger_description }`, `configure()` calls `ConfigureShortcuts` (version ≥ 2), `changed_by_desktop()` true; without the interface report `WindowOnly`; crates/platform/src/linux/hotkeys.rs picks X11, portal or window-only
- [X] T018 [US1] App hotkeys (data-model.md "Hotkey binding", research R8): app/src/hotkeys.rs reads the bindings from the settings (invalid or not `allowed()` → default, logged; two equal keys → the first action keeps it, logged), starts `Hotkeys` after the window exists, maps `Pressed("screenshot")` to the screenshot request, and on `WindowOnly` matches winit key events against the bindings while the window has the focus (and only then); tests for the reading rules
- [X] T042 [US1] Silent refusals (spec FR-008, maintainer 2026-11-22; changes T009 and T011): `Screenshots::request` returns `Refused::{NoImage, Busy}` and `not_taken` nothing, the app only logs them; the event notice kinds `nothing` and `busy`, their texts and pt-BR translations, and their rows in the notice preview are removed
- [X] T019 [US1] Wiring (data-model.md "Outcome"): in app/src/app.rs the request path (key, later the button and `--screenshot-every`) → `Screenshots` → `Inbox::request_still` → `taken` → sound (T023) and worker → `written` → event notice (T009) with the texts of spec FR-008; **Show** calls `platform::show_file`; the preview state decides `shows_image`; a request that takes no frame (hidden to the tray, no image, busy, 500 ms) only logs (T042)
- [X] T020 [P] [US1] Flash (FR-010a, research R11; outline since 2026-11-22): app/src/render/composite.wgsl adds `opacity × max(line, glow)` white over the letterboxed video inside its rounded corners, `line` a 3 logical px anti-aliased stroke on the rounded edge and `glow` an inner Gaussian (deviation 9 logical px, 35 %); the uniform carries the opacity, line width and deviation in physical px; app/src/render/mod.rs computes the opacity from `flash_at` per present (40 ms ease-out rise, 400 ms ease-in fall), drawn under the UI layer, and keeps presenting UI-only frames while a flash runs if no video frame comes; a new flash restarts it
- [X] T021 [P] [US1] Sound (research R10, data-model.md "Sound"): crates/audio/src/sound.rs: WAV reader (RIFF/WAVE, `fmt ` PCM 16/24/32-bit and float 32-bit, `WAVE_FORMAT_EXTENSIBLE`, any rate and channels; other chunks skipped), `Sound::Screenshot` with its bytes passed in by the app, `prepare(&decoded, output Format) -> Prepared` with `interpolate` and `ChannelMap`, `Voice` with `play()` (lock-free restart request) and `mix(out)` (adds `gain 0.5`, clamps, nothing allocated, no lock waited on); tests: each sample format, resampling 44.1→48 kHz length, mono→stereo, restart, past the end silent
- [X] T022 [US1] Output-only session (contracts/platform.md "Output-only session"): `SessionRequest.input: Option<String>` in crates/platform/src/audio.rs; with `None` crates/platform/src/windows/audio/stream.rs and crates/platform/src/linux/pipewire.rs open only the output side (`Opened` for it, `Closed` when it ends; `reopen_input` ignored); update the existing callers to `Some(..)`; check with crates/platform/examples/audio_session.rs gaining `--output-only` (plays a 440 Hz tone 2 s) on both systems
- [X] T023 [US1] Sound player (data-model.md "Sound player"): app/src/sounds.rs routes `play(Sound::Screenshot)` to the preview `Feed`'s voice when its output side is open, else to an output-only session on the output resolved as the preview audio's (`OutputChoice`, missing-output rule), shared, opened on the first play, closed 3 s after the sound ends, a play while opening waits for `Opened` at most 300 ms (FR-010; then dropped and logged); the preview session opening closes the output-only one; app/src/sound.rs: the `Feed` holds the `Voice` and calls `mix` after `bridge.pull` in `fill`, the `Prepared` made on the main thread when the output format is known; tests for the routing with a fake session
- [X] T024 [P] [US1] Debug flag (contracts/cli.md): `--screenshot-every <ms>` in app/src/cli.rs, debug builds only, driving the same request path as the key
- [ ] T025 [US1] (maintainer) Windows: quickstart checks 1–7 and 10 with the ezcap; record in measurements/acceptance.md
- [ ] T026 [US1] (maintainer) Linux (CachyOS, Wayland; X11 if installed): quickstart checks 1, 2, 4–7 and 10; record in measurements/acceptance.md
- [ ] T027 [US1] (maintainer) Latency check (SC-003, research R16): quickstart "Latency check" at 1080p60, 1080p120 and 4K30 on both systems; record in measurements/latency.md with SC-001's times from the log lines

**Checkpoint**: the MVP: a screenshot by key, on both systems, with no preview cost.

---

## Phase 4: User Story 2 - Screenshot button (Priority: P1)

**Goal**: the right dock appears with the screenshot button.

**Independent Test**: quickstart check 8 and the tooltip.

- [x] T028 [US2] Right dock (FR-011): in app/ui/main.slint show the right dock (device preview FR-002a placement) with a camera button (`camera.svg`), disabled while the video area shows a state; tooltip "Screenshot (<key>)" with the key shown per platform (`Hotkey::shown`), or "Screenshot" when none; the click goes to T019's request path; both themes, all layouts down to 640 × 360
- [x] T029 [US2] (maintainer) quickstart check 8 and the dock in both themes and the compact layout

---

## Phase 5: User Story 3 - Change the key (Priority: P2)

**Goal**: the Hotkeys category records, clears and resets keys.

**Independent Test**: quickstart checks 11–13.

- [X] T030 [US3] Key recorder (FR-016–FR-018): pure state in app/src/hotkeys.rs: `start(action)` (suspends the hotkeys, FR-017), `key(event)` → accepted (saved, re-registered at once), refused (not `allowed()`, or used by another action: which one), Esc cancels; `clear`, `reset` (refused when another action has the default); the window's winit key events mapped with `from_window` (research R14), modifiers alone ignored; tests for each
- [ ] T031 [US3] Hotkeys category (FR-016, FR-012): in app/ui/components/settings-panel.slint add `hotkeys` after `screenshot` (icon `keyboard.svg`), one row per action: name, key button ("Press a key…" while recording, the refused reason), Clear, Reset (only when not the default); on Wayland with the portal the key shown is the desktop's and Change calls `configure()` or shows "Change it in the system settings"; window-only shows "Works only while CapturaX is focused"; key names shown per platform and language (`Win`/`Super`, the locale's names such as `PgUp`) come from app/ui/texts.slint and the catalog
- [ ] T032 [US3] (maintainer) quickstart checks 11–13 on both systems

---

## Phase 6: User Story 4 - Choose the folder (Priority: P2)

**Goal**: the Screenshot category picks, opens and resets the folder.

**Independent Test**: quickstart check 14.

- [ ] T033 [P] [US4] Files, part 2 (research R12): `pick_folder(owner, start, done)` in crates/platform/src/files.rs: windows/files.rs `IFileOpenDialog` with `FOS_PICKFOLDERS` on its own STA thread owned by the main window; linux/files.rs portal `FileChooser.OpenFile` with `directory: true`, the answer from the `Request`'s `Response` signal; false at once without the portal
- [ ] T034 [US4] Screenshot category (FR-020–FR-022): in app/ui/components/settings-panel.slint add `screenshot` after `audio` (icon `camera.svg`) with the folder row: the folder shortened, the full path in its tooltip, **Change** (T033; the answer saved as an absolute path; "No folder picker on this desktop" when none), **Open** (`open_folder`), **Reset** (only when not `"default"`)
- [ ] T035 [US4] (maintainer) quickstart check 14 on both systems

---

## Phase 7: User Story 5 - Clear failures (Priority: P3)

**Goal**: every failure has its notice and next action.

**Independent Test**: quickstart checks 9 and 15.

- [ ] T036 [US5] Failure texts and actions (FR-008, data-model.md "Outcome"): reasons from `WriteError` (`folder not found`, `no permission`, `disk full`, `write error`) and the conversion's `image error`; **Settings** opens the settings panel on the Screenshot category; tests for the outcome → kind/text mapping in app/src/screenshot.rs
- [ ] T037 [US5] (maintainer) quickstart checks 9 and 15 on both systems

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T038 [P] Docs: README (screenshot, the key, the folder), AGENTS.md "Status", docs/product.md Spec column for F19 and F50 (framework, screenshot), docs/roadmap.md #5 done with its links
- [ ] T039 (maintainer) quickstart check 16 (languages and themes) and the flash's look (curve and timings, research R11), tuned with the maintainer; the spec is updated if the values change
- [X] T040 Final sound: the maintainer sends the WAV; it replaces app/sounds/screenshot.wav, is removed from `.git/info/exclude`, the gain is tuned with the maintainer by ear, and the file is committed with the code that embeds it (handoff)
- [ ] T041 Handoff and acceptance summary in measurements/acceptance.md; the maintainer validates; the branch is ready for the pull request

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → stories. T003 gates T013; T004 gates T017.
- US1 needs T005–T009. Inside US1: T010, T011, T012, T015, T016, T020, T021, T024 in
  parallel; T013 after T003; T014 after T010, T013; T017 after T004; T018 after T015–T017; T022
  before T023; T019 after T011, T014, T018, T023; manual tasks last.
- US2 (T028) after T019. US3 after T018. US4: T033 any time after Phase 1, T034 after T006.
  US5 after T019.
- T040 last before the final commit of the code that embeds the sound.

### Parallel example (US1)

```text
T010 file writing    T011 state    T012 files part 1    T015 Windows hotkeys
T016 X11 hotkeys     T020 flash    T021 sound           T024 debug flag
```

## Implementation Strategy

1. Phase 1 and 2, with the probes run early on both PCs (batch the ezcap's checks where it is).
2. US1 as the MVP: key → file, clipboard, sound, flash, notice; measure SC-003 before going on.
3. US2 (button), then US3 and US4 (settings categories), then US5.
4. Polish; the final sound replaces the placeholder before the code that embeds it is committed.
