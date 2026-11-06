# Contract: Settings File

`capturax.toml` in the settings location ([data-model.md](../data-model.md)). UTF-8 TOML,
written by the app, readable and editable by people.

## Version 1

```toml
version = 1

[general]
language = "auto"          # "auto" or a code from the language list ("en", "pt-BR")
theme = "auto"             # "auto", "dark", "light"
start_fullscreen = false
keep_in_tray = false

[window]
width = 1280               # logical pixels, at least 640
height = 720               # logical pixels, at least 360
x = 320                    # physical pixels in desktop coordinates; absent = centered
y = 180
maximized = false
monitor = "\\\\.\\DISPLAY1" # monitor name as the OS reports it; absent = primary
```

## Rules

1. **Categories** are TOML tables. A later spec adds its own table (`[preview]`, `[audio]`, …)
   and never renames or repurposes an existing key (FR-031).
2. **Version**: the root `version` is the format version. A change that cannot be read leniently
   bumps it, and the app migrates older files on load. A file with a higher version than the app
   knows is read leniently.
3. **Lenient read**: a missing or invalid value takes its default; the rest of the file still
   applies. An unknown `language` or `theme` becomes `"auto"`.
4. **Unknown keys and tables are kept** on every save (FR-030).
5. **Damaged file** (not valid TOML, not UTF-8, or unreadable): renamed to
   `capturax.toml.damaged-<unix seconds>`, defaults are used and the notice `settings-damaged` is
   shown. The app always starts (US3 scenario 4).
6. **Atomic save**: written to a temporary file (`.capturax-*.tmp`) in the same folder, synced,
   then replacing `capturax.toml`; an interruption leaves either the old or the new file
   (FR-029). The temporary file of an interrupted save stays behind, so at start, once this
   instance holds the single-instance channel (no other instance can be saving there), the app
   removes every `.capturax-*.tmp` in the settings folder (maintainer, 2026-11-06).
7. Comments written by people are not kept when the app saves (the `toml` crate does not keep
   them); values are.

## Log

`capturax.log` next to the settings file: plain text, one line per event, at most 1 MiB; when
full it is renamed `capturax.log.1` (replacing the previous one) and a new file starts (FR-032).
Never sent anywhere.
