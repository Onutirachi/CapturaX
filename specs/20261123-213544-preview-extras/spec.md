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
6. **Given** the ambient light is on at 1080p60 and at 1080p120, **When** the statistics are
   compared with it off, **Then** the frame ready → presented time and the dropped frames are
   within the limits of SC-001.

---

### User Story 2 - Hide the preview (Priority: P2)

A user who does not need to see the capture (for example while recording or streaming, or just
listening) hides the preview from the right dock: the video stops, the device is released (a
webcam's light turns off) and the app uses almost nothing. **Show preview** brings it back.

**Why this priority**: it saves resources and matters most once recording and streaming exist;
today it is mostly a way to release the device without closing the app.

**Independent Test**: with a webcam previewing, hide the preview and check that the webcam's
light turns off and the device can be opened by another app; show it again and check that it
reopens in the same mode.

**Acceptance Scenarios**:

1. **Given** a device is previewing, **When** the user clicks the preview button on the right
   dock, **Then** the video area shows the "Preview hidden" state (icon `preview_off`) with a
   **Show preview** action, and the device is released within 1 s.
2. **Given** the preview is hidden, **When** the user clicks **Show preview** (in the state or on
   the dock), **Then** the same device reopens in the same mode and range, as when picked in the
   device menu.
3. **Given** the preview is hidden, **When** the user picks a device in the device menu,
   **Then** the preview is shown again with that device.
4. **Given** the preview is hidden, **When** the user looks at the docks, **Then** the screenshot
   button is disabled (no video), as with any state.
5. **Given** recording or streaming uses the preview's device (specs #9 and #10), **When** the
   preview is hidden, **Then** the device stays open for them, only the preview stops, and the
   state says that recording or streaming continues.
6. **Given** the preview's audio is playing, **When** the preview is hidden, **Then** the sound
   stops too and the audio input and output are released (FR-012); **Show preview** brings back
   both video and sound.
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
   shows a pinned mark and stays visible after the controls hide, packed against its corner.
2. **Given** a bubble at the bottom right, **When** the user moves it to the top left, **Then** it
   shows in that corner, in the order chosen, and the bubbles left behind close the gap.
3. **Given** three bubbles in one corner, **When** the user moves one before another, **Then** the
   order changes at once and is kept after a restart.
4. **Given** the controls hide, **When** pinned and unpinned bubbles share a corner, **Then** only
   the pinned ones show, packed against the corner without gaps, in their order.
5. **Given** a notice appears (spec #3 FR-027) in a corner that holds bubbles, **Then** it never
   covers them while the controls show, and keeps spec #3's rules for the bottom right corner in
   every corner.
6. **Given** a small window (640 × 360) with bubbles in several corners, **When** the controls
   show, **Then** no bubble covers the toolbar, a dock or another bubble (app shell FR-010).
7. **Given** the bubbles were arranged, **When** the user wants the default back, **Then** one
   action restores it (all four at the bottom right, in today's order, none pinned).

---

### Edge Cases

- Ambient light with a video that fills the whole video area (no bars): nothing to light; no
  cost beyond what the plain background costs.
- Ambient light on a mostly black frame (a console starting, a dark scene): the glow fades to the
  background instead of drawing a grey haze.
- Ambient light while the window is resized or enters fullscreen: the glow follows the new bars
  in the same frame as the video.
- Hide preview while the device is still opening: the opening is cancelled and the device
  released.
- Hide preview while the device is disconnected: the "Preview hidden" state replaces the
  disconnected state; when the device returns it is not reopened until **Show preview**; its
  "reconnected" notice still shows.
- Hide preview with no device open ("no source" state): the preview button is disabled.
- A screenshot hotkey while the preview is hidden: refused like any state (screenshot spec),
  with its notice.
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
  (spec #3 FR-002a order), a toggle with an on state; its tooltip is "Ambient light". It is
  enabled whenever the preview shows video; while a state shows it keeps its value but nothing
  glows. The choice is saved and applies to every device.
- **FR-002**: With the ambient light on, the video background around the video MUST glow with
  the colors near the matching edge of the current frame, blurred and fading towards the window's
  edges, drawn over the video background (F56) and under the watermark (F57), the shadow (F58)
  and the video. The watermark shows clearer over it (F57). Off by default.
- **FR-003**: The glow MUST follow the video smoothly: a color change reaches the glow within
  about 0.5 s, eased, never flashing on a single frame (a camera flash, a scene cut); a dark frame
  dims it towards the plain background.
- **FR-004**: The ambient light MUST NOT add latency or dropped frames to the preview
  (constitution I): the video frame is never held for the glow; the glow may use an older frame.
  Budget in SC-001.
- **FR-005**: **Video shadow → With ambient light** (spec #3 FR-030) MUST draw the shadow exactly
  while the glow is drawn.

**Hide preview (F10)**

- **FR-006**: The right dock MUST gain the preview button, first in the dock (spec #3 FR-002a
  order: preview, screenshot, record, stream), a toggle: "Hide preview" while showing, "Show
  preview" while hidden (tooltips). It is disabled when no device is open.
- **FR-007**: Hiding the preview MUST stop presenting video and, unless recording or streaming
  uses the device, close the device within 1 s (a webcam's light turns off; another app can open
  it).
- **FR-008**: While hidden, the video area MUST show the "Preview hidden" state with the app
  shell's state component: icon `preview_off`, the text "Preview hidden" and a **Show preview**
  action. When recording or streaming uses the device it adds a line saying they continue
  (specs #9 and #10 supply the case).
- **FR-009**: **Show preview** (state or dock) MUST reopen the same device in its remembered mode
  and range; picking any device in the device menu MUST also show the preview.
- **FR-010**: While hidden, the bubbles that read the video (mode, frame rate, latency) MUST show
  nothing (they are hidden); the source bubble keeps the device's name.
- **FR-011**: The hidden state MUST be saved: an app started after the preview was hidden lists
  the devices but opens none (no webcam light, no device taken from another app, no preview
  sound) and shows the "Preview hidden" state at once. Showing the preview, or picking a device,
  opens it as usual (maintainer, 2026-11-23: some users only record or stream and should not
  have to hide the preview at every start).
- **FR-012**: The preview is video and audio together: hiding it MUST also stop the preview's
  audio (F12–F14) and release its input and output, unless recording or streaming uses that
  input; showing it again reopens the audio as at start (paired or chosen input, chosen output,
  volume and mute kept). While hidden the volume button keeps its level and mute but plays
  nothing (maintainer, 2026-11-23; decides product.md's "Hide preview and its audio").

**Info overlay (F42)**

- **FR-013**: Each info bubble (today: source, mode, frame rate, latency; later: recording and
  live status, specs #9 and #10) MUST have a corner (one of the four of the video area), a place
  in its corner's order, and a pinned flag. Defaults: all at the bottom right, in today's order
  (source, mode, frame rate, latency), none pinned, which is today's look.
- **FR-014**: While the controls show, all bubbles MUST show in their corners; pinned ones carry
  a pinned mark. While the controls are hidden, only the pinned ones show, packed against their
  corner in their order.
- **FR-015**: The user MUST be able to pin and unpin a bubble, move it to another corner and
  change its place in the order, with the mouse and with an alternative that needs no dragging
  (the bubble's menu, spec #3 FR-016's settings button). The exact gestures and the menu's look
  are decided on the mockup page before the plan.
- **FR-016**: The arrangement MUST be saved (one for the app, not per device) and applied at once.
  A **Reset bubbles** action (in the settings panel's Preview category) restores the defaults.
- **FR-017**: Bubbles in a corner MUST lay out like today's row (spec #3 FR-002a): a row from
  the corner along the bottom or top edge, fitting between the docks; a column when the row does
  not fit. A top corner's row never goes under the toolbar: when there is no room beside it, the
  row goes below the toolbar.
- **FR-018**: The notices (spec #3 FR-027, FR-033) MUST keep their rules in every corner: in a
  corner that holds bubbles they join the bubbles' group at the corner while the controls are
  hidden and move past them while the controls show. Only the pinned bubbles count while the
  controls are hidden.
- **FR-019**: All new texts (button tooltips, state, menu items, **Reset bubbles**) MUST be in
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
  (spec #3 FR-018), on Windows and on Linux (CachyOS). The glass-to-glass check of spec #3
  (SC-007) is repeated once with the ambient light on, within its noise.
- **SC-002**: Hiding the preview releases the device within 1 s (webcam light off; another app
  opens it), and **Show preview** shows video again within the device's normal opening time
  (spec #3) plus 0.5 s.
- **SC-003**: With the preview hidden and nothing else running, the app's CPU and GPU use drops
  to that of the "no source" state (within 1 percentage point), measured over 30 s.
- **SC-004**: A bubble arrangement (one pinned, one in another corner, order changed) is set in
  under 30 s with the mouse, survives a restart unchanged, and **Reset bubbles** restores the
  default in one action.
- **SC-005**: At 640 × 360, 800 × 520, 1280 × 720 and fullscreen 1920 × 1080, in both themes and
  both languages, with bubbles in all four corners: no bubble overlaps the toolbar, a dock,
  another bubble or a notice while the controls show.

## Assumptions

- The ambient light is one on/off choice with a fixed look (no intensity or size settings):
  constitution II, an option only for a demonstrated need. The look (blur, reach, strength) is
  chosen on the mockup page.
- The ambient light is drawn by the app's own renderer from the presented frame; it is part of
  the preview only, never of screenshots, recordings or streams. The viewer page's ambient light
  (F38) is spec #10's.
- The recording and live status bubbles and the "recording and streaming continue" line of the
  hidden state are defined here and delivered by specs #9 and #10; until then nothing uses the
  device besides the preview, so hiding always releases it.
- Hiding the preview has no hotkey; F50 defines only screenshot, record and stream keys.
- The bubbles' arrangement is one for the app, not per device: the bubbles describe the preview,
  whatever device it shows.
- No bubble can be hidden for good: unpinned bubbles show only with the controls, which is the
  way to keep them out of sight. Adding "never show" waits for a demonstrated need.
- The detached info window considered before the stack spike stays dropped.
- Visual details (the glow's look, the dock icons, the pinned mark, the move gestures, the
  hidden state) are decided on an HTML mockup page with the app's palette and both themes before
  the plan, as in the previous specs.
