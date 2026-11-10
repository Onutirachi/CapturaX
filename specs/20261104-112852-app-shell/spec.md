# Feature Specification: App Shell

**Feature Branch**: `feat/20261104-112852-app-shell`

**Created**: 2026-11-04

**Status**: Approved (2026-11-04)

**Input**: User description: "App shell (see docs/roadmap.md #2): the Rust + Slint application skeleton chosen by the stack spike: own winit + wgpu render loop with the video presented from its own thread and Slint drawn as a layer over it (stack spike findings, research R13 and R15), the Float direction's frameless window and chrome. Covers F8, F39, F40, F46, the settings storage part of F41, and a common way to show empty and error states (F11). Includes the theme tokens from the spike's chosen visual direction, i18n infrastructure (English and Portuguese), the portable settings file with the per-user fallback, and CI (format, lint, tests, build on Windows and Linux)."

## Context

This is the first spec that ships application code. It delivers the window every later feature
lives in, not a feature of its own: a user who opens this build sees the CapturaX window, can
move, resize and fullscreen it, switch language and theme, and their choices survive a restart.
There is no device yet; the video area shows an explicit empty state (device preview is spec #3).

The render architecture is not open here: the stack spike decided it and the constitution
(1.1.0, Technology and Product Constraints) requires it. The app owns the window and presents
video from a thread of its own; the interface is drawn as a layer over the video; the video never
waits for the interface or the window
([findings](../20261026-154808-stack-spike/findings.md) "Decisions", research R13, R15). The look
is the spike's Float direction with its theme (T027) and components (T028), reviewed by the
maintainer in T033. This spec states what the shell must do and how it is accepted; the plan
states how.

The stack spike left four open questions for this spec (findings "Open questions for later
specs", App shell): the dock covering the info bubbles' hover and dropdowns cut off in a small
window, the frame rate drop during fast window moves on Windows, the snap layouts flyout, and
shadows above 100% display scale. Each is answered below.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A window that never holds the video back (Priority: P1)

The user opens CapturaX and gets a frameless window in the app's own style: a thin title bar with
the logo and window controls, the video area below it, and controls floating over the video that
hide when the pointer leaves. They move it, resize it, maximize it, snap it to a screen edge and
put it in fullscreen with F11, and whatever they do with the window or the controls, the video
area keeps updating at the source's rate.

**Why this priority**: every later feature lives in this window, and the preview's latency is the
product (constitution, principle I). The shell's render loop is the part of the preview path that
all later specs inherit; if it adds delay or stalls, nothing built on it can recover.

**Independent Test**: start the app with the developer test video (FR-004), exercise every window
action and every chrome state, and read the presentation statistics the app reports (FR-005).

**Acceptance Scenarios**:

1. **Given** the app is open, **When** the user drags the title bar, **Then** the window moves;
   a maximized window restores under the pointer; a double click on the title bar maximizes or
   restores it.
2. **Given** the app is open and not maximized, **When** the user drags any edge or corner,
   **Then** the window resizes, down to the minimum size of 640 × 360 (FR-009).
3. **Given** the app is open, **When** the user presses F11 or the fullscreen control, **Then**
   the window fills the screen, the title bar hides and the video takes the whole screen; F11,
   the control again or Esc returns to the previous window state.
4. **Given** a menu is open in fullscreen, **When** the user presses Esc, **Then** only the menu
   closes; a second Esc leaves fullscreen; Esc never quits the app.
5. **Given** the test video at 1080p60 is playing, **When** the user moves, resizes or holds the
   window still mid-drag, **Then** the video never stops updating (no gap of a second as in the
   spike before T052), and the app reports the longest gap between two presented frames.
6. **Given** the pointer leaves the video area, **When** 2 s pass, **Then** the toolbar, docks
   and overlay hide; **When** the pointer comes back over the top of the video, **Then** they
   show again.
7. **Given** a window smaller than the Float layout needs, **When** the user opens any menu or
   hovers any control, **Then** no control overlaps another, menus stay inside the window, and
   hover areas are not covered by other controls.
8. **Given** Start in fullscreen is on, or the app is started with the fullscreen command-line
   option, **When** the window appears, **Then** it is in fullscreen; Esc or F11 returns it to
   the saved window state. Started with the windowed option, it opens windowed whatever the
   setting.
9. **Given** Keep in tray when closed is on, **When** the user closes the window, **Then** it
   hides and a tray icon remains; clicking the icon shows it again; Quit in the tray menu quits.
10. **Given** Start with the system and Keep in tray when closed are on, **When** the user signs
    in, **Then** CapturaX starts with only its tray icon, no window; with the tray option off it
    starts with its window.
11. **Given** Windows 11, **When** the user drags the window to a screen edge or uses Win+arrow,
   **Then** it snaps as a normal window does, including to half of a 1080p screen.

---

### User Story 2 - The app speaks my language and wears my theme (Priority: P2)

The user opens the settings panel (inside the main window, over the video) and picks the language (Automatic, English, Português (Brasil)) and
the theme (Automatic, Dark, Light). The whole interface changes at once, without a restart.
On first run both follow the system.

**Why this priority**: required from day one by the constitution (internationalization) and by
F39 and F46; every later spec adds text and surfaces that must already be translatable and
themed.

**Independent Test**: switch the system language and theme, start the app, then change both in
the settings and check every visible text and surface.

**Acceptance Scenarios**:

1. **Given** a system in Portuguese and no settings file, **When** the app starts, **Then** every
   text is in Portuguese (Brazil); in any language other than Portuguese it is in English.
2. **Given** the app is open, **When** the user picks another language in the settings, **Then**
   every visible text changes at once, open menus and tooltips included, without a restart.
3. **Given** the app is open, **When** the user picks Dark or Light, **Then** every surface,
   text, icon and the logo change at once; with Automatic the app follows the system theme,
   also when the system theme changes while the app runs.
4. **Given** either theme, **When** any text is shown over its background, **Then** it reaches a
   contrast of at least 4.5:1, and every icon of a control at least 3:1.

---

### User Story 3 - My settings are kept next to the app (Priority: P2)

The user's choices (language, theme, window size and place) are saved in a file in the app's own
folder, so copying the folder is a complete backup. When that folder cannot be written, the app
says so once and keeps the settings in the user's configuration folder instead.

**Why this priority**: the portable promise (constitution, principle VI) is decided by where the
first setting is written; changing it later would strand users' files. Every later spec adds its
own settings category to this file (F40).

**Independent Test**: run the app from a writable folder, change settings, restart; copy the
folder to another place and start it there; run it from a read-only folder.

**Acceptance Scenarios**:

1. **Given** the app's folder is writable, **When** the user changes a setting and restarts the
   app, **Then** the change is kept and the settings file is in the app's folder.
2. **Given** the app's folder was copied to another disk or computer, **When** the app starts
   there, **Then** it uses the copied settings.
3. **Given** the app's folder is not writable, **When** the app starts, **Then** it shows a
   notice naming the folder it uses instead (the per-user configuration folder of the OS), and
   settings changed afterwards survive a restart.
4. **Given** the settings file is damaged or unreadable, **When** the app starts, **Then** it
   keeps a copy of the damaged file, starts with defaults and says so; it never fails to start
   because of the settings file.
5. **Given** the computer loses power while a setting is being saved, **When** the app starts
   again, **Then** the settings file holds either the old or the new values, never a broken file.

---

### User Story 4 - Problems are shown with the next step (Priority: P3)

Wherever the app has nothing to show or something failed, it shows an explicit state: a short
title, an icon, and the action that helps, never an empty area. In this spec the video area shows
the "no source yet" state; later specs reuse the same component for no devices, access blocked,
device in use, device disconnected and black image (F11).

**Why this priority**: the constitution requires explicit states (principle II); defining the
component once keeps every later spec consistent. The shell itself has few states to show.

**Independent Test**: start the app with no source and with each shell-level problem forced
(settings folder not writable, damaged settings file) and check what is shown.

**Acceptance Scenarios**:

1. **Given** no source is open, **When** the app is shown, **Then** the video area shows an empty
   state with an icon and a short title in the current language, not a black or blank area.
2. **Given** a state with an action, **When** the user activates the action, **Then** the action
   runs; a state can have no action, one action, or a primary and a secondary action.
3. **Given** a notice (for example the settings fallback), **When** it is shown, **Then** it does
   not block the window, can be dismissed, and is not shown again in the same run.
4. **Given** the app cannot start its renderer (no usable graphics device), **When** it starts,
   **Then** the user gets a system message that says so in their language, and the app exits.

---

### User Story 5 - Every change is checked on both platforms (Priority: P3)

A contributor opens a pull request; it is checked automatically for formatting, lint, tests and
a build on Windows and on Linux, and cannot be merged until all pass.

**Why this priority**: the constitution requires CI before merging (Development Workflow) and
both platforms in every spec (principle IV). It must exist before the first feature spec so no
feature lands untested on one platform.

**Independent Test**: open a pull request with a formatting error, then a failing test, then a
change that only breaks one platform's build; each is reported and blocks the merge.

**Acceptance Scenarios**:

1. **Given** a pull request, **When** it is opened or updated, **Then** formatting, lint, tests
   and build run on Windows and Linux and report their result on the pull request.
2. **Given** any check fails, **When** someone tries to merge, **Then** the merge is blocked.
3. **Given** a user-facing text written outside the locale files, or a text missing from the
   Portuguese catalog, **When** checks run, **Then** they fail and name the text.

---

### Edge Cases

- The window was last closed on a monitor that is no longer connected: it opens on the primary
  monitor, fully visible.
- The saved window size is larger than the current screen: it is reduced to fit.
- The window is dragged between monitors with different display scales or refresh rates: the
  interface stays sharp at the new scale and the video presents at the new monitor's rate.
- Display scale above 100% (125%, 150%, 200%): icons, logo and text are sharp, not stretched;
  shadows, soft by nature, scale with the window and show no visible steps or seams (spike open
  question: shadows need a @2x set; research R8).
- The app is started a second time: the running window comes to the front and the second start
  exits, so two instances never write the same settings file or open the same device. Where the
  platform does not allow raising another app's window (Wayland), the second start exits after
  telling the user that CapturaX is already open.
- The settings file was written by a newer version of the app: unknown values are kept when the
  file is saved again, not dropped.
- The settings file names a language or theme the app does not know: the app falls back to
  Automatic for that setting.
- The system language is Portuguese but not Brazil (for example pt-PT): the app uses Português
  (Brasil), the only Portuguese it has.
- Both the app's folder and the per-user folder are not writable: the app runs with the settings
  in memory and says that changes will not be kept.
- The app's folder is read-only but has a settings file (for example a backup on read-only media):
  the app reads it and saves later changes to the per-user folder.
- On Linux run as an AppImage, "the app's folder" is the folder that holds the AppImage file, not
  the read-only mounted image.
- Fast window moves on Windows: the spike saw the video drop to 40–50 fps during fast moves,
  likely the system compositor. Accepted as a known limit, not investigated (maintainer,
  2026-11-04): nobody plays through the preview while moving the window. What matters is that the
  window itself moves smoothly and the video never stalls.

## Requirements *(mandatory)*

### Functional Requirements

**Window and render loop**

- **FR-001**: The app MUST present each video frame as soon as it arrives, from a thread that
  never waits for the interface or for the window's own thread (constitution, Technology and
  Product Constraints; research R13, R15).
- **FR-002**: The interface MUST be drawn as a layer over the video, and the video MUST always be
  composited with the last finished interface image; a change in the interface never delays a
  video frame.
- **FR-003**: The preview path MUST use the presentation that the spike measured fastest on each
  platform (findings "Decisions": Windows Direct3D 12 with Mailbox, Linux Vulkan with Mailbox),
  falling back to the next available one, and the app MUST report which one is in use.
- **FR-004**: The app MUST offer a developer option that plays a synthetic moving test video
  (with a selectable resolution and frame rate, at least 1080p60 and 1440p60) in the video area,
  so the render loop can be checked and measured without a capture card. It is not shown to
  users in the interface.
- **FR-005**: The app MUST be able to report presentation statistics per second: frames
  received, frames presented, frame ready → presented time (median and maximum), longest gap
  between two presented frames, and interface redraws and their time. This is the measurement
  behind the latency budget (SC-001).
- **FR-006**: The window MUST be frameless with the app's own title bar: logo, minimize, maximize
  or restore, and close. The logo comes from `brand/`: `CapturaX-light.svg` (white text) in the
  dark theme and `CapturaX-dark.svg` (black text) in the light theme; the window, taskbar and tray
  icon comes from `brand/X.svg`; move by dragging the title bar, maximize by double click, resize from
  every edge and corner, and system snapping by dragging and keyboard.
- **FR-007**: F11 MUST toggle fullscreen; Esc MUST close, in this order, an open menu, else the
  settings panel, else leave fullscreen, and MUST never quit the app (F8). In fullscreen the
  title bar hides and the video takes the whole screen.
- **FR-008**: The chrome (toolbar over the top of the video, left and right docks, overlay) MUST
  hide 2 s after the pointer leaves the video area and show again when the pointer reaches the top
  of the video, as in the spike's Float direction.
- **FR-009**: The window MUST have a minimum size of 640 × 360 (enough for half of a 1080p
  screen in Windows' snap layouts). Below the size the Float layout needs (800 × 520 in the
  spike), the main screen MUST switch to a compact layout in which no control overlaps another
  (F8).
  *Revised 2026-11-09*: the docks no longer move into the toolbar's row in the compact layout;
  they stay in the middle of the sides, and settings moves to the left dock (device preview
  spec FR-002a).
- **FR-010**: Menus, dropdowns and tooltips MUST stay inside the window, and a control's hover
  area MUST never be covered by another control (spike open question).
- **FR-011**: On Windows 11 the app SHOULD show the snap layouts flyout when the pointer rests
  on the maximize button. Best effort: the spike found it needs the window to answer the system's
  hit test as a maximize button, which the window library does not expose, so it takes
  platform-specific code in the Windows platform module. If that code proves fragile (breaks
  moving, resizing, or the button's own hover and click), the flyout is dropped and recorded as a
  known limit; it is not an acceptance gate.
- **FR-012**: The window's size, position, maximized state and monitor MUST be saved and
  restored on the next start, within the visible area of the connected monitors.
- **FR-012a**: A setting **Start in fullscreen** (off by default) MUST make the app start in
  fullscreen. Command-line options MUST override it for that start only, in both directions
  (start in fullscreen, start windowed), without changing the saved setting, so two shortcuts can
  start the app in different ways (F8).
- **FR-013**: Only one instance MUST run per settings location; a second start brings the running
  window to the front where the platform allows it and exits.
- **FR-014**: Closing the window MUST quit the app and release everything it holds, unless the
  tray option is on (FR-014a); the app MUST stop presenting before the window is destroyed (as in
  the spike).
- **FR-014a**: A setting **Keep in tray when closed** (off by default) MUST make closing the window
  hide it and leave a tray icon; clicking the icon shows the window again, and its menu has Open
  and Quit. While hidden the app does not present video (as hiding the preview, F10). A second
  start (FR-013) shows the hidden window. Later specs add their own tray menu entries (for
  example record and stream). Where the desktop has no tray (for example GNOME without an
  extension), the setting is not shown and closing quits (F48).
- **FR-014b**: A setting **Start with the system** (off by default) MUST start the app when the
  user signs in, on Windows and on Linux (the desktop's autostart). Started this way with Keep in
  tray when closed on, the app MUST start straight to the tray with no window shown, whatever
  Start in fullscreen says; with the tray off or unavailable it opens the window as usual. The
  setting MUST show the system's real state (if the user removed the entry in the system's own
  tools, it shows off), and when the app runs from another folder with the setting on, it MUST
  point the entry to itself, so a moved portable folder keeps starting (F49).
- **FR-015**: The shell MUST show only controls whose feature exists. In this spec the docks hold
  fullscreen and settings; later specs add their own controls in the places Float defines.

**Look**

- **FR-016**: The interface MUST use the theme from the spike (T027): one cool neutral in four
  tone steps, orange `#FF7304` accent with dark ink on orange, `accent-text` for orange text in
  light, Inter bundled with the app, depth by tone steps and shadow images, semi-transparent
  panels only (dock, title bar, overlay), opaque menus, buttons and icons, a 2 px focus ring as
  the only outline. The theme values MUST be defined in one place and used by name.
- **FR-017**: Dark and Light themes MUST exist, plus Automatic (follow the system and its
  changes while running), which is the default (F46).
- **FR-018**: Every text pair MUST reach a contrast of at least 4.5:1 in both themes, and every
  icon of a control at least 3:1 (WCAG non-text contrast); an icon on a semi-transparent panel
  MUST reach it over any video behind it.
- **FR-019**: The interface MUST stay sharp at display scales from 100% to 200%, including when
  the window moves to a monitor with another scale.
- **FR-020**: Every interactive control MUST be reachable and usable with the keyboard and show
  the focus ring when focused by keyboard. The title bar's window buttons (minimize, maximize or
  restore, close) are the exception: as in native apps they are not in the Tab order, and the
  system keys cover them (Win+arrows, Alt+F4; F11 for fullscreen).

**Language**

- **FR-021**: All user-facing text MUST come from locale files; English is the source and
  fallback, Português (Brasil) is complete (F39, constitution).
- **FR-022**: The language MUST be Automatic by default (system language: any Portuguese →
  Português (Brasil), anything else → English) or chosen in the settings, and MUST switch at once
  without a restart, text produced by the app's logic included.
- **FR-023**: The language list MUST name each language in itself ("English", "Português
  (Brasil)").
- **FR-024**: Adding a third language MUST need only a new locale file and its name in the list,
  no change to the interface or logic.

**Settings**

- **FR-025**: Settings MUST be a panel inside the main window, centered over the video, opened
  from the dock and closed with Esc, its close control or a click outside it; the video keeps
  playing behind it, dimmed, also in fullscreen. While it is open, keyboard focus stays inside
  it. The app opens no other windows. The panel lists its categories, each with an icon and its
  name, in a column on its left, and shows the chosen category's settings on the right; the
  chosen category is tinted with the accent, as a toggled tool button. Its header holds the settings icon and the title on the left and
  the close control at the other end, the header every small window inside the main window
  shares. In the compact layout (FR-009) the column keeps
  only the icons, with the names in tooltips. In this spec: **General** (Start in
  fullscreen, FR-012a; Keep in tray when closed, FR-014a; Start with the system, FR-014b),
  **Appearance** (language, theme) and **About** (the logo, FR-006, and the app's version). Later specs add their
  categories to the same panel (F40). (Maintainer, 2026-11-05: a central panel with categories
  replaces the panel beside the dock, because later settings include lists such as audio
  devices and hotkeys.)
- **FR-026**: Settings MUST be stored in one human-readable file in the folder that holds the
  executable (or the AppImage file on Linux) when that folder is writable (F41, constitution
  principle VI).
- **FR-027**: When that folder is not writable, the app MUST use the per-user configuration folder
  of the OS and show a non-blocking notice naming it, once per run. If the app's folder holds a
  readable settings file, it is read first.
- **FR-028**: When no location is writable, the app MUST run with settings in memory and say that
  changes will not be kept.
- **FR-029**: Settings MUST be saved soon after each change, without a save button, and written
  so that an interruption at any moment leaves either the old or the new file.
- **FR-030**: A damaged settings file MUST be kept aside as a copy, replaced by defaults, and
  reported to the user; values the app does not know (from a newer version) MUST be preserved
  when the file is saved.
- **FR-031**: The settings file MUST be organized in categories that later specs add without
  changing existing ones, and MUST carry a version so a later app can migrate it.
- **FR-032**: The app MUST write a local log file next to the settings, limited in size, for
  diagnosis. Nothing is ever sent anywhere (constitution, principle V).

**Empty and error states**

- **FR-033**: The app MUST have one component for empty and error states with an icon, a short
  title, an optional one-line detail, and zero, one or two actions (primary and secondary), in
  both themes and both languages (F11).
- **FR-034**: The app MUST have one non-blocking, dismissible notice for shell-level messages
  (settings fallback, damaged settings, settings not kept), shaped as the settings panel: a
  header with an icon for its kind, a title of its own and the close control (FR-025), then its
  text. (Superseded by the device preview spec's FR-033, 2026-11-09: these notices now use the
  notice balloon, in the corner of the Notification position setting.)
- **FR-035**: The video area MUST show the empty state "no source" when nothing is open, never a
  black or blank area.
- **FR-036**: If the renderer cannot start, the app MUST show a system message in the user's
  language and exit.

**Platforms and CI**

- **FR-037**: Every requirement above MUST work on Windows 11 and on Linux (X11 and Wayland, KDE
  Plasma checked as in the spike), except where a requirement names a platform. Platform-specific
  code stays behind platform interfaces (constitution, principle IV).
- **FR-038**: Every pull request MUST run formatting, lint, tests and a build on Windows and
  Linux, and the merge MUST be blocked until all pass.
- **FR-039**: CI MUST fail when a user-facing text is not in the locale files or is missing from
  the Portuguese catalog.

### Key Entities

- **Settings**: the user's choices, grouped in categories (in this spec: general — language,
  theme, start in fullscreen, keep in tray, start with the system (its true state lives in the
  OS); window — size, position, maximized, monitor), with a format version. Stored in one file
  in the settings location.
- **Settings location**: where the settings file and the log live; the app's folder, else the
  per-user configuration folder, else memory only. Known at startup and shown to the user when it
  is not the app's folder.
- **Theme**: named color, type, spacing, radius and shadow values for Dark and Light.
- **Locale catalog**: the translated texts of one language, keyed by the English source text.
- **State message**: icon, title, optional detail, up to two actions; used for empty and error
  states. **Notice**: a non-blocking, dismissible message.
- **Presentation statistics**: per-second counts and times reported by the render loop (FR-005).

## Success Criteria *(mandatory)*

### Measurable Outcomes

Latency budget (constitution, principle I). The shell adds the render loop to the preview path.
Its budget is the time from a video frame being ready in the app to it being presented,
measured by the app itself (FR-005) with the test video at 1080p60, Windows with Direct3D 12 +
Mailbox and Linux with Vulkan + Mailbox. Glass-to-glass latency with a real card is measured in
spec #3 against the spike's 30.4 ms median; this spec only must not add to it.

- **SC-001**: Frame ready → presented has a median of at most 1.5 ms in every second, and a
  per-second maximum of at most 3 ms in at least 95% of the seconds, with the 1080p60 test video on a 1080p screen and on a 1440p screen (NVIDIA DSR,
  as in the spike; the maintainer has no native 1440p monitor), windowed and fullscreen, in every
  interface state: idle, chrome showing and hiding, a menu opening and closing, language and
  theme switching (spike T049 and T051: 0.8–1.1 ms median, 1080p frames from the card; the
  3.4 ms worst case came from the interface drawing on the video's thread, which FR-001 removes).
  A 1440p video is measured with a real card in spec #3 if the card captures it. Stalls at a
  change of the system's presentation mode (focus change, entering fullscreen) are recorded
  apart and do not count (maintainer, 2026-11-05: on Windows they come from the system and the
  driver, 50–400 ms, research R4; their investigation is planned for spec #3).
- **SC-002**: With the test video at 60 fps and the window still, at least 99% of the seconds
  present 59–61 frames, in every interface state of SC-001.
- **SC-003**: While the window is held still mid-drag, the video keeps presenting at the source's
  rate (spike before T052: one frame per second). While the window moves or is resized, the video
  keeps updating, with no stall of hundreds of milliseconds; a frame rate below the source's and
  short hitches of the video and the window are accepted (Edge Cases; maintainer, 2026-11-05:
  only a still window matters).
- **SC-004**: The window appears with the empty state within 2 s of starting the app on the
  maintainer's machines (findings, environment).
- **SC-005**: Language and theme switches complete in under 100 ms, and no text is left in the
  previous language or theme.
- **SC-006**: 100% of user-facing texts are in the locale files and translated to Português
  (Brasil), checked by CI (FR-039).
- **SC-007**: After 100 forced interruptions during a save (in automated tests), the app starts
  every time with a valid settings file.
- **SC-008**: A pull request is checked on Windows and Linux in under 20 minutes.
- **SC-009**: The maintainer accepts the window on Windows 11 and on CachyOS KDE Plasma Wayland
  against the acceptance scenarios of User Stories 1 to 4 (manual acceptance, constitution
  principle VII), at 100% and at 150% or 200% display scale.

## Assumptions

- The look is the spike's Float direction as reviewed in T033 (screenshots in
  [measurements/screens/](../20261026-154808-stack-spike/measurements/screens/)); this spec does
  not reopen it. The design system artifact kept outside the repository is a reference, not a
  dependency.
- The toolbar's dropdowns (video, audio, mode), the info bubbles and the dock's feature buttons
  belong to the specs that deliver those features (#3, #4, #6 and later); the shell provides the
  places for them and the chrome's show and hide behavior, not their content.
- "Automatic" is the default for both language and theme; it counts as the same setting with one
  more value, not an extra option.
- Closing the window quits the app by default; keeping it in the tray is an option (FR-014a).
- Starting in fullscreen is a setting; the command-line options only override it for one start
  (for shortcuts). Starting without the preview belongs to the record / stream only mode, which
  brings its own setting and option when its spec exists.
- The developer test video and the presentation statistics are kept in the real app (behind a
  command-line option) because spec #3 and later specs need them for their latency budgets.
- The spike's latency tools in [tools/latency/](../../tools/latency/README.md) are reused, not
  rebuilt.
- CI runs on GitHub Actions hosted runners (the repository is on GitHub); GPU presentation cannot
  run there, so the render loop's timing is checked by the manual acceptance (SC-001 to SC-003)
  and its logic by automated tests.
- Linux is checked on CachyOS with KDE Plasma on Wayland as in the spike; X11 is supported by the
  window library and checked when available, not as an acceptance gate.

## Out of Scope

- Device listing, opening and the preview's own states (spec #3), audio (spec #4), the info
  overlay's content and placement saving (spec #6).
- The installer, bundled GStreamer packaging, firewall and updates (spec #13). This spec's builds
  are for development and CI.
- Third-party license notices shown in the app (spec #13, with packaging).
- Global hotkeys (F50: spec #9 for recording and screenshot, spec #10 for streaming).
- Recording and streaming status on the tray icon and the taskbar button (F51, specs #9 and
  #10).
- A start without the preview, for recording and streaming only (docs/product.md "To
  reconsider"); this spec only keeps it possible (the window can exist without a source).
