# Windows Stalls (T052, SC-006)

Investigation of research R8, run on 2026-11-11 by the agent (scripted runs) and the maintainer
(PresentMon elevated, DWM keys, reboots, language switches).

## Setup

- Windows 11 Pro, RTX 5070 Ti, AOC 27G2G4 (1080p, 144 Hz, G-SYNC), window 1071 × 565 on it.
- ezcap Live GAMER ULTRA at 1920x1080@60 NV12, fed by the CachyOS PC's HDMI out (desktop).
- Build `8b0a033` plus T054 (snapped fullscreen return) in the working tree; presentation
  Lowest latency (Mailbox, DX12).
- PresentMon 2.3.1 elevated (`--process_name capturax.exe --qpc_time --v1_metrics`), one CSV per
  app run; the app with `--stats --present-log <run>-app.tsv`; both matched by QPC.
- Each run, scripted with real clicks and key presses: 10 s warm-up, ten focus changes out and
  back (a window on another monitor), ten F11 entries and exits 3 s apart, 30 s idle in
  fullscreen. The language run is manual: ten switches in the panel, about 3 s apart.
- A stall: a gap over 50 ms between two app presents, or between two display changes in
  PresentMon.

Raw files: [raw/t052/](raw/t052/) (`<run>-analysis.txt` per run, the script steps, the
`--stats` output, `notes.md`). The PresentMon CSVs and present logs (2.5 MB and 0.5 MB per run)
stay out of the repository.

## A false lead first: the `--stats` writes

The first runs had random stalls of 130–680 ms away from any event, with the card's frames
stopping too (`recv` down to 25 a second). They sat on the 1 s stats tick: `print_stats`
(app/src/render/mod.rs) writes stdout and `capturax.log` from the video thread **inside the inbox
lock**, and the app ran from D:, a hard disk. Confirmed with D: kept busy by a writer (4 MiB with
fsync plus random reads, about 13 MB/s), the app idle and windowed for 60 s:

| Run | Exe, stdout and log on | Seconds with a stall | Worst |
|---|---|---|---|
| hdd-d | D: (HDD) | 44 of 57 | 718 ms |
| hdd-c | C: (NVMe) | 0 of 60 | under 40 ms |

Without `--stats` the playing loop writes nothing, so users do not see this; but it pollutes
every measurement taken with `--stats`, likely T028's 50–675 ms ones too. From `base-c` on, the
runs went from a copy on C:. Fix in T053: format the line inside the lock, write it outside, on
a writer thread (as the present log does).

## Runs

Exits are F11 out of fullscreen; "dropped" is PresentMon's `Dropped` for the app's present.

| Run | Config | Focus (20) | Entries (11) | Exits (11) | Idle 30 s |
|---|---|---|---|---|---|
| base | G-SYNC on, defaults; after a monitor-off cycle | display gap 50–57 ms after each | 60–208 ms | 60–208 ms, 1 dropped each | clean |
| gsync-off | NVIDIA profile Fixed refresh for capturax.exe | clean | clean | 53–82 ms, 1 dropped | clean |
| visual | `WGPU_DX12_PRESENTATION_SYSTEM=visual` | clean | clean | 52–83 ms, none dropped | clean |
| base2 | as base | clean | clean | 52–90 ms, 1 dropped | clean |
| base3 | as base, NVIDIA profile removed | clean | clean | 53–90 ms, 1 dropped | stats burst* |
| base-boot | fresh reboot | clean | clean | 54–59 ms (3 of 11) | clean |
| base-monoff | after `nircmd monitor off` and wake | clean | clean | 52–90 ms (7 of 11) | stats stall* |
| **base-c** | as base, exe on C: (the reference) | clean | clean | 62–84 ms (6 of 11), 1 dropped | clean |
| minfps0 | `OverlayMinFPS=0`, reboot | display gap 55–57 ms (4 of 20) | display 50–88 ms | 51–73 ms (9 of 11), 1 dropped | clean |
| mpo-off | `OverlayTestMode=5`, reboot | clean | clean | 60–68 ms (5 of 11), 1 dropped | clean |
| lang | defaults back, reboot | — | — | — | — |

\* The `--stats` writes above, not presentation.

**PresentMode around the stalls**: windowed and fullscreen both run in *Hardware Composed:
Independent Flip* (overlay plane). Every fullscreen entry and exit passes through *Composed:
Flip* for 50–130 ms and back; the exit stall is that switch, with the present of the old size
dropped. Focus changes never left Independent Flip; their 50–57 ms gaps (base, minfps0) are a
frame shown one refresh later, with no gap in the app's own presents. `AllowsTearing` stays 0
throughout.

**Language switches** (lang, ten switches): no app gap and no display gap over 50 ms. Stage
times: catalog 0.0 ms, Slint properties 0.1–0.2 ms, UI draw 2.6–2.8 ms, upload 0.2–0.3 ms,
upload to present 0.4–7.8 ms. (Display gaps of up to 458 ms 10 s before the first switch were
the UAC prompt of the elevated PowerShell that started PresentMon.)

**MPO off** costs display latency: windowed the app falls back to *Composed: Flip*, with
`msUntilDisplayed` median 11.1 ms against 1.2 ms in base-c (fullscreen 0.4 ms in both).

## Conclusion

- **Focus changes**: no stall in the app. The first base's 50–57 ms display gaps did not come
  back in seven later runs with the same settings (only, partly, with `OverlayMinFPS=0`); not
  reproducible, not caused by the app's presents.
- **Language switches**: no stall; the whole switch costs under 11 ms.
- **Idle fullscreen**: no stall once the `--stats` writes are out of the way.
- **Fullscreen exits**: the one stall left in every configuration, 51–90 ms in 3 to 9 of 11
  exits, one dropped present. Its system part (Independent Flip → Composed: Flip → Independent
  Flip) is not removed by any DWM key, G-SYNC off or the DirectComposition swap chain. Its app
  part: around the exit the present calls themselves block 30–48 ms (acquire to present return,
  base-c) besides the presents the app holds for the new size (app.rs `apply_fullscreen`).
  Whether the app can shorten it is T053's question.
- **`--stats`**: an app bug for measurements only (above), fixed in T053.

No DWM key or NVIDIA profile is worth recommending to users: none removes the exit stall, and MPO
off makes windowed latency about 10 ms worse.
