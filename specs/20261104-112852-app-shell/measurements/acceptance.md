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
| 15 | Pass | A second start shows the running window and a message (Wayland does not let it come to the front). The maintainer asked for a shorter message shown as a warning, not an error: now a kdialog warning box "O CapturaX já está aberto." (changed after this run, checked again). |
| 16 (150%) | Pass, one limit | `kscreen-doctor output.HDMI-A-1.scale.1.5`: title bar, logo, icons, tooltips, the panel's text and toggles sharp. A saved 1280 × 720 does not fit the 1280 × 720 logical screen, so the size is fitted to the work area, but with the monitor scale winit reports before the window knows its fractional scale (2 instead of 1.5): the window opens at 960 × 540 logical instead of about 1280 × 690. Fully visible. Moving between monitors of different scale needs a second monitor (with the maintainer). |
| 18 | Pass | In the window, Tab moves between the dock's two controls only (not the title bar's buttons), ring and tooltip shown; Enter opens the panel; inside it Tab goes through the three categories, the three toggles and the close control, then back to the first category (the docks under the panel are left out). Space switches a toggle (saved, then switched back). Down opens a select's menu, Down and Enter choose. |

Not done yet on Linux: 6 (Windows only), 13 and 14 (tray, autostart, sign-out: with the
maintainer), 16 across monitors and 17 (a second monitor: with the maintainer), and 200%.

### Fixes found by these checks

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
