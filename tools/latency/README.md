# Latency tools

Glass-to-glass latency measurement, adopted from the stack spike
([findings](../../specs/20261026-154808-stack-spike/findings.md), research R2, R3 and R12). Every
spec that touches the preview path states a latency budget and measures it (constitution
principle I); these tools are the method the spike used.

| File | Purpose |
|---|---|
| `flash.html` | Page that flashes white for 50 ms once per second. Serve it from the PC (for example `live-server`) and open it in the console's browser. |
| `make_sync_video.py` | Makes a 1080p60 video with a white flash and a 1 kHz beep once per second, for a media player on the console. |
| `analyze.py` | Measures the delays in the clips and writes CSV samples and a summary into a spec's measurements folder. |
| `pipewire-measure.sh` | Linux: records the preview audio's own delay inside the PC with a PipeWire null sink (below). |
| `keylat.py` | Windows: how late an app plays a microphone, from the keyboard's clicks (below). |

Run from the repository root with [uv](https://docs.astral.sh/uv/), which installs the
dependencies declared in each script.

## Method

1. Show the source on the reference display (the capture card's passthrough) and the preview
   fullscreen next to it.
2. Film both screens for 25 seconds at 240 fps.
3. Put the clip in `clips/` at the repository root (ignored by git) and analyze it:

   ```sh
   uv run tools/latency/analyze.py --measurements specs/<spec>/measurements video clips/<clip>.mp4 \
       --path <path> --variant <variant> --mode 1920x1080@60:NV12 --platform windows
   ```

   On the first run a window asks for the reference region, then the preview region; they are
   saved next to the clip. Add `--fps 240` when the clip stores its playback rate.
4. Audio: record where every sound is heard twice, reference first (for example the reference
   on the left channel and the preview on the right with `--stereo`), and run `audio`:

   ```sh
   uv run tools/latency/analyze.py --measurements specs/<spec>/measurements audio clips/<clip>.wav \
       --stereo --platform linux --output <device or setting> --minute 0
   ```

   `--minute` is the minute of the session the clip was taken in, for a run measured in short
   clips.
5. Drift: record a whole long run (30 minutes or more, a sound every few seconds) and run
   `audio` with `--drift` instead of `--minute`. Each pair's minute is then its time in the
   recording (by the reference's onset). It prints each minute's pairs, median and p95, then the
   medians of the first and the last minute with at least 5 pairs, their difference, and the
   least-squares slope of every pair's delay over its time, in ms per minute (positive: the
   preview falls behind). Those go to `audio-drift.csv`, one row per clip, and the pairs to
   `audio-samples.csv` as usual. Onsets are found to the millisecond.
6. `summary` writes `summary.md` with median, p95 and dropped frames per path and mode, the audio
   per minute, and a table of the drift runs.

## Preview audio inside the PC (Linux)

Linux has no second recorder, so the preview audio is measured inside PipeWire (research R10 of
the [preview-audio spec](../../specs/20261113-103922-preview-audio/research.md)). With the app
playing a preview with sound:

```sh
tools/latency/pipewire-measure.sh start                       # needs jq and the PipeWire tools
tools/latency/pipewire-measure.sh record clips/linux-1.wav 25
tools/latency/pipewire-measure.sh stop
uv run tools/latency/analyze.py --measurements specs/<spec>/measurements audio clips/linux-1.wav \
    --stereo --platform linux --output pipewire-128 --minute 0
```

`start` creates a null sink `capturax-measure` and links into it:

- **left**: the left channel of the device that feeds the app's audio input (the card), as it
  arrives;
- **right**: the app's playback left channel, the same sound after the preview's path.

The app's own links are untouched, so it keeps playing on the real output. Every linked node
runs under one driver and both channels are written in the same graph cycles, so the clip's
delay is the app's own; add the real output's latency, `out_latency_ms` in the app's
`[audio-source]` stats line (`--stats`) of the same run. `record` records the sink's monitor
for the given seconds (the whole run for a drift recording) and refuses when the links are gone,
as after an app restart: run `start` again. `stop` removes the sink and its links.

## A microphone played by the app (Windows)

`keylat.py` needs no capture card or camera: with the app playing a microphone (for example the
C920's as a video device's input) on some output, it records on one clock the space bar's presses,
the microphone directly and a loopback of that output:

```sh
uv run tools/latency/keylat.py --mic C920 --loopback "27G2G4" --seconds 40 --save clips/k-1.npz
```

Tap the space bar of a loud mechanical keyboard (short taps, any window focused) or speak near the
microphone. It prints each press's click onset in both streams (key → microphone is the
microphone's own delay) and, by cross-correlation in 4 s windows, how far the output trails the
microphone: what the app adds plus the output endpoint up to its loopback tap. A loopback taps
where Windows mixes the endpoint, not where the sound is heard, and an endpoint held exclusive
(such as a headset Voicemeeter uses) cannot be looped back; on a virtual endpoint compare with a
reference on the same one, such as Windows' "Listen to this device". Results of the preview-audio
spec: its measurements/acceptance.md, T044 on Windows.

Options, file formats and the rules for rejected samples are in the spike's
[contracts/cli.md](../../specs/20261026-154808-stack-spike/contracts/cli.md),
[contracts/measurements.md](../../specs/20261026-154808-stack-spike/contracts/measurements.md) and
[data-model.md](../../specs/20261026-154808-stack-spike/data-model.md).
