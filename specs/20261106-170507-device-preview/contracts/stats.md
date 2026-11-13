# Contract: Presentation Statistics (additions)

Extends the app shell's [stats contract](../../20261104-112852-app-shell/contracts/stats.md):
existing fields keep their meaning and order; new fields are added at the end, so measurement
scripts keep working (FR-018).

```text
[present] backend=dx12 mode=mailbox adapter="NVIDIA GeForce RTX 5070 Ti" surface=1920x1080
[source] device="ezcap Live Gamer Ultra" id="mf:…" mode=1920x1080@60 format=NV12 matrix=bt709 range=full(auto)
[loop] t=12 recv=60 pres=60 repl=0 ready_ms=0.9/1.4 gap_ms=17.9 ui=0 ui_ms=-/- moves=0 lat_ms=2.1/3.0 rep=0
```

| Line / field | Meaning |
|---|---|
| `[present] mode` | Adds `fifo-follow` for the Follow the monitor setting (research R9) |
| `[source]` | Printed when a source starts playing or its range in use changes: device, id, mode, format, matrix, range (`(auto)` when inferred) |
| `lat_ms` | Measured latency, captured → presented: median / max this second (research R11); `-/-` without frames |
| `rep` | Presents that repeated the last frame (Follow the monitor only; 0 otherwise) |

Frames dropped before the app show as `recv` below the mode's nominal rate; frames dropped by
the app show as `repl`.

## Present log (`--present-log <file>`)

For the stall investigation (research R8). One line per event, tab-separated, raw counter values
from `platform::qpc()`:

```text
acquire	123456789012
present	123456789345	frame=1
event	123456790000	focus=false
event	123456800000	fullscreen=true
stage	123456810000	language=catalog	0.4
```

- `acquire`: just before asking the swap chain for an image, so a blocking acquire shows as the
  time to the next `present`.
- `present`: when the present returned; `frame` counts the video thread's presents from 1.
- `event`: when the window gains or loses focus, and when the app asks to enter or leave
  fullscreen.
- `stage`: the per-stage timing of a language switch in ms (FR-024), written when the stage ends:
  `catalog` (selecting the catalog), `property` (the new texts to Slint, the tray and the title),
  `draw` (the first UI image after the switch, main thread), `upload` (that image, video thread),
  `present` (from that upload to the return of the present that shows it).

The file is created or emptied at start, after the single-instance check; if it cannot be
created the app exits with code 64. With `--stats` the stages are also printed as
`[language] stage=catalog ms=0.4`.
