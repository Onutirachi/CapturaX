# Data Model: Stack Spike

The spike stores measurements and findings, not application data.

## Environment

Recorded once per platform at the top of the findings report.

| Field | Example |
|---|---|
| platform | Windows 11 24H2 / CachyOS (kernel, session: Wayland or X11) |
| gpu, driver | RTX 5070 Ti, driver version |
| capture card, firmware | ezcap Live Gamer Ultra |
| source | PlayStation 5, A/V sync test video |
| reference display | TV model, game mode on |
| monitor | model, refresh rate |
| camera | Galaxy S23 FE, 240 fps |
| versions | GStreamer, gstreamer-rs, Slint, OBS, Edge |

## Video sample

One row per flash detected in a clip.

| Field | Rule |
|---|---|
| clip | file name of the slow-motion clip |
| path | `native`, `slint-cpu`, `slint-gpu`, `browser`, `obs` |
| variant | sink or renderer used (e.g. `d3d12videosink`, `encoder-on`) |
| mode | e.g. `1920x1080@60 NV12` |
| platform | `windows` or `linux` |
| flash_index | order in the clip |
| delay_ms | preview flash time − TV flash time; must be ≥ 0 |

Validation: a clip with fewer than 5 detected flash pairs is rejected; delays above 500 ms are
flagged as detection errors and excluded.

## Audio sample

| Field | Rule |
|---|---|
| clip | file name |
| platform | `windows` or `linux` |
| output | output device |
| minute | minutes since playback started (for drift) |
| beep_index | order in the clip |
| delay_ms | PC beep time − TV beep time |

## Pipeline stats

One row per second of a run with `--stats`: run, path, variant, mode, platform, second,
rendered frames, dropped frames, in-pipeline latency (ms). Source of the dropped-frame percentage
for SC-004.

## Summary row

Per (path, variant, mode, platform): sample count, median, 95th percentile, min, max, dropped
frames (when available). Audio: median, 95th percentile, A/V offset (audio median − video median of
the same session), drift (last − first median).

## Capability result

| Field | Values |
|---|---|
| capability | one of the items in spec User Story 4 |
| outcome | `working`, `limited`, `not possible` |
| platform | `windows`, `linux`, `both` |
| notes | limits, workaround, effort |

## Findings report sections

Environment; method; video results (table per platform); encoder results; audio results;
capabilities; visual directions (screenshots, choice and reasons); decisions (Slint status,
chosen path, chosen direction); open questions for later specs.
