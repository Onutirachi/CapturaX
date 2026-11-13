# Contract: Command Line (additions)

Adds developer options to the app shell's [command line](../../20261104-112852-app-shell/contracts/cli.md);
start options and exit codes are unchanged. Not shown in the interface.

| Option | Effect |
|---|---|
| `--list-devices` | Prints every device (name, id) with its modes, the shown automatic modes and what they resolve to, then exits 0; exit 69 if GStreamer cannot start. One mode per line in aligned columns, its settings text first: `1920x1080@60   NV12  [Full HD] [16:9]` (data-model.md "Mode", Text output) |
| `--present-log <FILE>` | Writes the present log (contracts/stats.md) |

`--test-video` now plays through the capture crate in NV12 (research R12), with the same presets.
With `--test-video`, no device is opened.
