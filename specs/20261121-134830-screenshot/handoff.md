# Handoff: Screenshot

**Updated**: 2026-11-22 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261121-134830-screenshot`

## State

Spec approved (maintainer, 2026-11-22). Plan, tasks (T001–T041) and `/speckit-analyze` done: no
critical issue; fixed FR-001 (next frame presented), FR-010 (300 ms when the output opens
first), FR-015 (window-only exception) and small wording. Docs committed on this branch, not
pushed. Implementation started: T001 (dependencies) and T002 (placeholder sound generator)
done and committed; the probes' code (T003, T004) is committed and waits for the maintainer's
runs.

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

## Decisions

- The F50 global hotkey framework and the screenshot key (Ctrl+F8) come in this spec, not in
  spec #9 (maintainer, 2026-11-21). Roadmap updated.
- The screenshots folder is a setting, default Pictures/CapturaX (maintainer, 2026-11-21).
- FR-010: a capture sound on the preview's output; no thumbnail (maintainer, 2026-11-21).
  **The implementation ships a placeholder sound (`app/sounds/screenshot.wav`, from
  `tools/make_screenshot_sound.py`); after the implementation is validated and before it is committed,
  the maintainer sends the final sound file and it replaces the placeholder.** Do not commit the
  implementation without it.
- FR-010a: an edge flash: white at the video area's edges fading to transparent about 20 %
  inward, fast fade in and out (maintainer, 2026-11-22). Curve and timings tuned with the
  maintainer in the app (research R11).
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
2. Agent: T005 (still conversion), T006 (settings), T007 (hotkey parsing), which do not depend on
   the probes. T013 waits for T003, T017 for T004.

The placeholder sound stays untracked and the code that embeds it waits for the final sound
before its commit (tasks.md "Commits"; maintainer, 2026-11-22).
