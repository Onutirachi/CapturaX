# Acceptance (quickstart manual checks)

## T008: ambient light, Windows (RTX 5070 Ti, Windows 11), 2026-11-24

The ezcap at 1080p60 with the PS5 in Astro's Playroom, window 1720 × 720 (bars at the sides),
release build. The maintainer looked; the agent ran the app.

| # | Check | Result |
|---|---|---|
| 1 | The bulb on the left dock lights the bars with the video's side colors; orange chip; tooltip | Pass |
| 2 | Scene changes; Video shadow → With ambient light, toggling the bulb | Pass: the glow follows smoothly, the shadow comes and goes with it |
| 3 | White flash, dark scene, PS5 boot | Pass: no flash. Dark scenes faded to the plain background then; with the gate dropped they show their dark colors (checked again after T008a) |
| 4 | Resize, F11 16:9 (no bars), 4:3 | Pass |
| 5 | Unplug the card; restart with the bulb on (without the hide step, which waits for T016) | Pass |
| 6 | Both themes, Background Theme and Black, watermark on | Pass. The watermark stays drawn over the glow in the theme's colour (dark on the light theme, light on the dark one), so it looks different between themes; the maintainer keeps it so |

Tuning (research R2): the maintainer asked for a stronger glow than CachyOS's alpha 0.9 / gate
0.12. A gain was added, and the maintainer set the five draw values live with temporary sliders
(not kept): alpha 0.9, saturation 1.5, gain 2. Then the maintainer asked that the glow never
disappear: the luminance gate was dropped (FR-003, research R2, T008a), so dark scenes show their
dark colors. In GTA V's daylight the gain blew out (yellow and near white bars, maintainer's
screenshot), so it became a lift (power 0.5, research R2).

The glow updated only 39 times a second at 60 fps here (latency.md); the pacing's minimum went
from 16.6 ms to 15 ms (research R1) and it updates 60 times a second.

## T008e: ambient light intensity, Windows, 2026-11-24

Same setup (ezcap 1080p60, PS5), release build; the maintainer looked, the agent ran the app.

| # | Check | Result |
|---|---|---|
| 1a | The bulb's popover, slider, wheel, 0 %, a move while off, restart, keyboard | Pass. The volume button was checked again on the shared slider button (T008c): unchanged |

The maintainer asked for the bulb without its rays while it does not glow (off, or on at 0 %, as
the volume's icon shows mute at 0 %): `bulb_off.svg`, spec FR-001 updated. Checked: pass.
