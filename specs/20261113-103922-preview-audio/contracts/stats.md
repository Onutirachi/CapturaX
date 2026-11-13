# Contract: Statistics (additions)

Extends the [device preview's stats contract](../../20261106-170507-device-preview/contracts/stats.md):
existing lines and fields are unchanged; two new lines (FR-019, research R11).

```text
[audio-source] session=3 input="Digital Audio Interface (ezcap Live Gamer Ultra)" in_mode=exclusive in_format=48000/2/i32 in_period_ms=3.00 output="Speakers (Logitech G533)" out_mode=shared out_format=48000/2/f32 out_period_ms=2.67 out_latency_ms=8.1
[audio] t=12 wait_ms=6.3/7.9 rate_ppm=+42 gap_ms=0.0 drop_ms=0.0
```

| Line / field | Meaning |
|---|---|
| `[audio-source]` | Printed when an audio session opens and when a side reopens |
| `in_mode`, `out_mode` | `shared`, `exclusive`, `shared-in-use` (exclusive asked and refused) |
| `in_format`, `out_format` | rate / channels / sample format the device runs in |
| `in_period_ms`, `out_period_ms` | the period each side opened with |
| `out_latency_ms` | the latency the output reports (Windows `GetStreamLatency`, Linux `pw_stream_get_time` delay); added to the Linux measurement (research R10) |
| `[audio] t` | seconds, as `[loop] t` |
| `wait_ms` | audio waiting in the app at output events this second: median / max; `-/-` when not playing |
| `rate_ppm` | the follower's mean correction this second |
| `gap_ms` | audio the output asked for and did not get this second (played as silence) |
| `drop_ms` | audio dropped this second (priming after the first second, stalls) |

The `[audio]` line is printed only while an audio session is open.
