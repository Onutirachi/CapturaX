# Linux engine probe (T006)

The gate of research R3: `crates/platform/examples/pipewire_probe.rs` (T005) plays the ezcap's
PipeWire node on the default output through two streams in one node group, as the app will.
Steps and criteria: quickstart "Linux engine probe".

## CachyOS — 2026-11-16

**Environment**: the maintainer's Linux PC, kernel `7.3.1-1-cachyos`, PipeWire 1.6.10
(`pipewire --version`: compiled and linked with libpipewire 1.6.10), WirePlumber 0.5.18,
`pipewire` crate 0.10. Graph settings: `clock.rate` 48000, `clock.quantum` 1024,
`clock.min-quantum` 32. Input: `ezcap Live GAMER ULTRA Estéreo analógico`
(`alsa_input.usb-ezcap_ezcap_Live_GAMER_ULTRA_00000001-02.analog-stereo`, S16LE, 2 channels,
48000 Hz) with the PS5 on: a 3 s `pw-record` of the node peaked at −30 dBFS, so it carried
sound. Default output: `Áudio interno Estéreo digital (IEC958)`
(`alsa_output.pci-0000_00_1f.3.iec958-stereo`).

**Conditions**: run by the agent over SSH with the release build
(`target/release/examples/pipewire_probe`, defaults: `--input ezcap --latency 128`); nobody
listened to the output. OBS was open during every run, with one stream on the ezcap's node and
one on the default output's monitor (both asking for a 1200-frame quantum). The probe compiled
on the first build, unchanged from T005.

| Step | Criterion | Result | |
|---|---|---|---|
| 1. Hang | 10 × `timeout -s INT -k 5 3`: exit within 1 s of the signal, no stuck client | 10 of 10: exit code 0, `closed in 0 ms`, 3002 ms from start to exit; no probe node or client left in `pw-cli ls` | pass |
| 2. Starvation | 60 s: delivered rate within ±500 ppm of 48 kHz, `gap_ms` 0 after the first second | 59 stats lines, `gap_ms` 0.0 and `drop_ms` 0.0 in all of them; `delivered_ppm` +348 at 59 s; from second 7 on the input delivered 48000 ± 1 frames each second | pass |
| 3. Format | `pw-dump`: capture stream at 2 channels FL FR, F32 | both streams `F32LE`, 48000 Hz, 2 channels, `["FL", "FR"]` | pass |
| 4. One cycle | `pw-top`: both streams under one driver; rate difference within ±50 ppm | the ezcap's node drives at 128/48000; the output, both probe streams and OBS follow it; `delivered_ppm − played_ppm` between −1 and +1 in every line | pass |

**Gate: passed.** The Linux engine is PipeWire directly (research R3); T019 and T020 go ahead.

### Step 1

`timeout --preserve-status` was used so the probe's own exit code shows (plain `timeout` returns
124 whenever it had to send the signal); a first series with the quickstart's exact command gave
the same `closed in 0 ms` ten times.

```text
run 1 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 2 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 3 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 4 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 5 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 6 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 7 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 8 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 9 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
run 10 exit=0 total_ms=3002 closed in 0 ms stats_lines=2 gaps=0
```

### Steps 2 to 4 (one 60 s run)

```text
   0.001 PipeWire library 1.6.10
   0.002 default sink: {"name":"alsa_output.pci-0000_00_1f.3.iec958-stereo"}
   0.002 input: alsa_input.usb-ezcap_ezcap_Live_GAMER_ULTRA_00000001-02.analog-stereo
   0.002 capture: Unconnected -> Connecting
   0.002 playback: Unconnected -> Connecting
   0.003 capture: Connecting -> Paused
   0.003 playback: Connecting -> Paused
   0.011 capture: format AudioFormat::F32LE 48000 Hz 2 channels [3, 4]
   0.012 capture: Paused -> Streaming
   0.014 playback: format AudioFormat::F32LE 48000 Hz 2 channels [3, 4]
   0.014 playback: Paused -> Streaming
   1.002 [probe] delivered_ppm=+4617 played_ppm=+4616 gap_ms=0.0 drop_ms=0.0 wait_ms=2.7 wait_max_ms=5.3 in_period=128 out_period=128
   2.002 [probe] delivered_ppm=+6323 played_ppm=+6323 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   3.002 [probe] delivered_ppm=+6888 played_ppm=+6888 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   4.002 [probe] delivered_ppm=+5825 played_ppm=+5825 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   5.002 [probe] delivered_ppm=+4657 played_ppm=+4657 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   6.002 [probe] delivered_ppm=+3432 played_ppm=+3432 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   7.002 [probe] delivered_ppm=+2940 played_ppm=+2940 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
   ...
  58.002 [probe] delivered_ppm=+354 played_ppm=+354 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
  59.002 [probe] delivered_ppm=+348 played_ppm=+348 gap_ms=0.0 drop_ms=0.0 wait_ms=4.7 wait_max_ms=4.7 in_period=128 out_period=128
  60.000 signal: closing
  60.000 capture: Streaming -> Paused
  60.000 capture: Paused -> Unconnected
  60.000 playback: Streaming -> Paused
  60.000 playback: Paused -> Unconnected
  60.000 loop quit
  60.000 closed in 0 ms
```

`wait_ms` stayed at 4.7 ms (224 frames: one 128-frame period plus the 2 ms margin) from second 2
to the end. `pw-top` 20 s into the run (`+` marks a follower of the driver above it):

```text
S   ID  QUANT   RATE    WAIT    BUSY   W/Q   B/Q  ERR FORMAT           NAME
R   55    128  48000  23,1us   0,4us  0,01  0,00    0    S16LE 2 48000 alsa_input.usb-ezcap_ezcap_Live_GAMER_ULTRA_00000001-02.analog-stereo
R   56      0      0   2,3us   5,1us  0,00  0,00    6    S32LE 2 48000  + alsa_output.pci-0000_00_1f.3.iec958-stereo
R   81   1200  48000   2,4us   2,2us  0,00  0,00    0    S32LE 2 48000  + OBS
R   82   1200  48000   3,7us   3,3us  0,00  0,00    0    S16LE 2 48000  + OBS
R   97    128  48000   4,6us   2,6us  0,00  0,00    0    F32LE 2 48000  + pipewire_probe
R   98    128  48000   7,3us   1,7us  0,00  0,00    0    F32LE 2 48000  + pipewire_probe
```

The output's `ERR` 6 was already 6 before the probe started. `pw-dump` at the same moment:

```text
97 pipewire_probe Stream/Input/Audio  node.group=capturax.preview node.latency=128/48000 media.role=Game state=running
   target.object=alsa_input.usb-ezcap_ezcap_Live_GAMER_ULTRA_00000001-02.analog-stereo
   Format: {"mediaType": "audio", "mediaSubtype": "raw", "format": "F32LE", "rate": 48000, "channels": 2, "position": ["FL", "FR"]}
98 pipewire_probe Stream/Output/Audio node.group=capturax.preview node.latency=128/48000 media.role=Game state=running
   Format: {"mediaType": "audio", "mediaSubtype": "raw", "format": "F32LE", "rate": 48000, "channels": 2, "position": ["FL", "FR"]}
links (2 each, active): ezcap node -> pipewire_probe, pipewire_probe -> alsa_output.pci-0000_00_1f.3.iec958-stereo
```

## CachyOS — 2026-11-16, second series, OBS closed

Same PC, build and commands, about ten minutes later, after the maintainer closed OBS: the
ezcap's node and the output were both suspended before each start, and the probe's two streams
were the only clients.

| Step | Result | |
|---|---|---|
| 1. Hang | 10 of 10: exit code 0, `closed in 0 ms`, 3001–3002 ms from start to exit; no probe node or client left | pass |
| 2. Starvation | 59 stats lines, `gap_ms` 0.0 and `drop_ms` 0.0 in all of them; `delivered_ppm` +573 at 1 s, +55 at 59 s; 48000 frames each second from second 2 on, but for one second (12 to 13) with 128 more, on both sides | pass |
| 3. Format | both streams `F32LE`, 48000 Hz, 2 channels, `["FL", "FR"]` | pass |
| 4. One cycle | the ezcap's node drives at 128/48000, the output and both probe streams follow; `delivered_ppm − played_ppm` between 0 and +10 (the playback's first buffer came 3 ms after the capture's) | pass |

`wait_ms` was 4.7 ms in every line. PipeWire's journal has no entry for the whole series.

```text
R   55    128  48000  16,7us   0,3us  0,01  0,00    0    S16LE 2 48000 alsa_input.usb-ezcap_ezcap_Live_GAMER_ULTRA_00000001-02.analog-stereo
R   56      0      0   2,1us   7,7us  0,00  0,00    6    S32LE 2 48000  + alsa_output.pci-0000_00_1f.3.iec958-stereo
R   80    128  48000   2,5us   2,8us  0,00  0,00    0    F32LE 2 48000  + pipewire_probe
R   82    128  48000   5,3us   1,0us  0,00  0,00    0    F32LE 2 48000  + pipewire_probe
```

## Findings

- **Both start effects below came with OBS.** With OBS closed the input was 27 frames ahead of
  the wall clock from the first second (no fast start) and the journal logged no `resync`. They
  are what happens when the preview starts while another app already runs the card's node and
  the output at a larger quantum (OBS asked for 1200 frames): the graph drops to 128 and the
  devices catch up. Still worth hearing in T029 with another app open on the card.
- **The first seconds run fast (first series, OBS open).** `delivered_ppm` is counted since the first input buffer; it
  starts high and falls as 1/t (+4617 at 1 s, +2056 at 10 s, +348 at 59 s). Per second, the
  input delivered about 385 frames more than 48000 in seconds 2 and 3, was 1116 frames ahead of
  the wall clock at second 4, gave 129 back at second 6 and then stayed 986 ± 1 frames ahead to
  the end. The output played the same counts, so nothing was cut or filled: the whole graph ran
  up to 0.8 % fast for about 4 s while the ezcap's node took over as driver. The 3 s runs show
  the same start (+2730 to +17820 at 1 s). The steady rate is 48000 Hz within 21 ppm (one frame
  a second, the resolution of this count).
- **The output resyncs at the start (first series, OBS open).** PipeWire's journal has one line `spa.alsa: iec958:0p:
  follower avail:258 delay:258 target:128 thr:128, resync` (with 2 to 6 more suppressed) within
  3 s of the start of each series and of the 60 s run, none later in that run and none in the
  three days before. The probe's `gap_ms` cannot see it: it is the output device joining the
  ezcap's cycle. Whether it is heard when the preview's sound starts is for the listening test
  (T029).
- **For T019**: the `default` metadata's `default.audio.sink` value is JSON
  (`{"name":"alsa_output.pci-0000_00_1f.3.iec958-stereo"}`), as the task says.
- **Not covered**: an output on another card (HDMI on the NVIDIA card); nobody listened in
  either series.
