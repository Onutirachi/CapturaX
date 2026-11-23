# Acceptance Checks

Quickstart "Manual checks". T025 (Windows) and T026 (Linux) fill this file.

## Windows, first pass (maintainer, 2026-11-23)

**Environment**: Windows 11 Pro 26300, release build of the uncommitted T013/T019, ezcap at
1080p60 **with no source connected** (the card's own "Please Connect a Video Source" picture), so
every file is the same flat image (38 014 bytes, `png 0 ms`). Times from the log lines
(contracts/cli.md).

| # | Result | Notes |
|---|---|---|
| 1 | pass | sound, outline, "Screenshot saved"; 1920×1080 in Pictures/CapturaX; pasted in Paint, GIMP or Krita, Discord; taken +1–15 ms, rgb 22 ms, written 1–2 ms, copied 8–9 ms |
| 2 | pass | browser focused and minimized: system notification, focus unchanged |
| 4 | pass | held key: one screenshot |
| 5 | pass | 6 presses ≈ 100 ms apart: each done (≈ 40 ms) before the next, so all 6 written, ` (2)`/` (3)` in the same second; no refusal reached |
| 6 | pass | window small and fullscreen: file at the mode's resolution |
| 7 | **not run** | no 4K30 screenshot in the log: to do with a source connected |
| 15 | pass | folder on a missing drive: `CopiedNotSaved(FolderNotFound)`; write-denied folder: `CopiedNotSaved(NoPermission)`; Settings button, image pasted |
| quit | not run | quit right after a screenshot (file complete): left to T025 |

## Windows, second pass (maintainer, 2026-11-23)

Same build, the PS5 as the source.

| # | Result | Notes |
|---|---|---|
| 1 | pass | real picture at 1080p60 (home, GTA VI screens): 6 files of 2.0–2.9 MB, colors right; taken +2–14 ms, rgb 22 ms, png 2–9 ms, written 1–35 ms, copied 11–26 ms: notice ≈ 60–90 ms after the key |
| 4 | pass | held key: one screenshot (maintainer) |
| 5 | pass, limit not reached | 11 presses ≈ 110 ms apart at 2560×1440: all written in order, ` (2)`…` (9)` in one second; each done in ≈ 75 ms (rgb 40 ms), so none waited; the limit of 4 needs a faster source of requests (T027's `--screenshot-every`) |
| 7 | **not run** | the "4K" mode was 2560×1440 (`auto-max-resolution`) with the PS5 at 1080p: the card showed "Check Resolution Settings", so the burst files are that flat picture; a 3840×2160 paste was checked only with the T003 probe's pattern |
| quit | not run | the maintainer cannot quit fast enough by hand |

Still for T025: check 7 with the PS5 set to 4K output, check 3 (fullscreen game), check 10, and a
quit right after a screenshot (e.g. `--screenshot-every` plus the tray Quit).

## Windows, dock button (maintainer, 2026-11-23, T029)

Debug build of the uncommitted T028, ezcap with the PS5.

| # | Result | Notes |
|---|---|---|
| 8 | pass | card unplugged: button disabled, its tooltip still shown; click and Ctrl+F8 do nothing |
| dock | pass | tooltip "Captura de tela (Ctrl+F8)"; click: sound, outline, saved notice; Tab reaches it after the left dock, Enter takes one; both themes; small window: the bubbles' column first stood behind the right dock (fixed: the column always ends 8 px left of it, FR-002a), then checked again |

## Windows, folder row (maintainer, 2026-11-23, T035 Windows half)

Release build of the uncommitted T034, ezcap with the PS5.

| # | Result | Notes |
|---|---|---|
| 14 | pass | Change to another folder (system dialog over the window), screenshot saved there, Open and Reset work; a screenshot with the settings panel open works too; first build had **Change** and **Open** stretched over the row (no width), fixed; the category became **Folders** (maintainer) and was checked again |

## Windows, failure notices (agent and maintainer, 2026-11-23, T037 Windows half)

Debug build of the uncommitted T036, test video 1080p60. The agent ran the failing folders with
`--screenshot-every 3000` and read the log; the maintainer clicked the notices.

| # | Result | Notes |
|---|---|---|
| 15 | pass | `Z:/CapturaX` (no Z: drive): "not saved (FolderNotFound), copied", notice `CopiedNotSaved(FolderNotFound)`; `C:/Windows/System32/CapturaX`: "not saved (NoPermission), copied", nothing made there; copy ≈ 290 ms in debug |
| 9 | pass | **Settings** on the in-app notice opens the panel on Folders; closed and opened from the dock it is on General again; the system notification's **Settings** (another app focused, `-> system` in the log) shows the window on Folders |

## Linux Wayland, portal hotkeys (agent over SSH, CachyOS, 2026-11-23, T017)

KDE Plasma 6.8.2 Wayland, release build of the uncommitted T017, C920 at 1080p. Keys and clicks
sent with `ydotool` (uinput), screens read with Spectacle; the KDE binding and the desktop entry
removed first.

| # | Result | Notes |
|---|---|---|
| 13 | pass | Start: "wrote the hidden desktop entry …/capturax.desktop" (`NoDisplay=true`), KDE's question listed "Captura de tela" Ctrl+F8; OK → "screenshot on Ctrl+F8". Ctrl+F8 with Dolphin focused: screenshot 1 saved and copied, focus stayed on Dolphin. Hotkeys row: "Captura de tela  Ctrl+F8", no Clear or Reset (the key moved to the row's edge after the first look). Click on the key: System Settings → Shortcuts on CapturaX; Ctrl+Shift+F8 added, Ctrl+F8 unchecked, Apply → "screenshot has no key", then "on Ctrl+Shift+F8"; the row followed; Ctrl+F8 did nothing, Ctrl+Shift+F8 saved screenshot 2. Restart: no question, Ctrl+Shift+F8 kept, dock tooltip "Captura de tela (Ctrl+Shift+F8)". Run from another folder (with a space, fresh settings proposing Ctrl+F8): "desktop entry … now names this program", `Exec` updated, Ctrl+Shift+F8 still the key and works. "Change it in the system settings" (portal version 1) not reachable on KDE. |

File names came in local time (`14-38-06` for a 17:38 UTC log line): `utc_offset` checked on
Linux. The test screenshots (webcam pictures) were deleted afterwards.

## Linux Wayland, screenshots (agent over SSH, CachyOS, 2026-11-23, T026, T035, T037)

KDE Plasma 6.8.2 Wayland, release build of `99aae30`, i3-10105 + RTX 3050. The ezcap is on the
Windows PC, so the sources are the C920 (1920×1080@30 MJPG) and the test video (`1080p60`,
`3840x2160@30`). Keys, clicks and pastes sent with `ydotool`, screens read with Spectacle. The
sound was checked by recording: the app's output set to a null sink `capx_test` (not the
system default `vban_out`) and its monitor recorded with `parecord`. The KDE binding was cleared
first: the start asked with Ctrl+Shift+F8, OK.

| # | Result | Notes |
|---|---|---|
| 1 | pass | C920, window focused: outline seen fading in a print taken 120 ms after the key, "Captura de tela salva" with **Mostrar**; 1920×1080 in ~/Imagens/CapturaX; taken +42–55 ms, rgb 72 ms, png 15 ms, written 9 ms, copied 24 ms. Pasted in Dolphin and in Firefox (a local page reading the `paste` event: `image/png 1920×1080`). Krita pasted with the T003 probe (probes.md) |
| 2 | pass | Firefox focused over the window: system notification "Captura de tela salva" with **Mostrar**, Firefox kept the focus; minimized (KWin): a real frame (taken +4 ms), system notification, focus unchanged |
| 4 | pass | Ctrl+Shift+F8 held 2 s (test video 1080p60): one screenshot |
| 5 | pass, limit reached | 6 presses 25 ms apart: screenshots 2–6 written in order (`15-38-11`, ` (2)`…` (5)`), the 6th "not taken (Busy)": the first time the limit of 4 waiting was reached (screenshot 2 ended before the 5th press, freeing a place) |
| 6 | pass | window 700×420 with the toolbar, both docks and a notice shown, then fullscreen: both files 1920×1080, only the video; their left, right and bottom 6 px rows are black (no outline in the image) |
| 7 | pass (test video) | `3840x2160@30`: notice ≈ 350 ms after the key (taken +25–36 ms, rgb 267–280 ms on this CPU, png 4–8 ms of a pattern that compresses well, copied 34 ms). A real 4K picture would take longer to encode: to measure in T027 |
| 8 | pass | C920 unplugged (`usb` unbind): "desconectado" state and notice (no action); the key gives "not taken (NoImage)"; the dock's camera button dimmed, tooltip "Captura de tela (Ctrl+Shift+F8)" on hover, a click does nothing. Plugged back: "Reconnected" notice (no action, the device reopens by itself) |
| 9 | pass | `keep_in_tray`, window closed to the tray: the key gives "not taken (NoImage)", no notification |
| 10 | pass | output `capx_test` (not the default): capture sound recorded at −16.4 dBFS peak (the file's −10.4 with gain 0.5) with the preview at volume 5 %, muted, and with the C920's audio set to "No audio" (output-only "sound session 1" opened in 112 ms, the sound ≈ 100 ms later than the others). Exclusive is Windows only |
| 14 | pass | Settings → Pastas → **Alterar**: KDE's "Selecionar pasta" (portal) opened in the current folder; a vfat image mounted on `/mnt/capxusb` (another file system) chosen; two screenshots in one second written there as `15-40-47` and ` (2)` (vfat: the check-then-rename path); **Abrir** opened Dolphin on the folder; **Mostrar** on the notice opened Dolphin with the new file selected; **Restaurar** back to ~/Imagens/CapturaX, its button hidden again. The chosen path is stored and shown as `/mnt/capxusb/` (trailing slash from the portal) |
| 15 | pass, reason questioned | drive unmounted (empty mount point left), mount point removed (what udisks does to `/run/media/<user>/<label>`), file system read-only: each "Não foi possível salvar a captura de tela: sem permissão. Copiada para a área de transferência" with **Configurações**. A removed drive gives **NoPermission**, not FolderNotFound: the mount point's parent belongs to root, so `make_folder`'s `create_dir` is refused. A folder whose parent is missing (`/mnt/gone/CapturaX`) gives "pasta não encontrada" |
| 9 (Windows' meaning) | pass | **Configurações** on the in-app failure notice opens the panel on Pastas; on the system notification (Firefox focused, `notify: action for settings:screenshot`) the window comes forward on Pastas |

Also checked: `audio_session --output-only --output capx_test`: Opened after 0.12 s, a 440 Hz
tone recorded at −14 dBFS for ≈ 2 s, no event after the input reopen, Closed 0.2 ms after the
close. X11 checks not run: no X11 session installed. The test sink, the vfat image, the
screenshots and the prints (webcam pictures) were removed afterwards; the KDE binding was left
on Ctrl+Shift+F8.
