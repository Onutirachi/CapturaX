# Contract: Presentation Statistics (additions)

Extends the [device preview's stats contract](../../20261106-170507-device-preview/contracts/stats.md).
Existing fields keep their meaning and order. Two fields are added at the end of the `[loop]` line:

```text
[loop] t=12 recv=60 pres=60 repl=0 ready_ms=0.9/1.4 gap_ms=17.9 ui=1 ui_ms=0.6/0.6 moves=0 lat_ms=2.1/3.0 rep=0 glow=60 up_ms=0.1/0.1
```

| Field | Meaning |
|---|---|
| `glow` | Glow updates this second (research R1). It is 0 when the ambient light is off, when the video fills the area (no bars), or when no video is on screen. At 120 fps it is at most 60. |
| `up_ms` | UI image upload on the video thread: median / max (`-` when none), the time `set_ui` takes for each of this second's `ui` images (research R9). `ui_ms` keeps its meaning: draw and copy on the main thread. |

SC-001 runs compare `ready_ms` and `repl` with `glow` > 0 against runs with `glow=0`. The `ui`
field (UI uploads per second) shows the pinned bubbles' redraws, and `up_ms` the cost of
each upload, which the partial upload keeps small (research R9).
