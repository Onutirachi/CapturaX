# Contract: Audio Crate

Crate `capturax-audio` (`crates/audio`): the pure part of the preview's sound (research R1, R4,
R5, R8). No OS calls, no GStreamer, std only. Names are indicative; tasks fix the exact
signatures.

## Bridge

Shared by the two platform streams of one audio session (`Arc`).

| Item | Meaning |
|---|---|
| `Bridge::new(input: Format, output: Format) -> Bridge` | `Format { rate, channels }`; empty, not primed, gain 0 ramping to the current volume |
| `Bridge::push(&self, samples: &[f32])` | Input thread: appends interleaved input frames |
| `Bridge::pull(&self, out: &mut [f32])` | Output thread: fills interleaved output frames (follower, interpolation, channel map, gain ramp); silence and a gap count when not primed |
| `Bridge::set_input_period(frames)` | The input's packet size, for priming |
| `Bridge::set_gain(gain)` | Any thread; applied with the 10 ms ramp |
| `Bridge::take_stats() -> BridgeStats` | Stats printer, once per second: waiting ms median/max, ratio ppm, gap ms, dropped ms; resets the counters |

Rules: `push` and `pull` never allocate after the first second (buffers grow once); the lock is
held only for the copy; `pull` never blocks on `push`.

## Pairing and choices

| Item | Meaning |
|---|---|
| `pair(video: &VideoKey, inputs: &[Endpoint]) -> Option<&str>` | data-model.md "Pairing"; `VideoKey { link: Option<String>, name: String }` |
| `clean_name(name) -> String` | The decorations removed (research R5 step 2) |
| `AudioChoice::parse / to_string`, `OutputChoice::parse / to_string` | Settings texts (data-model.md) |
| `resolve(choice, pair, inputs) -> Resolved` | `Play(id)`, `NoPair`, `Missing(id)`, `Off` |
| `output_for(choice, outputs) -> (Option<id>, fallback: bool)` | The output to open (`None` = the system default) and whether it is a fallback |

## Volume

| Item | Meaning |
|---|---|
| `gain(level, muted) -> f32` | `(level/100)³`, 0 when muted |
| `icon(level, muted) -> VolumeIcon` | `Off`, `Low`, `Medium`, `High` (data-model.md "Volume") |
| `wheel(level, notches) -> u8` | ±5 per notch, clamped to 0–100 |
