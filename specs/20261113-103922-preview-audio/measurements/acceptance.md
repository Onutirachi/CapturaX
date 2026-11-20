# Acceptance Measurements

Raw logs and screenshots stay on the test PC (`~/capx/out`, `capturax.log`); they are not
committed.

## T042: SC-008 states and closing on Linux

### CachyOS — 2026-11-18

**Environment**: CachyOS KDE Plasma Wayland, release build of `43a5eaa` (with the
`audio session N closed in X ms` line), ezcap (PCIe) and C920 (USB). System default output: the
`vban_out` null sink (the VBAN service). Driven over SSH with `ydotool` and `spectacle`
screenshots; both languages by `--lang en` and `--lang pt-BR`.

**How each state was forced on Linux**:

| State | How | English | Portuguese |
|---|---|---|---|
| No paired audio | `v4l2loopback` camera ("Virtual Cam", no audio link), Auto | header `No paired audio` / `Choose an input`; picking the ezcap's input played 195 ms after the pick | `Sem áudio pareado` / `Escolha uma entrada`; played 197 ms after the pick |
| Disconnected | the C920's microphone picked as the ezcap's input, then `echo 1-1 > /sys/bus/usb/drivers/usb/unbind` (as an unplug) and `bind` | `Disconnected` tag (warn) on the select, header with the input's name and `Waiting to reconnect`; played again 223 ms after the input came back | `Desconectado`, `Aguardando reconexão`; played again 235 ms after |
| Output fallback | a null sink `capx-test` picked as the output, then its module unloaded and loaded again | header `CapturaX-Test disconnected` / `Playing on the system default`; on the default 76 ms after the removal; back on `capx-test` when it returned | `CapturaX-Test desconectado` / `Tocando na saída padrão`; 160 ms after the removal; back when it returned |

Blocked, In use and Failed are forced on Windows only (quickstart): Linux has no microphone
privacy switch the app reads, and PipeWire has no exclusive mode.

**Closing**: 11 starts (alternating English and Portuguese), 101 audio closes in a row with sound
playing: 90 by switching the video device (mostly ezcap ↔ C920: the Device menu lists the recent
device first; a few to the Virtual Cam), 11 by quitting with the window's close button. The
ezcap's input was the C920's microphone (left from the Disconnected step), the C920 used its own.

| Closes | Times | Max | Over 1 s | `did not close` |
|---|---|---|---|---|
| 90 switch (`audio session N closed in X ms`) | 1 ms ×47, 2 ms ×2, 6–9 ms ×40, 15 ms ×1 | 15 ms | 0 | — |
| 11 quit (`closed X ms into the quit`) | 1 ms ×11 | 1 ms | 0 | 0 |

Every closed session had reached Playing (`Off → Opening → Playing → Off` for each, no `Gone`). The app never hung; after the run `pactl info` answered at once and the default sink was
still `vban_out`.

**Found**: the C920 resets on the USB bus ("device firmware changed", then a new device number)
when it resumes from USB autosuspend (`power/control = auto`, 2 s): the first touch after it was
idle, such as starting the app, removes it for about 3 s. The app shows its input Disconnected and
plays it again when it returns (Added → Playing in 223–240 ms). This is the camera with the
kernel's autosuspend, not the app; the earlier "C920 drops by itself" observations are this.

## T042: SC-008 states and closing on Windows

### Windows 11 — 2026-11-19

**Environment**: Windows 11 Pro 26300, release build of `f1da398` (states) and `9ebf0fd`
(closes), ezcap Live GAMER ULTRA (USB) and C920. System default output: Voicemeeter Input.
Driven by a PowerShell script (window screenshots, `SetCursorPos`/`mouse_event` clicks), both
languages by `--lang en` and `--lang pt-BR`. Raw logs in `raw/` (not committed).

| State | How | English | Portuguese |
|---|---|---|---|
| Blocked | ezcap, `HKCU\…\ConsentStore\microphone\NonPackaged` `Value` = `Deny` (Settings' "Let desktop apps access your microphone" off) | `Blocked` tag (bad), header `Access blocked` / `Desktop app permission is off`, `Open privacy settings` opened the Settings app; with `Allow` again the input opened 1.2 s later (3 s check) | `Bloqueado`, `Acesso bloqueado`, `Abrir configurações de privacidade`; the detail is cut (`Permissão para aplicativos da área de trabalh…`) |
| In use | ezcap's input held exclusive by `audio_session --exclusive-in` while the app opens it shared (`AUDCLNT_E_DEVICE_IN_USE`) | `In use` tag (warn), header `Audio device in use`, `Try again`; after the holder quit, Try again played | `Em uso`, `Dispositivo de áudio em uso`, `Tentar novamente` |

Later the same day (release builds of the working tree, before `c434a91`):

| State | How | Result |
|---|---|---|
| No paired audio | the ezcap's `HDMI (ezcap Live GAMER ULTRA)` recording endpoint disabled in `mmsys.cpl` by the maintainer, then the app started | `No paired audio` / `Choose an input` and `Sem áudio pareado` / `Escolha uma entrada`, value `Auto (none)` |
| Disconnected | the same endpoint disabled while playing (Portuguese) | `Desconectado` tag, the session closed in 18 ms; Auto's input that left is Disconnected, not No paired audio (by design) |
| Output fallback | the ARZOPA monitor's speakers chosen, unplugged and plugged back three times by the maintainer (Portuguese) | on the default 45–60 ms after the removal, back on the ARZOPA when plugged; found that the menu then marked nothing in the Output group and the notice's title cut `desconectado` off a long name, which led to the card notices of `c434a91` |

The NVIDIA Broadcast camera cannot serve No paired audio: it opens only while the Broadcast app
runs, and then Auto pairs `Microfone (NVIDIA Broadcast)` by name. At its largest mode (3840×2160)
it failed to start and showed the video state `In use` (spec #3's preview; noted in handoff.md);
at 1920×1080 it played.

Failed: accepted by the unit tests (maintainer, 2026-11-19); no error other than gone, in use or
blocked could be forced without administrator rights.

Second run, same day (release build of `c434a91`, card notices; English; system default output
`Speakers (NVIDIA Broadcast)`):

| State | How | Result |
|---|---|---|
| Disconnected | the C920's microphone picked as the ezcap's input, then the C920 unplugged by the maintainer and plugged into another USB port | `Disconnected` tag (warn) on the select; yellow card `Device disconnected` / `Microfone (HD Pro Webcam C920)` / `Waiting to reconnect`; the input's row stays in the menu marked `Disconnected`. Back on the other port the endpoint kept its id and played 87 ms after it was announced (`added` → Playing); the open menu dropped the card and the mark in place |
| Output fallback | the ARZOPA's speakers chosen, unplugged and plugged back by the maintainer (twice; the first return was spoiled by a stray click and Enter that went to the app) | yellow card `Device disconnected` / `1 - ARZOPA (AMD High Definition Audio Device)` / `Using the system default`; on the default 43 and 45 ms after the removal; back on the ARZOPA when it returned |
| Light theme | Blocked (registry `Deny`, as above) and In use (the ezcap's input held by `audio_session --exclusive-in`) with `theme = "light"` | red and yellow cards readable, the action a neutral button; Try again played 47 ms after the holder quit; with `Allow` again the input opened by itself (3 s check) |

The C920's microphone could not be held exclusive for In use: another app (likely NVIDIA
Broadcast) had it open, so `audio_session` fell back to `SharedAfterInUse`; the ezcap's input
works. Every quit of this run logged `closed X ms into the quit` (14–23 ms).

**Closing**: 100 quits in a row with the ezcap's sound playing (start with `--stats`, 4 s of play
after the window shows, `WM_CLOSE` to the window): every one logged `audio session 1 closed X ms
into the quit`, 3–13 ms (median 8); the process exited 160–445 ms after the close (median 315);
no `did not close within` line, no hang (the system's sound not checked by ear). A first run of the
script sent `WM_CLOSE` to winit's hidden `Winit Thread Event Target` window (the process's first
`MainWindowHandle`), which did nothing; it is not counted.

## T044: SC-006 responsiveness on Linux (partial)

### CachyOS — 2026-11-18

From the same logs (106 video sessions):

- Sound and first frame: in 102 sessions the audio reached Playing 167–311 ms **before** the
  first frame (median 283 ms before). The 4 others waited by design: 2 No paired audio (sound
  after an input is picked) and 2 starts with the C920 input gone by the autosuspend reset.
- Input picked → Playing: 195 and 197 ms.
- Input back (PipeWire `added`) → Playing: 223–240 ms (5 times). From the USB `bind` the C920
  needs about 3.3 s, most of it the camera's own enumeration (`device added` 250 ms before the
  audio one).
- Chosen output removed → playing on the default: 76 and 160 ms.

Second run, same day (release `43a5eaa`, ezcap with the C920's microphone as input, output
System default = `vban_out`, `--lang pt-BR --stats`, driven over SSH with `ydotool`):

- Volume and mute: `pw-record` linked to the app's own playback node (`capturax`, by its
  `object.serial`) while `ydotool` clicked the volume button (mute) or turned the wheel over it
  (10 notches = 50 points, 80 % ↔ 30 %, about ±26 dB). The click/wheel time is `date` just before
  `ydotool`; the recording's start is its stop time minus its length, which places a change no
  earlier than it happened (the measured delay can only be too long).

  | Change | Count | Measured | Budget |
  |---|---|---|---|
  | Mute (click): last sample of sound → 20 ms of digital zero | 5 | 40–41 ms (includes the 10 ms ramp) | 100 ms |
  | Unmute (click): first non-zero sample | 5 | 29–30 ms | 100 ms |
  | Volume down (wheel): 10 ms RMS below 60 % of before | 5 | window starting 1–7 ms after | 100 ms |
  | Volume up (wheel): 10 ms RMS above before / 0.6 | 5 | window starting 0 ms after | 100 ms |

  The first attempt's ten clicks changed nothing (the level stayed and the recording never hit
  zero); a screenshot then showed the next click muting, and the repeated run above toggled every
  time. Likely the first click went to a window that had not taken the pointer yet; not seen by
  hand.
- Changed system default output: a null sink `capx-test` loaded, then `wpctl set-default`
  alternating it with `vban_out`, polling `pw-link -l` every 10 ms until `capturax:output_FL`
  linked to the new default: 104–120 ms (6 changes). WirePlumber moves the stream (no
  `target.object` on System default); the audio session does not reopen. The menu named the new
  default at once (`Padrão do sistema (CapturaX-Test)`).
- Audio devices in the menu: with the Audio menu open, a null sink `capx-new` loaded and then
  unloaded; a screenshot finished 864 ms after the load showed `CapturaX-New` in the Output group
  and one finished 761 ms after the unload showed it gone (the open menu updates in place).

Linux SC-006 is met for everything it covers.

## T044: SC-006 responsiveness on Windows

### Windows 11 — 2026-11-19

Release build of `c434a91`, ezcap (no HDMI signal: the PS5 was off), driven by a PowerShell script;
times from `capturax.log` (UTC).

- Sound and first frame: in 66 video starts of the day with sound, the audio reached Playing
  130–438 ms **before** the first frame (most 425–430 ms). The 14 others waited by design
  (Blocked, No paired audio, In use).
- Input picked → Playing: Auto (the ezcap's HDMI) 7–9 ms, the C920's microphone 81–83 ms (6 picks).
- Output picked → playing on it: 69 ms (ARZOPA).
- Reconnected input (C920 back, endpoint `added`) → Playing: 87 ms.
- Chosen output removed → playing on the default: 43 and 45 ms; back on it when it returned.
- Changed system default output (System default selected): the default switched 12 times by a
  script (`IPolicyConfig::SetDefaultEndpoint`, console and multimedia roles) between
  `Voicemeeter In 5` and `Speakers (NVIDIA Broadcast)`, 4 s apart; each reopened the app's output
  on the new default 28–56 ms after the call (`audio Output: Opened` with the new name).
- Audio devices in the menu: with the Audio menu open, screenshots every ~200 ms while the ARZOPA
  (a monitor with speakers) was unplugged and plugged back. The menu updated in place both ways,
  the return within 2.8 s of its output reopening; Windows was moving the window to the new
  monitor layout meanwhile, so no closer time. During each monitor change the render log fell
  0.5–3.4 s behind (the main thread waits on the display change); the audio reopened on time.
- Volume and mute (later the same day, release of `c434a91`, PS5 game music, output
  `Voicemeeter In 5`, volume 80–100): a script recorded a loopback of `Voicemeeter In 5` (only the
  app plays there; `soundcard`, 5 ms reads, each read's samples timed at its return so a delay can
  only be too long) and, on the same `perf_counter` clock, clicked the volume button or sent 10
  wheel notches over it (`mouse_event`). Clip `clips/t044-w-volmute-lb.wav`.

  | Change | Count | At the loopback |
  |---|---|---|
  | Mute (click): start of 20 ms of digital zero | 5 | 112–123 ms (includes the 10 ms ramp) |
  | Unmute (click): first non-zero sample | 5 | 103–111 ms |
  | Volume down (10 notches): 10 ms RMS below 60 % of before | 5 | 100–110 ms (one at 0: the music) |
  | Volume up (10 notches): 10 ms RMS above before / 0.6 | 5 | 100–110 ms (one at 30) |

  The loopback tap itself is late on these VAIO endpoints: beeps written by a Python stream
  (`sounddevice`, WASAPI shared, 22 ms output latency) reached the same loopback 121–142 ms after
  each write (10 beeps, the app muted). The app's change shows earlier than such a fresh sound, so
  click → app output is about 0–20 ms plus the mute's 10 ms ramp, within the 100 ms budget, as on
  Linux (29–41 ms). Recording `Voicemeeter Out B3` instead gave about 150 ms for every change:
  Voicemeeter's mixer and its B3 virtual output add their buffers too.
- Wheel: 10 notches 150, 16 or 5 ms apart moved the level 50 points each way, as designed; 10
  notches sent in the same instant by a script moved it 40 (two lost each way), which a physical
  wheel does not do. Not an app fault worth a change.
- Exclusive box (2026-11-20, release of the T037/T038 work, ezcap, VAIO3): checking the ezcap's
  input reopened only the input, `Exclusive` (I16, 3 ms period), 19 ms after the click; checking
  VAIO3 reopened only the output, `Exclusive` (I24, 2 ms), 12 ms after it; unchecking a box of an
  endpoint not in use reopened nothing. Within 1 s. The exclusive output's resync (T050) reopened
  it in 8 ms, three times, nothing heard.

**The C920's microphone heard late** (maintainer: "a big delay" listening to it). Measured with
`tools/latency/keylat.py` on one `perf_counter` clock: a global key hook for the space bar (a loud mechanical
key), the C920 recorded directly (`sounddevice`, WASAPI shared) and a loopback of the app's output
(`soundcard`); the app played the C920's microphone as the ezcap's input. Cross-correlating the two
recordings in 4 s windows:

| App output | App output behind the microphone |
|---|---|
| `Voicemeeter Input` (VAIO virtual endpoint, the system default then) | 111.4 ms, every window |
| `27G2G4` (NVIDIA HDMI audio, a real endpoint) | 23.6 ms (34 ms in the first second while the bridge settles) |
| Reference: Windows' "Listen to this device" on the C920, playing on `Voicemeeter Input` (app closed) | 144.1 ms, every window |

The app adds ~24 ms on a real endpoint (its `wait_ms` 11 ms, the 10 ms capture period, the render
queue) and is 33 ms faster than Windows' own monitoring on the same virtual endpoint; the
maintainer heard both late, alike. Anything played through Windows on `Voicemeeter Input` shows
~87 ms more at the loopback tap than on a real endpoint; whether that is the VAIO driver's buffer,
its loopback tap or Voicemeeter is not separated here (the maintainer finds Voicemeeter's own
engine fast: hardware devices in exclusive mode). The C920 itself delivers its sound 60–68 ms after
the key press (to any client). Not an app fault. Answered at T028 (latency.md, 2026-11-19): the
virtual inputs add their own buffer to the recorded figure (`Voicemeeter Input` 47 ms over the
hardware strip `Voicemeeter In 5`), so the Windows measurements now play on `In 5`.

A headset plugged as the new default (T036) was not tested: the maintainer has no other headset;
the default change and a device appearing in the menu cover it (above).

## T045: SC-009 acceptance through User Stories 1–5

Most scenarios were already accepted with their measurements: US1 in T028, T029 and T039
(latency.md, drift.md) and in T042 and T044 (closes, sound before the first frame); US2 in T033
and T044; US3 in T036, T042 and T044; US4 in T037–T039 and T042; US5 in T042. T045 checks by hand
what those did not cover.

### Windows 11 — 2026-11-20

Release builds of the working tree, ezcap (PS5) and C920, the ezcap's input and VAIO3 exclusive.

| Scenario | Check | Result |
|---|---|---|
| US1.3 | the video mode, the color range, the theme, the language, the menus and the settings panel changed while playing | **Found**: a mode change closed the audio session with the video session and opened a new one 5–10 ms before the old one had released the ezcap's exclusive input, so the input played shared (`exclusive refused (another app is using the device)`, `SharedAfterInUse`) with `Device in use` / `Exclusive mode unavailable`; the sound went on, reopened. Fixed by T051 and T052 (below) |
| US1.4 | hidden to the tray, shown again, quit | sound stops with the video and returns with it |
| US2.4 | muted, and the volume at 0 | the button shows the muted speaker |
| US1 | playing on the PS5 for a few minutes | sound and image feel right (maintainer) |
| US4.4 | the ezcap's input exclusive, then another input or the video device closed | another app can use the input at once |

After T051 and T052 (release build of the fix): six mode changes in a row (1080p60 ↔ 1080p30)
each logged `video session N on the same device keeps the audio session`; audio session 1 played
through all of them, exclusive, with no new open and no notice. Then ezcap → C920 → ezcap: each
new session logged `waits for session(s) [n] to close` and opened 7 and 16 ms later, when the
older one's `Closed` came; the ezcap's input came back `Exclusive` (3 ms) with VAIO3 `Exclusive`
(2 ms), no notice. The maintainer heard no gap.

### CachyOS — 2026-11-20

| Scenario | Check | Result |
|---|---|---|
| US4.5 | the settings panel (release of `9f5ecef`, pt-BR) | categories Geral, Aparência, Preview, Sobre: no Audio category and no exclusive mode |
