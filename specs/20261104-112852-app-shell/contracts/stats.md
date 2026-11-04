# Contract: Presentation Statistics

Printed with `--stats` to stdout and the log (FR-005). One header when the video thread starts
or the presentation changes, then one line per second. Later specs (#3 and on) keep these fields
and may add new ones at the end, so measurement scripts keep working.

```text
[present] backend=dx12 mode=mailbox adapter="NVIDIA GeForce RTX 5070 Ti" surface=1920x1080
[loop] t=12 recv=60 pres=60 repl=0 ready_ms=0.9/1.4 gap_ms=17.9 ui=0 ui_ms=-/- moves=0
```

| Field | Meaning |
|---|---|
| `t` | Seconds since the video thread started |
| `recv` | Frames received from the source this second |
| `pres` | Presents with a new video frame |
| `repl` | Frames replaced in the inbox before being presented |
| `ready_ms` | Frame ready → present returned: median / max (SC-001) |
| `gap_ms` | Longest time between two presents with a new frame (SC-003) |
| `ui` | UI images handed to the video thread |
| `ui_ms` | UI draw + copy on the main thread: median / max (`-` when none) |
| `moves` | Window move events (shows when a reading was taken during a move) |

SC-002 reads `pres` (59–61 at 60 fps); SC-001 reads `ready_ms`; SC-003 reads `gap_ms` and `pres`
while moving.
