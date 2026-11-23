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

Checked through the app itself once T013/T019 were wired (2026-11-23, quickstart check 1): the
same `arboard` call the probe makes, from the `clipboard` thread, with another app focused too.

| Target | Pasted | Notes |
|---|---|---|
| GIMP or Krita | yes | maintainer, check 1 |
| Paint | yes | maintainer, check 1 |
| Discord (browser) | yes | maintainer, check 1 |

### CachyOS, Wayland (agent over SSH, 2026-11-23)

**Environment**: KDE Plasma 6.8.2 Wayland, release build of `99aae30`. The probe ran with no
window of its own and no app focused by it (keys and pastes sent with `ydotool`); the app's own
copies (quickstart check 1) went through the same pastes.

| Target | Pasted | Notes |
|---|---|---|
| set call time (3840 × 2160) | 6.2 ms | release |
| GIMP or Krita | yes (Krita) | the official Krita 6.0.4 AppImage (`QT_QPA_PLATFORM=wayland`; the system's package database was too old to install GIMP without a full upgrade): Ctrl+Shift+N "new image from clipboard" asked for the missing color profile, sRGB → a 3840 × 2160 RGBA image with the bars and gradient |
| Dolphin | yes | Ctrl+V in a folder: "paste clipboard contents" dialog, OK → `arquivo colado.png`, PNG 3840 × 2160 RGBA; the app's copy too (1920 × 1080) |
| Discord (browser) | stand-in | a local page in Firefox reading the `paste` event (what Discord's web client reads): `image/png 3840×2160`, the bars drawn; the app's copy `image/png 1920×1080` |

### CachyOS, X11

Not run: no X11 session is installed (`/usr/share/xsessions` does not exist; only
`plasma.desktop` under wayland-sessions).

## T004: GlobalShortcuts (agent over SSH, CachyOS, 2026-11-23)

`cargo run -p capturax-platform --example shortcuts_probe -- CTRL+F8`

**Environment**: CachyOS, KDE Plasma 6.8.2 Wayland, xdg-desktop-portal 1.22.1,
xdg-desktop-portal-kde 6.8.2, commit `1baad8d`. Keys sent with `ydotool` (kernel uinput, so
the compositor sees them as a real keyboard), clicks the same way, screens read with Spectacle.

**Result: passes**, with one condition: the app id needs a `.desktop` file.

| Check | Result |
|---|---|
| `Registry.Register` available | Yes, but it **fails without a `capturax.desktop`** in the data dirs: "Could not register app ID: App info not found for 'capturax'"; `CreateSession` is then refused with `NotAllowed` "An app id is required". With `~/.local/share/applications/capturax.desktop` (also with `NoDisplay=true`, hidden from the menu) it succeeds. Its `Name=` is the name KDE shows. |
| GlobalShortcuts version | 2 |
| What KDE asks the first time | `BindShortcuts` waits on a dialog "Atalhos globais solicitados: CapturaX deseja registrar o seguinte atalho:", one row "Screenshot" with the key button (Ctrl+F8, can be changed there) and a clear button, OK / Cancelar. |
| `trigger_description` in the response | `"Ctrl+F8"` after OK. **Cancel** gives response 1 and KDE stores `none`: the next run does **not** ask again and answers response 0 with `trigger_description ""` (no key until set in System Settings). |
| Key with another app focused | Works (Dolphin focused, focus unchanged). Held 1.5 s: one `Activated`, one `Deactivated`, no repeats. The `Activated` timestamp is always 0. |
| Key in a fullscreen game | No game installed; two stand-ins, both focused and fullscreen at 1920×1080: `vkcube --wsi xcb` (Vulkan on XWayland, like a Proton game) and `mpv --fs` (native Wayland). Works in both. |
| After stopping and running again: kept or asked again | Kept, no dialog (`~/.config/kglobalshortcutsrc` `[capturax] screenshot=Ctrl+F8,Ctrl+F8,Screenshot`). |
| The app asks a different key later | Ignored: once bound, the desktop's key wins. After the user set Ctrl+Shift+F8, runs with `preferred_trigger` `CTRL+F8` and `CTRL+F10` both answered `"Ctrl+Shift+F8"`. |
| A preferred key the desktop already uses (`CTRL+F9`, KWin's "Expose") | The first dialog shows a ⚠ beside the key; OK binds it (`trigger_description "Ctrl+F9"`, stored), but KWin keeps the key: Ctrl+F9 opened KWin's window overview and the probe got no `Activated`. Nothing tells the app. KDE's defaults also use Ctrl+F10 ("ExposeAll") and Ctrl+F12; Ctrl+F8 is free. |
| `--configure`: what it shows | System Settings → Shortcuts opens on the CapturaX entry ("Screenshot:", default Ctrl+F8, custom shortcuts, Add…). A key taken by another action asks to reassign (Ctrl+F9 belongs to KWin). After unchecking Ctrl+F8, adding Ctrl+Shift+F8 and Apply: two `ShortcutsChanged` (first `trigger_description ""`, then `"Ctrl+Shift+F8"`); Ctrl+F8 no longer fires, Ctrl+Shift+F8 does. |

Left as found: the binding unregistered (`KGlobalAccel.unregister capturax screenshot`) so the
app sees the first dialog; `~/.local/share/applications/capturax.desktop` (hidden) kept for the
T017 tests.
