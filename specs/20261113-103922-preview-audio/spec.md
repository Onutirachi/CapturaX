# Feature Specification: Preview Audio

**Feature Branch**: `feat/20261113-103922-preview-audio`

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: "Preview audio (see docs/roadmap.md #4): covers F12-F14 from docs/product.md. Lowest-latency playback of an audio input on a chosen output, paired automatically with the video device, with volume and mute, and F47 (exclusive mode per device). On Windows through WASAPI directly, as the stack spike's engine; it must measure the 30-minute drift the spike left open, and the Linux path. Must state a latency budget and how it is measured."

## Context

The device preview (spec #3) shows the console's image with a delay the player cannot tell from
the passthrough, but plays no sound. This spec adds the sound: the audio of the capture card,
picked automatically with the video device, played on the user's headset or speakers with the
lowest delay the platform allows, with volume and mute. It is the "C. Listening" group of
docs/product.md (F12, F13, F14) and the per-device exclusive mode of F47.

The engine is not open on Windows. The stack spike (findings, "Audio (Windows, via Voicemeeter)";
research R7) replaced GStreamer's audio elements with a WASAPI engine of its own: each side opens
shared (the shortest period the Windows mixer allows) or exclusive (the device's own format), and
between them the app holds about 2 ms of spare audio by adjusting the playback rate instead of
cutting. Measured against the passthrough monitor's own audio: 59 ms shared, 46 ms with the card
exclusive, 18 ms with both exclusive; against the chosen video path the audio was 29 ms behind
(shared) to 12 ms ahead (both exclusive), within the lip-sync limits.

Inherited open questions (stack spike findings, "Open questions for later specs", Preview audio),
each answered below:

- **Drift over 30 minutes** with the WASAPI engine was never measured (the spike's SC-005 stayed
  open on drift). The GStreamer path it replaced drifted 2.6 ms per minute (+63 ms in 30 minutes).
- **The delay with a hardware output** instead of the Voicemeeter virtual input was not measured.
- **The Linux path** was only heard (through VBAN), never measured. On Linux the spike found that
  GStreamer's PipeWire output hangs on stop (4 and 7 runs in 10, freezing every app that played
  sound), that GStreamer's PipeWire input starves next to the PulseAudio output (17–25 % of the
  audio delivered), and that the PipeWire input negotiated mono unless held to the device's own
  format. It fell back to the PulseAudio compatibility layer on both sides as a workaround and
  recommended PipeWire directly, as WASAPI directly on Windows.
- **Pairing by name** on Linux: the card's audio node (`ezcap Live GAMER ULTRA Estéreo
  analógico`) only matched its video device (`ezcap Live GAMER ULTRA: ezcap L`) once the suffixes
  were left out.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear the console while playing (Priority: P1)

The user starts CapturaX with the console on the capture card. The card's image appears, and its
sound plays on the user's headset at the same time, with no setup: the card's audio input is
paired with its video automatically and plays on the system's default output. The sound is close
enough to the image and to the passthrough that playing feels right.

**Why this priority**: hearing the console is half of playing through the preview (product.md
F12, constitution principle I). Every other story only adjusts it.

**Independent Test**: with the PS5 on the ezcap card, start the app with default settings, play,
and measure the sound's delay against the passthrough monitor's audio (SC-001), and the offset
against the image (SC-004).

**Acceptance Scenarios**:

1. **Given** the capture card is opened by the preview and has an audio input, **When** its video
   starts, **Then** its sound plays on the system's default output within the time of SC-006,
   without any choice by the user.
2. **Given** the preview plays video and sound, **When** the user plays for 30 minutes, **Then**
   the delay stays within the budget at the start and at the end (SC-001, SC-003), with no
   audible gaps, clicks or pitch changes (SC-005).
3. **Given** the preview plays sound, **When** the user changes the video mode, the color range,
   the theme, the language or opens the menus and the settings panel, **Then** the sound plays on
   without a gap.
4. **Given** the preview plays sound, **When** the video device is closed (the user picks another
   device, the window is hidden to the tray, the app quits), **Then** the sound stops with it, and
   when the device opens again its sound returns.
5. **Given** the preview plays sound, **When** the image is measured as in spec #3, **Then** the
   video still meets spec #3's budgets (SC-007).

---

### User Story 2 - Volume and mute (Priority: P1)

The user turns the preview's sound up or down, or mutes it, without touching the system's volume
or other apps' sound. The level and the mute are kept for the next start.

**Why this priority**: without it the only way to quiet the console is the system mixer, which
also changes every other app (product.md F14).

**Independent Test**: while the preview plays sound, change the volume and mute from the app's
control and check the system mixer and another app's sound do not change; restart the app and
check the level and the mute are kept.

**Acceptance Scenarios**:

1. **Given** the preview plays sound, **When** the user moves the volume, **Then** the sound
   follows at once (SC-006), without clicks, and the system's volume and other apps are unchanged.
2. **Given** the preview plays sound, **When** the user mutes it, **Then** it goes silent at once,
   and unmuting restores the same level.
3. **Given** a volume and a mute state were set, **When** the app starts again, **Then** both are
   the same.
4. **Given** the sound is muted or at zero, **When** the user looks at the volume control,
   **Then** its icon shows it (a muted speaker), so silence is never a mystery.

---

### User Story 3 - Choose the audio input and the output (Priority: P2)

The user picks another audio input for the preview (a different capture card's audio, a
microphone, a line input) or no audio at all, and picks the output the sound plays on (the
headset instead of the speakers). The choices are remembered.

**Why this priority**: the automatic pairing and the default output cover the main case; some
setups need another input (a card whose audio comes through a separate line input) or another
output (the headset while the system default is the speakers) (product.md F12, F13).

**Independent Test**: with the ezcap card and the C920 connected, pick each input for each video
device and another output, restart, and check every choice is kept and plays.

**Acceptance Scenarios**:

1. **Given** a video device is open, **When** the user opens the audio input choice, **Then** it
   lists **Auto** (naming the paired input, or saying there is none), every audio input of the
   computer, and **No audio**, the current one marked.
2. **Given** the user picks an input for a video device, **When** that device opens again (now or
   after a restart), **Then** the same input plays; other video devices keep their own choice
   (Auto by default).
3. **Given** the output is **System default**, **When** the system's default output changes
   (the user switches it in the system, or plugs in a headset that becomes the default), **Then**
   the sound moves to the new default output within the time of SC-006.
4. **Given** the user picks a specific output, **When** the app starts again, **Then** it plays on
   that output; while that output is missing, the sound plays on the system default and says so,
   and moves back when it returns. Meanwhile the menu keeps the missing output, marked, with
   **Disconnected**, as it does for a missing chosen or paired input, until it returns or the user
   picks another (maintainer, 2026-11-19).

---

### User Story 4 - Exclusive mode for the lowest delay (Priority: P2)

On Windows, the user marks the capture card's audio input, the headset, or both as exclusive in
the settings. The preview's sound then skips the system mixer on those devices, for the lowest
delay the hardware allows, knowing that no other app can use them meanwhile.

**Why this priority**: it cuts the delay from about 59 ms to about 18 ms on the spike's setup
(product.md F47), which matters to players who hear the difference; most users never need it.

**Independent Test**: on Windows, mark the ezcap's input and the output exclusive, measure the
delay (SC-001), and check another app cannot play on the exclusive output while the preview
uses it.

**Acceptance Scenarios**:

1. **Given** the settings panel is open on Windows, **When** the user opens its **Audio**
   category, **Then** it lists every audio input and every output of the computer, each with an
   **Exclusive mode** checkbox (off by default), and says that no other app can use an exclusive
   device while the preview uses it.
2. **Given** a device in use by the preview is marked exclusive, **When** the box changes,
   **Then** the sound reopens that device in the new mode within the time of SC-006.
3. **Given** a device is marked exclusive but exclusive mode is refused (the system setting
   forbids it, or the device takes no usable format), **When** the preview opens it, **Then** the
   preview plays it shared instead and shows **Audio device in use** / **Exclusive mode
   unavailable**; it tries exclusive again the next time it opens the device.
4. **Given** a device is marked exclusive, **When** the preview stops using it (another input,
   another output, the video device closes), **Then** it is released at once for other apps.
5. **Given** the app runs on Linux, **When** the settings panel is open, **Then** there is no
   exclusive mode (product.md F47 is Windows only); the Linux path already skips what it can
   (FR-016).
6. **Given** another app holds a device alone (exclusive mode), **When** the preview opens it, or
   that app takes it while the preview plays it shared, **Then** that side stops and shows
   **Audio device in use** / **Try again**; **Try again** opens it again (it works once the other
   app releases it). Taking a device in exclusive mode is expected to cut it off from apps that
   used it shared (measured on Windows, 2026-11-13).

---

### User Story 5 - Clear states for the sound (Priority: P3)

When the sound cannot play, the user sees why and what to do, instead of a silent preview: the
video device has no audio, the input or the output disappeared, the input is blocked by the
system's privacy settings, or a device is in use.

**Why this priority**: silent failure is the worst audio bug to diagnose (constitution principle
II: empty and error states shown explicitly); the preview works without it.

**Independent Test**: force each state as in the quickstart (no paired input with the C920 on
Linux if it has none, unplug the headset, turn microphone access off on Windows, hold a device
exclusive in another app) and check what the app shows.

**Acceptance Scenarios**:

1. **Given** the open video device has no paired input, **When** the input is Auto, **Then** the
   audio choice reads **Auto (none)**, its menu says **No paired audio** / **Choose an input**,
   and nothing plays; the user can pick another input.
2. **Given** the chosen input disappears while playing, **When** it comes back, **Then** its
   sound returns by itself within the time of SC-006; meanwhile the audio choice shows
   **{device} disconnected** / **Waiting to reconnect**, as the video devices do.
3. **Given** the system blocks the app from audio inputs (Windows microphone privacy setting),
   **When** the preview opens an input, **Then** the audio choice shows **Access blocked**, with
   **Open privacy settings**, which opens the system's microphone privacy page.
4. **Given** the output fails (removed, or held exclusive by another app), **When** the preview
   plays sound, **Then** it falls back to the system default and shows **{output} disconnected** /
  **Playing on the system default**, as in User Story 3.

The wording of every state is in FR-022.

### Edge Cases

- **Feedback loop**: the user picks as input the recording side of the output they play on (for
  example a virtual mixer's output as input and its input as output). The app does not detect it;
  the volume and mute stop it. Detecting it is out of scope.
- **Webcam microphones**: the C920 has a microphone, so Auto pairs it with the C920's video and
  the user hears the room through the headset. This is accepted (maintainer, 2026-11-13): there
  is no reliable way to tell a capture card from a webcam, so pairing stays simple; the user picks
  **No audio** once and it is remembered for that device.
- **Different sample rates or channel counts** between the input and the output (48 kHz card,
  44.1 kHz headset; stereo card, 7.1 output; mono microphone): the sound plays correctly, mono
  on both ears, with no audible artifact.
- **Two video devices with audio of the same name** (two identical cards): each video device
  pairs with its own input, never the other card's.
- **Sleep and wake** while the preview plays sound: the sound returns with the video after wake,
  as spec #3's reconnect.
- **The system default output changes to the device the input comes from** (or a device without
  output): the sound follows the default as long as it is an output; nothing crashes.
- **Silence**: the card sends silence (no HDMI input, "EZCAP CONNECT SOURCE"): nothing is shown as
  an error; silence is not a state.
- **The video device closes while the audio input is still opening**: no sound plays after the
  video is gone.
- **Exclusive output and system sounds**: while the output is exclusive, the system's own sounds
  and other apps are silent on it; the settings line says so (User Story 4).

## Requirements *(mandatory)*

### Functional Requirements

**Playback**

- **FR-001**: The app MUST play the preview's audio input on the preview's output while the
  preview's video device is open (Opening, Playing, No signal of spec #3), and stop it, releasing
  both audio devices, when the video device closes (product.md F12).
- **FR-002**: The sound MUST play each piece of audio as soon as it arrives, with the smallest
  buffering the platform allows on each side, not on a clock shared with the video (constitution
  principle III: the preview keeps audio and video close by low latency on both sides).
- **FR-003**: The app MUST keep the audio waiting between input and output steady over any length
  of play, without cutting or repeating audio, by following the difference between the input's
  and the output's clocks (the spike's rate correction), so the delay does not grow (SC-003) and
  the pitch change is not audible (SC-005).
- **FR-004**: The sound MUST play correctly whatever the input's and the output's formats (sample
  rate, channels, sample format): mono inputs on both channels, extra channels mixed down.
- **FR-005**: Changing the video mode, the color range, or anything in the interface MUST NOT
  interrupt or delay the sound; changing the audio input, the output or an exclusive box reopens
  only the audio side, never the video device.
- **FR-006**: Opening, reopening and closing the audio devices MUST NOT block the interface or
  the video, and closing MUST always finish (the spike's Linux output hang must not be possible:
  a stuck close never freezes the app, the system's sound or the app's exit).

**Pairing and choices**

- **FR-007**: The app MUST pair each video device with the audio input of the same physical
  device (product.md F13). The pairing MUST use the system's own link between the two when there
  is one (the same hardware device or container), and fall back to matching names with the
  platform's decorations left out; it never pairs a device with another device's input. A
  webcam's microphone is paired like any other input (maintainer, 2026-11-13: keep it simple).
- **FR-008**: The toolbar MUST gain the **Audio** dropdown already counted by spec #3's FR-002a
  (icon, title and value; icon only in narrow windows): **Auto** with the paired input's name (or
  **Auto (none)**), then every audio input of the computer, then **No audio**. The choice is
  remembered per video device; the default is Auto.
- **FR-009**: The app MUST offer the output choice: **System default** (naming the current
  default, following it when it changes) and every output of the computer, remembered across
  restarts (one choice for the app, not per device). Default: System default. It is offered in
  the Audio dropdown's menu only, as an **Output** group under the inputs (maintainer,
  2026-11-18: the Output select once planned in the volume popover was dropped; the output stays
  one for the app, not per video device).
- **FR-010**: The lists MUST follow the system: audio devices plugged or removed appear in or
  leave the choices within the time of SC-006, as spec #3's video device list.

**Volume and mute**

- **FR-011**: The left dock MUST gain the volume button (spec #3's FR-002a: fullscreen, ambient
  light, volume, settings), applied to the preview's sound only, never to the system's volume or
  other apps (product.md F14). Its icon shows the level, and a muted speaker when muted or at
  zero. The volume popover holds a slider from 0 to 100 % with its value and nothing else: the
  button alone mutes, so no second speaker icon stands beside it. It works as follows
  (maintainer, 2026-11-13 on the mockup page, changed 2026-11-18):
  - **Mouse**: a click toggles mute. Resting the pointer on the button for about 300 ms opens the
    popover. The popover stays open while the pointer is on the button or on the popover, and
    closes shortly after it leaves both. A right click also opens it. The mouse wheel over the
    button changes the volume without opening anything.
  - **Touch**: a tap opens the popover (volume is used more often than mute); a tap outside
    closes it. Holding the button (a long press, 500 ms) toggles mute.
  - **Keyboard**: the button is one stop in the focus order. Focusing it (Tab) shows the popover,
    which takes no focus; Enter or Space toggles mute; the arrow keys move the volume by 5 %,
    Home and End to 0 and 100 %. The next Tab leaves the button and the popover closes.
  - Moving the volume (slider, wheel or keys) while muted unmutes.
- **FR-012**: The volume and the mute MUST be remembered across restarts (one level for the app,
  not per device), and the level MUST change smoothly (no click when moving or muting).
- **FR-013**: The preview's sound is only for the user: its volume, mute and output MUST NOT
  affect any recording or stream (product.md F14; recording and streaming have their own audio,
  specs #8 and later).

**Exclusive mode (Windows)**

- **FR-014**: The settings panel MUST gain an **Audio** category on Windows listing every audio
  input and output with an **Exclusive mode** checkbox each, off by default, and one short line
  saying that no other app can use an exclusive device while the preview uses it (product.md F47).
  The settings are remembered per audio device across restarts.
- **FR-015**: An exclusive device MUST be opened in exclusive mode, skipping the system mixer;
  when exclusive is refused (not allowed, format refused), the app MUST play it shared and show it
  in the audio choice (User Story 4, scenario 3); when another app holds the device alone, the side
  does not play and shows it (User Story 4, scenario 6). Exclusive devices are released as soon as the preview stops
  using them.
- **FR-016**: On Linux the app MUST play through the system's sound server directly (PipeWire)
  with the smallest buffering it grants the app, and show no exclusive mode. Whether the
  PulseAudio compatibility layer is ever needed is decided in the plan with a measurement.

**States**

- **FR-017**: The audio choice MUST show when the sound does not play as expected and why: a tag
  before the value in the dropdown and, at the top of its menu, a card with the state's title, the
  device's name when one device is at fault, the detail, and the action inside it (FR-022), tinted
  in the tag's tone (warn: yellow, bad: red; No paired audio neutral) with a neutral button
  (maintainer, 2026-11-19): no paired input, input disconnected (it returns by itself), access blocked
  (with an action that opens the system's microphone privacy page on Windows), exclusive device
  in use (it plays shared), device held by another app (with **Try again**), failed (with **Try
  again**). The output's fallback to the system
  default is shown the same way.
- **FR-018**: An audio problem MUST never stop or change the video, and a video state (No signal,
  Disconnected) MUST not be shown again as an audio state: when the video device is gone, the
  sound simply stops.

**Measurement**

- **FR-019**: The app's per-second statistics (app shell FR-005, `--stats`) MUST cover the sound:
  the audio waiting in the app (median, maximum), the rate correction in use, gaps (audio the
  output asked for and did not get) and audio dropped, on Windows and on Linux.
- **FR-020**: The latency tools (tools/latency) MUST measure the sound's delay against a reference
  and its drift over 30 minutes from one recording, following the spike's method (research R3,
  "Method used in practice"). On Linux the path is measured inside the PC through PipeWire
  (maintainer, 2026-11-13: the Linux PC has no speakers, and there is no free input on the Windows
  PC nor a splitter cable for an end-to-end recording): the card's audio node and the app's
  output are recorded together, and the latency the output device reports is added.

**Texts**

- **FR-022**: The texts MUST be these (maintainer, 2026-11-13, to be checked again in the
  interface); entries marked * already exist for the video devices and are reused:

  | Where | English | Portuguese (Brazil) |
  |---|---|---|
  | Toolbar dropdown title | Audio | Áudio |
  | Menu groups | Input · Output | Entrada · Saída |
  | Paired input | Auto ({}) | Auto ({}) |
  | No paired input | Auto (none) | Auto (nenhum) |
  | No input | No audio | Sem áudio |
  | Default output | System default ({}) | Padrão do sistema ({}) |
  | Tag on an exclusive device | Exclusive | Exclusivo |
  | Volume button tooltip | Volume {}% · Muted | Volume {}% · Mudo |
  | Settings category (Windows) | Audio | Áudio |
  | Settings section | Exclusive mode | Modo exclusivo |
  | Settings line | Lowest delay. No other app can use the device while the preview does. | Menor atraso. Nenhum outro app usa o dispositivo enquanto o preview usa. |
  | Settings groups | Inputs · Outputs | Entradas · Saídas |
  | Settings checkbox | Exclusive | Exclusivo |
  | Settings row mark | In use by the preview | Em uso pelo preview |

  States (title / detail / action), with the tag shown in the dropdown:

  | State | Tag | English | Portuguese (Brazil) |
  |---|---|---|---|
  | No paired input | — | No paired audio / Choose an input | Sem áudio pareado / Escolha uma entrada |
  | Input disconnected | Disconnected · Desconectado | Device disconnected / {} / Waiting to reconnect* | Dispositivo desconectado / {} / Aguardando reconexão* |
  | Exclusive device in use | In use · Em uso | Device in use / {} / Exclusive mode unavailable | Dispositivo em uso / {} / Modo exclusivo indisponível |
  | Device held by another app (Windows) | In use · Em uso | Device in use / {} / Try again* | Dispositivo em uso / {} / Tentar novamente* |
  | Access blocked (Windows) | Blocked · Bloqueado | Access blocked* / Desktop app permission is off* / Open privacy settings* | Acesso bloqueado* / Permissão para aplicativos da área de trabalho desativada* / Abrir configurações de privacidade* |
  | Failed | Failed · Falhou | Cannot open / {} / Try again* | Não foi possível abrir / {} / Tentar novamente* |
  | Chosen output missing | — | Device disconnected / {} / Using the system default | Dispositivo desconectado / {} / Usando a saída padrão |

  The `{}` line is the device's name (cut with "…" when long; the menu lists it in full, marked).
  Until 2026-11-19 the title held the name (`{} disconnected`, `Cannot open {}`), which a long
  name cut before its last word.

**Platforms**

- **FR-021**: Every requirement above MUST work on Windows 11 and on Linux (KDE Plasma on
  Wayland), except exclusive mode, which is Windows only (product.md F47); texts in English and
  Portuguese (Brazil).

### Key Entities

- **Audio input**: an audio capture device of the computer (a capture card's audio, a microphone,
  a line input), with a stable identity, a name, and its link to a video device when the system
  has one.
- **Audio output**: a playback device (headset, speakers, a virtual mixer's input), with a stable
  identity and a name; one of them is the system default at any time.
- **Pairing**: the audio input that belongs to a video device, found automatically (FR-007).
- **Audio choice per video device**: Auto, a specific input, or No audio; remembered.
- **Preview audio settings**: the output (System default or a specific one), the volume, the mute,
  and the exclusive flag per audio device (Windows); remembered.

## Success Criteria *(mandatory)*

### Latency budget and how it is measured

The delay is measured as in the stack spike (research R3, "Method used in practice"): the PS5
menu sounds reach the recorder twice, from the passthrough monitor's headphone jack through the
PC's line input (the reference) and from the preview's output; `analyze.py audio --stereo` finds
each pair (reference panned left, preview right) and reports the delay. On Windows the recorder
is Voicemeeter, as in the spike, so the preview's output is the Voicemeeter virtual input; the
reference itself is about 26 ms behind the card (the monitor's own audio delay), so these numbers
are relative to the passthrough the player hears, not to the card. The A/V offset is the audio
delay minus the video delay of spec #3 (glass to glass) measured in the same session.

### Measurable Outcomes

- **SC-001**: Windows 11 (the maintainer's PC, ezcap at 48 kHz, PS5, output through Voicemeeter
  as in the spike), against the monitor's audio: median at most **62 ms** shared, at most **50 ms**
  with the input exclusive, at most **22 ms** with both exclusive; p95 at most 5 ms above each
  median (the spike: 59, 46 and 18 ms).
- **SC-002**: Linux (CachyOS, KDE Plasma Wayland, PipeWire), same card and source: the delay from
  the card's audio reaching the PC to it leaving for the output, measured on the PC itself, median
  at most **25 ms**, p95 at most 30 ms (FR-020: recorded through PipeWire, plus the latency the
  output device reports; no end-to-end recording is possible with the maintainer's equipment).
- **SC-003**: Drift: over a 30-minute run in each setting of SC-001 (Windows) and on Linux, the
  delay at minute 30 is within **10 ms** of minute 0 (the spike's SC-005), with the app's
  statistics showing the audio waiting in the app steady over the whole run.
- **SC-004**: A/V offset: with the shared and the both-exclusive settings on Windows and with the
  Linux path, the sound is between 45 ms ahead and 125 ms behind the image (ITU-R BT.1359 limits,
  the spike's SC-005), and the maintainer notices no offset while playing. On Linux the offset is
  the SC-002 delay minus spec #3's glass-to-glass video delay; it leaves out what the card itself
  adds before the PC, so it is checked mainly by ear.
- **SC-005**: Continuity: over each 30-minute run of SC-003, no gap or dropped audio after the
  first second in the app's statistics, and the maintainer hears no click, dropout or pitch
  change while playing.
- **SC-006**: Responsiveness on the maintainer's machines: the sound starts within **1 s** of the
  video device's first frame; a change of input, output or exclusive box plays within 1 s; a
  volume or mute change is heard within 100 ms; an audio device plugged or removed appears in or
  leaves the choices within 3 s; a disconnected input or a changed default output plays again
  within 3 s of the system offering it.
- **SC-007**: With the sound playing, spec #3's SC-001 (glass to glass, Windows) and SC-003 (frame
  ready → presented and frame rate, Windows and Linux) still pass.
- **SC-008**: Each state of FR-017 is shown when forced as in User Story 5, in both languages, and
  its action works; closing the app or the device never hangs (100 closes in a row with sound
  playing on each system, none over 1 s).
- **SC-009**: The maintainer accepts the preview audio on Windows 11 and on CachyOS KDE Plasma
  Wayland against User Stories 1 to 5, with the ezcap card and the C920 (manual acceptance,
  constitution principle VII).

## Assumptions

- The reference setup is the maintainer's: ezcap Live Gamer Ultra with the PS5, the passthrough
  monitor's headphone jack wired to the Windows PC's line input, Voicemeeter as the recorder and
  as the output the headset (Logitech G533) listens through. A hardware output cannot be recorded
  next to the reference with this equipment, so the hardware output (the G533 directly, without
  Voicemeeter) is checked by ear and by the app's statistics, not by recording; this answers the
  spike's open question as far as the equipment allows.
- On Windows the engine is the stack spike's (WASAPI directly, research R7 "Revised again"),
  rewritten for the app; GStreamer is not used for the preview's sound on either system
  (constitution principle III allows it for the preview), and still carries audio for recording
  and streaming later (spec #8).
- Hiding the window to the tray closes the video device (spec #3), so the sound stops too.
  Whether hiding the preview inside the window (F10) keeps the sound is decided in spec #6
  (product.md "To reconsider").
- No manual audio delay (product.md "Dropped": lip-sync measured within limits); reconsider only
  if SC-004 fails.
- The capture card's audio is 48 kHz stereo on both systems; other rates and channel counts are
  covered by FR-004 and tested with the C920's microphone and a 44.1 kHz output.
- The PS5's audio over HDMI is stereo PCM to the card (the console's output setting); multichannel
  HDMI audio from the source is out of scope.

## Out of Scope

- Audio for recording and streaming, PC sound per app, the mix and separate tracks (spec #8).
- An audio latency or level bubble in the info overlay (spec #6 may add it).
- Audio from screens and windows as sources (spec #7 and #8).
- Detecting feedback loops, audio effects, equalizer, volume above 100 %.
- Exclusive mode on Linux (product.md F47).
