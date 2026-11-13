# Contract: Platform Interface (additions)

Adds to the [device preview's platform contract](../../20261106-170507-device-preview/contracts/platform.md);
everything there stays. Names are indicative; tasks fix the exact signatures.

## Audio devices

| Function | Windows | Linux | Spec |
|---|---|---|---|
| `audio::Monitor::start(on_event) -> Monitor` | `IMMDeviceEnumerator` + `IMMNotificationClient` (callbacks only post) | registry and `default` metadata on the `pipewire` thread | FR-009, FR-010, research R6 |
| `AudioEvent` | `Listed { inputs, outputs, default_output }` once, then `Added(Endpoint)`, `Removed(id)`, `DefaultOutput(Option<id>)` | same | |
| `audio::Endpoint` | data-model.md "Audio endpoint" (`id`, `name`, `flow`, `link`, `format`) | same | |
| `video_link(video_id) -> Option<String>` | container id of the Media Foundation device (`CM_*` calls, research R5) | the USB path from the V4L2 id's bus path, interface suffix removed | FR-007 |

## Audio session

| Function | Windows | Linux | Spec |
|---|---|---|---|
| `audio::Session::open(SessionRequest, bridge, on_event) -> Session` | two threads `audio-in`, `audio-out`, MMCSS Pro Audio, event-driven WASAPI (research R2) | two `pw_stream`s in one `node.group` on the `pipewire` thread (research R3) | FR-001 to FR-006 |
| `SessionRequest` | `{ number, input: id, output: Option<id> (None = default), input_exclusive, output_exclusive }` | exclusive ignored | FR-015, FR-016 |
| `Session::reopen_output(output, exclusive)`, `Session::reopen_input(input, exclusive)` | the side's thread closes and opens again | the stream is disconnected and connected again | FR-005 |
| `Session::close()` | stop flag, `Stop`, threads end on their own; returns at once | message to the `pipewire` thread; returns at once | FR-006, research R9 |
| `on_event(SessionEvent)` | `Opened { side, mode, format, period, reported_latency }`, `Error { side, error: AudioError }`, `Closed` — each with the session number | same | FR-017, FR-019 |
| `AudioError` | `Gone`, `InUse`, `Blocked`, `Format`, `Failed(String)`; mapping in research R7 | same, without `InUse` and `Blocked` | |

Rules:

- No function blocks the caller on a device: every outcome is an event (research R9).
- Windows exclusive: tried first when asked; `InUse` or `Format` → the platform opens the same side
  shared and reports `mode = SharedAfterInUse`; never asked again until the next open.
- Linux: `output = None` leaves `target.object` unset, so PipeWire follows the default itself; the
  `DefaultOutput` event still comes, for the name.

## Privacy

`camera_access()` and `open_camera_privacy()` become `access(Capability)` and
`open_privacy(Capability)` with `Capability::{Camera, Microphone}`; the microphone reads the
`microphone` consent store (all apps and `NonPackaged`, HKCU and HKLM) and opens
`ms-settings:privacy-microphone`. Linux: `Unknown`, and `open_privacy` does nothing (research R7).
