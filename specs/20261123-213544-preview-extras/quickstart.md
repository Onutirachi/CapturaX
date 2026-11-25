# Quickstart: Preview Extras

How to check the feature end to end. Hardware: the ezcap card with the PS5 and the C920, on the
Windows PC and the CachyOS PC (KDE Plasma, Wayland), as in specs #3 to #5. On CachyOS the agent
runs the builds, the runs and the stats over SSH; the maintainer only looks.

## Build and unit tests

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Manual checks (acceptance)

| # | Steps | Expected |
|---|---|---|
| 1 | ezcap at 1080p60, window 1720 × 720 (bars at the sides); click the bulb on the left dock | The bars glow with the video's side colors; orange chip; tooltip "Ambient light" (US1 #1) |
| 1a | Rest the pointer on the bulb; move the slider, then the wheel over the bulb; set 0 %; turn the light off and move the slider; restart; Tab to the bulb and use the arrows | The popover opens like the volume's; the glow weakens and strengthens at once; 0 % shows no glow; moving it while off turns the light on; the intensity is kept after the restart; the keyboard works as the volume's (FR-001a) |
| 2 | Game scene changes (menu → map); Settings → Video shadow → With ambient light, then toggle the bulb | Glow follows within about 0.5 s, smoothly; the shadow comes and goes with the glow (US1 #2, #5) |
| 3 | Photo-mode flash or a white flash frame; a dark scene; the PS5 boot | No flash in the glow; dark scenes show their dark colors and the glow never disappears (FR-003) |
| 4 | Resize the window, F11 to fullscreen 16:9 (no bars), then a 4:3 mode | Glow follows the bars in the same frame; none when there are no bars; glow at the sides for 4:3 |
| 5 | Unplug the card; hide the preview; restart the app with the bulb on | No glow in a state; the bulb is still clickable and changes the setting; still on after the restart (US1 #3, #4, Q1) |
| 6 | Both themes, Background Theme and Black, watermark on | Glow under the watermark (clearer), shadow and video |
| 7 | C920 previewing; click the preview button (right dock, first) | "Preview hidden" with `device_tv_off`, no button; webcam light off within 1 s; the Windows Camera app (Cheese on Linux) opens it; preview sound stops; no info bubble shows; screenshot button disabled; the hotkey refused with its notice (US2 #1, #4, #6) |
| 8 | Show it again; then hide, pick another device, a mode and a range in the toolbar, and show; then hide and click **Open** on a "connected" notice | Same device, mode and range; video within its normal opening time + 0.5 s; sound back with volume and mute kept; while hidden the toolbar names the hidden device, picks save and open nothing (range reads "Auto" alone), showing opens the picked device in the picked mode; **Open** shows the preview with its device (US2 #2, #3, FR-010a, FR-010b, SC-002) |
| 9 | Hide; unplug and replug the card; close to the tray and reopen; quit and start again | Notices show, nothing reopens, still "Preview hidden"; after the restart no webcam light and no device taken (US2 #7, edge cases) |
| 10 | Hide while a device is opening; no device connected | The opening is cancelled; the button is disabled with no device |
| 11 | Hidden, nothing else running, 30 s; then the "no source" state, 30 s (Task Manager / `top` and the GPU's usage) | Within 1 percentage point (SC-003) |
| 12 | Hover the latency bubble: pin; hide the controls | Pin mark with the controls; only latency shows when they hide, packed at its corner, still updating each second (US3 #1, #4) |
| 13 | Drag the latency bubble to the top left; drag mode before source; restart | Grab hand, then closed hand; corners light up, the target is marked, the orange bar shows the place; the gap closes; order and corner kept after the restart (US3 #2, #3, SC-004) |
| 14 | Press a bubble without moving; drag and press Alt+Tab mid-drag | Nothing moves; the drag is cancelled |
| 15 | Bubbles in all four corners at 640 × 360, 800 × 520, 1280 × 720 and fullscreen 1920 × 1080, both themes and languages; a device notice in each corner (Settings → Notification position); open the settings panel with bubbles in every corner | No overlap with the toolbar, docks, other bubbles or the notice while the controls show; top rows go under the toolbar when there is no room beside it; the bubbles stay under the settings panel and its backdrop (US3 #5, #6, SC-005, edge case) |
| 16 | Settings → Preview → Info bubbles → Reset | All four at the bottom right, today's order, none pinned; display choices unchanged (US3 #7) |
| 17 | Both languages | Every new text: Ambient light, Hide preview, Show preview, Preview hidden, Pin, Unpin, Info bubbles, Reset |

## Latency check (SC-001)

On both systems, ezcap, windowed with bars (1720 × 720), 60 s each, at 1080p60 and 1080p120:

```sh
capturax --stats          # ambient off (preview.ambient = false), baseline
capturax --stats          # ambient on; [loop] shows glow > 0
capturax --stats          # ambient on, fps and latency bubbles pinned, controls hidden (research R9)
```

Compare the median and p95 of `ready_ms` and the `repl` count (contracts/stats.md). Then repeat
spec #3's glass-to-glass check (SC-007) once with the ambient light on. Record everything in
`measurements/latency.md`. In the pinned run, `ui` shows about 1 upload per second, and
`ui_ms` stays well below today's full uploads (research R9).
