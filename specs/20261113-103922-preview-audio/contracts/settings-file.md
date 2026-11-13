# Contract: Settings File (additions)

Adds to the [device preview's settings file](../../20261106-170507-device-preview/contracts/settings-file.md);
its rules (lenient read, unknown keys kept, atomic save) apply unchanged. No version bump: one new
key in an existing table and two new tables.

```toml
[audio]
output = "default"                 # or an endpoint id (FR-009)
volume = 80                        # 0–100 (FR-012)
muted = false

[devices."mf:\\\\?\\usb#vid_32ed&pid_3201&mi_00#…"]
name = "ezcap Live Gamer Ultra"
mode = "auto-max-fps"
color_range = "auto"
audio = "auto"                     # "auto", "none", or an input endpoint id (FR-008)

[audio_devices."{0.0.1.00000000}.{6a1c…}"]
name = "Digital Audio Interface (ezcap Live Gamer Ultra)"   # for people; not used to match
exclusive = true                   # Windows only (FR-014); ignored on Linux
```

Rules:

1. A missing or unknown `output` or `audio` value takes its default; an id that is not present
   now is kept and treated as missing (data-model.md "Audio choice", "Output choice").
2. A `volume` outside 0–100 is clamped; a non-number takes 80.
3. `audio_devices` entries are written only when `exclusive` is true; at most 32 are kept, the
   least recently used dropped on save, as `devices`.
