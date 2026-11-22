# Handoff: Screenshot

**Updated**: 2026-11-22 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261121-134830-screenshot`

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
step).

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

## Decisions

- The F50 global hotkey framework and the screenshot key (Ctrl+F8) come in this spec, not in
  spec #9 (maintainer, 2026-11-21). Roadmap updated.
- The screenshots folder is a setting, default Pictures/CapturaX (maintainer, 2026-11-21).
- FR-010: a capture sound on the preview's output; no thumbnail (maintainer, 2026-11-21).
  **The implementation ships a placeholder sound (`app/sounds/screenshot.wav`, from
  `tools/make_screenshot_sound.py`); after the implementation is validated and before it is committed,
  the maintainer sends the final sound file and it replaces the placeholder.** Do not commit the
  implementation without it.
- FR-010a: an **outline flash** (maintainer, 2026-11-22): a thin white line on the video's
  rounded edge with a faint inner glow, 40 ms in, 400 ms out. Chosen from the mockup
  https://claude.ai/artifact/XknXi4EvntqmHHZU4axncC (option D) after the first edge glow was
  built and seen in the app and judged ugly (research R11).
- FR-020: the folder row goes in a new Screenshot category, Hotkeys after it (agent's choice;
  the maintainer reviews it in the app).
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

- Probes before their code: the clipboard while another app has the focus (R4, Wayland
  data-control on KWin) and the GlobalShortcuts portal on KDE (R7). A failure updates the spec
  first.
- Defaults chosen without asking, for review: button disabled while the video area shows a
  state; no screenshot when hidden to the tray; at most 4 waiting; file name
  `CapturaX <YYYY-MM-DD HH-MM-SS>.png`; sound gain 0.5.

## Next step

1. Maintainer: run the probes as quickstart "Probes" says and fill measurements/probes.md:
   clipboard pastes on Windows (GIMP/Krita, Paint, Discord) and on CachyOS (Wayland, X11 if
   installed); `shortcuts_probe` on CachyOS. On a new Linux checkout run
   `python tools/make_screenshot_sound.py` and add `app/sounds/screenshot.wav` to
   `.git/info/exclude` there too.
2. Done on Windows (2026-11-22): device notices (ezcap disabled/enabled in Device Manager:
   balloon and system notification, both logged), notice preview approved, `files` show/open
   OK, hand-off OK, outline flash approved. Left for CachyOS: the `files` example (show/open:
   Dolphin or the portal), `utc_offset` (a file name in local time once T014 writes files) and
   **Open** on a C920 notice (never exercised: needs a device that does not open by itself).
3. Done on Windows (2026-11-22): `hotkeys` example: both keys bound, nothing during the
   suspend, one `Pressed` per press with another app focused. `audio_session --output-only` on
   `Voicemeeter VAIO3 Input` and on the default output: `Opened` (output only, shared, 48 kHz
   stereo) in 18–19 ms, no event after the input reopen, ~48000 frames/s, `Closed` 3–4 ms after
   the close; the tone heard on both (maintainer). Not yet seen: a held key giving one `Pressed`,
   and a second copy getting `Bound { shown: None }` (check with T018 in the app).
   CachyOS: `cargo build` first (the PipeWire change was not compiled on Windows), then the same
   `audio_session --output-only`, and `hotkeys` in an X11 session if one is installed (on Wayland
   it prints `WindowOnly` until T017).
4. Agent: T023 (sound player) is next and can be written now; it embeds the placeholder, so it
   stays uncommitted until the final sound (Decisions). T013 waits for T003, T017 for T004,
   T018 for T017 (or start it with X11/Windows and WindowOnly and add the portal later).

The placeholder sound stays untracked and the code that embeds it waits for the final sound
before its commit (tasks.md "Commits"; maintainer, 2026-11-22).
