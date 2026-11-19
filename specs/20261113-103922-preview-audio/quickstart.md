# Quickstart: Preview Audio

How to validate the feature on the maintainer's machines. Build and run as in the
[README](../../README.md); `--stats` lines are in [contracts/stats.md](contracts/stats.md).
Results go to `measurements/` (`latency.md`, `drift.md`, `acceptance.md`). Following the batch
rule, every check that needs the ezcap on one PC is done before moving it.

## Before the engine: validation steps

These run first; the code that depends on them waits for their result.

### Linux engine probe (research R3)

A small example in `crates/platform/examples/pipewire_probe.rs` plays the ezcap's node on the
default output through two streams in one node group, with `--stats`.

1. **Hang**: 10 runs of `timeout -s INT -k 5 3 <probe>`; each must exit within 1 s of the signal,
   and `pw-top` must show no stuck client. (Spike: 4 and 7 of 10 hung with `pipewiresink`.)
2. **Starvation**: 60 s run; the input's delivered rate within ±500 ppm of 48 kHz and
   `gap_ms` 0 after the first second. (Spike: 17–25 % delivered with `pipewiresrc`.)
3. **Format**: `pw-dump` shows the capture stream at 2 channels FL FR, F32.
4. **One cycle**: `pw-top` shows both streams under the same driver; `rate_ppm` stays within
   ±50 ppm.

Pass → PipeWire directly. Fail → stop and decide with the maintainer (research R3).

### Pairing (research R5)

Run `capturax --list-devices` on each system with the ezcap and the C920 plugged in:

- Windows: the ezcap's video and its `Digital Audio Interface` input show the same container id;
  the C920's video and microphone the same; `pair` gives each video device its own input.
- Linux: the V4L2 bus path and the PipeWire device's bus path, both without the interface suffix,
  are equal for each device; `pair` as above.

If a link is missing, the name fallback must still pair them; record the printed keys in
`measurements/pairing.md`.

## Automated

```sh
cargo test -p capturax-audio          # bridge, drift simulation, formats, volume, pairing
cargo test -p capturax                # audio state machine, settings, boundary
```

## Latency budget (SC-001 to SC-004)

Method: research R10; reference and limits: spec "Latency budget and how it is measured".

**Windows** (ezcap, PS5 menu sounds, Voicemeeter's bus B3 carrying the line input panned left and
the app's output panned right, the app's output = `Voicemeeter In 5`, a hardware input strip in
Voicemeeter, not a virtual input; research R10). Record B3 (`Voicemeeter Out B3`) 25 s, stereo,
48 kHz, for example with `sounddevice`. In the same session, also record the spike's engine on
the same output (`audio-preview --engine wasapi --input ezcap --output "In 5"`, built from the
tag `spike/stack-20261026`) and one clip on the everyday virtual input (VAIO3) for its extra
delay:

| Run | Settings (Audio category) | Clip | Budget |
|---|---|---|---|
| shared | none exclusive | 25 s | median ≤ 62 ms, p95 ≤ median + 5 |
| input exclusive | the ezcap input | 25 s | ≤ 50 ms |
| both exclusive | ezcap input and the virtual input | 25 s | ≤ 22 ms |

```sh
uv run tools/latency/analyze.py --measurements specs/20261113-103922-preview-audio/measurements \
    audio clips/<clip>.wav --stereo --platform windows --output <run> --minute 0
```

A/V: in the same session measure spec #3's glass to glass (its quickstart), then offset = audio −
video; within 45 ms ahead and 125 ms behind (SC-004).

**Linux** (CachyOS, ezcap, PS5):

```sh
capturax --stats                            # preview of the ezcap, audio playing on the real output
tools/latency/pipewire-measure.sh start     # null sink capturax-measure: card left, app right
tools/latency/pipewire-measure.sh record clips/linux-<n>.wav 25
tools/latency/pipewire-measure.sh stop      # removes the sink and its links
uv run tools/latency/analyze.py … audio clips/linux-<n>.wav --stereo --platform linux --output pipewire-128 --minute 0
```

Read `out_latency_ms` from the `[audio-source]` line of the same run (the app keeps playing on
the real output while the sink records); the result is the measured median plus that value:
median ≤ 25 ms, p95 ≤ 30 ms (SC-002). Variant: `node.latency` 64/48000 and 256/48000, recorded
for comparison.

## Drift and continuity (SC-003, SC-005)

For each Windows setting and for Linux: 31 minutes of play with `--stats` logged (so minute 30
has pairs), recording as above for the whole run with a sound every few seconds, then:

```sh
uv run tools/latency/analyze.py … audio clips/<run>.wav --stereo --platform <os> --output <run> --drift
```

It prints each minute's median, minute 0 and the last minute, their difference and the slope in
ms per minute, and adds a row to `measurements/audio-drift.csv`. Pass: minute 30 within 10 ms of
minute 0; the `[audio]` lines show `wait_ms` steady, `gap_ms`
and `drop_ms` 0 after the first second; the maintainer hears no click, dropout or pitch change.
Record in `measurements/drift.md`.

Hardware output (spec Assumptions): one 30-minute run on the G533 directly, by ear and by the
stats only.

## Responsiveness (SC-006)

With `--stats` and the log's timestamps: sound within 1 s of the first frame; change of input,
output or exclusive box plays within 1 s; volume and mute heard within 100 ms; an audio device
plugged or removed in the menu within 3 s; a reconnected input and a changed default output play
within 3 s.

The log has no line for a volume change, a changed default or the menu's rows. On Linux:
volume and mute from `pw-record --target <object.serial of the capturax playback node>` while
`ydotool` clicks the volume button or turns the wheel over it (time the recording from its stop
minus its length); the default change by polling `pw-link -l` after `wpctl set-default`; the menu
with screenshots of the open Audio menu after loading or unloading a null sink (by id).

## Video unchanged (SC-007)

With the sound playing (shared and both exclusive on Windows; Linux): spec #3's glass to glass
(Windows) and frame ready → presented and frame rate (both) still within their budgets. Compare
with `--no-audio` in the same session.

## States and closing (SC-008)

Force each state, in English and Portuguese, and use its action:

| State | How |
|---|---|
| No paired audio | a video device without an audio input: a virtual camera on Windows, `v4l2loopback` on Linux |
| Disconnected | pick the C920's microphone as the ezcap's input and unplug the C920 (Linux: `echo 1-1 \| sudo tee /sys/bus/usb/drivers/usb/unbind`, then `bind`); plug it back: sound within 3 s. The ezcap is a PCIe card and cannot be unplugged |
| Blocked | Windows: Settings → Privacy → Microphone → desktop apps off |
| In use | Windows: hold the input exclusive in another app while ours opens shared: `cargo run --release -p capturax-platform --example audio_session -- <input id> --exclusive-in --output <another output> --seconds 60` (Try again plays once it quits) |
| Failed | Windows: an error other than gone, in use or blocked (for example the Windows Audio service not running); none found that can be forced without administrator rights |
| Output fallback | pick the G533, then unplug its receiver; plug it back: sound moves back. Linux: `pactl load-module module-null-sink sink_name=capx-test`, pick it, then `pactl unload-module <its id>` (never by name: that also removes other null sinks, such as VBAN's) and load it again |

Closing: 100 closes in a row with sound playing on each system (switching the video device back
and forth, and quitting with `--stats`), none over 1 s in the log (`audio session N closed in X ms`
after a switch, `closed X ms into the quit` at a quit; a `did not close within` line fails), no hang
of the app or of the system's sound.

## Acceptance (SC-009)

The maintainer plays on Windows 11 and on CachyOS KDE Plasma Wayland with the ezcap and the C920
through User Stories 1 to 5; results in `measurements/acceptance.md`.
