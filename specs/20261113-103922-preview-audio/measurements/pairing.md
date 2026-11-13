# Pairing keys (T008)

Research R5's rule checked with `crates/platform/examples/audio_endpoints.rs` (T007) on each
system, with the ezcap and the C920 plugged in.

## Windows 11 — 2026-11-13

**Environment**: Windows 11 Pro 26300, the maintainer's PC; ezcap Live Gamer Ultra and Logitech
HD Pro Webcam C920 on USB. Run by the agent:
`cargo run -p capturax-platform --example audio_endpoints`.

| Device | Video container id | Audio input | Input container id | Equal |
|---|---|---|---|---|
| ezcap Live GAMER ULTRA | `{b6f921b6-5905-5b63-b1a1-34d4a41a75c2}` | HDMI (ezcap Live GAMER ULTRA) | `{b6f921b6-5905-5b63-b1a1-34d4a41a75c2}` | yes |
| HD Pro Webcam C920 | `{ef06585b-1df1-51ae-b61b-8b660c165769}` | Microfone (HD Pro Webcam C920) | `{ef06585b-1df1-51ae-b61b-8b660c165769}` | yes |

Video links (Media Foundation symbolic links, `KSCATEGORY_VIDEO_CAMERA`):

- ezcap: `\\?\usb#vid_32ed&pid_3201&mi_00#9&37ee7830&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global`,
  instance `USB\VID_32ED&PID_3201&MI_00\9&37ee7830&0&0000`
- C920: `\\?\usb#vid_046d&pid_082d&mi_00#d&41d17f6&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global`,
  instance `USB\VID_046D&PID_082D&MI_00\d&41d17f6&0&0000`

Other inputs, each with its own container id: G533 microphone `{a717caec-…}`, USB PnP Audio
Device `{12a063e9-…}`, Mic Device `{48fce033-…}`; none equals a video device's.

**Finding**: every virtual or software endpoint (all Voicemeeter inputs and outputs, Steam
Streaming Microphone, Realtek USB Audio's line input) reports the same **null container id**
`{00000000-0000-0000-ffff-ffffffffffff}`, Windows' value for "no physical container". Taken as a
key it would make all of them one device; it must be read as no link (research R5 updated).

Rule: **holds** for both devices, with the null container id as no link.

## CachyOS — pending

Run on CachyOS with the ezcap and the C920:

```sh
cargo run -p capturax-platform --example audio_endpoints
gst-device-monitor-1.0 Video/Source | grep -E 'name|device.bus_path|device.path'
```
