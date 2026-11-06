# Acceptance: App Shell (T065)

Quickstart checks 1–18 ([quickstart.md](../quickstart.md)) on both platforms, at 100% and at 150%
or 200% (SC-009). Checks 8–11 are recorded with T055, checks 1 and 7 with T050 and the latency
budget in [latency.md](latency.md).

## CachyOS KDE Plasma Wayland — 2026-11-06

**Environment**: CachyOS, KDE Plasma 6 Wayland (KWin 6.8.2), NVIDIA GeForce RTX 3050, one 1080p
120 Hz monitor (HDMI-A-1), release build of `docs: record Linux latency and acceptance checks`
plus the fixes below, test video 1080p60. Driven by the agent over SSH with
`ydotool` (pointer and keys), KWin scripts (window geometry) and `spectacle` screenshots, checked
on the screenshots.

| # | Result | Notes |
|---|---|---|
| 2 | Pass | Move by the title bar; double click maximizes (1920 × 1034, work area) and restores; dragging the maximized title bar restores under the pointer at the same relative place; every edge and corner grows and shrinks, and stops at 640 × 360. |
| 3 | Pass after a fix | F11 and the dock's control enter and leave fullscreen. With the settings panel and a menu open in fullscreen, Esc closes the menu, then the panel, then leaves fullscreen; in a window Esc does nothing and the app keeps running. **Found**: leaving fullscreen put the window off its place (see Fixes). |
| 4 | Pass | The chrome is still shown 1 s after the pointer leaves, gone at 2.5 s; the pointer in the middle of the video does not bring it back, at the top it does. |
| 5 | Pass | At 640 × 360: docks at the top corners, no overlaps; the panel takes the compact layout (icon column, names in tooltips); tooltips of the docks, the panel's close control and the title bar's Close stay inside the window and do not cover their control; the Language and Theme menus open inside the window. Menu scrolling has no real case: the longest menu (Language) has 3 items. **Found**: two tooltip faults (see Fixes). |
| 7 (tooltip case) | Pass after a fix | Compact panel, pointer on the Appearance category (tooltip shown), the language changed from the keyboard through the Language menu: every text changes at once, the tooltip included; a system theme change (`plasma-apply-colorscheme`) with the tooltip shown recolors it at once. **Found**: the tooltip kept the old language and the focus jumped to the category (see Fixes). |
| 12 | Pass, one limit | Start in fullscreen on → starts fullscreen; `--windowed` overrides it; with it off `--fullscreen` starts fullscreen. Leaving a fullscreen the app started in gives the saved size at KWin's top left corner (0, 0): on Wayland the app cannot place its window and KWin has no windowed geometry to return to. |
| 13 | Pass after a fix, one limit | By the maintainer. Closing hides the window in the tray, a click on the icon shows it, Quit quits, a second start shows the hidden window. **Found**: the window stayed on screen, frozen (see Fixes). The window shown again has the saved size but KWin's placement, not its place before (research R11). |
| 14 | Pass | By the maintainer: started by the system with Keep in tray on, only the tray icon shows; with it off, the window; the entry turned off in the system's autostart settings, the panel shows off. |
| 15 | Pass | A second start shows the running window and a message (Wayland does not let it come to the front). The maintainer asked for a shorter message shown as a warning, not an error: now a kdialog warning box "O CapturaX já está aberto." (changed after this run, checked again). |
| 16 (150%) | Pass, one limit | `kscreen-doctor output.HDMI-A-1.scale.1.5`: title bar, logo, icons, tooltips, the panel's text and toggles sharp. A saved 1280 × 720 does not fit the 1280 × 720 logical screen, so the size is fitted to the work area, but with the monitor scale winit reports before the window knows its fractional scale (2 instead of 1.5): the window opens at 960 × 540 logical instead of about 1280 × 690. Fully visible. Moving between monitors of different scale needs a second monitor (with the maintainer). |
| 18 | Pass | In the window, Tab moves between the dock's two controls only (not the title bar's buttons), ring and tooltip shown; Enter opens the panel; inside it Tab goes through the three categories, the three toggles and the close control, then back to the first category (the docks under the panel are left out). Space switches a toggle (saved, then switched back). Down opens a select's menu, Down and Enter choose. |

Not done on Linux: 6 (Windows only); 16 across monitors and 17 (no second monitor on the Linux
PC; both pass on Windows); 200%.

### Fixes found by these checks

- **Closing to the tray froze the window (check 13)**: winit cannot hide a window on Wayland
  (`set_visible` does nothing), so the window stayed on screen while the video thread stopped
  presenting: it could be moved by KWin, nothing else answered. Where winit cannot tell whether
  the window is visible, closing to the tray now stops the video thread and destroys the window;
  showing it creates a new one and a new video thread on the same inbox (research R11; the test
  video keeps running).

- **Leaving fullscreen off its place (check 3)**: each F11 toggle shifted the window by
  (320, 180). The video thread's frames of the fullscreen size reached KWin after winit had
  acknowledged the smaller size; research R5 has the protocol details. The app now holds the
  video thread's presents and leaves fullscreen two refreshes later (`render::Inbox::hold_for_resize`,
  `App::set_fullscreen`). Before: 0 of 3 toggles in place; with the hold alone 3 of 9; with the
  hold and the wait 10 of 10 (F11 and Esc), no stray commit in the protocol log. Video at the toggles, 4 round trips each
  (`raw/linux-1080-fullscreen-toggles-{before,hold}.txt`): the same gaps before and after
  (25–62 ms in the seconds with a toggle).
- **A tooltip stuck after a size change or another tooltip (check 5)**: a button that showed its
  tooltip on keyboard focus kept the old token after the window's size changed (which hides every
  tooltip), so hovering it later showed nothing. `IconButton` and the settings categories now
  treat a token that no longer owns the tooltip as none.
- **Language switch with a tooltip shown (check 7)**: the categories' list held their translated
  names, so a language switch built the categories again. The new chosen category took the
  keyboard focus from the Language select, and the old one, destroyed while owning the tooltip,
  left it in the old language. The list now holds ids only and each category binds its own name;
  `IconButton`, the categories and the title bar's buttons show their tooltip again when its text
  changes.

Seen, not changed: in the compact panel a category's tooltip stays after a click (an `IconButton`
hides its tooltip on click), and it can cover the row's label to its right while the pointer stays
there.

The `ready_ms` of about 28.7 ms in some fullscreen seconds of the toggle runs is the RTX 3050's
idle clocks (latency.md, SC-001 on Linux), the same before and after the fix.

## Windows 11 — 2026-11-06

**Environment**: Windows 11 Pro 26300, NVIDIA GeForce RTX 5070 Ti, several monitors (main
1920 × 1080 at 144 Hz), release build of `docs: record Linux acceptance checks and the fullscreen
fix` plus the fixes below, test video 1080p60. Checked by the maintainer by hand.

First, the Linux session's fixes on Windows: F11, Esc and the dock's control leave fullscreen
without a visible delay or stall, the window back in place; a dock tooltip shows on hover after a
size change with that button focused; in the compact panel a category's tooltip follows a
language switch and the focus stays on the Language select. All pass.

| # | Result | Notes |
|---|---|---|
| 2 | Pass | |
| 3 | Pass | |
| 4 | Pass after a fix | **Found**: the chrome stayed shown for good (see Fixes). |
| 5 | Pass after a fix | **Found**: in a wide, low window (1920 × 360) the compact panel spread across the whole width (see Fixes). |
| 6 | Pass, one limit | Snap by dragging and by Win+arrows; the snap layouts flyout shows on the maximize button. Leaving fullscreen after a snap returns to the size and place from before the snap, not to the snapped one (see Known limits). |
| 7 | Pass | |
| 12 | Pass | |
| 13 | Pass | |
| 14 | Pass | |
| 15 | Pass | |
| 16 | Pass | Higher scale and moving between monitors of different scale. |
| 17 | Pass | With the monitor the window was on turned off, the app opens on the primary; with it back on, it opens there again. |
| 18 | Pass | Tab inside an open menu closes it, as in native menus; its items are chosen with the arrows, Home, End, Enter or Space (as designed, `menu.slint`). |

### Fixes found by these checks

- **The chrome stayed shown (check 4)**: a dock button reached with Tab keeps the keyboard focus
  after the mouse takes over (its ring hides), and that focus kept the chrome shown, so it never
  hid. The focus now holds the chrome only while the keyboard is in use (`Input.keyboard`, the
  ring shows); a key press brings it back.
- **The compact panel too wide (check 5)**: the compact layout filled the window, so a wide and
  low window stretched it. It is now at most 760 × 520 and centered, as in the Float layout, with
  8 px margins (plan.md, Settings panel; maintainer's request).

### Known limits

- **Fullscreen after a snap**: winit saves the placement (`GetWindowPlacement`) on entering
  fullscreen and restores it (`SetWindowPlacement`) on leaving; for a snapped window that
  placement is the one from before the snap. Browsers return to the snapped place, so it can be
  fixed (keep the window's rectangle when it is snapped and set it back after leaving). The
  maintainer wants it fixed, not in this spec: when it fits.

## CI (T062) — 2026-11-06

Pull request [#3](https://github.com/Onutirachi/CapturaX/pull/3). The first push did not start
the workflow (no run, no check suite, Actions on and GitHub operational); closing and reopening
the pull request started it.

| Run | `checks` | `build-test (ubuntu-24.04)` | `build-test (windows-latest)` | Result |
|---|---|---|---|---|
| 1, empty caches | 0:56 | 12:14 | 7:32 (failed) | `store::tests::several_updates_make_one_save` read the file before the debounced write ended (the count goes up first); test fixed with a flush |
| 2, warm Linux cache | 1:14 | 1:47 | 15:13 | Pass |

SC-008 (under 20 minutes): met, 15:13 for the slowest job. The Windows job had no Rust cache yet
(`Swatinem/rust-cache` saves only after a job that passes, and run 1 failed), so later runs
should be shorter; it is the job to watch, the one near the limit.

Rule on `main`: in a ruleset, a matrix job reports one check per entry, so the required checks
are `checks`, `build-test (ubuntu-24.04)` and `build-test (windows-latest)`.
