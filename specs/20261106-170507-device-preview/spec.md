# Feature Specification: Device Preview

**Feature Branch**: `feat/20261106-170507-device-preview`

**Created**: 2026-11-06

**Status**: Approved (2026-11-06)

**Input**: User description: "Device preview (see docs/roadmap.md #3): covers F1-F4, F6, F7, F11, F43 and F52 from docs/product.md. Device list with hot plug, native modes, the automatic modes (source, max resolution, max FPS), reconnect, latency and FPS measurement, explicit empty and error states (including a hint to turn off HDCP on the console when the image is black while a signal is present). Must state a latency budget and how it is measured (constitution principle I); the stack spike measured 30.4 ms median at 1080p60 on Windows with Direct3D 12 + Mailbox, and lists the open questions this spec inherits. Also investigate the Windows stalls at presentation mode changes the app shell measured (focus change 229-401 ms, entering fullscreen 51-73 ms; app shell research R4)."

## Context

This is the spec that makes CapturaX useful: a user plugs in a capture card, picks it, and plays
the console through the preview. Until now the video area showed only the "no source" state and a
developer test video (app shell, spec #2). This spec adds real devices: the list, the modes, the
preview itself, its measured latency and frame rate, and every state a device can be in.

The render path is not open here. The app shell built it as the stack spike decided (own window,
video presented from its own thread, interface as a layer; Direct3D 12 with Mailbox on Windows,
Vulkan with Mailbox on Linux), and its in-app budget (frame ready → presented, median at most
1.5 ms) was met on Windows with the test video. This spec feeds that path with a real device and
measures the whole chain, glass to glass, against the spike.

Inherited open questions, each answered below:

- From the stack spike (findings, "Open questions for later specs", Device preview): the Linux
  GPU stepping down to idle clocks under the preview's light load (≈ 4.6 ms median, ≈ 9 ms p95);
  Linux capture through V4L2 against PipeWire's camera; the 3–5 ms cost of capturing a 60 Hz
  signal in a 120 Hz mode and how to read the incoming signal (Auto source); latency levers not
  tested (120 Hz source, variable refresh, independent flip, zero-copy upload); the color range
  setting; the proposal for the device list's detail line.
- From the app shell (handoff, research R4, measurements/latency.md): SC-001 not met on Linux at
  idle GPU clocks; the Windows stalls at presentation mode changes (focus change 229–401 ms on the
  native 1080p monitor, one 51–73 ms stall about 18–20 s after entering fullscreen, one
  unexplained 625 ms stall); the language switch stalls (about one switch in ten, 110–960 ms);
  the presentation setting for variable refresh monitors (F52); leaving fullscreen back to a
  snapped place on Windows.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Play the console through the preview (Priority: P1)

The user connects a console to a capture card and starts CapturaX. The card is already picked
(it was last time, or it is the only device), it opens in the best automatic mode, and the
console's image fills the video area with a delay they cannot tell from the passthrough monitor.

**Why this priority**: this is the product (constitution, principle I; product.md main
scenario). Every other story only matters once the preview works.

**Independent Test**: with the PS5 on the ezcap card, start the app, play, and measure glass to
glass with the latency tools (tools/latency: flash page, 240 fps camera, `analyze.py`) against
the card's passthrough.

**Acceptance Scenarios**:

1. **Given** one capture device is connected and the app never opened one, **When** the app
   starts, **Then** it opens that device in its default automatic mode (FR-008) and shows its
   video.
2. **Given** the user previewed a device in a given mode, **When** the app starts again with that
   device connected, **Then** it opens the same device in the same mode (automatic or manual).
3. **Given** the preview is showing, **When** the user plays, **Then** the delay against the
   passthrough is within the latency budget (SC-001, SC-002), windowed and in fullscreen.
4. **Given** the preview is showing, **When** the interface is used (chrome showing and hiding,
   menus, settings panel, language and theme switches), **Then** the video keeps its rate and its
   latency (SC-003).
5. **Given** the device sends full-range video while declaring limited range (the ezcap does),
   **When** the color range is Auto, **Then** the preview matches the passthrough (no crushed
   blacks) and the color range dropdown shows "Auto (Full)"; Full and Limited, chosen in that
   dropdown, force the range.

---

### User Story 2 - Pick a device and a mode (Priority: P1)

The user opens the device menu in the toolbar and sees every capture card and webcam connected
now, with a resolution badge each. They open the mode menu and see the automatic modes first,
then only the modes the device really supports, each with its badge. Choosing one switches the
preview.

**Why this priority**: without it only the default device and mode can be used; webcams, second
cards and non-default modes need it.

**Independent Test**: with the ezcap card and a webcam connected, open both menus, check their
contents against what the drivers report, and switch between devices and modes.

**Acceptance Scenarios**:

1. **Given** devices are connected, **When** the user opens the device menu, **Then** it lists
   every capture card and webcam (not screens or windows: product.md F5), each with its name, a
   detail line (FR-004) and its resolution badge (F43), the open one marked.
2. **Given** a device is open, **When** the user opens the mode menu, **Then** it shows an
   Automatic group (only the automatic modes the device supports, without duplicates of each
   other, FR-007) and a Modes group with each native resolution × frame rate once, largest first,
   each with its badge.
3. **Given** the mode menu is open, **When** the user picks a mode, **Then** the preview switches
   to it and shows video within the time of SC-004; the choice is remembered for that device.
4. **Given** the user picks another device, **When** it opens, **Then** it uses that device's
   remembered mode, else its default automatic mode.
5. **Given** no device is open (the "no source" state), **When** the user activates its action,
   **Then** the device menu of the toolbar opens (product.md, note under F5).

---

### User Story 3 - See how the preview is doing (Priority: P2)

While playing, the user can see the measured latency and frame rate, updated live, next to the
source and mode, to know the preview is healthy (and to compare modes).

**Why this priority**: it backs the product's promise with numbers and helps diagnose a bad
setup; the preview works without it.

**Independent Test**: preview the card at 1080p60 and at 1080p120 with a 60 Hz source, and
compare the values shown with the app's per-second statistics (app shell FR-005).

**Acceptance Scenarios**:

1. **Given** the preview is showing, **When** the chrome is visible, **Then** an overlay shows
   the source, the mode, the frame rate and the latency, updated every second (FR-016).
2. **Given** a 60 Hz source captured in a 120 fps mode, **When** the frame rate bubble is read
   in its default display, **Then** it shows what is presented against the mode's rate (about
   "111 / 120 fps" on the ezcap).
3. **Given** the pointer is over the latency or frame rate bubble, **When** the user opens its
   settings button, **Then** a menu offers that bubble's displays (FR-016a, FR-017); the choice
   applies at once and is kept after a restart.

---

### User Story 4 - Every device problem says what to do (Priority: P2)

Whatever happens to a device, the video area says so with the next action, never a black or
frozen picture: no devices, access blocked by the system, device in use by another app, device
disconnected, or a black image while a signal is present (likely HDCP).

**Why this priority**: the constitution requires explicit states (principle II); a black preview
with no explanation was the old app's most common support question.

**Independent Test**: force each state (no device, camera access off in Windows privacy
settings, the card open in another app, unplugging the card, HDCP on in the PS5) and check what
is shown and that its action works.

**Acceptance Scenarios**:

1. **Given** no capture device is connected, **When** the app is shown, **Then** the video area
   shows "No devices" with a short hint, and the device menu shows the same state instead of an
   empty list; plugging a device in opens it (FR-010).
2. **Given** the system blocks camera access for the app (Windows privacy settings; on Linux the
   device node not readable), **When** the device is opened, **Then** the state says access is
   blocked; on Windows its action opens the system's camera privacy page; **Try again** reopens.
3. **Given** another app holds the device, **When** CapturaX opens it, **Then** the state says the
   device is in use, with **Try again**.
4. **Given** the open device is unplugged, **When** it disappears, **Then** the state says it is
   disconnected (naming it); **When** it comes back, **Then** it reopens by itself in the same
   mode (F4) within the time of SC-004.
5. **Given** frames arrive but are entirely black for 5 s, **When** the condition holds, **Then**
   a hint over the video suggests turning off HDCP on the console (on the PS5: Settings → System →
   HDMI → Enable HDCP); it disappears as soon as a non-black frame arrives and can be dismissed
   for the session.
6. **Given** the device opens but no frame arrives within 5 s, **When** the timeout passes,
   **Then** the state says there is no signal, with **Try again** and the mode menu reachable.

---

### User Story 5 - Choose how the preview meets a variable refresh monitor (Priority: P3)

On a G-SYNC or FreeSync monitor, the default presentation makes the monitor follow the video, so
frames show at once and evenly, but the desktop and cursor run at the video's rate while the app
has focus. A user who prefers a smooth cursor switches the presentation setting to follow the
monitor.

**Why this priority**: only users with variable refresh notice it, and the default is the right
one for playing (F52; app shell research R4).

**Independent Test**: on the maintainer's 144 Hz G-SYNC monitor, switch the setting while
previewing and watch the cursor; read the presentation statistics in both modes.

**Acceptance Scenarios**:

1. **Given** the settings panel, **When** the user opens its Video category, **Then** it shows
   **Presentation** with **Lowest latency** (default) and **Follow the monitor**, and a tooltip
   says it only matters with variable refresh.
2. **Given** Follow the monitor is chosen, **When** the app has focus on a variable refresh
   monitor, **Then** the cursor moves at the monitor's refresh rate and the video still never
   waits for the interface; the change applies without restarting the preview.

---

### Edge Cases

- Two identical capture cards: both are listed and told apart (numbered names), and each keeps
  its own remembered mode, as long as the system gives them stable identities.
- The remembered device is not connected at start: the app opens another connected device if
  there is one (the most recently used, else the first capture card, else the first webcam) and
  keeps the remembered choice, so it is taken back when it reappears only if the user has not
  picked another device meanwhile.
- The remembered mode no longer exists (driver update, other device with the same name): the
  device opens in its default automatic mode.
- The device delivers fewer frames than its mode says (a 60 Hz source in a 120 fps mode): the
  frame rate shows what arrives; Auto source avoids the mismatch where the device reports the
  signal (spike: every path 3–5 ms slower in the mismatched mode).
- The source changes resolution or frame rate while previewing (console switching 60/120 Hz, a
  game changing output): with Auto source the preview follows within the time of SC-004; with
  other modes it keeps the mode and the device scales or converts as it does.
- A device offers the same resolution and frame rate in several formats: it appears once, in the
  format with the lowest latency (uncompressed before compressed, NV12 first, as the spike);
  compressed-only modes (for example MJPEG on webcams) are listed and decoded.
- The window is closed to the tray: the device is released (the webcam light turns off) and
  reopened when the window shows again. Minimized: the device stays open and nothing is
  presented, so restoring is immediate.
- The window moves to a monitor with another refresh rate or with/without variable refresh: the
  preview keeps presenting each frame on arrival; the presentation setting applies there too.
- A device is unplugged while its menu is open: it disappears from the open menu; the menus never
  show a device that is gone.
- The device is removed and another one is plugged in while the "disconnected" state shows: the
  app keeps waiting for the device it had open (the new one is in the menu); the user can pick it.
- The system goes to sleep and wakes up with the device open: the device is reopened as after a
  reconnect.
- Camera access is turned off in the system while previewing: the preview shows the
  access-blocked state.
- Linux without the user in the group that can read video devices, or inside a sandboxed package
  (Flatpak): devices that cannot be opened show the access-blocked state; how the app reaches
  cameras in a sandbox is for the plan (V4L2 against PipeWire's camera, spike open question).

## Requirements *(mandatory)*

### Functional Requirements

**Devices**

- **FR-001**: The app MUST list every video capture device the system offers (capture cards and
  webcams) on Windows and Linux, and update the list when a device is plugged in or removed,
  without the user asking (F1).
- **FR-002**: The device menu MUST be the toolbar's video source dropdown (Float direction):
  devices only, each with its name, a detail line and its resolution badge (F43), the open device
  marked. The "no source" state's action opens this same menu.
- **FR-003**: Each device MUST have a stable identity used to remember its choices across
  restarts and reconnects; devices with the same name are numbered in the list. A device whose
  identity depends on its USB port (no serial) moved to another port counts as a new device: its
  choices start from the defaults (maintainer, 2026-11-06).
- **FR-004**: The detail line of each device MUST show the mode it will open in: its remembered
  mode, else its default automatic mode, with the resolved resolution and frame rate (spike
  proposal, adopted as the default here).
- **FR-005**: The app MUST open the last used device at start when it is connected, else follow
  the Edge Cases order, and remember the user's device choice.

**Modes**

- **FR-006**: The app MUST open a device in a native mode (resolution × frame rate × format
  reported by the driver) and present it without software scaling or frame rate conversion of
  its own (constitution, principle I; F2). The image is fitted to the video area by the GPU when
  it is presented, keeping its aspect ratio.
- **FR-007**: The mode menu MUST list an **Automatic** group and a **Modes** group (F2, F3):
  - **Auto source**: follows the signal the device receives (resolution and frame rate); shown
    only when the device reports it;
  - **Auto max resolution**: the largest resolution at about 60 fps (58–62);
  - **Auto max FPS**: the highest frame rate at 1080p, or at the largest resolution below 1080p;
  - an automatic mode that resolves to the same mode as another automatic mode is not shown;
  - Modes: each native resolution × frame rate once, in its lowest-latency format (Edge Cases),
    sorted by resolution then frame rate, largest first, with a resolution badge each (F43). The
    resolved mode is shown next to each automatic one.
- **FR-008**: The default mode of a device MUST be Auto source when it is shown, else Auto max
  FPS (F3; maintainer, 2026-11-06: the focus is playing with low latency and smooth motion).
  Auto source stays in the menu for devices that report their input.
- **FR-009**: The app MUST remember the chosen mode per device (automatic or manual) and use it
  the next time that device opens.
- **FR-010**: A **Color range** choice per device (Auto, Full, Limited; default Auto) MUST
  correct devices that declare the wrong range; Auto infers full range from the video's levels
  (values below 16 on 8-bit video) and switches without restarting the preview (F2; spike
  "Image"). It is a dropdown of its own in the toolbar, next to the mode dropdown, remembered per
  device; with Auto it shows the range it detected (for example "Auto (Full)") (maintainer,
  2026-11-06).

**Preview**

- **FR-011**: The preview MUST present each frame as soon as it reaches the app, through the
  app shell's render path (app shell FR-001 to FR-003), within the latency budget (SC-001 to
  SC-003), on Windows and Linux.
- **FR-012**: The preview's latency MUST NOT depend on the GPU's power state: on a GPU that steps
  down to idle clocks under the preview's load (spike: RTX 3050 on Linux), the app MUST keep the
  frame ready → presented time within the app shell's budget without root, administrator rights
  or system-wide driver settings, at a GPU cost the plan states and measures.
- **FR-013**: Opening, switching and reopening a device MUST NOT block the interface or the
  window; while a device is opening, the video area shows an opening state (no frozen last
  frame from another device).
- **FR-014**: When the open device disappears, the app MUST show the disconnected state and reopen
  the same device in the same mode when it comes back (F4); a system sleep and wake is treated as
  a reconnect.
- **FR-015**: Measuring the preview (statistics, black-image detection, color range inference)
  MUST NOT add to the frame ready → presented time beyond the app shell's budget.

**Measurement (F7)**

- **FR-016**: The app MUST show, over the video and with the chrome, the source name, the mode,
  the frame rate and the latency, updated once per second, as the first info bubbles in a fixed
  spot; pinning, ordering and corners are spec #6 (F42). A bubble that has display choices shows
  a settings button on hover, next to its other hover controls, which opens a small menu with
  its choices; the choice is saved (maintainer, 2026-11-06).
- **FR-016a**: The latency bubble MUST offer three displays (maintainer, 2026-11-06):
  - **Measured**: the app's own part, measured: from the frame's capture timestamp to its
    presentation (for example "3 ms");
  - **Measured + estimated** (default): the measured part plus an estimate of the parts the app
    cannot measure, the device receiving the whole frame and the monitor showing it, marked as
    estimated (for example "3 ms + ~24 ms");
  - **Total**: the sum of the two, marked as estimated (for example "~27 ms").
  A tooltip says what each part is. The estimate is computed from the source's frame rate and
  the monitor's refresh; it is checked against the glass-to-glass measurement (SC-007).
- **FR-017**: The frame rate bubble MUST offer three displays (maintainer, 2026-11-06):
  **Presented** (frames presented per second, for example "60 fps"), **Presented / nominal**
  (default: presented against the mode's nominal rate, for example "111 / 120 fps") and
  **Nominal** (the mode's rate). Dropped frames are not shown in the bubble; they are in the
  statistics (FR-018).
- **FR-018**: The app shell's per-second statistics (app shell FR-005) MUST also cover a real
  device: frames delivered by the device, frames reaching the app, frames presented, dropped and
  repeated frames, and frame ready → presented time, so the budget can be checked with a card.

**States (F11)**

- **FR-019**: The video area MUST show, with the app shell's state component (app shell FR-033),
  an explicit state with its next action for: no devices; opening; access blocked by the system;
  device in use; device disconnected; no signal (no frame within 5 s of opening); device failed
  (any other error, with **Try again** and the error in the log). Texts are short and in both
  languages.
- **FR-020**: When frames arrive and are entirely black for 5 s, the app MUST show a dismissible
  hint over the video that suggests turning off HDCP on the console, and hide it when a non-black
  frame arrives (F11). Dismissed, it stays hidden until the app restarts.
- **FR-021**: The access-blocked state MUST offer, on Windows, an action that opens the system's
  camera privacy settings; on Linux it names the device that could not be opened.

**Presentation setting (F52)**

- **FR-022**: The settings panel MUST gain a **Video** category with **Presentation**: **Lowest
  latency** (default: one present per video frame) and **Follow the monitor** (present at every
  monitor refresh, repeating the last frame). A tooltip says it only matters with variable
  refresh. The change applies at once, without reopening the device.

**Windows presentation stalls (investigation)**

- **FR-023**: The plan MUST investigate the Windows stalls the app shell measured at presentation
  mode changes (focus change 229–401 ms and entering fullscreen 51–73 ms on the native 1080p
  monitor, none at 1440p through DSR; app shell research R4) with the system's presentation
  tracing (present mode per frame: independent flip, composed), with variable refresh off and
  with multiplane overlay off, and record the cause. If the cause is in the app's control, it
  MUST be fixed (SC-006); if not, the finding and any user-side workaround are recorded and the
  maintainer decides.
- **FR-024**: The same investigation MUST cover the language switch stalls (about one switch in
  ten, 110–960 ms, video and interface; app shell measurements T050), with per-stage timing.

**Window (carried from the app shell)**

- **FR-025**: On Windows, entering fullscreen from a snapped window and leaving it MUST return the
  window to its snapped place, not to its place before the snap (app shell open question;
  maintainer, 2026-11-06: wanted, not urgent).

**Platforms**

- **FR-026**: Every requirement above MUST work on Windows 11 and on Linux (KDE Plasma on Wayland
  checked as in the spike; X11 when available), except where a requirement names a platform.
  Device discovery and opening, camera privacy and power handling stay behind platform
  interfaces (constitution, principle IV).

### Key Entities

- **Device**: a video capture device the system offers: stable identity, name (numbered when
  repeated), kind (capture card or webcam, when known), native modes, whether it reports the
  incoming signal, and its state (available, open, in use, blocked, gone).
- **Mode**: resolution, frame rate and format reported by the driver; an automatic mode (Auto
  source, Auto max resolution, Auto max FPS) resolves to one native mode for a device.
- **Device choices** (settings, per device identity): remembered mode, color range. **Preview
  choices** (settings): last used device, presentation (Lowest latency, Follow the monitor),
  latency and frame rate bubble displays.
- **Preview state**: what the video area shows: video, opening, or one of the states of FR-019,
  plus the HDCP hint.
- **Preview measurement**: per-second frame counts (delivered, received, presented, dropped,
  repeated), latency values and frame ready → presented times; shown in the overlay (FR-016,
  FR-017) and reported by the statistics (FR-018).

## Success Criteria *(mandatory)*

### Measurable Outcomes

Latency budget (constitution, principle I). **What**: the delay of the preview against the
capture card's HDMI passthrough, glass to glass. **How**: the spike's method and tools
(tools/latency): the flash page in the PS5 browser at 1080p60, the passthrough monitor and the
preview monitor filmed side by side by a 240 fps camera (≈ 4.17 ms per camera frame), each clip
analyzed by `analyze.py`, at least 20 samples per run, median and p95, the app in fullscreen and
windowed, with the chrome idle. **Reference**: spike slint-layered 30.4 ms median (32.9 p95) on
Windows; 33.4 ms (38.1) on Linux with GPU clocks locked and 38.0 ms (47.3) without. The app shell
added the real interface and settings since; this spec must not lose what the spike measured.

- **SC-001**: Windows 11 (RTX 5070 Ti, C24FG7x at 144 Hz, Direct3D 12 + Mailbox), ezcap at
  1080p60 with the PS5: median at most 32 ms and p95 at most 35 ms, windowed and fullscreen.
- **SC-002**: Linux (CachyOS, KDE Plasma Wayland, RTX 3050), same card and source, with the
  driver's default power settings (no clocks locked): median at most 35 ms and p95 at most 40 ms.
- **SC-003**: With the real card at 1080p60, frame ready → presented meets the app shell's
  SC-001 (median at most 1.5 ms every second, maximum at most 3 ms in 95% of the seconds) and 99%
  of the seconds present 59–61 frames, on Windows and on Linux with default power settings, in
  every interface state of the app shell's SC-001. Stalls at presentation mode changes are
  counted apart and judged by SC-006.
- **SC-004**: On the maintainer's machines: a device opens and shows its first frame within 2 s
  of being chosen or of a mode change; a plugged or removed device appears in or leaves the list
  within 3 s; a disconnected device that comes back shows video again within 3 s of the system
  offering it.
- **SC-005**: Each state of FR-019 and the HDCP hint is shown when forced as in User Story 4, in
  both languages, and its action works; no device problem ever leaves a black, blank or frozen
  video area without a state.
- **SC-006**: The cause of the Windows focus change and fullscreen stalls is identified and
  recorded. If it is in the app's control: no stall over 50 ms (3 frames at 60 fps) at a focus
  change or when entering and leaving fullscreen on the native 1080p monitor, in 10 tries each.
  If not: the maintainer accepts the recorded finding.
- **SC-007**: The presented frame rate shown matches the statistics within ±1 per second; the
  measured latency matches the statistics within 1 ms; and on the reference setups of SC-001 and
  SC-002 the Total display (measured + estimated) is within 5 ms of the glass-to-glass median.
- **SC-008**: The maintainer accepts the preview on Windows 11 and on CachyOS KDE Plasma Wayland
  against User Stories 1 to 5, with the ezcap card and with a webcam (manual acceptance,
  constitution principle VII).

## Assumptions

- The look is the app shell's (Float direction): the device, mode and color range menus are the
  toolbar's dropdowns, built with the app shell's select and menu components; the toolbar slot the shell
  left empty gets its panel here. The audio dropdown is spec #4.
- The info bubbles here are a fixed first version (source, mode, frame rate, latency) shown with
  the chrome; spec #6 adds pinning, order, corners and the recording and live status (F42).
- The automatic modes are computed from the modes the driver reports; reading the incoming signal
  for Auto source depends on what each platform and device expose, which the plan researches. A
  UVC card that does not report it gets no Auto source (F3: shown only when reported).
- Capture uses the platforms' own camera paths through GStreamer, as the spike (Media Foundation
  on Windows, V4L2 on Linux); whether PipeWire's camera is used on Linux, at least for sandboxed
  packages, is decided in the plan with a latency measurement (spike open question).
- The latency levers the spike did not test (a 120 Hz source through the PC's HDMI output into
  the card, variable refresh on the preview monitor, independent flip confirmed by presentation
  tracing, zero-copy upload such as DMA-BUF on Linux) are tried in the plan where cheap; none is
  required to meet the budget, and results are recorded.
- The ezcap Live Gamer Ultra with the PS5 is the reference card and source; a webcam is used for
  webcam behavior (compressed modes, privacy light).
- Preview audio is spec #4: this spec shows no audio controls and plays no sound.
- The developer test video stays available behind its command-line option (app shell FR-004).

## Out of Scope

- Preview audio and pairing an audio input with the device (spec #4).
- Screenshots (spec #5), ambient light, hiding the preview and the full info overlay (spec #6).
- Screens and windows as sources (spec #7); they never appear in the device menu.
- Recording and streaming, and sharing one open device with them (specs #9, #10); this spec only
  keeps the device opened once and owned by the media pipeline (constitution, principle III).
- Device controls beyond mode and color range (brightness, exposure, the card's own settings).
