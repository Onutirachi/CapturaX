# Contract: Capture Crate

Crate `capture` (`crates/capture`): devices, modes and the preview pipeline over GStreamer. No OS
calls (research R12). Names are indicative; tasks fix the exact signatures.

## Devices

| Item | Meaning |
|---|---|
| `Monitor::start(on_event) -> Monitor` | Starts the device monitor on its own thread (research R1); `on_event` gets `Added(Device)`, `Removed(id)` and, once at start, `Listed(Vec<Device>)` |
| `Monitor::devices() -> Vec<Device>` | The current list (also used for the 3 s re-read while waiting) |
| `Monitor::stop()` | Stops the thread; called at quit |
| `Device` | data-model.md "Device" |
| `find(remembered_id, devices) -> Option<&Device>` | Exact id (research R2) |
| `numbered(devices)` | Applies " (2)", " (3)" to repeated names |

## Modes (pure)

| Item | Meaning |
|---|---|
| `modes_from_caps(caps) -> Vec<Mode>` | One per resolution × rate, best format, sorted (research R3) |
| `resolve(choice, &[Mode], signal) -> Option<Mode>` | data-model.md "Mode choice" |
| `shown_automatic(&[Mode], signal) -> Vec<(ModeChoice, Mode)>` | The automatic entries to list, duplicates removed |
| `default_choice(signal, &[Mode]) -> ModeChoice` | FR-008 |
| `tags(width, height) -> Tags { name: Option<&'static str>, family: Option<Ratio>, aspect: Ratio }` | F43, data-model.md "Mode": `name` None is Custom; `family` the standard's ratio family; `aspect` the snapped or exact ratio |
| `ModeChoice::parse / to_string` | Settings text |

## Session

One open device; owns its pipeline (constitution III: later specs attach outputs here).

| Item | Meaning |
|---|---|
| `Session::open(device, mode, range_choice, on_frame, on_event) -> Session` | Builds and plays `source ! capsfilter [! decoder] [! videoconvert] ! appsink` (research R4); returns at once, the outcome arrives as events |
| `Session::test_video(width, height, fps, …)` | Same, from `videotestsrc` in NV12 (replaces `app/src/test_video.rs`) |
| `Session::set_range(choice)` | Full, Limited or Auto; applies to the next frame, no restart |
| `Session::close()` | Pipeline to Null; idempotent; also on drop |
| `on_frame(Frame)` | Called on the streaming thread for each frame (data-model.md "Frame") |
| `on_event(SessionEvent)` | `Playing { format, matrix }`, `FirstFrame`, `Range(in_use)`, `Error(DeviceError)` |
| `DeviceError` | `InUse`, `Blocked`, `Gone`, `NoSignal`, `Failed(String)`; mapping in research R6 (Windows `Blocked` needs `platform::camera_access()`, passed in by `app`) |

Rules:

- No frame within 5 s of `Playing` → `Error(NoSignal)`.
- Analysis (research R5) runs on the streaming thread once per 500 ms, never per frame on the
  video thread.
- `Frame.captured` follows research R11; `do-timestamp` stays false.
- Every GStreamer error and its debug text are logged before mapping.
