# Contract: Command Line

Binary `capturax`. Users see only the start options; the developer options are not shown in the
interface (FR-004) but are listed by `--help`.

## Start options (FR-012a, FR-014b)

| Option | Effect |
|---|---|
| `--fullscreen` | Start in fullscreen for this start only; the saved setting is unchanged |
| `--windowed` | Start windowed for this start only, whatever Start in fullscreen says |
| `--autostart` | Written into the system's autostart entry; with Keep in tray on and a tray available, start hidden in the tray |

`--fullscreen` and `--windowed` together are an error (exit code 64).

## Developer options

| Option | Effect |
|---|---|
| `--test-video <PRESET\|WxH@FPS>` | Play the synthetic moving test video; presets `1080p60`, `1440p60` (FR-004) |
| `--stats` | Print presentation statistics once per second to stdout and the log ([stats.md](stats.md)) |
| `--backend <dx12\|vulkan\|gl>` | Force the graphics backend (research R4) |
| `--present <mailbox\|immediate\|fifo>` | Force the present mode |
| `--lang <CODE>` | Use this language for this start, without saving it |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Normal quit, or a second start that handed over to the running instance |
| 2 | No usable renderer (FR-036), after the system message |
| 64 | Invalid command line |
| 69 | `--test-video` asked but GStreamer or `videotestsrc` is missing |
