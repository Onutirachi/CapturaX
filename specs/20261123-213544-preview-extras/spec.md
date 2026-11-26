# Feature Specification: Preview Extras

**Feature Branch**: `feat/20261123-213544-preview-extras`

**Created**: 2026-11-23

**Status**: Draft

**Input**: User description: "Preview extras (see docs/roadmap.md #6): covers F9, F10 and F42 from
docs/product.md: ambient light, hiding the preview, and the capture info overlay (bubbles that can
be pinned, reordered and anchored to any corner; the detached window was dropped in the stack
spike). Also decide the open "Hide preview and its audio" question from docs/product.md (To
reconsider)."

## Context

Three additions to the preview built in spec #3 (device preview):

- **Ambient light (F9)**: the area around the video glows with the video's colors, over the
  video background (F56), making the watermark (F57) clearer, under the video shadow (F58). It
  also gives **Video shadow → With ambient light** (spec #3 FR-030) its meaning.
- **Hide preview (F10)**: stop showing the video to save resources and release the device,
  unless recording or streaming uses it. Recording (spec #9) and streaming (spec #10) do not
  exist yet, so this spec defines the rule for them and the tests cover the case without them.
- **Info overlay (F42)**: today's info bubbles (spec #3 FR-016: source, mode, frame rate,
  latency) sit in one row at the bottom right and show only with the controls. This spec lets
  each one be pinned (shown also while the controls are hidden), moved in the order and anchored
  to any of the four corners. The recording and live status bubbles named in F42 are added by
  specs #9 and #10 into this system.

The docks already plan these buttons (spec #3 FR-002a): the left dock holds fullscreen, ambient
light, volume and settings; the right dock holds preview, screenshot, record and stream.

## Clarifications

### Session 2026-11-23

- Q: While the video area shows a state (no signal, opening, disconnected, preview hidden), is
  the ambient light button still clickable? → A: Yes, always clickable; in a state it only
  changes the saved value and nothing glows until video returns (FR-001).
- Q: Does the volume button stay usable while there is no sound to play (No audio, no input, a
  state, preview hidden)? → A: Yes, like the ambient light: always usable, it changes the
  saved level and mute (FR-012). The roadmap #17 note to dim it is dropped.
- Q: Should the tray menu gain Show preview / Hide preview? → A: No; hiding matters only with
  the window open, and closing to the tray already releases the device (spec #3).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ambient light around the video (Priority: P1)

A user playing in a window, or on a monitor wider than the capture, turns on the ambient light:
the bars around the video glow softly with the colors at the video's edges, following the game
as it changes, without any added delay in the preview.

**Why this priority**: it is the most visible extra and it completes the look of the video area
(background, watermark, shadow) built in spec #3.

**Independent Test**: preview the card at 1080p60 in a window wider than 16:9, turn the ambient
light on from the left dock, and compare the per-second statistics with it on and off.

**Acceptance Scenarios**:

1. **Given** the preview shows video with bars around it, **When** the user clicks the ambient
   light button on the left dock, **Then** the bars glow with the colors near the video's edges
   and the button shows the on state.
2. **Given** the ambient light is on, **When** the scene changes color (for example a red screen
   to a blue one), **Then** the glow follows within about half a second, smoothly, without
   flicker.
3. **Given** the ambient light is on, **When** the user restarts the app, **Then** it is still on.
4. **Given** the ambient light is on, **When** the video area shows a state (no signal, opening,
   disconnected) or the preview is hidden, **Then** there is no glow: the plain background (and
   the watermark) shows.
5. **Given** **Video shadow** is **With ambient light**, **When** the ambient light is turned on
   and off, **Then** the shadow appears and disappears with it.
6. **Given** the ambient light is on, **When** the user rests the pointer on its button and
   moves the slider in the popover, **Then** the glow gets weaker or stronger at once, and the
   intensity is the same after a restart.
7. **Given** the ambient light is on at 1080p60 and at 1080p120, **When** the statistics are
   compared with it off, **Then** the frame ready → presented time and the dropped frames are
   within the limits of SC-001.

---

### User Story 2 - Hide the preview (Priority: P2)

A user who does not need to see the capture (for example while recording or streaming) hides
the preview from the right dock: the video stops, the device is released (a webcam's light turns
off) and the app uses almost nothing. The same button brings it back.

**Why this priority**: it saves resources and matters most once recording and streaming exist;
today it is mostly a way to release the device without closing the app.

**Independent Test**: with a webcam previewing, hide the preview and check that the webcam's
light turns off and the device can be opened by another app; show it again and check that it
reopens in the same mode.

**Acceptance Scenarios**:

1. **Given** a device is previewing, **When** the user clicks the preview button on the right
   dock, **Then** the video area shows the "Preview hidden" state (icon and text, no action) and
   the device is released within 1 s.
2. **Given** the preview is hidden, **When** the user clicks the preview button again, **Then** the same device reopens in the same mode and range, as when picked in the
   device menu.
3. **Given** the preview is hidden, **When** the user picks a device in the device menu,
   **Then** the preview stays hidden and the toolbar shows that device, which opens when the
   preview is shown (FR-010a); **Open** on a "connected" notice shows the preview with its device.
4. **Given** the preview is hidden, **When** the user looks at the docks, **Then** the screenshot
   button is disabled (no video), as with any state.
5. **Given** recording or streaming uses the preview's device (specs #9 and #10), **When** the
   preview is hidden, **Then** the device stays open for them, only the preview stops, and the
   state says that recording or streaming continues.
6. **Given** the preview's audio is playing, **When** the preview is hidden, **Then** the sound
   stops too and the audio input and output are released (FR-012); showing the preview brings
   back both video and sound.
7. **Given** the preview was hidden, **When** the app starts again, **Then** it shows "Preview
   hidden" without opening any device (FR-011).

---

### User Story 3 - Arrange the info bubbles (Priority: P2)

A user who plays with the controls hidden wants to keep the latency in sight, in the top left
corner, and the rest out of the way. They pin the latency bubble and move it to the top left;
the other bubbles stay at the bottom right and show only with the controls.

**Why this priority**: the bubbles exist already; arranging them is a refinement, but pinning is
what lets the measurements be read while playing.

**Independent Test**: pin one bubble, move another to another corner, change the order, hide the
controls and restart the app; the arrangement is kept and only the pinned bubble shows with the
controls hidden.

**Acceptance Scenarios**:

1. **Given** the controls are showing, **When** the user pins the latency bubble, **Then** it
   shows a pin before its text and stays visible after the controls hide, packed against its corner.
2. **Given** a bubble at the bottom right, **When** the user drags it to the top left, **Then**
   the target corner glows while dragging, an orange slot shows where it will land, and on release it sits in
   that corner, in the order chosen, and the bubbles left behind close the gap.
3. **Given** three bubbles in one corner, **When** the user drags one before another, **Then** the
   order changes at once and is kept after a restart.
4. **Given** the controls hide, **When** pinned and unpinned bubbles share a corner, **Then** only
   the pinned ones show, packed against the corner without gaps, in their order.
5. **Given** a notice appears (spec #3 FR-027) in a corner that holds bubbles, **Then** it never
   covers them while the controls show, and keeps spec #3's rules for the bottom right corner in
   every corner.
6. **Given** a small window (640 × 360) with bubbles in several corners, **When** the controls
   show, **Then** no bubble covers the toolbar, a dock or another bubble (app shell FR-010).
7. **Given** the bubbles were arranged, **When** the user wants the default back, **Then** one
   action restores it: **Reset** in the settings panel's Preview category (all four at the bottom
   right, in today's order, none pinned).

---

### Edge Cases

- Ambient light with a video that fills the whole video area (no bars): nothing to light; no
  cost beyond what the plain background costs.
- Ambient light on a mostly black frame (a console starting, a dark scene): the glow shows the
  frame's own dark colors; it never fades out to the plain background (maintainer 2026-11-24).
- Ambient light while the window is resized or enters fullscreen: the glow follows the new bars
  in the same frame as the video.
- Hide preview while the device is still opening: the opening is cancelled and the device
  released.
- Hide preview while the device is disconnected: the "Preview hidden" state replaces the
  disconnected state; when the device returns it is not reopened until the preview is shown; its
  "reconnected" notice still shows.
- The window is shown again from the tray while the preview is hidden: it stays hidden and no
  device is opened.
- Hide preview with no device open ("no source" state): the preview button is disabled.
- Start while hidden and the device hidden last is not connected: it stays hidden, the toolbar
  shows the start order's device, and showing opens that one; the last used device coming back
  meanwhile only joins the menu, as when shown (FR-010a).
- The device the toolbar shows leaves while hidden: the Device select names it as missing, as
  for a disconnected device; showing waits for it (Disconnected).
- A screenshot hotkey while the preview is hidden: refused like any state (screenshot spec),
  with its notice.
- The controls' hide timer runs out while a bubble is being dragged: the controls stay shown
  until the drag ends.
- A pinned bubble whose value is unknown (latency before the first frame): it shows its
  placeholder, as today.
- Bubbles in a top corner next to the toolbar: they never sit under the toolbar; when the toolbar
  leaves no room beside it, they go below it.
- A corner holding all bubbles in a small window: they stack in a column, as the bottom right
  does today (spec #3 FR-002a).
- The settings panel open: bubbles and glow stay under it and its backdrop, as the video does.

## Requirements *(mandatory)*

### Functional Requirements

**Ambient light (F9)**

- **FR-001**: The left dock MUST gain the ambient light button, between fullscreen and volume
  (spec #3 FR-002a order), a toggle with an on state (the soft orange chip of a toggled tool),
  icon a light bulb (Tabler `bulb`, its rays only while it glows, on and above 0 %, as the
  volume's icon shows mute at 0 %: maintainer 2026-11-24); its tooltip is "Ambient light". It
  is always clickable: while a state shows (no signal, opening, disconnected, preview hidden) it
  changes the saved value and nothing glows until video returns. The choice is saved and
  applies to every device.
- **FR-001a**: The ambient light button MUST have an intensity popover that works as the
  volume button's (spec #4 FR-011: mouse, touch and keyboard alike): a slider from 0 to 100 %
  with its value and nothing else; a click still turns the light on and off, resting the
  pointer opens the popover, and the wheel over the button changes the intensity by 5 %. The
  intensity scales the glow's opacity (100 % is the tuned look, 0 % draws nothing). Default
  100 %. Moving the slider or the wheel while the light is off turns it on, so the change shows.
  Saved, applies to every device (maintainer 2026-11-24: one fixed look was right in one game
  and too bright in another).
- **FR-002**: With the ambient light on, the whole video background around the video MUST glow
  with the video's colors: a much reduced, strongly blurred copy of the frame stretched over the
  video area, so each bar takes the colors of the video's nearest side, strong and diffuse
  (variant "Fill" of the mockup page, maintainer 2026-11-23), drawn over the video background (F56) and under the watermark (F57), the shadow (F58)
  and the video. The watermark shows clearer over it (F57). Off by default.
- **FR-003**: The glow MUST follow the video smoothly: a color change reaches the glow within
  about 0.5 s, eased, never flashing on a single frame (a camera flash, a scene cut). It always
  shows the video's colors, bright or dark: no luminance gate.
- **FR-004**: The ambient light MUST NOT add latency or dropped frames to the preview
  (constitution I): the video frame is never held for the glow; the glow may use an older frame.
  Budget in SC-001.
- **FR-005**: **Video shadow → With ambient light** (spec #3 FR-030) MUST draw the shadow while
  the ambient light is on and video shows, fading in and out with the glow's toggle.

**Hide preview (F10)**

- **FR-006**: The right dock MUST gain the preview button, first in the dock (spec #3 FR-002a
  order: preview, screenshot, record, stream), a toggle with the settings' Preview category icon
  (`device_tv`) while showing and the same icon struck through (`device_tv_off`) on the toggled
  chip while hidden;
  tooltips "Hide preview" and "Show preview". It is disabled when there is no device to show
  (the "no devices" and "no source" states); a device's own states (opening, no signal,
  disconnected, error) keep it enabled (maintainer, 2026-11-23).
- **FR-007**: Hiding the preview MUST stop presenting video and, unless recording or streaming
  uses the device, close the device within 1 s (a webcam's light turns off; another app can open
  it).
- **FR-008**: While hidden, the video area MUST show the "Preview hidden" state with the app
  shell's state component: icon `device_tv_off` (the dock button's; the "no source" state uses it too, the text tells them apart), the text "Preview hidden" and
  no action: the dock button shows it again (maintainer, 2026-11-23: less on screen; this is a
  state the user chose, not an empty or error state). When recording or streaming uses the device it adds a line saying they continue
  (specs #9 and #10 supply the case).
- **FR-009**: Showing the preview (the dock button) MUST reopen the device the toolbar shows in its
  remembered mode and range; **Open** on a "connected" notice MUST also show the preview, with that
  device. Picking a device in the device menu does not show it (FR-010a).
- **FR-010**: While hidden, no info bubble MUST show, the source bubble included: the bubbles
  belong to the video (maintainer, 2026-11-24; a lone source bubble that hid and showed again
  around each hide or show was not worth its own rule).
- **FR-010a**: While hidden, the toolbar MUST show and keep working for the device that showing
  would open (the one hidden; after a start while hidden, the start order's device): **Device**
  names it, **Mode** offers its modes with its remembered one marked, **Color range** its range,
  and **Audio** its input choice and the output. Picking a device, a mode, a range or an audio
  choice saves it as usual, opens nothing and keeps the preview hidden; showing then opens with
  it. Everything else behaves as when shown, as if the toolbar's device were open: the start
  order picks it once, a device that comes only joins the menu, the toolbar's device leaving is
  waited for. Hidden is one switch for the app, not per device: it lasts through restarts and
  device changes until the user shows the preview (maintainer, 2026-11-24: empty selects looked
  broken; hiding only hides, all else as before).
- **FR-010b**: The **Color range** select on Auto MUST name a range ("Auto (Full)", "Auto
  (Limited)") only while video shows, from the first frame, with the range found in it; with no
  video (hidden, opening, a device's state) it reads "Auto" alone. This changes spec #3's label,
  which named the driver's declared range while opening and Limited with no device (maintainer,
  2026-11-24: no range is known without video).
- **FR-011**: The hidden state MUST be saved: an app started after the preview was hidden lists
  the devices but opens none (no webcam light, no device taken from another app, no preview
  sound) and shows the "Preview hidden" state at once. Showing the preview opens the toolbar's
  device as usual (maintainer, 2026-11-23: some users only record or stream and should not
  have to hide the preview at every start).
- **FR-012**: The preview is video and audio together: hiding it MUST also stop the preview's
  audio (F12–F14) and release its input and output, unless recording or streaming uses that
  input; showing it again reopens the audio as at start (paired or chosen input, chosen output,
  volume and mute kept). While hidden the volume button stays usable (it changes the saved
  level and mute) but plays nothing (maintainer, 2026-11-23; decides product.md's "Hide preview and its audio").

**Info overlay (F42)**

- **FR-013**: Each info bubble (today: source, mode, frame rate, latency; later: recording and
  live status, specs #9 and #10) MUST have a corner (one of the four of the video area), a place
  in its corner's order, and a pinned flag. Defaults: all at the bottom right, in today's order
  (source, mode, frame rate, latency), none pinned, which is today's look.
- **FR-014**: While the controls show, all bubbles MUST show in their corners; pinned ones show a
  small orange pin (Tabler `pin`, filled) in their top right corner, as the stack spike
  (maintainer, 2026-11-25). While the controls are hidden, only the pinned ones show, packed against their
  corner in their order, without the pin.
- **FR-015**: Moving a bubble is click and drag, the only way (maintainer, 2026-11-23): the
  pointer shows the grab hand over a bubble and the closed hand from the press on, before it moves (Linux: the
  cursor theme's own; Windows, which has none: the maintainer's drawings in
  app/ui/assets/cursors, 22 px in a 32 px cursor at 100 %, maintainer 2026-11-26); the bubble
  follows the pointer, the corner it would land in (the pointer's quadrant) glows orange, fading
  towards the middle, the bubbles there make room and a solid orange slot the bubble's size
  shows its place in that corner's order, and releasing puts it there (maintainer, 2026-11-25:
  variant F, "slot + glow", of the drop zones mockup
  https://claude.ai/artifact/D4WycMtijeV9PbFtqTgEkC, slot without a dashed border). A press without
  movement is not a drag. The hover controls (spec #3 FR-016) hold a **Pin** toggle (the pin
  icon, on chip while pinned; tooltip "Pin" / "Unpin") and, for bubbles with display choices, the
  display button; no corner or order items in any menu. The pointer and the keyboard use the
  same controls (maintainer, 2026-11-25): every bubble takes the focus, and Enter, Space, Up or
  Down opens its hover controls with the focus on **Pin**; Tab, Shift+Tab, Left and Right move
  between the controls, Enter or Space presses one, and Esc, or Tab past the last control (Shift+Tab
  before the first), closes them and gives the focus back to the bubble.
- **FR-016**: The arrangement MUST be saved (one for the app, not per device) and applied at once.
  The settings panel's Preview category MUST have an **Info bubbles** row with a **Reset**
  button that restores the defaults (FR-013), with no text about what the default is
  (maintainer, 2026-11-23).
- **FR-017**: Bubbles in a corner MUST lay out like today's row (spec #3 FR-002a): a row from
  the corner along the bottom or top edge, fitting between the docks; a column when the row does
  not fit. A top corner's row never goes under the toolbar: when there is no room beside it, the
  row goes below the toolbar. Bubbles slide, never fade, as in the stack spike (maintainer,
  2026-11-25): when the controls hide, the unpinned bubbles slide out past the nearest edge (a top
  corner's up, a bottom corner's down), each keeping its place along the edge, and then the pinned
  ones close up against their corner; when the controls show, the pinned ones make room first and
  then the others slide in (at once when none is pinned). A bubble that gains or loses its value
  slides in or out the same way, and the others of its corner slide to their new places. A new
  source's bubbles slide in together once the video's fade-in (spec #3, 150 ms) is over and its
  first second's frame rate and latency are known, so the slide is seen and nothing enters twice
  (maintainer, 2026-11-25).
- **FR-018**: The notices (spec #3 FR-027, FR-033) MUST keep spec #3's bottom right rules in every
  corner: the event notice joins the bubble row of its edge at the corner when the row reaches the
  corner and has room for it (the bubbles slide aside); else it moves past the bubbles of its edge
  (above them at the bottom, below them at the top), or covers them when that would leave the video
  area. The settings notice stacks past the event notice and moves past the bubbles likewise. Only
  the pinned bubbles count while the controls are hidden.
- **FR-019**: All new texts (button tooltips, the state, **Info bubbles** and **Reset**) MUST be in
  English and Portuguese (Brazil), and every new control MUST work in both themes and down to
  640 × 360.

### Key Entities

- **Ambient light setting**: on or off; saved; applies to every device.
- **Preview visibility**: shown or hidden; saved (FR-011).
- **Bubble arrangement**: per bubble kind (source, mode, frame rate, latency; later recording,
  live): corner, place in the corner's order, pinned. Saved; one for the app. Each bubble's
  display choice (spec #3 FR-016a, FR-017, FR-017a) stays as it is.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Latency budget (constitution I). With the ambient light on, at 1080p60 and at
  1080p120 on the ezcap card, windowed with bars, 60 s each: the median frame ready → presented
  time grows by at most 0.5 ms and the p95 by at most 1 ms over the same run with it off, and no
  frame is dropped that is not dropped with it off. Measured with the app's per-second statistics
  (spec #3 FR-018), on Windows and on Linux (CachyOS). The same limits hold for a run with the
  frame rate and latency bubbles pinned and the controls hidden (they update each second). The
  glass-to-glass check of spec #3 (SC-007) is repeated once with the ambient light on, within
  its noise.
- **SC-002**: Hiding the preview releases the device within 1 s (webcam light off; another app
  opens it), and showing the preview again shows video within the device's normal opening time
  (spec #3) plus 0.5 s.
- **SC-003**: With the preview hidden and nothing else running, the app's CPU and GPU use drops
  to that of the "no source" state (within 1 percentage point), measured over 30 s.
- **SC-004**: A bubble arrangement (one pinned, one in another corner, order changed) is set in
  under 30 s with the mouse, survives a restart unchanged, and **Reset** restores the
  default in one action.
- **SC-005**: At 640 × 360, 800 × 520, 1280 × 720 and fullscreen 1920 × 1080, in both themes and
  both languages, with bubbles in all four corners: no bubble overlaps the toolbar, a dock,
  another bubble or a notice while the controls show.

## Assumptions

- The ambient light has an on/off choice and one intensity (FR-001a); its look is otherwise
  fixed (no size, color or blur settings): constitution II, an option only for a demonstrated
  need. The intensity's need was shown on 2026-11-24 (Astro's Playroom and GTA V).
- The ambient light is drawn by the app's own renderer from the presented frame; it is part of
  the preview only, never of screenshots, recordings or streams. The viewer page's ambient light
  (F38) is spec #10's.
- The recording and live status bubbles and the "recording and streaming continue" line of the
  hidden state are defined here and delivered by specs #9 and #10; until then nothing uses the
  device besides the preview, so hiding always releases it.
- Hiding the preview has no hotkey; F50 defines only screenshot, record and stream keys. It has
  no tray menu entry either: closing the window to the tray already releases the device
  (spec #3), and hiding matters only with the window open.
- The bubbles' arrangement is one for the app, not per device: the bubbles describe the preview,
  whatever device it shows.
- No bubble can be hidden for good: unpinned bubbles show only with the controls, which is the
  way to keep them out of sight. Adding "never show" waits for a demonstrated need.
- The detached info window considered before the stack spike stays dropped.
- Visual decisions taken on the mockup page (https://claude.ai/artifact/C7uffxbE8YBmFXSGkNNbPz,
  maintainer 2026-11-23): D1 Fill glow, D2 bulb, D3 the Preview category's `device_tv` icon, D4 no
  action in the hidden state, D5 drag only with pin and display in the hover controls, D6 pin
  shown only with the controls, D7 Reset in the Preview category.
- Moving bubbles has no keyboard alternative (drag only, D5); keyboard users keep pin and the
  display menu.
