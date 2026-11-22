# Data Model: Screenshot

## Screenshot request (app, `Screenshots`, pure)

| Field | Meaning |
|---|---|
| `waiting` | requests sent or being written, 0–4 (FR-007) |
| `serial` | number of the next request, carried by its frame and its outcome |

Rules:

1. `request(preview_shows_image)`: not an image → outcome `Nothing`; `waiting == 4` → outcome
   `Busy`; else `waiting += 1` and the command `Take(serial)` (send to the `Inbox`).
2. `taken(serial, frame)`: commands `PlaySound(Screenshot)`, `Write(serial, frame, folder)`.
3. `not_taken(serial)` (500 ms with no frame, or the session closed): `waiting -= 1`, outcome
   `Nothing`.
4. `written(serial, result)`: `waiting -= 1`, outcome from the result (below).

## Frame hand-off (renderer `Inbox`)

| Field | Meaning |
|---|---|
| `stills` | queue of `(serial, reply)` waiting for a frame, in order |
| `flash_at` | when the latest flash started, or none |

The video thread, after uploading a presented frame (or on a frame's arrival while not shown),
pops one request, moves the frame into its reply and sets `flash_at` (only while shown). A
request older than 500 ms is answered empty. `clear_video` answers all waiting requests empty.

## Still image (`crates/capture` `still.rs`)

`Still { width, height, rgb: Vec<u8> }` from `still(&Frame) -> Still`: rows of `width × 3`
bytes, sRGB, the frame's matrix and range applied as in the shader (research R2). The clipboard
takes RGBA (`Still::rgba()`).

## Outcome

| Saved | Copied | Kind | Text (FR-008) | Action |
|---|---|---|---|---|
| yes | yes | `saved` | Screenshot saved | Show |
| yes | no | `saved-not-copied` | Screenshot saved, not copied | Show |
| no | yes | `copied-not-saved` | Couldn't save the screenshot: <reason>. Copied to the clipboard | Settings |
| no | no | `failed` | Couldn't take the screenshot: <reason> | — |
| — | — | `nothing` | Nothing to capture | — |
| — | — | `busy` | Too many screenshots at once | — |

Reasons (a few words, from the error kind): `folder not found`, `no permission`, `disk full`,
`write error`; for `failed` also `image error`.

## File name (pure)

`CapturaX YYYY-MM-DD HH-MM-SS.png` from the local time the frame was taken (UTC plus
`platform::utc_offset`, the date by the log's `civil_from_days`); `n ≥ 2` gives
`CapturaX YYYY-MM-DD HH-MM-SS (n).png`. The temporary file is `.<name>.part` in the same folder.

## Hotkey (`crates/platform` `hotkeys.rs`, pure part)

| Field | Meaning |
|---|---|
| `modifiers` | set of Ctrl, Alt, Shift, Super |
| `key` | one of the closed list (research R14) |

- Text: `Ctrl+Alt+Shift+Super+<Key>` (only the modifiers present, in that order); key names
  `A`–`Z`, `0`–`9`, `F1`–`F24`, `Space`, `Insert`, `Delete`, `Home`, `End`, `PageUp`,
  `PageDown`, `Up`, `Down`, `Left`, `Right`, `Pause`, `PrintScreen`. Parsing ignores case and
  spaces; an unknown text is invalid.
- `allowed()`: F1–F24 alone; any other key with a modifier; A–Z, 0–9 and Space need Ctrl, Alt
  or Super.
- Shown to the user with the platform's names (`Win` on Windows, `Super` on Linux) and the
  locale's key names where they differ (`PgUp`…), from the catalogs.

## Hotkey binding (settings)

| Action | Default | Spec |
|---|---|---|
| `screenshot` | `Ctrl+F8` | this |
| `record` | `Ctrl+F10` | #9 |
| `live` | `Ctrl+F9` | #10 |

Value: a `Hotkey`, or none (cleared). Two actions never share a key (FR-016); a settings file
that has two equal keys keeps the first action's and clears the other's (logged).

## Hotkey state (platform → app)

`HotkeyEvent::{Pressed(action), Bound { action, shown: String }, Unavailable { reason }}`:

- `Pressed`: the action's key was pressed (once per press).
- `Bound`: the key the system uses for the action, as text to show (on Wayland the desktop's
  `trigger_description`, which may differ from the setting).
- `Unavailable { WindowOnly }`: no global keys (Wayland without the portal): the app matches key
  events in the window (research R8) and the Hotkeys category shows the line.

## Screenshots folder (settings)

`"default"` or an absolute path. The folder used: the path, or the system's Pictures folder plus
`CapturaX` (research R12). An empty or relative path reads as `"default"` (logged).

## Sound (`crates/audio` `sound.rs`)

| Part | Meaning |
|---|---|
| `Sound` | the sound's name: `Screenshot` (only value), with its WAV |
| `Prepared` | the WAV converted to one output format, made off the audio thread when an output's format becomes known |
| `Voice` | position in the prepared samples and a lock-free restart request; `play()` restarts, `mix(out)` adds and clamps; past the end it is silent |
| `gain` | fixed, 0.5; not the preview volume nor its mute |

## Sound player (app, `sounds.rs`)

| State | Where `play` goes |
|---|---|
| The preview's session has its output open | that session's `Feed` voice |
| Otherwise | an output-only session on the resolved output, opened on the first play and closed 3 s after the sound ends |

The output is resolved as the preview audio's (`OutputChoice` with the missing-output rule); the
exclusive flag is not used for the output-only session (shared). A play asked while the
output-only session opens waits for its `Opened` (at most 300 ms, FR-010, then dropped and logged).

Refinements made with T023 (2026-11-22):

- While the preview's session or its output side is opening (open, queued, or an output reopen),
  a play waits for that output's `Opened` (at most 300 ms) instead of opening an output-only
  session, so two streams never compete for a device the preview may ask exclusive. An output
  side error with no reopen sends the next plays to an output-only session.
- The preview opening closes the output-only session, and the new preview session waits for its
  `Closed` as it waits for an older preview session (at most 1 s), so an exclusive output is free.
- The voice is prepared for the format of the output's last `fill` (on Linux it can differ from
  `Opened`'s device format), else `Opened`'s; prepared on the main thread at the play, kept for
  the next one. An output-only session that fails drops the play waiting for it.
- Output-only session events reach the app as `UserEvent::SoundSession`, numbered on their own;
  the output choice changed since that session opened closes it and opens one on the new output.
