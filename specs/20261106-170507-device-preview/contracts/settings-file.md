# Contract: Settings File (additions)

Adds tables to the app shell's [settings file](../../20261104-112852-app-shell/contracts/settings-file.md),
version 1; its rules (lenient read, unknown keys kept, atomic save) apply unchanged. No version
bump: new tables only.

```toml
[preview]
device = "mf:\\\\?\\usb#vid_32ed&pid_3201&mi_00#7&1a2b3c&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\\global"
recent = ["mf:…", "mf:…"]          # most recent first, at most 8
presentation = "lowest-latency"    # or "follow-monitor" (F52)
background = "theme"               # or "black" (F56)
watermark = true                   # the X in the video background (F57)
shadow = "always"                  # "always", "ambient", "off" (F58)

[overlay]
latency = "measured+estimated"     # "measured", "measured+estimated", "total"
fps = "presented/nominal"          # "presented", "presented/nominal", "nominal"

[devices."mf:\\\\?\\usb#vid_32ed&pid_3201&mi_00#7&1a2b3c&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\\global"]
name = "ezcap Live Gamer Ultra"    # for people; not used to match
mode = "auto-max-fps"              # "auto-source", "auto-max-resolution", "1920x1080@60", "1920x1080@59.94"
color_range = "auto"               # "full", "limited"
```

Rules:

1. An unknown `presentation`, `background`, `watermark`, `shadow`, `latency`, `fps`, `mode` or
   `color_range` value takes its default
   (data-model.md "Settings additions").
2. A `mode` naming a size and rate the device no longer offers opens the default mode;
   the stored value is replaced only when the user picks again.
3. At most 32 `devices` entries are kept; the least recently used is dropped on save.
