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
   on the left channel and the preview on the right with `--stereo`), and run `audio`.
5. `summary` writes `summary.md` with median, p95 and dropped frames per path and mode.

Options, file formats and the rules for rejected samples are in the spike's
[contracts/cli.md](../../specs/20261026-154808-stack-spike/contracts/cli.md),
[contracts/measurements.md](../../specs/20261026-154808-stack-spike/contracts/measurements.md) and
[data-model.md](../../specs/20261026-154808-stack-spike/data-model.md).
