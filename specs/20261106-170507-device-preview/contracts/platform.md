# Contract: Platform Interface (additions)

Adds to the app shell's [platform contract](../../20261104-112852-app-shell/contracts/platform.md);
everything there stays. Names are indicative; tasks fix the exact signatures.

| Function | Windows | Linux | Spec |
|---|---|---|---|
| `camera_access() -> Access` (`Allowed`, `Denied`, `Unknown`) | `CapabilityAccessManager\ConsentStore\webcam` and `webcam\NonPackaged` `Value` under HKCU and HKLM: any `Deny` → Denied | `Unknown` (no system switch; node permissions show as errors) | FR-019, FR-021, research R6 |
| `open_camera_privacy()` | `ShellExecuteW("ms-settings:privacy-webcam")` | not offered (the state names the node) | FR-021 |
| `gpu_performance(pci_bus_id) -> PerformanceGuard` | no-op guard | NVIDIA: `libnvidia-ml.so.1` via `libloading`, `nvmlDeviceSetPowerMizerMode_v1(PREFER_MAXIMUM_PERFORMANCE)`, previous mode restored on drop (the request is per process: other processes' requests are not touched); anything missing → logged once, no-op | FR-012, research R7 |
| `init_notifications(icon, on_action)` once (the app icon's RGBA pixels; `on_action(tag)` from any thread), then `notify(Notice { title, action: Option<label>, tag }) -> bool` | WinRT toast, app id `CapturaX` registered under `HKCU\Software\Classes\AppUserModelId\CapturaX`, `IconUri` to `%LOCALAPPDATA%\CapturaX\notification.ico` written from the pixels; button through `Activated`; previous toast hidden after the new one shows | `org.freedesktop.Notifications.Notify` via `zbus` after `CloseNotification` of the previous one, the pixels in `image-data`; `ActionInvoked` signal | FR-027, research R15 |
| `qpc() -> i64` | `QueryPerformanceCounter` | `CLOCK_MONOTONIC` in ns (for the same log format) | FR-023, research R8 |
| `arranged_rect(window) -> Option<rect>` | `IsWindowArranged` → `GetWindowRect` | none | FR-025, research R10 |
| `set_outer_rect(window, rect)` | `SetWindowPos` (no z-order or activation change) | no-op | FR-025 |

`gpu_performance` is held by the video thread while it presents device frames (not for the test
video unless `--stats` asks for a comparison) and dropped when presenting stops (hidden, device
closed, quit).
