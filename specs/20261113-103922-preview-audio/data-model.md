# Data Model: Preview Audio

Entities of [spec.md](spec.md) "Key Entities", with the rules the plan adds. Names are
indicative; tasks fix the exact types.

## Audio endpoint

One audio input or output of the computer (research R6).

| Field | Meaning |
|---|---|
| `id` | Stable key: Windows endpoint id (`{0.0.1.00000000}.{guid}`), Linux `node.name` (`alsa_input.usb-…`) |
| `name` | The system's name (Windows friendly name, Linux `node.description`); numbered " (2)" when repeated, as video devices |
| `flow` | `Input` or `Output` |
| `link` | The physical device key for pairing (research R5): Windows container id, Linux USB path without the interface suffix; `None` when the system gives none |
| `format` | Rate, channels, sample format the device runs in (for stats and the exclusive probe) |

The system default output is a separate value (`default_output: Option<id>`), updated by the
monitor.

## Pairing (FR-007)

`pair(video_link, video_name, inputs) -> Option<input id>`, pure (research R5):

1. Inputs whose `link` equals the video's: one → it; several → the name match among them, else
   the first by id.
2. No input with the video's link: inputs **without** a link, or with no link on the video side,
   matched by cleaned name.
3. An input whose link is present and different is never paired.

The platform reports no link (`None`) where the system has none, including Windows' null
container id `{00000000-0000-0000-ffff-ffffffffffff}` (every virtual endpoint has it).

## Audio choice per video device (FR-008)

| Value | Settings text | Plays |
|---|---|---|
| `Auto` (default) | `"auto"` | the pair; nothing when there is none (state No paired input) |
| `Input(id)` | the endpoint id | that input; Disconnected while it is missing |
| `None` | `"none"` | nothing, no state |

Stored per video device id in `[devices."<id>"] audio`. An unknown id is kept (the device may
return) and shown as Disconnected.

## Output choice (FR-009)

| Value | Settings text | Plays on |
|---|---|---|
| `Default` (default) | `"default"` | the current default; follows its changes |
| `Device(id)` | the endpoint id | that output; while missing, the default, with the fallback shown |

One for the app (`[audio] output`).

## Volume (FR-011, FR-012)

`level` 0–100 (integer %, default 80) and `muted` (default false), one for the app. Gain
`(level/100)³`, 0 when muted (research R8). The icon: muted or 0 → `volume_off`; ≤ 33 →
`volume_1`; ≤ 66 → `volume_2`; else `volume`. Unmuting restores `level`; moving the slider while
muted unmutes.

## Exclusive flags (FR-014, Windows)

Per endpoint id, `[audio_devices."<id>"] exclusive = true` (default false). Ignored on Linux.

## Audio session

One at a time, owned by the platform layer; the app holds its number.

| Field | Meaning |
|---|---|
| `number` | Increases per open; events of older numbers are ignored (research R9) |
| `video_session` | The video session it belongs to; when that closes, this closes |
| `input`, `output` | Endpoint ids actually opened (output may be the default after a fallback) |
| `input_mode`, `output_mode` | `Shared`, `Exclusive`, `SharedAfterInUse` (exclusive asked, refused) |

Opens when the video session starts (preview states Opening, Playing, No signal) and the choice
resolves to an input; closes when the video session closes, the choice becomes None, or the app
quits. A change of input, output or exclusive flag reopens **only the side it touches**
(FR-005): output change → output side; input change → both sides (the bridge restarts empty);
exclusive flag → that side.

## Audio state (FR-017)

What the Audio dropdown shows. Never shown while the video device is not open (FR-018).

| State | When | Tag | Header (FR-022) | Action |
|---|---|---|---|---|
| `Off` | choice None, or no video device open | — | — | — |
| `Opening` | open requested, no audio yet | — | — | — |
| `Playing` | both sides running | `Exclusive` when a side is exclusive | — | — |
| `NoPair` | Auto and no pair | — | No paired audio / Choose an input | — |
| `Disconnected(name)` | the chosen or paired input is missing or was removed | Disconnected | {} disconnected / Waiting to reconnect | — (reopens on `Added` with the same id) |
| `InUse` | playing, an exclusive side was refused (mode `SharedAfterInUse`) | In use | Audio device in use / Exclusive mode unavailable | — (exclusive tried again at the next open) |
| `Held(name)` | a side reported `InUse`: another app holds the device alone (Windows), at open or taken while playing shared | In use | Audio device in use / Try again | Try again (reopen) |
| `Blocked` | Windows microphone privacy denies the input | Blocked | Access blocked / Desktop app permission is off | Open privacy settings |
| `Failed(name)` | any other open error, or a shared open refused | Failed | Cannot open {} / Try again | Try again (reopen) |

**Output fallback** is a flag beside the state, shown the same way when the state is `Playing` or
`InUse`: header `{output} disconnected / Playing on the system default`, no tag. It is set when a
`Device(id)` output is missing, fails or is held by another app, cleared when the output reopens on it.

Transitions (beyond the table): `Disconnected` → `Opening` on the input's `Added`; any state →
`Off` when the video session closes; `Failed` and `Held` → `Opening` on Try again (no automatic
retry: Windows does not say when the other app releases the device), with one exception: an
output side that failed while playing on the system default is reopened when `DefaultOutput`
names an output again (Linux after PipeWire restarts; research R7); `Blocked` → `Opening`
when the privacy switch, read every 3 s while `Blocked` as the camera's (device preview FR-021),
was read off and then no longer reads off.

## Bridge (research R4)

| Value | Meaning |
|---|---|
| `margin` | 2 ms of spare audio held above what the output takes |
| `ratio` | read rate correction, `1 ± 0.001` at most, set every 500 ms to correct in 2 s |
| `cut_above` | 40 ms spare: the oldest audio dropped down to one input period + margin, as priming (down to the margin alone left a 5 ms gap at the next output event in tests) |
| `primed` | false at start and after an empty ring; priming drops down to one input period + margin |
| `gain`, `target_gain` | current and target gain; 10 ms linear ramp; after silence (start or a gap) the gain ramps from 0 again |
| counters | waiting audio per output event (median, max per second), gap frames, dropped frames, mean ratio |

## Settings additions

See [contracts/settings-file.md](contracts/settings-file.md).
