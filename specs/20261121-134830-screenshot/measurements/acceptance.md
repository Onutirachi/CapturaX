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
