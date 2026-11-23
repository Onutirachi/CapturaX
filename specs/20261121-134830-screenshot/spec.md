# Feature Specification: Screenshot

**Feature Branch**: `feat/20261121-134830-screenshot`

**Created**: 2026-11-21

**Status**: Approved (maintainer, 2026-11-22)

**Input**: User description: "Screenshot (see docs/roadmap.md #5): covers F19 from docs/product.md."

## Context

The device preview (spec #3) shows the console's image and the preview audio (spec #4) plays its
sound. This spec adds the first feature of the right dock: saving the current image of the
preview as a PNG file at the device's native resolution and copying it to the clipboard (F19,
group "E. Screenshot" of docs/product.md).

F19 is triggered by **Ctrl+F8**, a hotkey of F50: it must work while another app (a game, a
browser over the preview) has the focus. The roadmap had the global hotkey framework in spec #9
(Recording). **Decision (maintainer, 2026-11-21)**: the framework comes in this spec, with the
screenshot key and its setting; spec #9 only adds Ctrl+F10 and spec #10 Ctrl+F9. A screenshot
taken by clicking a button while playing is not usable, so the key belongs to the feature.

**Decision (maintainer, 2026-11-21)**: the folder is a setting, **Pictures/CapturaX** by
default (F19 named only the default), as recording has its own folder (F20).

The previous version (old/capturax-react) had a dock button and a key (F9), saved
`CapturaX <timestamp>.png` in Pictures/CapturaX, numbered names taken in the same second, and
showed "Screenshot saved and copied: <file>". This spec keeps that behavior where it fits.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Save the moment while playing (Priority: P1)

The user plays the console through the preview. Something worth keeping happens; they press
Ctrl+F8 without leaving the game. The image the preview shows at that moment is saved as a PNG at
the device's native resolution in the screenshots folder and copied to the clipboard; a short
capture sound on their headset, a quick white flash on the video's edges and a notice confirm
it, so they know it worked without looking
away from the game. The preview does not stutter.

**Why this priority**: it is the whole of F19; the key is how the user takes a screenshot while
their hands are on the controller or the keyboard.

**Independent Test**: with the PS5 on the ezcap card at 1080p60 (and at 4K30, 1080p120 where the
card offers them), press Ctrl+F8 with CapturaX focused, then with another app focused over it;
check the file (size equal to the mode's resolution, the same image as the preview), paste it in
an image editor and a chat app, and check the preview's frame counters show no dropped frame
(SC-003).

**Acceptance Scenarios**:

1. **Given** the preview shows a device's image, **When** the user presses Ctrl+F8, **Then** a
   PNG of the frame shown at that moment is saved in the screenshots folder at the mode's native
   resolution, copied to the clipboard, the capture sound plays on the preview's output (FR-010),
   the video's outline flashes (FR-010a) and a notice says so, within the time of SC-001.
2. **Given** the preview shows a device's image and another app has the focus (CapturaX's window
   visible, behind or beside it), **When** the user presses Ctrl+F8, **Then** the same happens,
   the focus stays where it was, and the confirmation is a system notification.
3. **Given** the window is smaller than the image, fullscreen, or the controls and info bubbles
   are visible, **When** a screenshot is taken, **Then** the file has the native resolution and
   contains only the device's image: no interface, no bubbles, no notice, no letterbox bars.
4. **Given** the user holds Ctrl+F8 down, **When** the key repeats, **Then** only one screenshot
   is taken; pressing it again takes another.
5. **Given** two screenshots are taken within the same second, **When** they are saved, **Then**
   neither overwrites the other.

---

### User Story 2 - Screenshot button (Priority: P1)

The user clicks the screenshot button on the right dock and gets the same result as the key.
The button's tooltip names the action and its current key.

**Why this priority**: the button makes the feature discoverable and is the way to take one
with the mouse; the right dock appears for the first time with it (device preview FR-002a).

**Independent Test**: click the button with the preview showing an image and in every state of
the video area (no device, no signal, connecting, error); check what each one does.

**Acceptance Scenarios**:

1. **Given** the preview shows an image, **When** the user clicks the screenshot button, **Then**
   the result is the same as Ctrl+F8 (US1 scenario 1).
2. **Given** the video area shows a state instead of an image (no device, connecting, no signal,
   device error), **When** the user looks at the button, **Then** it is disabled, and its tooltip
   still names the action and the key.
3. **Given** the user hovers the button, **When** the tooltip shows, **Then** it reads
   "Screenshot (Ctrl+F8)", with the key the user set, or only "Screenshot" when no key is set.

---

### User Story 3 - Change the key (Priority: P2)

The user's game or another program already uses Ctrl+F8, or they prefer another key. In the
settings they press the new key for **Screenshot**; it works at once, globally, and is kept for
the next start.

**Why this priority**: F50 says keys can be changed; a key taken by another program is the user's
to solve, and this is how (F50).

**Independent Test**: set the key to Ctrl+Shift+S, check the old key no longer takes screenshots
and the new one does, inside and outside the window; restart and check it is kept; clear it and
check no key takes screenshots and the button's tooltip has no key.

**Acceptance Scenarios**:

1. **Given** the settings are open, **When** the user opens the **Hotkeys** category, **Then** it
   lists **Screenshot** with its current key (Ctrl+F8 by default).
2. **Given** the user starts changing the key, **When** they press a combination allowed by
   FR-016 (a function key alone, or a key with a modifier), **Then** it becomes the screenshot key at once and is saved; Esc cancels the change.
3. **Given** the user is changing the key, **When** they press a combination another CapturaX
   action already uses, **Then** it is refused and the row says which action has it.
4. **Given** the user clears the key, **When** they press the old key, **Then** nothing happens;
   **Reset** puts back Ctrl+F8.
5. **Given** a key was set, **When** the app starts again, **Then** the same key works.

---

### User Story 4 - Choose the folder (Priority: P2)

The user wants the screenshots on another drive or in a synced folder. In the settings they pick
the folder; new screenshots go there. They can open the folder from the settings and from the
saved notice.

**Why this priority**: the default folder works without configuration; the setting is for users
who keep their media elsewhere (maintainer, 2026-11-21).

**Independent Test**: pick a folder on another drive, take a screenshot, check it is there; open
it from the notice and from the settings; restart and check the folder is kept; reset to the
default.

**Acceptance Scenarios**:

1. **Given** the settings are open, **When** the user looks at the screenshots folder row,
   **Then** it shows the current folder (Pictures/CapturaX by default, shortened when long, full
   path in its tooltip) with **Change** and **Open**.
2. **Given** the user clicks **Change**, **When** they pick a folder in the system's folder
   dialog, **Then** the next screenshots are saved there and the choice is kept across restarts.
3. **Given** a screenshot was saved, **When** the user clicks **Show** on its notice (in the app
   or the system notification), **Then** the system's file manager opens the folder with the file
   selected (or the folder alone where the file manager cannot select a file).
4. **Given** the user chose a folder, **When** they click **Reset**, **Then** the default folder
   is used again.

---

### User Story 5 - Clear failures (Priority: P3)

When a screenshot cannot be taken, saved or copied, the user is told what happened and what to
do, instead of nothing happening.

**Why this priority**: a key press that silently does nothing looks like a broken key
(constitution principle II, explicit error states).

**Independent Test**: force each case of the Edge Cases (no image, folder not writable, disk
full, clipboard taken) and check the notice and its action.

**Acceptance Scenarios**:

1. **Given** no image is shown (no device, a state in the video area, the window hidden to the
   tray), **When** the user presses the key, **Then** nothing happens: no sound, no flash, no
   notice, no file (only a log line). The missing click tells the user no screenshot was taken
   (maintainer, 2026-11-22).
2. **Given** the folder cannot be written (removed drive, no permission, disk full), **When** a
   screenshot is taken, **Then** a notice says "Couldn't save the screenshot" with the reason in
   a few words and **Settings** (opens the folder row); the image is still copied to the
   clipboard and the notice says so.
3. **Given** the clipboard cannot be written, **When** a screenshot is taken, **Then** the file
   is saved and the notice says "Screenshot saved, not copied".

---

### Edge Cases

- **The window is hidden to the tray**: the device is closed (device preview), so there is no
  image; the key does nothing (US5 scenario 1), so a key pressed in another app is not answered
  by a system notification. The device is not opened for a screenshot.
- **Window minimized**: the device stays open (device preview); the screenshot is taken from the
  latest frame received.
- **A frame with HDCP or a black signal**: it is an image; it is saved as received.
- **The mode changes, or the device reconnects, while a screenshot is being written**: the frame
  already taken is written; a screenshot asked while the video area shows a state follows US5.
- **Many presses in a row** (faster than the files can be written): each press takes its frame
  at that moment and each is written in order; at most 4 wait to be written, further presses
  meanwhile are refused silently, like US5 scenario 1: no sound, so the missing click tells it
  (the preview is never slowed to catch up).
- **Folder deleted between screenshots**: it is created again when the folder's parent exists;
  otherwise US5 scenario 2.
- **Default folder**: the system's Pictures folder (as the user's system names and places it,
  including a moved or redirected Pictures folder) plus `CapturaX`, created at the first
  screenshot.
- **Hotkey on Linux under Wayland**: global keys go through the desktop's global shortcuts
  portal. The desktop may ask the user to confirm the key the first time, and may let the user
  change it in its own settings; the Hotkeys category then shows the key the desktop reports.
  Where the desktop has no such portal, the key works only while CapturaX's window has the
  focus, and the Hotkeys category says so in one line.
- **Hotkey on Linux under X11**: the key is global, as on Windows.
- **A key already taken by another program**: not detected or handled (F50); the user changes it.
- **Two CapturaX instances**: not possible (single instance, app shell).
- **The clipboard on Linux**: the image stays pasteable while CapturaX runs; after it quits, only
  if the desktop keeps clipboard contents (a clipboard manager). Not handled further.

## Requirements *(mandatory)*

### Functional Requirements

**Taking the screenshot**

- **FR-001**: A screenshot MUST capture the next frame the preview presents after the request
  (within one frame period of it; while the window is minimized, the next frame received), at the device mode's native resolution, with the
  same colors the preview shows (the color range correction of F2 applied), without the
  interface, overlays, notices or letterbox bars.
- **FR-002**: Taking, encoding, saving and copying a screenshot MUST NOT add latency or dropped
  frames to the preview or glitches to the preview audio (constitution principle I); this is
  measured (SC-003).
- **FR-003**: The screenshot MUST be saved as PNG (lossless, 8 bits per channel, sRGB, no
  transparency) and copied to the system clipboard as an image that image editors, chat apps and
  browsers can paste.
- **FR-004**: A request MUST be possible only while the video area shows an image. Otherwise the
  button is disabled and the key does nothing (US5 scenario 1).
- **FR-005**: The file name MUST be `CapturaX <YYYY-MM-DD HH-MM-SS>.png` in local time; when that
  name exists, ` (2)`, ` (3)`… is added before the extension. A file is never overwritten.
- **FR-006**: A file MUST be written so that no other program sees a partial PNG under the final
  name (a crash or a full disk leaves no truncated screenshot under that name).
- **FR-007**: At most 4 screenshots MUST wait to be written at a time; a request beyond that is
  refused silently (Edge Cases).

**Feedback**

- **FR-008**: After each screenshot whose frame was taken, the app MUST show one notice, using
  the rules of device
  preview FR-027 (inside the window when it has the focus, at the notification position, the
  latest replacing the current; a system notification otherwise):
  - saved and copied: "Screenshot saved", with **Show**;
  - saved, not copied: "Screenshot saved, not copied", with **Show**;
  - not saved, copied: "Couldn't save the screenshot: <reason>. Copied to the clipboard", with
    **Settings**;
  - nothing saved or copied: "Couldn't take the screenshot: <reason>".

  A request that takes no frame (no image, too many waiting, no frame within 500 ms) shows no
  notice and plays no sound: the capture sound is the confirmation, and its absence says the
  screenshot was not taken (maintainer, 2026-11-22). It is logged.
- **FR-009**: **Show** MUST open the system's file manager on the folder with the file selected
  where the platform allows it, else on the folder.
- **FR-010a**: When a screenshot's frame is taken, the video area MUST show an outline flash
  (maintainer, 2026-11-22; changed from an edge glow after seeing it in the app and comparing
  options, same day): a thin white line (about 3 px) along the video's rounded edge with a soft
  glow just inside it (about 18 px, faint), the rest of the picture untouched. It lights up fast
  (about 40 ms) and fades out (about 400 ms), then is gone. It is drawn over the video in the
  window only: never in the saved image, never delaying a frame (FR-002).
  It covers the video area, not the toolbar or the docks; a new screenshot during the flash
  restarts it. It is not shown when no frame is taken (FR-008), nor while the window is minimized
  or hidden. It starts with the frame being taken, not when the
  file is written, so it comes at once; a later failure to save and copy is told by the notice
  (plan, 2026-11-22).
- **FR-010**: When a screenshot's frame is taken, the app MUST also play a short capture
  sound; no thumbnail in the notice (maintainer, 2026-11-21). The sound:
  - plays on the output chosen for the preview audio (Audio output, preview audio FR-009),
    also when the preview plays no audio, with the same rule when that output is missing (the
    system default meanwhile), within 100 ms of the frame being taken while the preview plays
    audio, and within 300 ms when the output has to be opened first, so the player hears it on
    their headset while playing (maintainer, 2026-11-22);
  - is independent of the preview's volume and mute (a muted preview still confirms the
    screenshot), and never interrupts or delays the preview audio (FR-002);
  - is not played when no frame is taken (FR-008); it plays when the
    frame is taken, before the file is written, so a later failure to save and copy is told by
    the notice alone (plan, 2026-11-22);
  - one sound per screenshot; a burst plays one per screenshot taken, never overlapping into a
    longer sound;
  - has no setting to turn it off (constitution principle II; added only if a need shows).
  Later specs may add sounds for other events (recording, live, viewers; maintainer idea,
  2026-11-22); this spec builds only what the screenshot sound needs, organized so another
  sound can be added later without reworking how sounds reach the output (plan).
  The first sound is a placeholder made for this spec; the maintainer provides the final sound
  after the implementation is validated, before it is committed (maintainer, 2026-11-21).

**Right dock**

- **FR-011**: The right dock (device preview FR-002a) MUST appear with a screenshot button, the
  first of the features (future order: preview, screenshot, record, stream). The button is
  disabled while the video area shows a state (FR-004); its tooltip is "Screenshot (<key>)", or
  "Screenshot" when no key is set.

**Hotkeys (F50 framework)**

- **FR-012**: The app MUST provide hotkeys that work both while CapturaX's window has the focus
  and while another app has it, on Windows and on Linux under X11; on Linux under Wayland through
  the desktop's global shortcuts portal where it exists, else only while the window has the focus
  (the Hotkeys category states it). There are no separate in-window keys (F50).
- **FR-013**: The only hotkey in this spec is **Screenshot**, default **Ctrl+F8**. The framework
  MUST let later specs add actions (Record Ctrl+F10 in spec #9, Live Ctrl+F9 in spec #10) with
  their default keys and their rows in the Hotkeys category, without changes to this behavior.
- **FR-014**: A held key MUST trigger its action once (auto-repeat ignored). A hotkey MUST NOT
  take the focus from the app that has it nor show CapturaX's window.
- **FR-015**: Hotkeys MUST work while the window is visible, minimized or hidden to the tray, and
  while the settings panel is open (except while a key is being recorded, FR-017, and where keys
  work only in the window, FR-012).
- **FR-016**: A **Hotkeys** category in the settings panel MUST list each action with its key.
  The key is changed by clicking it and pressing the new combination: a function key alone
  (F1–F24), or a key with at least one modifier, where a letter, digit or Space needs Ctrl, Alt
  or Win/Super (Shift alone would take a capital letter from every app) (plan, 2026-11-22); Esc
  cancels; a combination another CapturaX action uses is
  refused, naming that action; each row can be cleared (no key) and reset to its default.
- **FR-017**: While a key is being recorded, hotkeys MUST NOT trigger their actions.
- **FR-018**: A key change MUST apply at once (the old key released, the new one working) and be
  kept in the settings file; a cleared key is kept as cleared.
- **FR-019**: A key is not checked against other programs (F50); a key the system refuses to
  register is logged once, and the app keeps running.

**Folder**

- **FR-020**: The settings panel MUST offer the screenshots folder: the current folder shortened
  with the full path in its tooltip, **Change** (the system's folder dialog), **Open** (the file
  manager on the folder) and **Reset** (only while a folder other than the default is set).
  The row, named **Screenshots**, sits in a new **Folders** category (folder icon), after Audio;
  the Hotkeys category comes after it. Screenshots have no other setting, and recording's
  options live in its Record panel (F25), so the recordings folder joins this category as a
  second row in spec #9 (maintainer, 2026-11-23, replacing a Screenshot category per output).
- **FR-021**: The default folder MUST be the user's Pictures folder as the system defines it,
  plus `CapturaX`; it is created at the first screenshot (or by **Open** in the settings), as is
  a chosen folder that was removed when its parent exists.
- **FR-022**: The chosen folder is kept in the portable settings file (constitution principle VI)
  as an absolute path; the default is kept as "default", so a copied installation on another
  system uses that system's Pictures folder.

**Texts**

- **FR-023**: Every new text (button tooltip, notices, the Hotkeys category, the folder row, key
  names) MUST be in the locale files in English and Portuguese (Brazil). Key names follow the
  platform's (Ctrl, Alt, Shift, Win on Windows, Super on Linux).

**Platforms**

- **FR-024**: Everything in this spec MUST work on Windows and Linux; the degraded behaviors are
  the Wayland hotkey without a portal (FR-012) and the Linux clipboard after the app quits (Edge
  Cases).

### Key Entities

- **Screenshot**: one frame taken from the open device at a moment; its resolution, the time it
  was taken, the file it was written to, whether it was saved and copied.
- **Hotkey binding**: an action (Screenshot; later Record, Live), its key combination or none,
  its default.
- **Screenshots folder**: "default" or an absolute path.

## Success Criteria *(mandatory)*

### Latency budget and how it is measured

A screenshot is not on the preview path, but runs during it (constitution principle I). Budget:
**zero added latency and zero dropped frames in the preview** while screenshots are taken. Measured
with the device preview's own frame counters and frame-time log (spec #3's method): the ezcap at
1080p60, 1080p120 and 4K30 (the most pixels per frame), 60 s of preview with one screenshot a
second and a burst of 10 presses, on Windows and on Linux, compared with 60 s without
screenshots. The glass-to-glass latency of spec #3 is measured once with screenshots running, to
confirm it does not move beyond the measurement's spread.

### Measurable Outcomes

- **SC-001**: From the key press to the notice: under 1 s at 1080p and under 2 s at 4K, on
  both test PCs; the file is complete and the clipboard holds the image by then.
- **SC-002**: The saved image has exactly the mode's resolution, and its pixels match the frame
  the preview presented at that moment (checked against a test pattern: same colors within ±1
  per channel after the color range correction).
- **SC-003**: The preview drops 0 frames and its frame-time spread does not grow during the
  measurement above; the preview audio has no gap.
- **SC-004**: Ctrl+F8 takes a screenshot with CapturaX focused, with another app focused, with
  the window minimized, and from a fullscreen game window on Windows; on Linux under KDE Plasma
  (Wayland, portal) and under an X11 session where available.
- **SC-005**: A changed key works at once and after a restart in 10 of 10 tries; the old key
  does nothing.
- **SC-006**: Pasting works in Paint, GIMP or Krita, a browser text box (Discord or WhatsApp Web)
  on Windows and on Linux.
- **SC-007**: Every failure of US5 shows its notice; no failure is silent.
- **SC-008**: The capture sound is heard on the chosen output for each screenshot, with the
  preview muted and unmuted, with the preview on No audio, with a chosen output that is not the
  system default, in shared and exclusive output mode on Windows and on Linux, and the preview
  audio shows no gap while it plays.

## Assumptions

- The test hardware is the one used by specs #3 and #4: the ezcap card with the PS5 and the C920,
  on the Windows PC and the CachyOS PC (KDE Plasma, Wayland).
- HDR is not handled: the image is saved as the preview shows it, 8 bits per channel.
- The device modes the preview opens deliver frames that can be converted to an 8-bit RGB image
  (as the preview does to present them).
- The screenshot uses the source the preview shows. Screen and window sources (spec #7) will be
  captured the same way when they arrive; nothing here is specific to capture cards.
- A system notification's **Show** works where the platform's notifications support actions
  (Windows notification center, Linux notification daemons with actions); elsewhere the
  notification has no action.
- Keys taken by another program are not detected (F50); on Windows a registration the system
  refuses is only logged (FR-019).

## Out of Scope

- Screenshots of the whole screen or a window not shown in the preview (spec #7 adds those as
  preview sources).
- Other formats (JPEG, WebP, AVIF), HDR images, a quality setting.
- Editing, cropping, annotating or uploading screenshots; a gallery.
- A thumbnail in the notice (FR-010).
- The Record and Live hotkeys (specs #9 and #10), and the tray menu entries of F48.
- Screenshots while the window is hidden to the tray (the device is closed then).
