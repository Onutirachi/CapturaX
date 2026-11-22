# Probe Measurements

Probes from tasks T003 (clipboard, research R4) and T004 (GlobalShortcuts, research R7), run
before the code that depends on them (quickstart "Probes").

## T003: Clipboard

`cargo run --release -p capturax-platform --example clipboard_probe -- <width> <height>`

### Windows: set call time (agent, 2026-11-22)

**Environment**: Windows 11 Pro 26300, commit after `d7a7705`, run from a shell with no app
switch and no paste (the pastes are below, for the maintainer).

| Size | Release | Debug |
|---|---|---|
| 3840 × 2160 | 22.1 ms | 945.3 ms |
| 1920 × 1080 | 8.5 ms | — |

The 2 s clipboard timeout (T014) leaves room even in a debug build.

### Windows: pastes (maintainer)

Focus another app within 5 s of the start, then paste.

| Target | Pasted | Notes |
|---|---|---|
| GIMP or Krita | | |
| Paint | | |
| Discord (browser) | | |

### CachyOS, Wayland (maintainer)

| Target | Pasted | Notes |
|---|---|---|
| set call time (3840 × 2160) | | |
| GIMP or Krita | | |
| Dolphin | | |
| Discord (browser) | | |

### CachyOS, X11 (maintainer, if a session is installed)

| Target | Pasted | Notes |
|---|---|---|
| set call time (3840 × 2160) | | |
| GIMP or Krita | | |
| Dolphin | | |

## T004: GlobalShortcuts (maintainer, CachyOS)

`cargo run -p capturax-platform --example shortcuts_probe -- CTRL+F8`

| Check | Result |
|---|---|
| `Registry.Register` available | |
| GlobalShortcuts version | |
| What KDE asks the first time | |
| `trigger_description` in the response | |
| Key with another app focused | |
| Key in a fullscreen game | |
| After stopping and running again: kept or asked again | |
| `--configure`: what it shows | |
