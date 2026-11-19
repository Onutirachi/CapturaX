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

Linux SC-006 is met for everything it covers. Left for Windows: the whole T044 Windows half,
including the exclusive box (Windows only).
