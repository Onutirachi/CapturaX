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

## CachyOS — 2026-11-16

**Environment**: kernel `7.3.1-1-cachyos`, PipeWire 1.6.10, the ezcap in the PC and the C920
on USB (plugged in later the same day; the ezcap's values did not change). Run by the agent over
SSH:

```sh
cargo run -p capturax-platform --example audio_endpoints
capturax --list-devices | grep 'id:'
```

| Device | Video bus path | Audio input | Input `device.bus-path` | Key of both | Equal |
|---|---|---|---|---|---|
| ezcap Live GAMER ULTRA | `pci-0000:03:00.0-usb-0:3:1.0` | ezcap Live GAMER ULTRA Estéreo analógico | `pci-0000:03:00.0-usb-0:3:1.2` | `pci-0000:03:00.0-usb-0:3` | yes |
| HD Pro Webcam C920 | `pci-0000:00:14.0-usb-0:1:1.0` | HD Pro Webcam C920 Estéreo analógico | `pci-0000:00:14.0-usb-0:1:1.2` | `pci-0000:00:14.0-usb-0:1` | yes |

- Video ids from the app: `v4l2:32ed:3201:ezcap_ezcap_Live_GAMER_ULTRA_00000001@pci-0000:03:00.0-usb-0:3:1.0`
  and `v4l2:046d:082d:046d_HD_Pro_Webcam_C920_E3B29C8F@pci-0000:00:14.0-usb-0:1:1.0`; udev's
  `ID_PATH` of each `/dev/video*` and PipeWire's own V4L2 devices give the same paths.
- Other audio devices keep a PCI path with no interface suffix, each its own key: internal audio
  `pci-0000:00:1f.3` (analog input and IEC958 output), NVIDIA HDMI `pci-0000:01:00.1`.

**Findings**:

- The registry's global for a device has no `device.bus-path` (only `device.name`,
  `device.description`, `device.api` and a few more): the example printed no key until it bound
  each device and read its info. The Linux monitor (T019) must do the same.
- `gst-device-monitor-1.0 Video/Source` lists PipeWire's entry for the card here, which has no
  `device.bus_path`; the app's own id is the place to read the video path.

Rule: **holds** for both devices; on both the video is interface `1.0` and the audio `1.2`.
