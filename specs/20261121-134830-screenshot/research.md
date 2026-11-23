# Research: Screenshot

Decisions behind [plan.md](plan.md). Each item: decision, rationale, alternatives. Items marked
**probe** are checked on real hardware by the first tasks, before the code that depends on them.

## R1. Where the frame comes from

**Decision**: the video thread hands the frame over. A request goes into the renderer's `Inbox`
(`request_still(reply)`); the video thread fulfils it with the **next frame it presents**, after
that frame's planes are uploaded to the GPU: instead of dropping the `Frame` (its buffers going
back to the pool), it moves it into the reply. While the window is not shown (minimized; frames
arrive but are not presented) it fulfils it with the next frame that arrives. A request not
fulfilled within 500 ms (no frame comes) is answered empty (no notice, FR-008). Requests wait in
order; one frame fulfils one request.

**Rationale**: the frame already holds the device's own planes in memory with the matrix and
range the shader converts with (`capture::Frame`), so nothing is read back from the GPU and
nothing is copied on the video thread: moving the `Frame` costs nothing there. The frame is the
one on screen within one frame period of the press (≤ 17 ms at 60 fps, ≤ 33 ms at 30 fps). The
plane pool allocates a new buffer for the source the next time it is empty; that happens on the
appsink thread, at most 4 times in a burst (FR-007), and is measured by SC-003.

**Alternatives**: copy the frame in `send_frame` (on the appsink thread: 12 MB at 4K NV12, a few
ms added to that frame's path, against principle I); read the presented texture back from the GPU
(a copy and a map on the video thread's queue, and the image would include the letterbox and
scale); keep the last presented frame in a slot (holds one frame's buffers out of the pool all
the time, and two presses in one frame period need the next frame anyway).

## R2. Conversion and PNG

**Decision**: a pure function in `crates/capture` (`still.rs`) converts a `Frame` to 8-bit RGB
with the same rows as the shader (`conversion(matrix, range)`, moved from `app/src/render/gpu.rs`
to `crates/capture` so both use one copy) and the same chroma placement (bilinear between chroma
samples for NV12 and I420, the shader's hand filter for YUY2/UYVY; BGRx/RGBA copied). The RGB
image is encoded with the `png` crate (0.18), `Compression::Fast`, adaptive filter, sRGB chunk,
no alpha. All of it runs on the screenshot worker thread (R5).

**Rationale**: the saved pixels must match the preview's (SC-002, ±1 per channel): the same
conversion code guarantees it, and it is unit tested with the test patterns the shader tests use.
A still image is not a media path: one function and one encoder call. Fast compression keeps a
4K image under about 0.5 s on the test PCs (SC-001 allows 2 s); files are about 20–40 % larger
than at default compression.

**Constitution III**: media is built on GStreamer. This is a deviation, justified in the plan's
Complexity Tracking: a GStreamer pipeline (`appsrc ! videoconvert ! pngenc ! appsink`) per
screenshot would convert with `videoconvert`'s own rows and chroma filter, not the shader's
(colors could differ from the preview), needs `pngenc` bundled (libpng from plugins-good), and
needs a pipeline built, run and torn down per press. No muxer, protocol or codec of a media
stream is written.

**Alternatives**: GStreamer as above; the `image` crate (larger API, same `png` underneath);
`Compression::Default` (≈2× slower at 4K, for ~25 % smaller files: not worth the wait while
playing).

## R3. Writing the file without a partial name

**Decision**: the worker writes `<folder>/.CapturaX <time>.png.part`, flushes and syncs it, then
links it to the final name with `std::fs::hard_link` (fails if the name exists: the next number
is tried, FR-005) and removes the `.part`. Where the file system has no hard links (FAT32,
exFAT: the call fails with an unsupported error) it checks the name is free and renames. A `.part`
left by a crash is removed at the next screenshot into that folder if older than 1 minute.

**Rationale**: no program ever sees a partial PNG under the final name (FR-006), and an existing
file is never replaced (FR-005): `std::fs::rename` replaces on Windows. The worker is the only
writer, so the check-then-rename fallback has no race inside the app.

**Alternatives**: reserve the name with `create_new` and write into it (a 0-byte or partial file
is visible under the final name while writing); `MoveFileExW` without replace (Windows only; the
platform layer would need a call for a portable rule).

## R4. Clipboard **(probe)**

**Decision**: `arboard` 3.7 with its `image-data` feature, in `crates/platform` (`clipboard.rs`),
one `Clipboard` kept for the app's lifetime on its own thread (`clipboard`), which takes the RGBA
image from the worker. On Windows arboard writes `CF_DIBV5` and the registered `PNG` format; on
Linux it offers `image/png`, under X11 by owning the selection, under Wayland through the
data-control protocol (`ext-data-control-v1` or `zwlr-data-control-v1`), which works while
another app has the focus. Linux keeps the image pasteable while the app runs (Edge Cases).

**Rationale**: arboard is already in the build (Slint's winit backend uses it for copy and paste,
with `wayland-data-control` on), so only the `image-data` feature is new; it handles the X11
selection protocol and the Wayland data-control protocol, which would be several hundred lines of
our own. MIT/Apache-2.0, maintained by 1Password, 3.x stable.

**Probe** (first Linux task, on CachyOS KDE Plasma Wayland and an X11 session if one is
installed; on Windows too): set a 3840×2160 image from a small example while another app has the
focus, paste in GIMP, Krita, Dolphin and Discord in the browser; time the set call. Pass: pastes
everywhere, set under 500 ms. If KWin offers no data-control protocol, the fallback is the
standard `wl_data_device` selection, which needs CapturaX's window to have the focus: the
Wayland row of FR-024 then gains "copied only while CapturaX has the focus", and the spec is
updated before the code.

**Alternatives**: our own code per platform (Win32 clipboard is short, but the X11 INCR transfer
for a 20 MB image and the Wayland protocols are not); `wl-clipboard` command-line tools (not
bundled, a subprocess per copy).

## R5. Threads of a screenshot

**Decision**: the main thread decides (the app's `Screenshots` state, pure): it refuses, silently
(FR-008), when the preview state is not an image or when 4 are already waiting, and otherwise sends the request to the `Inbox` and counts it.
When the frame comes back (through the event loop's proxy), the main thread starts the sound
(R10), the video thread has already started the flash (R10), and the frame goes to one worker
thread (`screenshot`, started at the first screenshot, kept): convert (R2) → encode → write
(R3) → hand the RGBA to the clipboard thread (R4) and wait for its result (at most 2 s) → post
the outcome. The outcome makes the notice (FR-008).

**Rationale**: the video thread only moves a value; the main thread never waits; one worker
writes the files in order (FR-005, R3); the RGBA buffer is shared (`Arc`) between the PNG
encoder and the clipboard, made once.

**Alternatives**: a thread per screenshot (files written out of order; more memory at once in a
burst); converting on the main thread (blocks the UI for tens of ms at 4K).

## R6. Hotkeys on Windows

**Decision**: a `hotkeys` thread with a message loop calls `RegisterHotKey(NULL, id, MOD_* |
MOD_NOREPEAT, vk)` for each binding; `WM_HOTKEY` reports the action. Changes and the suspend
during key recording (FR-017) are posted to the thread (`PostThreadMessageW`), which unregisters
and registers. A refused registration (`ERROR_HOTKEY_ALREADY_REGISTERED`, a reserved Win key) is
logged once per key (FR-019).

**Rationale**: the documented way to get a key while another app has the focus; `MOD_NOREPEAT`
gives one event per press (FR-014); no hook on every key press of the system, and no focus
change. A registered key reaches CapturaX also while its own window has the focus, so no
in-window handler is needed on Windows.

**Alternatives**: a low-level keyboard hook (`WH_KEYBOARD_LL`: sees every key of the system,
which anti-cheat software and antivirus flag, and adds work to every key press); the
`global-hotkey` crate (the same calls, plus its own event loop types; no Wayland).

## R7. Hotkeys on Linux under Wayland **(probe)**

**Decision**: the XDG desktop portal's `org.freedesktop.portal.GlobalShortcuts`, over `zbus`
(already used for notifications and the theme), on the `hotkeys` thread: `CreateSession`, then
`BindShortcuts` with one shortcut per action (`screenshot`, description "Screenshot",
`preferred_trigger` such as `CTRL+F8`), `Activated` reports the action. Before it, the app
registers its id with `org.freedesktop.host.portal.Registry.Register("capturax")` where the
portal offers it (an unsandboxed app has no id otherwise). The trigger the desktop actually
bound comes back in the `BindShortcuts` response and in `ShortcutsChanged` (its
`trigger_description`); the Hotkeys row shows it. Changing the key calls `ConfigureShortcuts`
(portal version 2) when offered, which opens the desktop's own dialog; otherwise the row says
the key is changed in the system settings. Without the portal interface the key works only in
the window (R8) and the row says so.

**Rationale**: the only way for a Wayland client to get keys while another app has the focus
(F50 names it). KDE Plasma implements it (the maintainer's CachyOS); GNOME from 48.

**Probe** (first Linux task, on CachyOS): a small example binds `CTRL+F8`, prints `Activated`
with another app focused, with a fullscreen game, after a restart of the example (is the binding
kept or asked again?), and what `ConfigureShortcuts` shows. Pass: the key arrives in all three.
What KDE asks the user the first time is recorded in the quickstart.

**Alternatives**: reading `/dev/input` (needs the `input` group: elevation, against principle V);
X11 grabs through XWayland (only while an XWayland window has the focus).

## R8. Hotkeys in the window (fallback)

**Decision**: when the platform reports that global keys are not available (Wayland without the
portal), the app matches winit's key events against the bindings while the window has the focus.
On Windows, X11 and the Wayland portal the app does not match key events itself (the global
registration already delivers them; matching both would take two screenshots).

## R9. Hotkeys on Linux under X11

**Decision**: `x11rb` (0.13, already in the build through winit and arboard) on the `hotkeys`
thread with its own connection: `GrabKey` on the root window for each binding, four times (with
and without Num Lock and Caps Lock), keysym to keycode from `GetKeyboardMapping`. Auto-repeat:
`XkbPerClientFlags` detectable auto-repeat, so a held key gives one press and one release; a
press is reported only after a release (FR-014).

**Rationale**: the standard X11 global key, no extra library.

## R10. The capture sound

**Decision**: the sound always plays on the output chosen for the preview audio, resolved as the
preview audio resolves it (`OutputChoice`, the missing-output rule). Only what this sound needs
is built (maintainer, 2026-11-22: simple, working and organized; a sound engine with several
voices and a bank of sounds for events no spec has yet was considered and left out).

- **The sound** (`crates/audio` `sound.rs`, pure): one PCM WAV, `app/sounds/screenshot.wav`,
  embedded with `include_bytes!`, read by a small WAV reader (16-, 24- and 32-bit integer and
  32-bit float, any rate and channels) and converted to the output's format with the bridge's
  `interpolate` and `ChannelMap`, off the audio thread, when that output's format becomes known.
  A `Sound` enum names it (`Screenshot`, its only value): a later sound is one more value and one
  more file. One voice: `play()` is a lock-free request, `mix(out)` adds the sound from its
  position and clamps; a new `play()` restarts it (a burst never layers into a longer sound).
  A fixed gain (0.5, about -6 dB, retuned when the final sound comes), not the preview volume nor
  its mute (FR-010). `mix` takes no lock it waits on and allocates nothing.
- **Where it plays** (app `sounds.rs`), the part any later sound reuses as is:
  - the preview's session has its output open: the `Feed`'s `fill` calls `mix` after the
    bridge's samples. Shared and exclusive outputs both pass through `fill`, so an exclusive
    output needs nothing more, and no second stream competes with the preview's;
  - otherwise (No audio, no input paired, the input missing, no device open): an **output-only
    session** on the resolved output: the same platform session with no input side
    (contracts/platform.md), opened on the first play, closed 3 s after the sound ends, shared
    mode. When the preview's session opens, it takes over and the output-only session closes. A
    play asked while that session opens waits for its `Opened` (at most 300 ms, FR-010).

**Placeholder**: a 120 ms two-tone click (1.8 kHz then 1.2 kHz with fast decays, 48 kHz mono
16-bit), generated by `tools/make_screenshot_sound.py` (written for this spec, no third-party audio) and
committed. The maintainer's own WAV replaced it (2026-11-22): 44.1 kHz stereo 16-bit, made with
an instrument plugin, part of CapturaX under its license; later sounds come from the same plugin,
timing and pattern. CC0 candidates (OpenGameArt shutters, Kenney Interface Sounds) were heard
and set aside for that reason. Any PCM WAV the reader accepts works without code changes.

**Rationale**: the player hears the sound on the headset the preview plays on, even when the
preview has no audio; reusing the preview's platform session (WASAPI, PipeWire) adds no playback
path to maintain, only an optional input. Several voices and a bank are added by the first spec
that needs two sounds at once.

**Alternatives**: the system's sound API (`PlaySound`, a desktop sound theme): plays on the
default output, not the chosen one, and never into an exclusive output; a second stream on the
same output beside the preview's (impossible when the preview holds it exclusive, and a second
WASAPI client adds the mixer's delay); a GStreamer one-shot pipeline (the spike's PipeWire
elements hang on stop, constitution 1.2.0); a general event-sound engine now (code and tests for
sounds no spec has).

## R11. The outline flash

**Changed (maintainer, 2026-11-22)**: the edge glow below was built and seen in the app, judged
ugly, and replaced after comparing eight options in a mockup (edge glow, soft glow, camera flash,
outline, press, shutter blink, viewfinder corners, none) by the **outline**: a white line 3
logical px wide on the video's rounded edge plus an inner glow (Gaussian, deviation 9 logical px,
35 % at its peak), opacity rising in 40 ms (ease-out) and falling in 400 ms (ease-in). The shader
gets the opacity, the line width and the glow's deviation in physical pixels (the scene's display
scale); the rest of this section (who starts it, presents, cost) holds. The edge glow's original
decision follows for the record.

**Decision**: drawn by the composite shader over the video and under the UI layer. `Inbox`
starts it (`flash_at = now`) when the video thread hands a frame to a screenshot (R1); the
video thread computes the flash's opacity for each presented frame from the time since then
(60 ms ease-out rise, 250 ms ease-in fall) and passes it to the shader with the scene's uniform.
The shader adds white over the letterboxed video rectangle with weight
`opacity × max(edge_x, edge_y)`, where `edge_x = (1 − dx / (0.2 × width))²` clamped to 0..1 for
the distance `dx` to the nearest left or right edge (and the same for y with the height), inside
the video's rounded corners. Presents happen with each video frame, so the animation runs at the
source's rate (9 steps at 30 fps); while the flash runs, a UI-only present also advances it.

**Rationale**: no Slint redraw of a full-window gradient (the software renderer would redraw and
upload a large image for 300 ms); the flash cannot reach the saved image (it is not in the
`Frame`) nor delay a frame (a few ALU operations per pixel). The squared falloff looks like a
glow rather than a band; the exact curve and timings are tuned with the maintainer in the app.

**Alternatives**: a Slint rectangle with a gradient (software rendering cost, above); a texture
(an asset for a formula).

## R12. Folder, picker and file manager

**Decision** (`crates/platform` `files.rs`):

- Default folder: `dirs::picture_dir()` (Windows: the Pictures known folder, moved or
  redirected included; Linux: `XDG_PICTURES_DIR`) plus `CapturaX`; when the system reports none,
  the home folder plus `Pictures/CapturaX`.
- Pick a folder: Windows `IFileOpenDialog` with `FOS_PICKFOLDERS`, on its own STA thread, owned
  by the main window; Linux the portal's `org.freedesktop.portal.FileChooser.OpenFile` with
  `directory: true` (its response comes as the `Request`'s `Response` signal). The answer comes
  back to the event loop; the UI stays responsive meanwhile. Without the portal, **Change** shows
  "No folder picker on this desktop" and the folder stays.
- Show a file: Windows `SHOpenFolderAndSelectItems`; Linux `org.freedesktop.FileManager1.ShowItems`
  (Dolphin, Nautilus), else the portal's `OpenURI.OpenDirectory` with the file, else the folder
  with `OpenURI.OpenURI`.
- Open a folder: Windows `ShellExecuteW("open")`; Linux `FileManager1.ShowFolders`, else
  `OpenURI.OpenURI` with the folder's `file://` URI.

**Rationale**: each is the system's own dialog or file manager, no new dependency (`windows`
features `Win32_UI_Shell_Common` and `Win32_System_Com` are added; `zbus` is in the build).

**Alternatives**: the `rfd` crate (pulls an async runtime on Linux for the portal; a GTK dialog
otherwise).

## R13. Notices

**Decision**: the device notice balloon (device preview FR-027) becomes the app's "event notice":
the same balloon and rules (one at a time, latest replaces, system notification when the window
has no focus), with a kind per message. Screenshot kinds: `saved`, `saved-not-copied`,
`copied-not-saved`, `failed` (`nothing` and `busy` were dropped: a screenshot that takes no
frame has no notice, FR-008, maintainer 2026-11-22), each with its icon (camera; camera with a slash
for the failures) and action (**Show**, **Settings** or none). The system notification's tag
carries the action: `show:<path>` or `settings:screenshot`.

**Rationale**: FR-008 asks for exactly these rules; one balloon avoids two notices racing for
the same corner.

## R14. Keys

**Decision**: a `Hotkey` value (`crates/platform` `hotkeys.rs`, pure part): modifiers (Ctrl, Alt,
Shift, Super) and one key from a closed list: A–Z, 0–9, F1–F24, Space, Insert, Delete, Home,
End, Page Up, Page Down, the four arrows, Pause, Print Screen. Text form for the settings file
and the UI: `Ctrl+Shift+F8` (modifiers in that order; `Win` is shown for Super on Windows, the
file always says `Super`). Rules of FR-016 in `Hotkey::allowed`. Mappings: Windows virtual-key
codes; X11 keysyms; the portal's trigger strings (`CTRL+SHIFT+F8`, `LOGO` for Super). The key
recorder reads the window's winit key events before Slint sees them and maps them with the app's
`from_window` (the key without modifiers, a digit also from the physical top row), the same
mapping as window-only matching (R8). Slint's key text was set aside (T030, 2026-11-23): it has
the modifiers applied (Ctrl+Shift+1 gives `!`, AZERTY's top row gives `&`), so digits could not
be read.

**Rationale**: a closed list maps the same on every platform and keeps keys that change with the
keyboard layout (punctuation) out.

## R15. Dependencies

| Crate | Where | Why | License | In the build already |
|---|---|---|---|---|
| `arboard` 3.7, feature `image-data` | platform | clipboard image (R4) | MIT/Apache-2.0 | yes (Slint), new feature |
| `png` 0.18 | capture | encoding (R2) | MIT/Apache-2.0 | yes (Slint) |
| `x11rb` 0.13 | platform, Linux | X11 keys (R9) | MIT/Apache-2.0 | yes (winit, arboard) |
| `windows` features `Win32_UI_Shell_Common`, `Win32_UI_Input_KeyboardAndMouse` | platform, Windows | R6, R12 | MIT/Apache-2.0 | crate yes |

All maintained, all GPL-3.0 compatible; `cargo deny` (deny.toml) covers them.

## R16. Measurement

**Decision**: SC-003 with the device preview's `--stats` lines (frames received, presented,
replaced, dropped; frame-time spread) over 60 s with a screenshot every second (a debug-only
`--screenshot-every <ms>` flag, contracts/cli.md) and a 10-press burst, against 60 s without, at
1080p60, 1080p120 and 4K30 on both systems; the audio stats line shows no gap. SC-001: the log
line of each screenshot carries the times of its steps (frame taken, converted, encoded,
written, copied, notice).
