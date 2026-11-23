# Handoff: Screenshot

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261121-134830-screenshot`

## State

Spec approved (maintainer, 2026-11-22). Plan, tasks (T001–T041) and `/speckit-analyze` done: no
critical issue; fixed FR-001 (next frame presented), FR-010 (300 ms when the output opens
first), FR-015 (window-only exception) and small wording. Docs committed on this branch, not
pushed. Implementation started: T001 (dependencies) and T002 (placeholder sound generator)
done and committed; the probes' code (T003, T004) is committed and waits for the maintainer's
runs. T005–T007 (still conversion, settings, hotkey value) done and committed. T008 (frame
hand-off) committed. T009–T012, T020 and T024 tested on Windows by the maintainer and committed
(`c5cd7e1` platform files and local time, `21cd3ae` outline flash, `fb9c21a` event notice and
screenshot state/writing, then the debug flag with its temporary wiring). T015, T016, T021 and
T022 done and committed as unwired code (fmt, clippy, tests); their manual checks wait (Next
step). T018 (app hotkeys) and T042 (silent refusals) checked on Windows and committed
(`f3c5505`); T023 (sound player) committed with the maintainer's final sound (T040). T014
(worker) committed as unwired code. T013 (clipboard) and T019 (wiring) done 2026-11-23 and
checked on Windows by the maintainer (measurements/acceptance.md, first pass). T028 (dock
button) done 2026-11-23 and checked by the maintainer (T029, acceptance.md). T030 (key
recorder) done 2026-11-23 as unwired code and committed; T031 (Hotkeys category) done
2026-11-23 and checked on Windows by the maintainer (quickstart 11 and 12). T033 (folder
dialog, `7b41db6`) and T034 (Folders category, `8d22042`, spec change `bca5ded`) done
2026-11-23; check 14 passes on Windows (T035's Windows half, acceptance.md). T036
(failure notices, **Settings** opens Folders) done 2026-11-23; checks 9 and 15 pass on Windows.
T038 (docs) done 2026-11-23. T004 (probe) run and T017 (Wayland portal hotkeys) built and checked
on CachyOS over SSH by the agent, 2026-11-23 (check 13 passes); validated and committed
(`docs(screenshot): record the shortcuts probe…`, `feat(platform): bind hotkeys through the
Wayland shortcuts portal`). Default keys moved to the Ctrl+Shift family (Decisions).
The Linux (Wayland) checks were run by the agent on CachyOS, 2026-11-23: T026, T032, T035 and
T037 done; T003 done (Krita AppImage).
T027's Windows half and the rest of T025 the agent can do (checks 5 and 7, a quit with screenshots
waiting) done by the agent on Windows, 2026-11-23 (measurements/latency.md, acceptance.md third
pass); SC-003 amended for 4K (Decisions). Checks 3 and 10 passed on Windows (maintainer,
2026-11-23): T025 done.

## Done in the last sessions

- 2026-11-21: branch and spec created; roadmap #5 and #9 and product.md F19 updated.
- 2026-11-22: flash added to the spec (FR-010a); plan written; sound simplified to one sound on
  the chosen output; tasks written.
- 2026-11-22: T001 (`ae01e6d`): `arboard`, `png`, `x11rb` and the Windows features; `cargo tree
  -d` shows `image` twice but both are 0.25.10 (a host/target feature split, not a second
  version). T002 (`d7a7705`): `tools/make_screenshot_sound.py`; `app/sounds/screenshot.wav` is
  generated locally and listed in `.git/info/exclude` (not .gitignore), so it stays untracked
  until T040. T003/T004 code (`c466794`): `clipboard_probe` and `shortcuts_probe` (the latter
  type-checked for Linux in a scratch crate with only `zbus`); clipboard set time on Windows
  recorded in measurements/probes.md (release 22 ms at 4K, debug 945 ms).
- 2026-11-22: T005 (`7dd54fd`): `crates/capture/src/still.rs` with `conversion` moved from
  gpu.rs (the shader's comment points there now), `still`, `Still::rgba`, `Still::png`; NV12/I420
  chroma taps at `(x + 0.5)·half/full − 0.5`, clamped (the sampler's positions). T006
  (`181f575`): `[screenshot] folder` and `[hotkeys] screenshot` in the settings crate; the folder
  rule uses `Path::is_absolute` (so it is the running system's notion of absolute); the hotkey
  text is kept raw (missing or non-text → `Ctrl+F8`), validity and duplicates are the app's
  (T018). T007 (`9b09bff`): `crates/platform/src/hotkeys.rs` pure part; `Key` is
  `Letter(u8)`/`Digit(u8)`/`F(u8)` plus named keys (contracts/platform.md updated to match);
  `parse` also reads `Win` as Super; `Modifiers::windows_mod()` and `x11_mask()` added for
  T015/T016; portal key names are XKB keysym names (`a`, `Page_Up`, `Print`), to check with the
  T004 probe.
- 2026-11-22: T008 (`19762e8`): `State.stills` (serial, asked, reply) and `flash_at` in
  app/src/render/mod.rs; the video thread hands a frame over right after `set_video` (lock taken
  once per frame, the frame dropped outside it); off screen (`!shown || hidden`) `send_frame`
  answers with the arriving frame instead of keeping it, no flash; an asker gone passes the frame
  to the next; 500 ms limit checked in the video loop (its wake includes the oldest deadline) and
  on each request; `clear_video` and a session change answer `None`; a new video thread keeps
  the waiting ones. `request_still` carries `expect(dead_code)` until T019.
- 2026-11-22: T009 (uncommitted): `EventNotice`, `Reason` and `NoticeAction` in
  app/src/notices.rs (kind, text, action; tag `show:<path>`, `settings:screenshot`, else the
  device id, so device tags are unchanged); `UserEvent::NoticeOpen` became `NoticeAction { tag,
  system }`; **Show** only logs until T012, **Settings** opens the panel (category with T034).
  Slint `DeviceNotice` → `EventNotice` with `action-label`/`action()`; main.slint
  `event-notice-*`. Icons `camera.svg`, `camera_off.svg` (Tabler 3.35.0). Colors (agent's choice,
  for review): saved green camera; saved-not-copied and copied-not-saved yellow camera; failed
  yellow camera-off; nothing and busy grey camera-off. Texts and pt-BR translations added
  ("Muitas capturas de tela ao mesmo tempo" for busy). Preview:
  `slint-viewer app/ui/previews/event-notice-preview.slint`.
- 2026-11-22: T010/T011: app/src/screenshot.rs (`Screenshots`, `Command`, `Written`,
  `LocalTime`, `file_name`, `write_png`, `WriteError` → `Reason`). Local time: new
  `platform::utc_offset` (Windows `FileTimeToLocalFileTime`, Linux `localtime_r` declared like
  `clock_gettime`, no new crate; contracts/platform.md and data-model.md updated); the date via
  `log::civil_from_days` (now `pub`). Hard link refused for any reason other than
  `AlreadyExists` → check-then-rename (Linux vfat gives EPERM, not "unsupported"). Disk full is
  std's `StorageFull`/`QuotaExceeded` (checked: Windows 112 and 39 map to it), no `cfg` in the
  app (the boundary test forbids it).
- 2026-11-22: T012: crates/platform/src/files.rs + windows/files.rs (`ILCreateFromPathW` +
  `SHOpenFolderAndSelectItems` on a COM STA thread, `ShellExecuteW`) + linux/files.rs
  (`FileManager1.ShowItems`/`ShowFolders`, else portal `OpenDirectory` with an fd, else
  `OpenURI`); type-checked for Linux in a scratch crate. Example
  `cargo run -p capturax-platform --example files [-- show <file> | open <folder>]`. **Show** on
  the notice now calls `show_file`.
- 2026-11-22: T020: the edge glow was built, seen in the app (works; judged ugly), and replaced
  by the **outline** (mockup option D; spec FR-010a, research R11, tasks T020 updated): the
  uniform grew to 28 floats with a `flash` vec4 (opacity, line 3 × scale px, glow deviation
  9 × scale px; scale = `scene.shadow[0]`); white line on the rounded edge + inner Gaussian glow
  35 %; curve `flash_opacity` 40 ms ease-out / 400 ms ease-in. The loop keeps presenting while
  `flash_at` is set and clears it on the first present after the end. Outline seen in the app
  and approved (maintainer, 2026-11-22). Hand-off measured: 91 screenshots, frame +0–14 ms after the request.
- 2026-11-22: T024: `--screenshot-every <ms>` (debug builds, 0 = off). **Temporary wiring** for
  the test: App `request_screenshot` → `Screenshots::request` → `Inbox::request_still`, a
  "screenshot wait" thread posts `UserEvent::Still { serial, asked, frame: Arc<Frame> }`; the
  frame is logged ("screenshot N: WxH taken +X ms", time until the event loop got it) and
  dropped, the slot freed with no notice. T014/T019 replace the end of `still()`.

- 2026-11-22: T015/T016: `Hotkeys`, `HotkeyEvent`, `Binding` in crates/platform/src/hotkeys.rs
  with a shared `Wanted` (bindings, suspend, refused keys logged once, `report` → one `Bound` per
  action, none while suspended). Windows (windows/hotkeys.rs): `hotkeys` thread, queue created
  with `PeekMessageW` before its id is handed back, commands through a channel + `WM_APP`,
  `RegisterHotKey(NULL, index + 1, mods | MOD_NOREPEAT, vk)`, `WM_QUIT` on drop. X11
  (linux/hotkeys/x11.rs): own `RustConnection` shared with an `Arc`, an InputOnly window woken by
  a `ClientMessage` sent to itself; `GrabKey` ×4 locks per keycode whose first group types the
  keysym (two levels: digits on AZERTY), each checked (BadAccess → refused, partial grab undone);
  detectable auto-repeat through XKB, else a press with the last release's keycode and time is a
  repeat; `MappingNotify` regrabs. linux/hotkeys.rs picks X11 (`XDG_SESSION_TYPE`, else `DISPLAY`
  without `WAYLAND_DISPLAY`) or reports `WindowOnly` (until T017). `x11rb` now names its `xkb`
  feature in crates/platform/Cargo.toml (was on through winit). Linux type-checked with clippy in a
  scratch crate (`x11rb` + `log`). Example `cargo run -p capturax-platform --example hotkeys --
  [<key>…] [--suspend-after <s>]`. The agent did not run it: registering global keys is a
  foreground test (maintainer's rule).
- 2026-11-22: T021: crates/audio/src/sound.rs: `decode` (RIFF/WAVE, PCM 16/24/32, float 32,
  extensible with the PCM/float subformat GUID, odd chunks padded, a `data` size past the file
  read to its end in whole frames), `prepare` (same rate: channel map only; else one silent frame
  before and two after so `interpolate` has its neighbours, length `ceil(frames·out/in)`),
  `Voice` (`play`, `playing`, `mix(out, format)`: adds nothing when the format is not the prepared
  one), `GAIN` 0.5. 9 tests.
- 2026-11-22: T022: `SessionRequest.input: Option<String>`. Windows: no input thread, `running`
  starts at 1, `reopen_input` ignored. Linux: `Session.output_only` in the `pipewire` thread (the
  input `Wanted` is never opened, so never `waiting`; `reopen` of the input returns) and in
  linux/audio.rs (`reopen_input` ignored; only the output's error when PipeWire is down). App
  callers pass `Some(..)` (behavior unchanged). `audio_session --output-only`: 440 Hz tone 2 s,
  a reopen of the input that must give no event, close at 3 s. The Linux part is reviewed, not
  compiled (pipewire-sys needs the library): build it on CachyOS first.

- 2026-11-22: T023 (**uncommitted**: it embeds the placeholder sound, see Decisions):
  app/src/sounds.rs: `Router` (pure; `PreviewOutput` Closed/Opening/Open, `Action`s; 11 tests)
  and `Sounds` (decodes the embedded WAV, prepares and caches the voice for the output's real
  format: the feed's last `fill`, else `Opened`'s; opens the output-only session, shared, events
  as `UserEvent::SoundSession`). app/src/sound.rs: `Feed` holds the `Voice` in `Formats` (same
  lock as the bridge) and mixes it after `pull`; `closing` became `Vec<Closing>` (`own` marks
  the sound player's sessions, logged "sound session N"), so a new preview session waits for the
  output-only one to close; `Sound::play`, `sound_session`, `tick_sounds`, `next_sounds`; the
  preview state comes from `Open`/`ReopenOutput`/`Close` commands, `Opened`/`Error` of the output
  side. `AudioController::sound_output`. Refinements written in data-model.md "Sound player".
  **Temporary wiring**: `still()` in app.rs plays the sound when a frame is taken (T019 keeps it
  there). fmt, clippy (workspace) and 247 app tests pass; not yet heard.

- 2026-11-22: T018 (uncommitted, waits for the maintainer's check): app/src/hotkeys.rs:
  `ACTIONS` (`screenshot` → `settings::DEFAULT_SCREENSHOT_KEY`, now re-exported with
  `settings::Hotkeys`), `read` (rules 2 and 3 of contracts/settings-file.md through `resolve`,
  which takes `(action, default, text)` so the duplicate rule is tested with three made-up
  actions), `from_window` (winit `key_without_modifiers` + modifiers → `Hotkey`; letters/digits
  from the layout's text, a digit also from the physical top row for AZERTY), `AppHotkeys`
  (`start`, `register` with the descriptions, `event` → the action pressed, `Bound` logged
  "hotkeys: screenshot on Ctrl+F8", `WindowOnly` sets `window_only`; `window_key` matches only
  then, pressed and not repeat). app.rs: `UserEvent::Hotkey`, `start_hotkeys` right after the
  window exists (before the start-hidden return, FR-015), `hotkey_action` → `request_screenshot`
  (so today: frame taken, sound, frame dropped: T019's temporary end), `ModifiersChanged`
  tracked and still passed to Slint, a matched window key not passed to Slint; on a language
  change the bindings are registered again with the translated description (new text
  `hotkey-screenshot` "Screenshot" / "Captura de tela", for the portal's list and T031). 8 new
  tests (255 app tests), clippy clean. Checked on Windows by the maintainer (2026-11-22, log
  read by the agent): Ctrl+F8 with another app focused fires (sound heard), a held key gives one
  press, minimized takes a real frame, hidden in the tray gives "no frame within 500 ms".
- 2026-11-22: T042 (uncommitted): after that check the maintainer asked for **silent
  refusals** (spec FR-008, US5 scenario 1, Edge Cases, FR-004, FR-007, FR-010/FR-010a,
  data-model, research R1/R5/R13, plan, quickstart 5/8/9 updated first): `Screenshots::request`
  returns `Refused::{NoImage, Busy}`, `not_taken` returns nothing; the app logs "screenshot: not
  taken (…)". `EventNotice::Nothing`/`Busy`, their texts, translations and preview rows removed.
- 2026-11-22: T040: the maintainer's sound replaced the placeholder (app/sounds/screenshot.wav,
  44.1 kHz stereo 16-bit, 1.6 s file, audible ≈ 430 ms, peak −10.4 dBFS; gain 0.5 kept); removed
  from `.git/info/exclude` and committed with T023. CC0 options (OpenGameArt shutters, Kenney
  Interface Sounds) were compared on https://claude.ai/artifact/QD1N5U1rv1YdEfcL4tuHV4 and set
  aside: the maintainer's own sounds keep one instrument, timing and pattern for the later ones.
  `tools/make_screenshot_sound.py` stays as a test-click generator.
- 2026-11-22: T018 checks done on Windows: refused key (the `hotkeys` example holding Ctrl+F8
  first: "Ctrl+F8 refused by the system … (os error 1409)", then "screenshot has no key"); tray
  press silent; No audio: sound session opened in 10 ms and closed ≈ 3 s after the sound. Hidden
  to the tray is refused at once (`Preview::hidden()`; the state still reads Playing there), seen
  with `--autostart --screenshot-every 1000`: "screenshot: not taken (NoImage)".
- 2026-11-22: docs/roadmap.md #17 **UI review** (new, any time after #5) collects small UI fixes:
  menu buttons' orange → the settings categories' light semi-transparent orange; F59 (new in
  product.md) a **Log** category showing the log, scrollable (maintainer).

- 2026-11-22: T014: app/src/screenshot.rs `Worker` (`screenshot` thread; `write(Job)` queues,
  `stop(limit)` lets the queue finish and waits at most `limit`, `QUIT_WAIT` 3 s, through a
  channel that disconnects when the thread ends), `Job { serial, frame: Arc<Frame>, taken,
  folder, time }` (the folder and the local time are the main thread's, at the frame's
  arrival), `folder(setting)` ("default" → `default_screenshots_folder()`; `settings::
  DEFAULT_FOLDER` now re-exported), `process`: `still` → frame dropped (back to the pool) →
  `png` → `write_png` → `rgba` as `Arc<[u8]>` → copy, one log line (contracts/cli.md now lists
  the failure endings). A PNG that cannot be made gives `ImageError` and is still copied. The
  clipboard is a `CopyImage` closure `(w, h, rgba, timeout)` called with `COPY_TIMEOUT` 2 s:
  T019 passes `Clipboard::set_image` (T013). `png` is a dev-dependency of the app (the test
  decodes the file). 5 new tests (260 app tests), clippy clean.

- 2026-11-23: T013 (uncommitted): crates/platform/src/clipboard.rs, `platform::Clipboard`
  (`start`, `set_image(w, h, Arc<[u8]>, timeout)`): a `clipboard` thread opens one
  `arboard::Clipboard` at the first image (again at the next when it failed) and keeps it until
  dropped; a wrong buffer size is refused before the thread; a timeout answers the caller while
  the image is still set later. 3 tests (no real clipboard touched: that is a foreground test).
  Started on the Windows set time alone; the probe's pastes are checked through the app itself.
- 2026-11-23: T019 (uncommitted): app.rs `still()` runs `Screenshots::taken`'s commands: sound,
  then a `Job` (folder from `[screenshot] folder` and local time at the frame's arrival) to the
  `screenshot_worker`, started at the first screenshot with `Clipboard::start()`;
  `UserEvent::Written` → `Screenshots::written` → `event_notice` (Show → `show_file`, already
  wired). The main thread's "taken" line is gone (the worker logs it). A worker that cannot
  start or is gone gives the `failed` notice (write error), since the sound already played.
  `exiting` stops the worker with `QUIT_WAIT` after the video thread, before the settings
  flush. The T019 `dead_code` markers in screenshot.rs and notices.rs removed. fmt, clippy,
  260 app tests pass. Checked in the app (acceptance.md). Note for testers: a hand-edited
  `capturax.toml` with a Windows path in `"…"` (a basic string, `\C` an invalid escape) is
  invalid TOML and the whole file is set aside as damaged; use `'…'` or forward slashes.
- 2026-11-23: roadmap #17 gained the Exclusive box's list flicker (maintainer).
- 2026-11-23: T028 (uncommitted): main.slint right dock (`right-area`/`right-dock`, mirrors the
  left: centered on the video area's height, under the toolbar in the compact layout, slides out
  to the right, after the left dock in the keyboard order, counts for `engaged`) with the
  `screenshot-button` (camera.svg, tooltip side left, `enabled: preview-state == "playing"`,
  which is also what the test video sets). `in screenshot-key` (set in `start_hotkeys` from
  `AppHotkeys::key`, new: the key the user set, shown with `Hotkey::shown(IS_WINDOWS)`, empty
  when none, so a key the system refused still shows) and `callback screenshot()` →
  `UserEvent::Screenshot` → log "screenshot: dock button" → `request_screenshot`. Tooltip
  `@tr("Screenshot ({})")` (pt-BR "Captura de tela ({})") or `@tr("Screenshot")`. IconButton:
  a disabled button now keeps its hover tooltip (US2 scenario 2; its TouchArea stays enabled,
  clicks, hold, wheel, right press and the hover tint ignored); no other IconButton is ever
  disabled today. Bubbles: `meets-right` → row and column end 8 px left of the right dock (the
  existing `column-end`); the bottom-right notice does not join the row then. The column always
  ends 8 px left of the right dock (the maintainer saw it behind the dock in a small window
  first: a column is taller than the one-row test). Notices keep clear
  of the right dock too (`right-dock-box`; `notice-past` beside a dock in the right half goes to
  its left). With one button the dock never reaches the bubble row (checked by numbers at
  640 × 360: dock bottom ≈ 234, row top ≈ 264). fmt, clippy, all tests pass. When T030/T031
  change a key, `set_screenshot_key` must be called again.
- 2026-11-23: T030 (unwired, committed): app/src/hotkeys.rs `Recorder` (pure: `start`, `press`,
  `cancel`, `recording()` → action and last `Refusal`), `Press::{Escape, Key, Other}` from the
  window's **winit** key events (`press_of`: modifiers alone, releases and repeats ignored; the
  rest through `from_window`), `Refusal::{NotAllowed, Taken(action)}`, `Recorded::{Accepted,
  Refused, Cancelled}`; a refusal keeps recording; the action's own key is accepted.
  `AppHotkeys`: `start_recording` (suspend), `record(&KeyEvent)` (accepted → bindings sent at
  once, then unsuspended), `cancel_recording`, `clear`, `reset` (refused when another action has
  the default), `register` now keeps the descriptions (`&mut self`) so a change re-sends them;
  `window_key` matches nothing while recording (FR-017). `set_setting(hotkeys, action, key)`
  writes the file form (`""` for none). Research R14 and T030 updated first: Slint's key text has
  the modifiers applied (Ctrl+Shift+1 → `!`, AZERTY `&`), so the recorder reads winit events
  before Slint, like window-only matching. 9 new tests (268 app tests). T031 must: route every
  `KeyboardInput` to `record` while `recording()` is some and not pass it to Slint (Esc must not
  close the panel), call `cancel_recording` when the panel closes or the window loses the focus,
  save with `set_setting` + `store.update`, and call `set_screenshot_key` after every change.

- 2026-11-23: T031: Hotkeys category (`hotkeys` before About; T034 puts Screenshot before it),
  icon keyboard.svg (+ backspace.svg, arrow_back_up.svg, Tabler 3.35.0). `HotkeyRow`: name, a
  line under it (the refusal in `warn`, or "Esc to cancel" while recording), key `ActionButton`
  (primary while recording, "Press a key…", "None"), Clear and Reset `IconButton`s that keep
  their places when hidden. They are placed by hand: a `HorizontalLayout` there gave the row its
  fixed width as a maximum and the row shrank (seen in the app, fixed). `Prefs.hotkeys`
  (`HotkeyRowData`), `hotkeys-window-only`, callbacks `hotkey-record/cancel/clear/reset`;
  category change and panel close call `hotkey-cancel`. app.rs: `UserEvent::HotkeyUi`,
  `show_hotkeys` (rows + dock tooltip; after every change, language switch and non-press
  hotkey event), `save_hotkey`, `record_hotkey` (first `KeyboardInput` arm while recording:
  neither Slint nor F11/Esc see the key), focus lost cancels, a refused Reset kept in
  `hotkey_refusal`. Key names: `hotkeys::shown` with the catalog's `key-*` texts (Win/Super
  from the platform); refusals `hotkey-not-allowed`/`hotkey-taken`. pt-BR added. The portal
  part (desktop's key, Change → `configure()`) moved to T017 (tasks.md). Clear and Reset use the new
  `IconButton.filled` (top fill, `radius-s`, as the text buttons). 269 app tests.
- 2026-11-23: roadmap #17: settings panel larger in big windows/fullscreen, maybe a UI scale
  above the system DPI; and one set of control tokens for the settings panel (maintainer).
- 2026-11-23: T033 (`7b41db6`): `platform::pick_folder(owner, start, done) -> bool`
  (`FolderPicked`). Windows `IFileOpenDialog` (`FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM`) on a
  "folder dialog" STA thread, owner HWND passed as an integer, cancel → None. Linux: the portal
  `FileChooser.OpenFile` (`directory`, `modal`, `current_folder` as NUL-ended bytes,
  `handle_token`), listening on the predicted `Request` path before the call (and on the
  returned one if it differs); owner `x11:<id>` on X11, none on Wayland (would need
  xdg-foreign); false at once when the bus neither runs nor can start
  `org.freedesktop.portal.Desktop`. `path_from_file_uri` with tests. The Linux file was only
  type-checked (a scratch crate with zbus 5.19 and winit 0.30.13, target Linux): the full
  crate needs PipeWire's pkg-config, and WSL Ubuntu has no Rust.
- 2026-11-23: T034 (`8d22042`): settings-panel.slint `FolderRow` in a **Folders** category
  (`folder.svg`, Tabler, after Audio), row **Screenshots**: `screenshot::short_folder` (whole up
  to 40 characters, else start (drive/root + first folder), "…", as many last folders as fit),
  full path in a tooltip after 500 ms on it, **Change**/**Open** (ActionButtons, width
  `max(96px, preferred)`, the first build had none and stretched), **Reset** with the default
  folder in its tooltip, "No folder picker on this desktop" until the category changes.
  app.rs `FolderUi` (`Change` ignored while a dialog is open, `Picked` kept only absolute and
  UTF-8, `Open` makes the folder by `screenshot::make_folder`, the writer's rule, `Reset`).
  The maintainer asked for Folders instead of a Screenshot category (spec FR-020, FR-021,
  roadmap #9 updated in `bca5ded`); roadmap #17 gained category/row heights, double click for
  fullscreen and overlays closing on resize (`1ccef52`).
- 2026-11-23: T036: the reasons, texts, pt-BR translations and outcome mapping were already
  in place (T009, T010, T014). Added: `Prefs.settings-category` (the `Category` enum is now
  exported from settings-panel.slint and declared above `Prefs`); the panel's `category` is
  two-way bound to it, main.slint sets it back to General when the panel closes, and the
  notice's **Settings** sets Folders before opening the panel (also when it is already open).
  Test `every_failure_has_its_notice` (each `WriteError` and `ImageError`, copied or not →
  kind and action). The reason texts need a Slint window, so they are checked by eye
  (quickstart 9). 272 app tests. T037's Windows half passes (checks 9 and 15,
  acceptance.md); its Linux half needs CachyOS.

- 2026-11-23: T004 run by the agent over SSH on CachyOS (measurements/probes.md): passes, but
  the portal refuses an app with no `capturax.desktop` ("An app id is required"); a hidden one
  works. KDE asks once (OK/Cancel; Cancel leaves the action with no key and never asks again);
  afterwards the desktop's key wins over `preferred_trigger`; `ConfigureShortcuts` opens System
  Settings → Shortcuts on CapturaX; `ShortcutsChanged` comes (`""` then the new key). Ctrl+F9 and
  Ctrl+F10 are KWin's defaults (bound but never fire for the app). Spec updated first (Edge Cases
  "Wayland: the desktop owns the key" and "the app's desktop entry", FR-016 note, R7 "Probe
  result", contracts/platform.md portal rules, data-model, quickstart 13, T017, roadmap #13 note).
- 2026-11-23: T017: crates/platform/src/linux/desktop_entry.rs (helpers moved from
  autostart.rs: `keys`, `exec(exe, args)`, `program`, `executable`; `ensure`: a package's or the
  user's entry kept, else a hidden marked one written, a marked one gets only `Exec` replaced, a
  marked one shadowing a package's removed; 7 tests). linux/hotkeys/portal.rs: `hotkeys` thread
  owns the session, a `hotkeys signals` thread forwards the portal's signals; `ensure`,
  `Register`, `CreateSession`, `BindShortcuts` (rebinds in a new session only when actions or
  descriptions change, e.g. a language switch), Response/`ShortcutsChanged` → `Bound`,
  `Activated` → `Pressed` unless suspended, `configure()` (version ≥ 2), `changed_by_desktop()`
  once the session exists; any failure → `WindowOnly` (logged). linux/hotkeys.rs picks X11,
  portal (Wayland session), else window-only. App: `AppHotkeys` keeps the reported keys
  (`desktop_key`), `by_desktop`, `configure`; no recording there. Hotkeys row: the desktop's key,
  no Clear/Reset, key at the row's edge; click → `configure()`, else the category line "Change it
  in the system settings" (pt-BR "Altere nas configurações do sistema") until another category;
  dock tooltip from the desktop's key. README and the `hotkeys` example's doc updated. fmt,
  clippy and all tests pass on Windows and CachyOS; `cargo build --release` on CachyOS (the first
  full Linux build of this branch: the PipeWire change of T022 compiles). Checked with the
  `hotkeys` example and in the app (acceptance.md "Linux Wayland, portal hotkeys");
  `utc_offset` checked on Linux too.
- 2026-11-23: Linux checks by the agent over SSH (release build of `99aae30`, C920 and test
  video; the ezcap is on Windows): T003 on Wayland (set 6.2 ms at 4K; pasted in Dolphin and in a
  local Firefox page reading the `paste` event, the stand-in for Discord's web client; Krita through
  the official 6.0.4 AppImage in `~/capx/krita`, run with `QT_QPA_PLATFORM=wayland`, since the
  package database is too old to install GIMP without a full `pacman -Syu`), quickstart 1, 2,
  4–10, 14 and 15 (measurements/acceptance.md "Linux Wayland, screenshots"), the `audio_session
  --output-only` example. The sound was checked by recording a null sink chosen as the app's
  output, so nothing played on the maintainer's VBAN. Check 5 reached the limit of 4 for the
  first time (6 presses 25 ms apart: 5 written, 1 Busy). No X11 session is installed. Left on
  CachyOS: the KDE binding on Ctrl+Shift+F8, `~/capx/lib.sh` now with the right ydotool socket,
  `~/capx/peaks.py`, `~/capx/rec.sh` and `~/capx/paste/index.html` (paste test page).

## Decisions

- SC-003 at 4K (maintainer, 2026-11-23): with a screenshot every second at 4K30, a few seconds
  have one frame presented up to 9.2 ms late (0 frames lost, no audio gap; nothing at 1080p).
  Accepted as rare and unseen, without looking for the cause (a fix was not sure to exist);
  SC-003 now allows up to 10 ms at 4K. Candidates, for a later improvement: RGB conversion or
  the 33 MB clipboard copy competing for memory, or the flash and notice (latency.md).

- The F50 global hotkey framework and the screenshot key (Ctrl+F8) come in this spec, not in
  spec #9 (maintainer, 2026-11-21). Roadmap updated.
- The screenshots folder is a setting, default Pictures/CapturaX (maintainer, 2026-11-21).
- FR-008 (maintainer, 2026-11-22): the capture sound is the confirmation. A request that takes no
  frame (hidden to the tray, a state in the video area, 4 already waiting, no frame in 500 ms)
  does nothing visible or audible, only a log line; notices only after the click (saved, or a
  save/copy failure). Minimized still captures (passthrough use; the device stays open). Small UI
  issues go to roadmap #17, not into this spec.
- FR-010: a capture sound on the preview's output; no thumbnail (maintainer, 2026-11-21).
  The sound is the maintainer's own (T040, 2026-11-22); later sounds (recording, live, viewers)
  come from the same instrument, timing and pattern.
- FR-010a: an **outline flash** (maintainer, 2026-11-22): a thin white line on the video's
  rounded edge with a faint inner glow, 40 ms in, 400 ms out. Chosen from the mockup
  https://claude.ai/artifact/XknXi4EvntqmHHZU4axncC (option D) after the first edge glow was
  built and seen in the app and judged ugly (research R11).
- FR-020 (maintainer, 2026-11-23, after seeing a Screenshot category in the app): the folder
  row goes in a **Folders** category, Hotkeys after it; spec #9 adds the recordings folder
  there (its other options live in the Record panel).
- Wayland desktop entry (maintainer, 2026-11-23): the app writes a **hidden** `capturax.desktop`
  when none is installed and only fixes its `Exec` later; showing it in the menu is an option
  for spec #13 (roadmap note).
- Default keys use **Ctrl+Shift** on both systems (maintainer, 2026-11-23): screenshot
  Ctrl+Shift+F8, live Ctrl+Shift+F9, record Ctrl+Shift+F10. KDE's defaults take Ctrl+F7, F9,
  F10, F12 (and F1–F4); Ctrl+Alt+F-keys switch virtual terminals on Linux. Checked free on
  Windows (`RegisterHotKey` of Ctrl+Shift+F8–F12, released at once) and on KDE (the three bound
  through the portal with no conflict mark, each fired with Dolphin focused). Spec FR-013,
  research R7, data-model, contracts, plan, quickstart, tasks, product.md and roadmap updated;
  the code default, its tests, the examples, README and AGENTS.md were folded into their
  original commits by fixup and `git rebase --autosquash` (now `1e5b211`, `c8ee8aa`, `135dda1`,
  `de3028b`, `3ddc939`, `088cd85`; every hash of the branch after `main` changed, so the older
  hashes quoted above are stale; the final tree was checked identical).
  A settings file that already holds `"Ctrl+F8"` keeps it (the key is written to the file):
  Reset in the Hotkeys row gives the new default. The records in measurements/ keep Ctrl+F8:
  that is what was tested.
- Folder errors and the picked folder (agent's proposal, maintainer's go, 2026-11-23, after
  the Linux checks): a folder that is missing and cannot be made is "folder not found" whatever
  the create's error (a removed drive on Linux gave "no permission"); "no permission" only when
  the folder exists. A picked folder is kept without a trailing separator, except a root.
  Spec FR-021, FR-022 and data-model updated; `make_folder` and `picked_folder` in
  app/src/screenshot.rs (2 tests), folded into the Folders commit; checked on CachyOS
  (acceptance.md "After the folder fixes").
- The agent may run, close and test anything on the CachyOS PC over SSH (maintainer,
  2026-11-23): Linux checks are the agent's work, not the maintainer's.
- Plan (2026-11-22), spec updated: the sound and the flash start when the frame is taken, not
  when the file is written; letters, digits and Space need Ctrl, Alt or Super (Shift alone
  refused).
- Sound output (maintainer, 2026-11-22): the sound plays on the output chosen for the preview
  audio, also when the preview has no audio (research R10: mixed into the preview's stream, or an
  output-only session on the same output). Only what this sound needs is built (one sound, one
  voice); the maintainer's idea of sounds for recording, live and viewers is left to those specs,
  which reuse the routing as is. A general engine was weighed and left out (simple first).
- Plan: the frame is handed over by the video thread after its upload (R1); conversion with the
  shader's rows and `png` encoding outside GStreamer (R2, the one constitution III deviation,
  justified); clipboard through `arboard` (R4); `arboard`, `png` and `x11rb` are already in the
  build through Slint and winit (R15).

## Open questions

- For the maintainer (from the Linux checks, 2026-11-23): (a) on Linux a **removed USB drive**
  gives "sem permissão", not "pasta não encontrada": udisks removes the mount point and its
  parent `/run/media/<user>` belongs to root, so `make_folder`'s `create_dir` is refused
  (screenshot.rs). Keep it, or map a refused create of a missing folder to FolderNotFound (a
  spec change first)? (b) The portal folder dialog returns `/mnt/capxusb/` and the row shows
  and stores the trailing slash; trim it? Decided (see Decisions), built and checked.

- Probes before their code: the clipboard while another app has the focus (R4, Wayland
  data-control on KWin) and the GlobalShortcuts portal on KDE (R7). A failure updates the spec
  first.
- Defaults chosen without asking, for review: button disabled while the video area shows a
  state; no screenshot when hidden to the tray; at most 4 waiting; file name
  `CapturaX <YYYY-MM-DD HH-MM-SS>.png`; sound gain 0.5.
- 2026-11-23: T038: README "Screenshots" section (key, dock button, flash, sound, notice,
  file names, Folders and Hotkeys categories, Linux X11/Wayland/clipboard limits) and
  `--screenshot-every` in the developer options; AGENTS.md "Status"; product.md Spec column for
  F19 and F50 (the first rows filled: earlier specs left theirs empty, not back-filled);
  roadmap #5 "done" with links. Written for the branch's final state: the README describes the
  Wayland portal (T017, not built yet) and the roadmap links `measurements/latency.md`, which
  T027 creates. Check both before the pull request (T041).

- Maintainer's remark, 2026-11-23: the volume button still works on "No audio"; added to the
  roadmap's #17 UI review (dim it when there is no sound to play).

## Next step

1. T027's Linux half: with the ezcap moved to CachyOS (agent over SSH; measurement build:
   `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build --release`), 1080p60, 1080p120 and
   4K30, baseline and `--screenshot-every 1000`, ~30 s each, as latency.md's Windows part.
2. T041 (pull request; check the README and the roadmap links first, T038 note).
