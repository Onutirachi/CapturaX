# Quickstart: Screenshot

How to check the feature end to end. Hardware: the ezcap card with the PS5 and the C920, on the
Windows PC and the CachyOS PC (KDE Plasma, Wayland), as specs #3 and #4.

## Build and unit tests

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Probes (before the code that depends on them)

1. **Clipboard** (research R4), on both systems:
   `cargo run -p capturax-platform --example clipboard_probe -- 3840 2160`, then focus another
   app within 5 s; paste in GIMP or Krita, Paint (Windows), Dolphin (Linux) and Discord in the
   browser. Record in `measurements/probes.md`: pasted where, time of the set call.
2. **GlobalShortcuts** (research R7), on CachyOS:
   `cargo run -p capturax-platform --example shortcuts_probe -- CTRL+SHIFT+F8`; press the key with
   another app focused and with a fullscreen game; stop and run again; try `--configure`.
   Record what KDE asks the first time and whether the binding is kept.

## Manual checks (acceptance)

| # | Steps | Expected |
|---|---|---|
| 1 | Preview the ezcap at 1080p60, press Ctrl+Shift+F8 with the window focused | Sound on the preview's output, outline flash, "Screenshot saved"; file 1920×1080 in Pictures/CapturaX; pastes in Paint (Windows), GIMP or Krita, and Discord or WhatsApp Web in the browser (SC-006) |
| 2 | Same with a browser focused over the window; with the window minimized | Same file; system notification; focus unchanged; no flash while minimized |
| 3 | Fullscreen game focused (Windows) | Key works, focus stays in the game |
| 4 | Hold Ctrl+Shift+F8 for 2 s | One screenshot |
| 5 | Press 6 times fast | 4 written in order with ` (2)`… names when in the same second; the rest give no click and no notice |
| 6 | Window small, controls and bubbles visible; then fullscreen | File at the mode's resolution, no interface, no flash in the image |
| 7 | 4K30 mode | Notice within 2 s |
| 8 | Unplug the card (state shown), press the key; click the dock button | No click, no flash, no notice; button disabled |
| 9 | Hide to the tray, press the key | Nothing: no click, no system notification |
| 10 | Choose a headset that is not the system default as Audio output; then preview muted; preview audio "No audio"; output exclusive (Windows) | Sound heard on the headset each time |
| 11 | Settings → Hotkeys: set Ctrl+Shift+S; press Ctrl+Shift+F8, then Ctrl+Shift+S, inside and outside the window; restart | Old key does nothing; new one works; kept |
| 12 | Try Shift+A, A alone; Clear; Reset | Refused (A alone, Shift+A); no key; Ctrl+Shift+F8 back; tooltip follows |
| 13 | Wayland, from a folder with no `capturax.desktop` installed: start; OK on the desktop's question; press Ctrl+Shift+F8 with another app focused; Hotkeys row; Change, set another key there and apply; restart; move the folder and start again | A hidden `capturax.desktop` appears in `~/.local/share/applications` (not in the menu); the key works; the row shows the desktop's key with no Clear or Reset; Change opens the desktop's shortcut settings on CapturaX; the row and the tooltip follow the new key, the old one does nothing; no question after the restart; after the move the entry's `Exec` names the new place and the key still works |
| 14 | Settings → Folders: Change to a folder on another drive; take one; Open; Show on the notice; Reset | Saved there; file manager opens (file selected where possible); default back |
| 15 | Folder on a removed USB drive; read-only folder | "Couldn't save the screenshot: …. Copied to the clipboard" with Settings |
| 16 | Languages and themes | Every new text in both languages; balloon and dock in both themes |

## Latency check (SC-003)

On both systems, at 1080p60, 1080p120 and 4K30:

```sh
capturax --stats                                  # 60 s, baseline
capturax --stats --screenshot-every 1000          # 60 s, one per second (debug build)
```

Plus a burst of 10 presses during a third run. Compare frames dropped and the frame-time spread
of each `--stats` line; the audio line shows no gap. Record in `measurements/latency.md`; the
screenshot log lines (contracts/cli.md) give SC-001's times.
