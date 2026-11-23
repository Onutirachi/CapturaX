# Implementation Plan: Screenshot

**Branch**: `feat/20261121-134830-screenshot` | **Date**: 2026-11-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261121-134830-screenshot/spec.md`

## Summary

A screenshot takes the next frame the preview presents, straight from the video thread after its
upload (no copy, no GPU read-back), and hands it to one worker thread that converts it to RGB
with the shader's own conversion, encodes a PNG, writes it under a temporary name and links it to
`CapturaX <time>.png`, and puts the image on the clipboard through `arboard`. The moment the
frame is taken, the composite shader draws the white outline flash under the UI and the capture
sound plays on the output chosen for the preview audio: mixed into the preview's own output
stream when it runs, else through an output-only session on that output. The outcome becomes one notice in the existing device-notice balloon, or a system
notification when the window has no focus. The F50 framework arrives here: a `hotkeys` thread in
`crates/platform` with `RegisterHotKey` on Windows, the GlobalShortcuts portal on Wayland, X11
grabs on X11, and window-only keys where none exist; a Hotkeys settings category records keys.
The right dock appears with the screenshot button, and a Screenshot settings category holds the
folder.

## Technical Context

**Language/Version**: Rust 1.99, edition 2024

**Primary Dependencies**: as the preview audio; direct use of `arboard` 3.7 (new feature
`image-data`), `png` 0.18 and, on Linux, `x11rb` 0.13, all three already in the build through
Slint and winit (research R15); more `windows`/`windows-sys` features

**Storage**: `capturax.toml`: new `[screenshot]` and `[hotkeys]` tables
(contracts/settings-file.md); PNG files in the screenshots folder

**Testing**: `cargo test`: conversion against the shader's rows and test patterns, PNG round
trip, file naming and the no-overwrite write, the `Screenshots` state, `Hotkey` parse/display/
rules and mappings, the WAV reader and the sound's voice, the sound player's routing, settings
read rules; hotkeys, clipboard,
sound, flash and the latency check by hand (quickstart.md)

**Target Platform**: Windows 11 and Linux (KDE Plasma Wayland checked; X11 where available)

**Project Type**: desktop app (Cargo workspace)

**Performance Goals**: key to notice < 1 s at 1080p, < 2 s at 4K (SC-001); sound and flash within
one frame period plus 100 ms of the press; 0 dropped preview frames, no audio gap (SC-003)

**Constraints**: nothing heavier than moving a value on the video thread; nothing that waits on
the main thread; no OS call outside `crates/platform`; no allocation and no waited lock in the
audio `fill`; every text in the catalogs

**Scale/Scope**: one action with a key (framework for three), at most 4 screenshots in flight,
two settings categories, one dock button, six notice kinds

No NEEDS CLARIFICATION remains. Two points are settled by probes before their code (research R4
clipboard on Wayland, R7 the GlobalShortcuts portal on KDE); a failure updates the spec first.

## Constitution Check

*Constitution 1.2.0. Checked before research and again after design: passes with one justified
deviation (Complexity Tracking).*

| Principle | How the plan complies |
|---|---|
| I. Preview latency | The video thread only moves the presented `Frame` (R1); conversion, encoding, writing and copying on a worker; the flash is a shader term (R11); the sound is mixed without locks or allocation, into the preview's own stream (R10); measured: SC-003 with `--stats` and `--screenshot-every` (R16) |
| II. Simple by default | Works with no setup (Ctrl+F8, Pictures/CapturaX); one new option the maintainer asked for (folder); no sound setting; every failure after the click has a notice with its next action; no click means no screenshot (FR-008) |
| III. One native pipeline | One process; frames come from the existing session. The PNG is made outside GStreamer: **deviation**, see Complexity Tracking |
| IV. Platform layer | Hotkeys, clipboard, folder dialog, file manager, output-only audio session only in `crates/platform` (contracts/platform.md); the boundary test covers the new modules; degraded behaviors (Wayland without the portal, Linux clipboard after quit) stated in the spec |
| V. Private and safe | No network; no keyboard hook (R6), no `/dev/input` (R7); files only in the chosen folder |
| VI. Portable | Folder `"default"` resolves per system, a chosen path is kept; keys in the settings file; no new runtime library to install (portal and X11 are the desktop's) |
| VII. Spec-driven, tested | Spec approved 2026-11-22; pure logic unit tested; manual checks in the quickstart; dependencies justified (R15), none new to the build |
| Technology constraints | Rust; Slint over the app's swap chain; texts in both catalogs; icons from the existing set plus a camera and a keyboard icon |
| Workflow | English; Conventional Commits without AI trailers; the implementation is committed only after validation **and** the maintainer's final sound file (handoff) |

## Design

### Threads and data flow

```text
key (hotkeys thread) ─┐
dock button (main) ───┴─► main: Screenshots.request ──► Inbox.request_still(serial)
                                                           │
video thread: after uploading the next presented frame ◄───┘
   moves the Frame into the reply, sets flash_at ──► main (event loop proxy): taken(serial, frame)
                                                        ├─► sound player: Screenshot → preview's Feed voice, or output-only session
                                                        └─► screenshot worker: still → png → .part → link
                                                              └─► clipboard thread: set_image (≤ 2 s)
                                                        ◄── written(serial, outcome) ── notice / system notification
```

- The `Inbox` gains a request queue and `flash_at`; `clear_video` and a session change answer the
  waiting requests empty (data-model.md "Frame hand-off").
- The worker reads the folder from the settings at the time of the frame (a folder change
  applies to the next screenshot).
- Sound: the WAV is converted to an output's format when that output's format is known (once
  per output open), so `fill` only mixes; the sound player routes `play` to the preview's
  `Feed` or to its output-only session (research R10, data-model.md "Sound player").

### Startup and quit

- Start: `Hotkeys::start` with the bindings from the settings after the window exists (the
  Wayland portal needs the app id registered first; it may show the desktop's confirmation the
  first time). The clipboard and worker threads start at the first screenshot.
- Quit: drop `Hotkeys` (unregisters); the worker finishes the file being written (at most the
  waiting ones, bounded by 4) with a 3 s limit, then the app ends; on Linux the clipboard's
  content goes with the app (spec Edge Cases).

### App (`app/src/`)

- `screenshot.rs` (NEW): the `Screenshots` state (pure, data-model.md), file naming, the worker
  (convert, encode, write, copy), the outcome → notice mapping.
- `hotkeys.rs` (NEW): the bindings from the settings, the key recorder's state (recording, the
  refused combination and which action has it), window-only matching (R8), the Hotkeys rows.
- `render/mod.rs`, `render/gpu.rs`, `render/composite.wgsl`: request queue and hand-off, the
  flash uniform and shader term; `conversion` moves to `crates/capture`.
- `sounds.rs` (NEW): the sound player (routing, the output-only session's open and close).
- `sound.rs`: the `Feed` holds the sound's voice and mixes it in `fill`; sessions may have no
  input.
- `app.rs`, `ui.rs`, `notices.rs`: wiring; the device notice becomes the event notice (R13);
  `--screenshot-every` (debug).
- `cli.rs`: the debug flag (contracts/cli.md).

### Crates

- `crates/capture/src/still.rs` (NEW): `still(&Frame) -> Still`, `conversion` (moved), PNG
  encoding (`png`).
- `crates/audio/src/sound.rs` (NEW): WAV reader, the sound's preparation and voice.
- `crates/platform/src/{hotkeys.rs, clipboard.rs, files.rs}` (NEW) with `windows/` and `linux/`
  parts; `audio.rs`: `SessionRequest.input` optional (output-only session) in
  `windows/audio/stream.rs` and `linux/pipewire.rs`.
- `crates/settings/src/model.rs`: `Screenshot { folder }`, `Hotkeys { screenshot }`.

### UI (`app/ui/`)

- `components/dock.slint` and `main.slint`: the right dock with the screenshot button (camera
  icon), disabled while the video area shows a state, tooltip "Screenshot (Ctrl+F8)".
- `components/settings-panel.slint`: categories `folders` (the Screenshots folder row: path,
  Change, Open, Reset; spec #9 adds the recordings folder) and `hotkeys` (one row per action: name, key button that records, Clear, Reset; the
  Wayland line and the "changed in the system settings" variant), after Audio.
- `components/notice.slint`: event notice kinds and icons (R13).
- `texts.slint` and the catalogs: every new text in English and Portuguese.

### Investigations inside the plan

- **Clipboard probe** (R4) on Windows and CachyOS before `clipboard.rs` is final.
- **GlobalShortcuts probe** (R7) on CachyOS before the Wayland hotkeys are built on it. Either
  failing stops that part for the maintainer's decision; the spec is updated first.
- **Flash look** (R11): the curve and timings are tuned with the maintainer in the app.

## Project Structure

### Documentation (this feature)

```text
specs/20261121-134830-screenshot/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R16
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── platform.md      # additions
│   ├── settings-file.md # additions
│   └── cli.md           # additions
├── checklists/requirements.md
├── handoff.md
├── measurements/        # probes.md, latency.md, acceptance.md (during implementation)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                       # arboard (image-data), png, x11rb; windows/windows-sys features
app/sounds/screenshot.wav        # NEW: placeholder sound (R10), replaced by the maintainer's
tools/make_screenshot_sound.py   # NEW: generates the placeholder

crates/capture/src/still.rs      # NEW (R2)
crates/audio/src/sound.rs        # NEW (R10)
crates/platform/src/
├── hotkeys.rs                   # NEW: Hotkey, HotkeyEvent, Hotkeys (R6–R9, R14)
├── clipboard.rs                 # NEW (R4)
├── files.rs                     # NEW (R12)
├── audio.rs                     # SessionRequest.input optional
├── windows/{hotkeys.rs, files.rs, audio/stream.rs}
└── linux/{hotkeys/{portal.rs, x11.rs}, files.rs, pipewire.rs}
crates/platform/examples/{clipboard_probe.rs, shortcuts_probe.rs}   # probes (R4, R7)
crates/settings/src/model.rs     # [screenshot], [hotkeys]

app/src/
├── screenshot.rs                # NEW
├── hotkeys.rs                   # NEW
├── sounds.rs                    # NEW: sound player (R10)
├── render/{mod.rs, gpu.rs, composite.wgsl}
├── sound.rs, notices.rs, app.rs, ui.rs, cli.rs
app/ui/components/{dock, settings-panel, notice}.slint, main.slint, texts.slint
app/ui/assets/icons/{camera, camera_off, keyboard}.svg
app/lang/pt-BR/LC_MESSAGES/capturax.po
app/tests/platform_boundary.rs   # + new platform modules
```

**Structure Decision**: no new crate. The still conversion sits beside `Frame` in
`crates/capture`, the sound beside the bridge in `crates/audio` (both pure), the OS parts in
`crates/platform`, the decisions in the app beside `preview.rs` and `audio.rs`.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because |
|---|---|---|
| III: the PNG is converted and encoded outside GStreamer | The saved colors must match the preview's (SC-002); the shader's own conversion rows are reused on the CPU; a still image is one function plus one encoder call (research R2) | A one-shot `appsrc ! videoconvert ! pngenc` pipeline converts with other rows and chroma filtering, needs `pngenc` bundled, and builds a pipeline per press |
