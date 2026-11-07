# Recorded device caps

Test data for `crates/capture/tests/modes.rs` and the menu tests of `app/src/preview.rs`, read
with `capture::recorded::device`: the block `gst-device-monitor-1.0` prints for one
device (name, caps, properties), kept as printed. GStreamer 1.28.8 on both machines.

| File | Device | Machine | How | Date |
|---|---|---|---|---|
| `c920-windows.txt` | Logitech HD Pro Webcam C920 | Windows 11 PC | `gst-device-monitor-1.0 Video/Source`, the `device.api = mediafoundation` block | 2026-11-06 |
| `ezcap-linux.txt` | ezcap Live Gamer Ultra | CachyOS PC, over SSH | `gst-device-monitor-1.0 -i Video/Source` (`-i` shows the V4L2 provider that PipeWire hides), the `v4l2src` block | 2026-11-06 |

`ezcap-windows.txt` is added when the ezcap is plugged into the Windows PC (same command as the
C920).
