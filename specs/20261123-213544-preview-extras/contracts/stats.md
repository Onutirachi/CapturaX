# Contract: Presentation Statistics (additions)

Extends the [device preview's stats contract](../../20261106-170507-device-preview/contracts/stats.md).
Existing fields keep their meaning and order. One field is added at the end of the `[loop]` line:

```text
[loop] t=12 recv=60 pres=60 repl=0 ready_ms=0.9/1.4 gap_ms=17.9 ui=0 ui_ms=-/- moves=0 lat_ms=2.1/3.0 rep=0 glow=60
```

| Field | Meaning |
|---|---|
| `glow` | Glow updates this second (research R1). It is 0 when the ambient light is off, when the video fills the area (no bars), or when no video is on screen. At 120 fps it is at most 60. |

SC-001 runs compare `ready_ms` and `repl` with `glow` > 0 against runs with `glow=0`. The `ui`
field (UI uploads per second) shows the pinned bubbles' redraws (research R9).
