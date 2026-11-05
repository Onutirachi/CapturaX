# Data Model: App Shell

Entities from the spec's Key Entities, with fields, rules and state changes. The file format of
Settings is in [contracts/settings-file.md](contracts/settings-file.md).

## Settings

Crate `settings`. The user's choices, grouped in categories; each later spec adds a category.

| Category | Field | Type | Default | Rule |
|---|---|---|---|---|
| (root) | `version` | integer | 1 | Format version; a file with a higher version is read leniently and its unknown values kept |
| `general` | `language` | `auto` \| language code | `auto` | Unknown code → `auto` (spec edge case) |
| `general` | `theme` | `auto` \| `dark` \| `light` | `auto` | Unknown → `auto` |
| `general` | `start_fullscreen` | bool | false | FR-012a |
| `general` | `keep_in_tray` | bool | false | FR-014a; ignored (treated as false) where no tray exists |
| `window` | `width`, `height` | logical px | 1280 × 720 | At least 640 × 360 |
| `window` | `x`, `y` | physical px, desktop coordinates | none (centered on primary) | |
| `window` | `maximized` | bool | false | |
| `window` | `monitor` | string (monitor name) | none | Used to prefer the same monitor when it is connected |

- **Start with the system** is not stored: its state is the OS entry (platform `autostart`).
- **Unknown values**: the loaded TOML table is kept in memory; known fields are written into it on
  save, so unknown keys and categories survive.
- **Saving**: every change marks the settings dirty; the saver writes 300 ms after the last change
  and once more on quit (research R6). When there was no file, or it was damaged and set aside,
  the current values are written at start, so the user finds a file next to the app.

## Settings location

Crate `settings`, resolved once at startup from probe results given by `platform`.

| Variant | Settings file | Log | Notice |
|---|---|---|---|
| `AppFolder(path)` | `<app folder>/capturax.toml` | `<app folder>/capturax.log` | none |
| `UserFolder(path)` | `<user config>/capturax.toml` | same folder | `settings-fallback` naming the folder (FR-027) |
| `Memory` | none | none (stderr only) | `settings-not-kept` (FR-028) |

- `read_from`: optional path of a readable file in a read-only application folder, loaded when
  the per-user folder has no newer file (spec edge case; research R6).
- App folder = folder of the executable, or of `$APPIMAGE` on Linux.

```text
start ─► app folder writable? ── yes ─► AppFolder
              │ no
              ▼
         user folder writable? ── yes ─► UserFolder (+ notice)
              │ no
              ▼
            Memory (+ notice)
```

## Load result

| Outcome | Settings used | Notice |
|---|---|---|
| No file | defaults | none |
| Valid TOML | file values; invalid values → defaults | none |
| Not TOML / not UTF-8 / unreadable | defaults; damaged file renamed `capturax.toml.damaged-<unix s>` | `settings-damaged` |

## Window placement

`app/src/placement.rs`, pure function with unit tests.

- Input: saved `window` category; connected monitors (name, position, size, scale); primary
  monitor.
- Output: position, size, maximized, monitor.
- Rules: prefer the saved monitor if connected, else the primary; reduce the size to fit the
  monitor's work area (never below the minimum window size, 640 × 360 logical at that monitor's
  scale); move the window so it is fully visible; without a saved position, or when the saved
  monitor is not connected, center on the chosen monitor (FR-012, edge cases). The size is
  returned in physical pixels at the chosen monitor's scale. No monitor known → the OS places
  the window. winit does not report work areas: `platform::work_area` gives it on Windows, and
  the caller uses the monitor's bounds where it gives none (Linux).
- Saved when the window moves, resizes or changes maximized (debounced as any setting); not while
  fullscreen (the window state to return to is kept) nor minimized. While maximized only
  `maximized` changes, so the size and position to restore to are kept; maximized on another
  monitor saves that monitor and clears the position (the restored window opens centered there).
  Where the OS gives no window position (Wayland), the position is not saved.

## Window mode (state)

```text
           F11 / control / --fullscreen / start_fullscreen
 Windowed ────────────────────────────────────────────► Fullscreen
 (normal or maximized)  ◄──── F11 / control / Esc ────   (borderless, title bar hidden)
     │  ▲
close│  │ tray click / second start / Open
 (keep_in_tray on)
     ▼  │
   Hidden (tray icon; video not presented)
```

- Esc closes an open menu first; with none open, leaves fullscreen; never quits (FR-007).
- Close with `keep_in_tray` off or no tray → quit: stop the test video, stop the video thread,
  flush settings, destroy the window (FR-014).
- Started with `--autostart`, `keep_in_tray` on and a tray available → Hidden, whatever
  `start_fullscreen` says (FR-014b).

## Startup options

From the command line (contracts/cli.md), not saved.

| Field | Values | Effect |
|---|---|---|
| `start_mode` | none \| `fullscreen` \| `windowed` | Overrides `start_fullscreen` for this start |
| `autostart` | bool | Started by the system's autostart entry |
| `test_video` | none \| `WxH@fps` | Plays the test video (FR-004) |
| `stats` | bool | Prints presentation statistics (FR-005) |
| `backend`, `present` | optional | Override the presentation choice (research R4) |

## Language

`app/src/i18n.rs`.

- `LANGUAGES`: list of `{ code, native_name }`: `en` "English", `pt-BR` "Português (Brasil)".
- `resolve(setting, system_locale) -> code`: `auto` → `pt-BR` if the system locale starts with
  `pt`, else `en`; a code not in `LANGUAGES` → as `auto`.
- Locale catalog: `app/lang/<code>/LC_MESSAGES/capturax.po`, msgid = English source text, no
  context; English has no catalog (source).

## Theme

`app/ui/theme.slint`, global `Theme`.

- `dark: bool` set from Rust; every token is a property that picks its Dark or Light value.
- Token groups: colors (four neutral tones, text levels, accent `#FF7304`, `accent-ink`,
  `accent-text`, focus ring, panel translucency), type (Inter sizes and weights), spacing, radius,
  shadow images (generated at the window's scale, research R8).
- `resolve(setting, system_theme) -> dark`: `auto` follows the system, unknown system → dark.

## State message and notice

Slint components with Rust-side identifiers (texts come from `Texts`, research R7).

| Entity | Fields | Rules |
|---|---|---|
| State message | `icon`, `title`, `detail?`, `primary?`, `secondary?` (action id + label) | 0, 1 or 2 actions; fills the video area; used for `no-source` in this spec (FR-033, FR-035) |
| Notice | `id`, `text` (may name a folder), dismissible | Non-blocking, top of the video area; each id shown at most once per run (FR-034) |

Notice ids in this spec: `settings-fallback(folder)`, `settings-damaged`, `settings-not-kept`.
System message ids (outside the window): `renderer-unavailable`, `already-open`.

## Presentation statistics

`app/src/stats.rs`, one record per second (format in [contracts/stats.md](contracts/stats.md)).

| Field | Meaning |
|---|---|
| `received` | Frames put in the inbox |
| `presented` | Presents with a new video frame |
| `replaced` | Frames replaced in the inbox before being presented |
| `ready_to_present_ms` | Median and max of frame ready → present returned |
| `max_gap_ms` | Longest time between two presents with a new frame |
| `ui_images`, `ui_ms` | UI images handed over; median and max of draw + copy |
| `moves` | Window move events (shows when a reading was taken during a move) |
| `presentation` | Backend and present mode in use (header line) |
