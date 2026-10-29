# Feature Specification: Stack Spike

**Feature Branch**: `feat/20261026-154808-stack-spike`

**Created**: 2026-10-26

**Status**: Draft

**Input**: User description: "Stack spike, before any feature work (see docs/roadmap.md #1). Two goals: (1) Preview latency: open a capture card in its native mode and measure glass-to-glass latency on three paths: GStreamer to a native window (D3D11/D3D12 on Windows, Vulkan or GL on Linux), GStreamer to a texture inside a Slint UI, and a browser getUserMedia preview as reference. Also play the card's audio on a chosen output with the lowest latency and measure it. (2) Slint UI: build 2-3 visual directions of the main screen (video taking most of the window, minimal chrome, capture info overlay, source picker, dock) to compare look and feel. Constraints: no outlines, very little text; Ninjoy prints in old/_refs/ninjoy-2026-10-23 are inspiration only; brand orange #FF7304 and logo from old/capturax-react/brand. Windows first, Linux checked too. Old implementation for reference: old/capturax-react."

## Context

This is a spike: its product is **evidence and a decision**, not features. The constitution
names Rust, GStreamer and Slint as the intended stack, with Slint "pending confirmation by the
first spike spec". This spec defines what must be measured and shown so the maintainer can
confirm or reject that stack. Because choosing technology is the goal, candidate technologies
are named here as the subject of the investigation; how they are wired together belongs to the
plan.

The spike code is throwaway and is not merged (constitution, principle VII). What is merged is
this spec and a findings report.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Know whether the preview is fast enough to play (Priority: P1)

The maintainer connects a capture card and wants to know, with numbers, how much delay each
candidate way of showing the video adds, so that the app is built on the fastest path that still
allows the interface they want.

**Why this priority**: Preview latency is the product (constitution, principle I). If the
preferred interface technology cannot show video fast enough, everything built on it is wasted.

**Independent Test**: Run the same measurement on every path with the same card, mode and
monitor, and compare the reported numbers.

**Acceptance Scenarios**:

1. **Given** a capture card in a native mode, **When** the latency of each path is measured with
   the same method, **Then** the report gives the median and the 95th percentile for each path:
   native window, video inside the Slint interface, browser preview (reference) and OBS
   (reference).
2. **Given** the same setup, **When** the measurement is repeated in at least two modes (1080p60
   and the card's highest frame-rate mode), **Then** the report shows whether the ranking of the
   paths changes with the mode.
3. **Given** the video-inside-Slint path, **When** a hardware encoder runs on the same video at
   the same time (as recording would), **Then** the report shows the preview latency and dropped
   frames with and without the encoder.
4. **Given** the fastest acceptable path on Windows, **When** it runs on the maintainer's Linux
   computer, **Then** the report states whether it works and its latency there.

---

### User Story 2 - Hear the card with the lowest delay (Priority: P2)

The maintainer wants to hear the console through the capture card's audio on a chosen output
(for example a headset) with as little delay as possible, and to know whether sound and image
arrive close enough together that no manual sync control is needed.

**Why this priority**: Playing requires hearing as well as seeing. The manual lip-sync control was
removed on the assumption that it is unnecessary; this story verifies that assumption.

**Independent Test**: Play the card's audio on a chosen output and measure its delay and its
offset to the video, without any interface.

**Acceptance Scenarios**:

1. **Given** the card's audio input and a chosen output device, **When** audio plays, **Then** the
   report gives its measured delay (median and 95th percentile).
2. **Given** audio and video playing together, **When** the offset between them is measured,
   **Then** the report states the offset and whether it is small enough to need no manual
   adjustment.
3. **Given** audio playing for at least 30 minutes, **When** the delay is measured at the start
   and at the end, **Then** the report shows whether it drifts and whether there were audible
   glitches.

---

### User Story 3 - Compare visual directions with live video (Priority: P3)

The maintainer wants to see two or three different visual directions for the main screen,
running with live video, so they can choose a direction (or a mix) before any feature work.

**Why this priority**: The previous app looked cluttered and verbose. Seeing real alternatives
early is cheaper than redesigning later, but it only matters once the video path is viable.

**Independent Test**: Open the prototype, switch between the directions and themes, and compare
them side by side with live video.

**Acceptance Scenarios**:

1. **Given** the prototype, **When** the maintainer switches between directions, **Then** each one
   shows the same elements: title bar with logo and window controls, video using most of the
   window, capture info overlay, source picker, and a dock with screenshot, record, stream,
   fullscreen and volume.
2. **Given** any direction, **When** it is shown in a dark and a light theme, **Then** both are
   complete and readable.
3. **Given** any direction, **When** its screens are reviewed, **Then** no element uses decorative
   outlines, and visible labels are at most three words, with longer explanations only in
   tooltips.
4. **Given** the maintainer's choice, **When** the spike ends, **Then** the chosen direction and the
   reasons are recorded in the findings report.

---

### User Story 4 - Confirm the interface can do what the product needs (Priority: P4)

The maintainer wants to know whether the interface technology can deliver specific elements
the product needs, so no feature spec runs into a dead end later.

**Why this priority**: Each of these was asked about explicitly; a "no" changes the design or the
stack.

**Independent Test**: Each capability is demonstrated in the prototype or reported as not
possible, with the reason.

**Acceptance Scenarios**:

1. **Given** the prototype, **When** each capability below is tried, **Then** the report marks it as
   working, working with limits (stated), or not possible:
   - ambient light glowing around the video with the video's colors;
   - capture info overlay over the video, semi-transparent, pinnable;
   - the same overlay detached as a floating window that stays on top of other apps;
   - resolution badges (HD, Full HD, QHD, 4K) with distinct colors;
   - a source picker with groups (devices, screens, windows) and badges;
   - depth effects similar in spirit to the Ninjoy prints (layered shadows, raised and sunken
     surfaces);
   - custom fonts and the brand logo;
   - switching language and theme without restart;
   - frameless window with its own title bar: move, resize, maximize, fullscreen, on Windows
     and Linux.

---

### Edge Cases

- No capture card is connected: the prototype says so instead of showing an empty or broken
  screen, and the visual directions can still be reviewed with a test video.
- The card is unplugged during a measurement: the run is discarded and noted, not reported as a
  result.
- A mode cannot be opened on one path (for example the browser does not offer it): the report
  marks that cell as not available rather than leaving it out.
- A hardware encoder is not available: the encoder scenario reports the fallback used.
- The Linux session type matters (Wayland or X11): the report states which one was tested.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The spike MUST measure preview latency on four paths: native window, video inside
  the Slint interface (a CPU-copy variant and, when possible, a GPU variant), browser preview
  (reference) and OBS (reference).
- **FR-002**: Every latency measurement MUST use the same method, card, mode and monitor for all
  paths, with at least 20 samples per path and mode, reporting median and 95th percentile.
- **FR-003**: Measurements MUST cover at least 1080p60 and the card's highest frame-rate mode.
- **FR-004**: The spike MUST measure preview latency and dropped frames on the
  video-inside-Slint path with and without a concurrent hardware encoder.
- **FR-005**: The spike MUST play the card's audio on a chosen output device and measure its
  delay, its offset to the video, and its drift over at least 30 minutes.
- **FR-006**: The spike MUST provide two or three visual directions of the main screen, switchable
  at runtime, each in a dark and a light theme, each showing the elements listed in User Story 3.
  *Revised 2026-10-29*: the maintainer chose the Float direction, laid out as the previous app,
  during the component review (T028); Frame and Rail are not built, so one direction with both
  themes is enough.
- **FR-007**: Visual directions MUST use the brand logo and orange `#FF7304` as the accent, no
  decorative outlines, and visible labels of at most three words.
- **FR-008**: The spike MUST report each capability listed in User Story 4 as working, working
  with stated limits, or not possible.
- **FR-009**: The fastest acceptable preview path and the chosen visual direction MUST be run on
  the maintainer's Linux computer, and the result reported.
- **FR-010**: The spike MUST produce a findings report in this feature directory with the
  measurements, the method, the environment (hardware, OS versions, session type), the
  capability results, the visual direction chosen and the decisions taken.
- **FR-011**: The spike code MUST stay on the spike branch; only the spec, plan, findings report
  and any documentation updates are merged.

### Key Entities

- **Path**: a way of showing the captured video (native window, inside Slint, browser, OBS).
- **Measurement**: path, mode, platform, samples, median, 95th percentile, notes.
- **Visual direction**: a named look for the main screen, with dark and light themes.
- **Capability result**: capability, outcome (working / limited / not possible), notes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For every path and mode, the report has at least 20 samples with median and 95th
  percentile, measured with one documented method.
- **SC-002**: Decision rule for the interface technology: if the video-inside-Slint path is within
  one frame (at the tested frame rate) of the native window path, Slint is confirmed; otherwise
  the report lists the alternatives and the maintainer decides.
- **SC-003**: The chosen path is no more than one frame slower than OBS in 1080p60.
- **SC-004**: Running a hardware encoder at the same time adds less than one frame of median
  latency and no more than 1% dropped frames to the preview.
- **SC-005**: Preview audio is no more than 45 ms ahead of or 125 ms behind the video (the
  detectability limits of ITU-R BT.1359), the maintainer notices no offset while playing, and the
  delay does not drift by more than 10 ms over 30 minutes.
- **SC-006**: The maintainer chooses a visual direction (or a combination) and every capability
  in User Story 4 has a recorded outcome.
- **SC-007**: The constitution's Slint status and the roadmap are updated to reflect the decision.

## Assumptions

- The maintainer's hardware is used: ezcap Live Gamer Ultra capture card fed by a PlayStation 5
  through its HDMI passthrough; Windows computer with an NVIDIA RTX 5070 Ti; separate Linux
  computer running CachyOS with an NVIDIA RTX 3050. The capture card is moved to the Linux
  computer for the Linux checks.
- The reference display is the 27G2G4 monitor (144 Hz) showing the card's HDMI passthrough; the
  preview runs fullscreen on the C24FG7x monitor (144 Hz, DisplayPort on the RTX 5070 Ti) right
  next to it. Where these documents say "TV", they mean this reference display.
- Glass-to-glass latency is measured by filming the passthrough display and the preview side by
  side with a Galaxy S23 FE in 240 fps slow motion (about 4 ms per camera frame, enough to resolve
  differences of one 60 fps frame), or an equivalent method chosen in the plan; the same method
  is used for every path.
- OBS, already installed, is used as an external reference only; no OBS code is involved.
- When no card is available, a test video stands in for the visual review only, never for
  latency results.
- The visual directions are prototypes: they show layout, depth and color, with no real settings,
  persistence or complete translations.
- The Ninjoy prints are inspiration; the directions do not copy them.

## Out of Scope

- Any product feature beyond what is needed to measure and show (recording files, streaming,
  settings, real device management).
- Production-quality code, tests or packaging for the spike prototype.
- macOS.
